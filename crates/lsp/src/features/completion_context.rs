use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

use crate::position::position_to_byte_offset;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    MemberAccess {
        receiver: String,
        partial: String,
    },
    CallableReference {
        receiver: Option<String>,
        partial: String,
    },
    Scope {
        partial: String,
    },
}

fn is_trailing_lambda_lookahead(slice: &[char]) -> bool {
    let mut idx = 0;
    while idx < slice.len() && slice[idx].is_whitespace() {
        idx += 1;
    }
    if idx >= slice.len() {
        return false;
    }
    if slice[idx] == '{' {
        return true;
    }
    if slice[idx] == '@' {
        idx += 1;
        while idx < slice.len() && slice[idx].is_whitespace() {
            idx += 1;
        }
        while idx < slice.len()
            && (slice[idx].is_alphanumeric() || slice[idx] == '_' || slice[idx] == '`')
        {
            idx += 1;
        }
        while idx < slice.len() && slice[idx].is_whitespace() {
            idx += 1;
        }
        return idx < slice.len() && slice[idx] == '{';
    }
    let start = idx;
    while idx < slice.len()
        && (slice[idx].is_alphanumeric() || slice[idx] == '_' || slice[idx] == '`')
    {
        idx += 1;
    }
    if idx > start && idx < slice.len() && slice[idx] == '@' {
        idx += 1;
        while idx < slice.len() && slice[idx].is_whitespace() {
            idx += 1;
        }
        return idx < slice.len() && slice[idx] == '{';
    }
    false
}

/// Extract receiver expression from the text preceding a dot
fn step_receiver_scan(
    c: char,
    i: usize,
    chars: &[char],
    paren_depth: &mut usize,
    bracket_depth: &mut usize,
    brace_depth: &mut usize,
    start_idx: &mut usize,
) -> bool {
    match c {
        ')' => *paren_depth += 1,
        '(' => {
            if *paren_depth > 0 {
                *paren_depth -= 1;
                *start_idx = i;
            } else {
                *start_idx = i + 1;
                return true;
            }
        }
        ']' => *bracket_depth += 1,
        '[' => {
            if *bracket_depth > 0 {
                *bracket_depth -= 1;
                *start_idx = i;
            } else {
                *start_idx = i + 1;
                return true;
            }
        }
        '}' => *brace_depth += 1,
        '{' => {
            if *brace_depth > 0 {
                *brace_depth -= 1;
                *start_idx = i;
            } else {
                *start_idx = i + 1;
                return true;
            }
        }
        '?' if i + 1 < chars.len() && chars[i + 1] == '.' => {
            *start_idx = i;
        }
        ',' | ';' | '=' | '+' | '-' | '*' | '/' | '!' | '&' | '|' | '>' | '<' | '?' | ':' => {
            if *paren_depth == 0 && *bracket_depth == 0 && *brace_depth == 0 {
                *start_idx = i + 1;
                return true;
            }
        }
        c if c.is_whitespace() => {
            if *paren_depth == 0 && *bracket_depth == 0 && *brace_depth == 0 {
                let prev_non_ws = chars[..i].iter().rev().find(|&&ch| !ch.is_whitespace());
                let next_non_ws = chars[i + 1..].iter().find(|&&ch| !ch.is_whitespace());
                let is_dot_adjacent = prev_non_ws == Some(&'.') || next_non_ws == Some(&'.');
                let is_lambda_call = is_trailing_lambda_lookahead(&chars[i + 1..])
                    && prev_non_ws.is_some_and(|&ch| {
                        ch.is_alphanumeric()
                            || ch == '_'
                            || ch == '`'
                            || ch == ')'
                            || ch == ']'
                            || ch == '>'
                    });
                if !is_dot_adjacent && !is_lambda_call {
                    *start_idx = i + 1;
                    return true;
                }
            }
        }
        _ => {
            *start_idx = i;
        }
    }
    false
}

/// Extract receiver expression from the text preceding a dot
pub fn extract_receiver_from_prefix(before_dot: &str) -> Option<String> {
    let before_dot = before_dot.trim();
    if before_dot.is_empty() {
        return None;
    }

    let mut start_idx = before_dot.len();
    let chars: Vec<char> = before_dot.chars().collect();
    let (mut paren_depth, mut bracket_depth, mut brace_depth) = (0, 0, 0);

    let mut i = chars.len();
    while i > 0 {
        i -= 1;
        if step_receiver_scan(
            chars[i],
            i,
            &chars,
            &mut paren_depth,
            &mut bracket_depth,
            &mut brace_depth,
            &mut start_idx,
        ) {
            break;
        }
    }

    let receiver = chars[start_idx..]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    (!receiver.is_empty()).then_some(receiver)
}

/// Extract receiver expression and partial query from text before cursor
pub fn extract_receiver_and_partial(line_prefix: &str) -> Option<(String, String)> {
    let trimmed = line_prefix.trim_end();

    let (dot_idx, dot_len) = if let Some(idx) = trimmed.rfind("?.") {
        (idx, 2)
    } else {
        (trimmed.rfind('.')?, 1)
    };

    let partial = trimmed[dot_idx + dot_len..].trim().to_string();
    if !partial
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '`')
    {
        return None;
    }
    let before_dot = trimmed[..dot_idx].trim();

    let receiver = extract_receiver_from_prefix(before_dot)?;
    Some((receiver, partial))
}

/// Extract receiver expression before a given AST node
pub fn extract_receiver_before_node(node: &Node, source: &str) -> Option<String> {
    if let Some(parent) = node.parent()
        && parent.kind() == "infix_expression"
        && parent.child(1).is_some_and(|c| c == *node)
    {
        let left = parent.child(0)?;
        let text = source.get(left.start_byte()..left.end_byte())?.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    let start_byte = node.start_byte();
    if start_byte == 0 || start_byte > source.len() {
        return None;
    }
    let prefix = &source[..start_byte];
    let trimmed = prefix.trim_end();
    let before_dot = if let Some(stripped) = trimmed.strip_suffix("?.") {
        stripped
    } else if let Some(stripped) = trimmed.strip_suffix("::") {
        let last_line = stripped.lines().last().unwrap_or("");
        if last_line.trim().is_empty() {
            return None;
        }
        stripped
    } else {
        trimmed.strip_suffix('.')?
    };
    extract_receiver_from_prefix(before_dot)
}

fn is_preceding_control_keyword(s: &str) -> bool {
    matches!(s, "return" | "throw" | "else" | "by" | "do") || s.starts_with("return@")
}

fn ends_with_control_keyword(text: &str, keyword: &str) -> bool {
    if text.ends_with(keyword) {
        let prefix_len = text.len() - keyword.len();
        if prefix_len == 0 {
            return true;
        }
        let prev_byte = text.as_bytes()[prefix_len - 1];
        return !prev_byte.is_ascii_alphanumeric() && prev_byte != b'_';
    }
    false
}

fn is_control_flow_condition(before_colons: &str, receiver: &str) -> bool {
    if !receiver.starts_with('(') || !receiver.ends_with(')') {
        return false;
    }
    let Some(pos) = before_colons.rfind(receiver) else {
        return false;
    };
    let before_paren = before_colons[..pos].trim_end();
    ["if", "while", "when", "catch"]
        .iter()
        .any(|kw| ends_with_control_keyword(before_paren, kw))
}

/// Extract receiver expression and partial query for a callable reference before cursor
pub fn extract_callable_ref_receiver_and_partial(
    line_prefix: &str,
) -> Option<(Option<String>, String)> {
    let trimmed = line_prefix.trim_end();
    let colons_idx = trimmed.rfind("::")?;

    // If there is a dot AFTER the "::", then the dot is the active operator, not "::"
    if trimmed[colons_idx + 2..].contains('.') {
        return None;
    }

    let partial = trimmed[colons_idx + 2..].trim().to_string();
    if !partial
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '`')
    {
        return None;
    }

    let before_colons = trimmed[..colons_idx].trim();
    let receiver = extract_receiver_from_prefix(before_colons).filter(|rec| {
        !is_preceding_control_keyword(rec.as_str())
            && !is_control_flow_condition(before_colons, rec.as_str())
    });
    Some((receiver, partial))
}

/// Determine whether the cursor position is a member access or general scope completion
pub fn determine_completion_context(pos: Position, source: &str) -> CompletionContext {
    let offset = position_to_byte_offset(pos, source);
    let prefix = &source[..offset];
    let last_line = prefix.lines().last().unwrap_or("");

    let last_colons = last_line.rfind("::");
    let last_dot = last_line
        .rfind("?.")
        .map(|i| i + 1)
        .or_else(|| last_line.rfind('.'));

    let is_colons_after_dot = match (last_colons, last_dot) {
        (Some(colons_pos), Some(dot_pos)) => colons_pos > dot_pos,
        (Some(_), None) => true,
        (None, _) => false,
    };

    if is_colons_after_dot {
        if let Some((receiver, partial)) = extract_callable_ref_receiver_and_partial(last_line) {
            return CompletionContext::CallableReference { receiver, partial };
        }
    } else if let Some((receiver, partial)) = extract_receiver_and_partial(last_line) {
        return CompletionContext::MemberAccess { receiver, partial };
    }

    let is_continuation = last_line.trim_start().starts_with('.')
        || last_line.trim_start().starts_with("?.")
        || prefix
            .lines()
            .rev()
            .skip(1)
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim().ends_with('.') || l.trim().ends_with("?."));

    if is_continuation && let Some((receiver, partial)) = extract_receiver_and_partial(prefix) {
        return CompletionContext::MemberAccess { receiver, partial };
    }

    let is_colons_continuation = prefix
        .lines()
        .rev()
        .skip(1)
        .find(|l| !l.trim().is_empty())
        .is_some_and(|l| l.trim().ends_with("::"));

    if is_colons_continuation
        && let Some((receiver, partial)) = extract_callable_ref_receiver_and_partial(prefix)
    {
        return CompletionContext::CallableReference { receiver, partial };
    }

    let partial = last_line
        .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
        .to_string();

    CompletionContext::Scope { partial }
}
