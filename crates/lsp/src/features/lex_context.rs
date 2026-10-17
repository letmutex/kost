use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

use crate::position::position_to_byte_offset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexMode {
    Expr(usize),
    LineComment,
    BlockComment(usize),
    SingleQuote,
    DoubleQuote,
    TripleQuote,
}

fn step_expr_mode(
    c: char,
    chars: &[char],
    i: &mut usize,
    depth: &mut usize,
    stack: &mut Vec<LexMode>,
) {
    let len = chars.len();
    if c == '/' && *i + 1 < len && chars[*i + 1] == '/' {
        stack.push(LexMode::LineComment);
        *i += 1;
    } else if c == '/' && *i + 1 < len && chars[*i + 1] == '*' {
        stack.push(LexMode::BlockComment(1));
        *i += 1;
    } else if c == '\'' {
        stack.push(LexMode::SingleQuote);
    } else if c == '"' && *i + 2 < len && chars[*i + 1] == '"' && chars[*i + 2] == '"' {
        stack.push(LexMode::TripleQuote);
        *i += 2;
    } else if c == '"' {
        stack.push(LexMode::DoubleQuote);
    } else if c == '{' {
        *depth += 1;
    } else if c == '}' {
        if *depth > 1 {
            *depth -= 1;
        } else if *depth == 1 && stack.len() > 1 {
            stack.pop();
        }
    }
}

fn step_string_or_comment(
    mode: &mut LexMode,
    c: char,
    chars: &[char],
    i: &mut usize,
    stack: &mut Vec<LexMode>,
) {
    let len = chars.len();
    match mode {
        LexMode::Expr(depth) => step_expr_mode(c, chars, i, depth, stack),
        LexMode::LineComment => {
            if c == '\n' {
                stack.pop();
            }
        }
        LexMode::BlockComment(depth) => {
            if c == '/' && *i + 1 < len && chars[*i + 1] == '*' {
                *depth += 1;
                *i += 1;
            } else if c == '*' && *i + 1 < len && chars[*i + 1] == '/' {
                *depth -= 1;
                if *depth == 0 {
                    stack.pop();
                }
                *i += 1;
            }
        }
        LexMode::SingleQuote => {
            if c == '\\' {
                *i += 1;
            } else if c == '\'' {
                stack.pop();
            }
        }
        LexMode::DoubleQuote => {
            if c == '\\' {
                *i += 1;
            } else if c == '"' {
                stack.pop();
            } else if c == '$' && *i + 1 < len && chars[*i + 1] == '{' {
                stack.push(LexMode::Expr(1));
                *i += 1;
            }
        }
        LexMode::TripleQuote => {
            if c == '"' && *i + 2 < len && chars[*i + 1] == '"' && chars[*i + 2] == '"' {
                stack.pop();
                *i += 2;
            } else if c == '$' && *i + 1 < len && chars[*i + 1] == '{' {
                stack.push(LexMode::Expr(1));
                *i += 1;
            }
        }
    }
}

/// Fallback character scanner to detect comments or plain string literals up to position
pub fn is_inside_comment_or_string_text(pos: Position, source: &str) -> bool {
    let offset = position_to_byte_offset(pos, source);
    let chars: Vec<char> = source[..offset].chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut stack = vec![LexMode::Expr(0)];

    while i < len {
        let c = chars[i];
        if let Some(mut mode) = stack.last().copied() {
            let old_len = stack.len();
            step_string_or_comment(&mut mode, c, &chars, &mut i, &mut stack);
            if stack.len() == old_len
                && let Some(top) = stack.last_mut()
            {
                *top = mode;
            }
        }
        i += 1;
    }

    !matches!(stack.last(), Some(LexMode::Expr(_)))
}

/// Check if cursor position is inside a comment or plain string literal
pub fn is_inside_comment_or_string(root: Option<Node>, pos: Position, source: &str) -> bool {
    let offset = position_to_byte_offset(pos, source);
    if let Some(root_node) = root {
        let query_byte = if offset > 0 { offset - 1 } else { 0 };
        let node_opt = root_node.descendant_for_byte_range(query_byte, query_byte);

        if let Some(leaf) = node_opt {
            let mut curr = Some(leaf);
            while let Some(n) = curr {
                let k = n.kind();
                if k == "line_comment" && offset > n.start_byte() && offset <= n.end_byte() {
                    return true;
                }
                if k.contains("comment") && offset > n.start_byte() && offset < n.end_byte() {
                    return true;
                }
                if k.contains("interpolation")
                    || k.contains("template")
                    || k.contains("interpolated")
                    || k.contains("string_expression")
                {
                    return false;
                }
                if (k.contains("string_literal") || k == "character_literal")
                    && offset > n.start_byte()
                    && offset < n.end_byte()
                {
                    return true;
                }
                curr = n.parent();
            }
        }
    }

    is_inside_comment_or_string_text(pos, source)
}
