use super::generics::split_type_arguments;

/// Call info extracted from an expression string (e.g. `listOf<Int>(1, 2)` or `foo.bar(x)`)
#[derive(Debug, Clone)]
pub struct CallInfo<'a> {
    pub callee: &'a str,
    pub explicit_type_args: Vec<String>,
    pub value_args: Vec<&'a str>,
}

/// Parse a call expression into callee name, explicit type arguments, and value arguments
pub fn parse_call_expression(call_expr: &str) -> CallInfo<'_> {
    let clean = call_expr.trim();
    if let Some(lparen) = clean.find('(')
        && clean.ends_with(')')
    {
        let before_paren = clean[..lparen].trim();
        let inside_paren = clean[lparen + 1..clean.len() - 1].trim();
        let (callee, explicit_type_args) = parse_callee_and_type_args(before_paren);
        let value_args = if inside_paren.is_empty() {
            Vec::new()
        } else {
            split_call_arguments(inside_paren)
        };
        return CallInfo {
            callee,
            explicit_type_args,
            value_args,
        };
    }

    let (callee, explicit_type_args) = parse_callee_and_type_args(clean);
    CallInfo {
        callee,
        explicit_type_args,
        value_args: Vec::new(),
    }
}

pub fn parse_callee_and_type_args(before_paren: &str) -> (&str, Vec<String>) {
    if before_paren.ends_with('>')
        && let Some(angle_idx) = before_paren.find('<')
    {
        let callee = before_paren[..angle_idx].trim();
        let inside_angles = before_paren[angle_idx + 1..before_paren.len() - 1].trim();
        let explicit_args = split_type_arguments(inside_angles);
        return (callee, explicit_args);
    }
    (before_paren, Vec::new())
}

/// Split comma-separated call arguments respecting strings, angle brackets, parentheses, and braces
pub fn split_call_arguments(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut start = 0;
    let bytes = s.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' && in_string {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        match b {
            b'<' | b'(' | b'{' | b'[' => depth += 1,
            b'>' | b')' | b'}' | b']' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                let trimmed = s[start..i].trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let trimmed = s[start..].trim();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }
    parts
}
