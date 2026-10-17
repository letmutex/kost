use smol_str::SmolStr;

use super::raw::RawSignatureParam;
use super::type_args::split_top_level_csv;
use crate::syntax::ast_call_view::find_top_level_eq;

pub fn find_parameter_list_open(sig: &str) -> Option<usize> {
    let start = ["fun ", "class ", "constructor"]
        .iter()
        .find_map(|k| sig.find(k).map(|p| p + k.len()))
        .unwrap_or(0);
    let (mut depth, mut last_dot) = (0, None);
    let bytes = &sig.as_bytes()[start..];
    for (i, c) in sig[start..].char_indices() {
        match c {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            '>' if depth > 0 && (i == 0 || bytes[i - 1] != b'-') => depth -= 1,
            ':' if depth == 0 => break,
            '.' if depth == 0 => last_dot = Some(i),
            _ => {}
        }
    }
    let fn_start = last_dot.map(|d| start + d + 1).unwrap_or(start);
    let mut d = 0;
    let fn_bytes = &sig.as_bytes()[fn_start..];
    for (i, c) in sig[fn_start..].char_indices() {
        match c {
            '<' | '[' | '{' => d += 1,
            ']' | '}' if d > 0 => d -= 1,
            '>' if d > 0 && (i == 0 || fn_bytes[i - 1] != b'-') => d -= 1,
            '(' if d == 0 => return Some(fn_start + i),
            _ => {}
        }
    }
    sig.find('(')
}

pub fn find_param_colon(s: &str) -> Option<usize> {
    let mut paren_depth = 0;
    let mut in_string = false;
    let mut in_annotation = false;
    let bytes = s.as_bytes();
    let mut chars = s.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if in_string {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => in_string = true,
            '@' => in_annotation = true,
            '(' | '[' | '{' | '<' => paren_depth += 1,
            ')' | ']' | '}' if paren_depth > 0 => {
                paren_depth -= 1;
            }
            '>' if paren_depth > 0 && (i == 0 || bytes[i - 1] != b'-') => {
                paren_depth -= 1;
            }
            c if c.is_whitespace() && paren_depth == 0 => {
                in_annotation = false;
            }
            ':' if paren_depth == 0 && !in_annotation => {
                let next_is_colon = chars.peek().map(|&(_, nc)| nc == ':').unwrap_or(false);
                if next_is_colon {
                    chars.next();
                } else {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse_single_param(raw: &str) -> Option<RawSignatureParam> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let colon_idx = find_param_colon(trimmed)?;
    let (name_part, type_and_default) = (&trimmed[..colon_idx], &trimmed[colon_idx + 1..]);
    let is_vararg = name_part.starts_with("vararg ") || name_part.contains(" vararg ");
    let name = SmolStr::new(
        name_part
            .split_whitespace()
            .next_back()
            .unwrap_or(name_part)
            .trim()
            .trim_matches('`'),
    );

    let (type_name, has_default) = match find_top_level_eq(type_and_default) {
        Some(eq_idx) => (SmolStr::new(type_and_default[..eq_idx].trim()), true),
        None => (SmolStr::new(type_and_default.trim()), false),
    };

    Some(RawSignatureParam {
        name,
        type_name,
        has_default,
        is_vararg,
    })
}

/// Find the character index bounds (open_paren_idx, close_paren_idx) of a Kotlin parameter list
pub fn find_parameter_list_bounds(sig: &str) -> Option<(usize, usize)> {
    let open_idx = find_parameter_list_open(sig)?;
    let mut depth = 0;
    for (idx, character) in sig[open_idx..].char_indices() {
        if character == '(' {
            depth += 1;
        } else if character == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((open_idx, open_idx + idx));
            }
        }
    }
    None
}

/// Parse parameter specifications from a Kotlin signature string
pub fn parse_signature_params(sig: &str) -> Vec<RawSignatureParam> {
    let Some((open_idx, close_idx)) = find_parameter_list_bounds(sig) else {
        return Vec::new();
    };
    let params_str = sig[open_idx + 1..close_idx].trim();
    if params_str.is_empty() {
        return Vec::new();
    }
    split_top_level_csv(params_str)
        .into_iter()
        .filter_map(|param| parse_single_param(&param))
        .collect()
}
