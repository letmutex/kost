use std::iter::Peekable;
use std::str::Chars;
use tree_sitter::Node;

/// Find the baseline indentation (number of leading spaces) of the declaration in the source
pub fn find_base_indent(node: &Node, source: &str) -> usize {
    let start_byte = node.start_byte().min(source.len());
    let prefix = &source[..start_byte];
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_prefix = &source[line_start..start_byte];
    if line_prefix.chars().all(|c| c.is_whitespace()) {
        line_prefix.len()
    } else {
        0
    }
}

/// Dedent multi-line text by removing `base_indent` spaces from subsequent lines
pub fn dedent_with_base(text: &str, base_indent: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    if lines.len() == 1 {
        return lines[0].trim().to_string();
    }

    let mut result = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            result.push("");
        } else if i == 0 {
            result.push(trimmed.trim_start());
        } else if line.len() >= base_indent {
            result.push(&line[base_indent..]);
        } else {
            result.push(line.trim_start());
        }
    }

    result.join("\n").trim().to_string()
}

/// Dedent multi-line text to remove shared base indentation
pub fn dedent_and_normalize(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    if lines.len() == 1 {
        return lines[0].trim().to_string();
    }

    let min_indent = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);

    dedent_with_base(text, min_indent)
}

fn parse_block_comment_in_sig(chars: &mut Peekable<Chars>) -> String {
    let is_kdoc = chars.peek() == Some(&'*');
    let mut comment_content = String::new();
    if is_kdoc {
        comment_content.push_str("/**");
        chars.next();
    } else {
        comment_content.push_str("/*");
    }

    let mut closed = false;
    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            comment_content.push_str("*/");
            closed = true;
            break;
        } else {
            comment_content.push(ch);
        }
    }

    if !closed {
        comment_content.push_str("*/");
    }

    comment_content
}

fn parse_line_comment_in_sig(chars: &mut Peekable<Chars>, clean_text: &mut String) -> String {
    let mut line_comment = String::new();
    for ch in chars.by_ref() {
        if ch == '\n' {
            clean_text.push('\n');
            break;
        } else if ch != '\r' {
            line_comment.push(ch);
        }
    }
    line_comment
}

fn skip_string_in_sig(chars: &mut Peekable<Chars>, clean_text: &mut String) {
    while let Some(ch) = chars.next() {
        clean_text.push(ch);
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                clean_text.push(escaped);
            }
        } else if ch == '"' {
            break;
        }
    }
}

/// Strip inline (//) and block (/* ... */) comments from signature text with base indentation
pub fn strip_comments_from_signature_with_base(
    text: &str,
    base_indent: usize,
) -> (String, Vec<String>) {
    if !text.contains('/') {
        if !text.contains('\n') {
            return (text.trim().to_string(), Vec::new());
        }
        let clean_sig = dedent_with_base(text, base_indent);
        return (clean_sig, Vec::new());
    }

    let mut clean_text = String::with_capacity(text.len());
    let mut extracted_comments = Vec::new();
    let mut saw_paren = false;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '(' {
            saw_paren = true;
            clean_text.push(c);
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let comment_content = parse_block_comment_in_sig(&mut chars);
            let cleaned = crate::doc::clean_kdoc(&comment_content);
            if !cleaned.is_empty() && !saw_paren {
                extracted_comments.push(cleaned);
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            let line_comment = parse_line_comment_in_sig(&mut chars, &mut clean_text);
            let trimmed = line_comment.trim();
            if !trimmed.is_empty() && !saw_paren {
                let cleaned = crate::doc::clean_kdoc(trimmed);
                if !cleaned.is_empty() {
                    extracted_comments.push(cleaned);
                }
            }
        } else if c == '"' {
            clean_text.push(c);
            skip_string_in_sig(&mut chars, &mut clean_text);
        } else {
            clean_text.push(c);
        }
    }

    let mut clean_lines = Vec::new();
    for line in clean_text.lines() {
        let trimmed = line.trim_end();
        if !trimmed.trim().is_empty() {
            clean_lines.push(trimmed);
        }
    }

    let clean_sig = dedent_with_base(&clean_lines.join("\n"), base_indent);
    (clean_sig, extracted_comments)
}

pub fn strip_comments_from_signature(text: &str) -> (String, Vec<String>) {
    strip_comments_from_signature_with_base(text, 0)
}
