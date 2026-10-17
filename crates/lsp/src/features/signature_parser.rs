use tower_lsp::lsp_types::Position;

pub use super::signature_param::parse_signature_parameters;
use crate::syntax::ast_call_view::{ParsedCallArg, parse_call_arg};

#[derive(Debug, Clone)]
pub struct CallContext {
    pub receiver: Option<String>,
    pub callee: String,
    pub active_parameter: u32,
    pub call_args: Vec<ParsedCallArg>,
    pub active_arg_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnclosingCallHeader {
    pub open_paren_char_idx: usize,
    pub receiver: Option<String>,
    pub callee: String,
}

pub fn find_enclosing_call_header(chars: &[char]) -> Option<EnclosingCallHeader> {
    if chars.is_empty() {
        return None;
    }

    let mut depth: usize = 0;
    let mut brace_depth: usize = 0;
    let mut bracket_depth: usize = 0;
    let mut unclosed_braces: usize = 0;
    let mut i = chars.len();

    while i > 0 {
        i -= 1;
        let c = chars[i];
        match c {
            '"' => i = skip_string_backward(chars, i),
            '\'' => i = skip_char_backward(chars, i),
            ')' => depth += 1,
            '}' => brace_depth += 1,
            ']' => bracket_depth += 1,
            '[' if bracket_depth > 0 => bracket_depth -= 1,
            '{' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                } else {
                    unclosed_braces += 1;
                }
            }
            ';' if depth == 0 && unclosed_braces == 0 => return None,
            '(' if depth > 0 => depth -= 1,
            '(' if depth == 0 => {
                let before: String = chars[..i].iter().collect();
                let (receiver, callee) = parse_callee_and_receiver(&before)?;
                return Some(EnclosingCallHeader {
                    open_paren_char_idx: i,
                    receiver,
                    callee,
                });
            }
            _ => {}
        }
    }
    None
}

pub fn find_enclosing_call_context(pos: Position, source: &str) -> Option<CallContext> {
    let offset = crate::position::position_to_byte_offset(pos, source);
    let prefix = &source[..offset];
    let chars: Vec<char> = prefix.chars().collect();
    let header = find_enclosing_call_header(&chars)?;
    let i = header.open_paren_char_idx;

    let after_paren: Vec<char> = chars[i + 1..].to_vec();
    let active_parameter = count_commas_at_top_level(&after_paren);

    let open_paren_byte = chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>();
    let close_byte = find_call_close_paren(source, open_paren_byte);
    let end_byte = close_byte.unwrap_or(offset.min(source.len()));
    let args_text = if open_paren_byte < end_byte {
        &source[open_paren_byte + 1..end_byte]
    } else {
        ""
    };

    let args_chars: Vec<char> = args_text.chars().collect();
    let raw_args = split_call_arguments(&args_chars);
    let call_args: Vec<ParsedCallArg> = if raw_args.len() == 1 && raw_args[0].trim().is_empty() {
        Vec::new()
    } else {
        raw_args
            .into_iter()
            .map(|s| parse_call_arg(s.trim()))
            .collect()
    };

    let active_arg_name = if (active_parameter as usize) < call_args.len() {
        call_args[active_parameter as usize].name.clone()
    } else {
        None
    };

    Some(CallContext {
        receiver: header.receiver,
        callee: header.callee,
        active_parameter,
        call_args,
        active_arg_name,
    })
}

fn find_call_close_paren(source: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0;
    let mut brace_depth = 0;
    let mut bracket_depth = 0;
    let chars: Vec<char> = source[open_idx..].chars().take(8192).collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '"' => i = skip_string_forward(&chars, i),
            '\'' => i = skip_char_forward(&chars, i),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && brace_depth == 0 && bracket_depth == 0 {
                    let byte_offset = chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>();
                    return Some(open_idx + byte_offset);
                }
            }
            '{' => brace_depth += 1,
            '}' if brace_depth > 0 => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '}' if depth <= 1 && brace_depth == 0 => return None,
            ';' if depth <= 1 && brace_depth == 0 => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

fn split_call_arguments(chars: &[char]) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut brace_depth = 0;
    let mut bracket_depth = 0;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => i = skip_string_forward(chars, i),
            '\'' => i = skip_char_forward(chars, i),
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            '{' => brace_depth += 1,
            '}' if brace_depth > 0 => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                let arg: String = chars[start..i].iter().collect();
                args.push(arg);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < chars.len() {
        let arg: String = chars[start..].iter().collect();
        args.push(arg);
    }
    args
}

fn skip_string_backward(chars: &[char], mut i: usize) -> usize {
    if i >= 2 && chars[i - 1] == '"' && chars[i - 2] == '"' {
        i = i.saturating_sub(2);
        while i >= 2 {
            if chars[i] == '"' && chars[i - 1] == '"' && chars[i - 2] == '"' {
                return i.saturating_sub(2);
            }
            i -= 1;
        }
        return 0;
    }
    while i > 0 {
        i -= 1;
        if chars[i] == '"' {
            let mut backslashes = 0;
            let mut k = i;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes % 2 == 0 {
                return i;
            }
        }
    }
    0
}

fn skip_char_backward(chars: &[char], mut i: usize) -> usize {
    while i > 0 {
        i -= 1;
        if chars[i] == '\'' {
            let mut backslashes = 0;
            let mut k = i;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes % 2 == 0 {
                return i;
            }
        }
    }
    0
}

fn count_commas_at_top_level(chars: &[char]) -> u32 {
    let mut comma_count = 0;
    let mut depth = 0;
    let mut brace_depth = 0;
    let mut bracket_depth = 0;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => i = skip_string_forward(chars, i),
            '\'' => i = skip_char_forward(chars, i),
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            '{' => brace_depth += 1,
            '}' if brace_depth > 0 => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                comma_count += 1;
            }
            _ => {}
        }
        i += 1;
    }
    comma_count
}

fn skip_string_forward(chars: &[char], mut i: usize) -> usize {
    if i + 2 < chars.len() && chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
        i += 3;
        while i + 2 < chars.len() {
            if chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
                return i + 2;
            }
            i += 1;
        }
        return chars.len().saturating_sub(1);
    }
    i += 1;
    while i < chars.len() {
        if chars[i] == '"' {
            let mut backslashes = 0;
            let mut k = i;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes % 2 == 0 {
                return i;
            }
        }
        i += 1;
    }
    chars.len().saturating_sub(1)
}

fn skip_char_forward(chars: &[char], mut i: usize) -> usize {
    i += 1;
    while i < chars.len() {
        if chars[i] == '\'' {
            let mut backslashes = 0;
            let mut k = i;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes % 2 == 0 {
                return i;
            }
        }
        i += 1;
    }
    chars.len().saturating_sub(1)
}

fn parse_callee_and_receiver(before: &str) -> Option<(Option<String>, String)> {
    let trimmed = before.trim_end();
    let last = trimmed
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next_back()?
        .trim();

    if last.is_empty()
        || matches!(
            last,
            "if" | "while" | "for" | "when" | "catch" | "synchronized"
        )
        || !last
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return None;
    }

    let before_word = trimmed[..trimmed.len() - last.len()].trim_end();
    let prev_keyword = before_word.split_whitespace().next_back().unwrap_or("");
    if matches!(
        prev_keyword,
        "fun" | "val" | "var" | "class" | "interface" | "object" | "enum" | "typealias"
    ) {
        return None;
    }

    let receiver = before_word.strip_suffix('.').and_then(|without_dot| {
        let trimmed = without_dot.trim_end();
        let clean = trimmed
            .strip_suffix('?')
            .map(str::trim_end)
            .unwrap_or(trimmed);
        crate::features::completion_context::extract_receiver_from_prefix(clean)
    });

    Some((receiver, last.to_string()))
}
