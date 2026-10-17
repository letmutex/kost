pub fn split_top_level_csv(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;
    let mut angle_depth = 0;

    for c in text.chars() {
        match c {
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if paren_depth == 0 && angle_depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

pub fn strip_type_annotations(mut s: &str) -> &str {
    s = s.trim();
    while s.starts_with('@') {
        let after = &s[1..];
        let end = match after.find(|c: char| c.is_whitespace() || c == '(') {
            Some(idx) if after.as_bytes()[idx] == b'(' => {
                let (mut d, mut e) = (0, idx + 1);
                for (i, c) in after[idx..].char_indices() {
                    if c == '(' {
                        d += 1;
                    } else if c == ')' {
                        d -= 1;
                        if d == 0 {
                            e = idx + i + 1;
                            break;
                        }
                    }
                }
                e + 1
            }
            Some(idx) => idx + 1,
            None => s.len(),
        };
        s = s[end.min(s.len())..].trim_start();
    }
    s
}

/// Split comma-separated type arguments respecting nested angle brackets and parentheses
pub fn split_type_arguments(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    let bytes = s.as_bytes();
    for (i, c) in s.char_indices() {
        if in_string {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => in_string = true,
            '<' | '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '>' if i == 0 || bytes[i - 1] != b'-' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

/// Extract type arguments from a generic type string (e.g. `Map<String, User>` -> `["String", "User"]`)
pub fn extract_type_arguments_list(type_str: &str) -> Vec<String> {
    let start_idx = match type_str.find('<') {
        Some(s) => s,
        None => return Vec::new(),
    };
    let bytes = type_str.as_bytes();
    let mut depth = 0;
    let mut end_idx = None;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in type_str[start_idx..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '<' => depth += 1,
            '>' if depth > 0 && (i == 0 || bytes[start_idx + i - 1] != b'-') => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(start_idx + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end_idx else {
        return Vec::new();
    };
    split_type_arguments(&type_str[start_idx + 1..end])
}
