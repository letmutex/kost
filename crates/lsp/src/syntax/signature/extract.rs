use super::is_kotlin_modifier;
use super::type_args::split_top_level_csv;

fn find_fun_keyword_index(s: &str) -> Option<usize> {
    for (idx, _) in s.match_indices("fun") {
        let prev_ok = idx == 0
            || s[..idx]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || c == ')' || c == '>');
        let next_idx = idx + 3;
        let next_ok = next_idx < s.len()
            && s[next_idx..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '<' || c == '(');
        if prev_ok && next_ok {
            return Some(idx);
        }
    }
    None
}

fn is_class_or_interface_signature(sig: &str) -> bool {
    let mut depth = 0;
    for token in sig.split_whitespace() {
        if depth > 0 {
            for c in token.chars() {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                }
            }
            continue;
        }
        if token.starts_with('@') {
            for c in token.chars() {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                }
            }
            continue;
        }
        if is_kotlin_modifier(token) {
            continue;
        }
        return matches!(token, "class" | "interface" | "object");
    }
    false
}

/// Extract return type from a signature string (e.g. `fun get(): User` -> `User`)
pub fn extract_return_type_from_signature(sig: &str) -> Option<String> {
    let clean = sig.trim();
    if is_class_or_interface_signature(clean) {
        return None;
    }
    let search_slice = if let Some(fun_idx) = find_fun_keyword_index(clean) {
        let lparen_idx = clean[fun_idx..].find('(').map(|i| fun_idx + i)?;
        let mut depth = 0;
        let mut rparen_idx = None;
        for (idx, c) in clean[lparen_idx..].char_indices() {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    rparen_idx = Some(lparen_idx + idx);
                    break;
                }
            }
        }
        &clean[rparen_idx? + 1..]
    } else {
        clean
    };

    if let Some(colon_idx) = search_slice.find(':') {
        let after_colon = search_slice[colon_idx + 1..].trim();
        if let Some(type_part) = extract_type_from_after_colon(after_colon) {
            return Some(type_part);
        }
    }

    if let Some(eq_idx) = search_slice.find('=') {
        let after_eq = search_slice[eq_idx + 1..].trim();
        let first_token = after_eq
            .split(|c: char| c == '(' || c == '{' || c == '<' || c.is_whitespace())
            .next()?
            .split('.')
            .next_back()?
            .trim();
        if !first_token.is_empty() && first_token.chars().next().is_some_and(|c| c.is_uppercase()) {
            return Some(first_token.to_string());
        }
    }

    None
}

fn extract_type_from_after_colon(after_colon: &str) -> Option<String> {
    let clean = after_colon.trim();
    let mut type_str = String::new();
    let mut angle_depth = 0;
    let mut paren_depth = 0;

    for (idx, c) in clean.char_indices() {
        if angle_depth == 0 && paren_depth == 0 {
            if c == ')' || c == '{' || c == '=' || c == ';' {
                break;
            }
            let rem = clean[idx..].trim_start();
            if rem.starts_with("where ")
                || rem.starts_with("where\t")
                || rem.starts_with("where\n")
                || rem.starts_with("where\r")
                || rem == "where"
                || rem.starts_with("by ")
                || rem.starts_with("by\t")
                || rem.starts_with("by\n")
                || rem.starts_with("by\r")
                || rem == "by"
            {
                break;
            }
        }

        match c {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            _ => {}
        }
        type_str.push(c);
    }
    let trimmed = type_str.trim().trim_end_matches('?').trim();
    if !trimmed.is_empty() {
        Some(type_str.trim().to_string())
    } else {
        None
    }
}

/// Extract the type of a function parameter at `param_index` from signature string
pub fn extract_param_type_by_index(sig: &str, param_index: usize) -> Option<String> {
    let lparen_idx = sig.find('(')?;
    let mut depth = 0;
    let mut rparen_idx = None;
    for (idx, c) in sig[lparen_idx..].char_indices() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                rparen_idx = Some(lparen_idx + idx);
                break;
            }
        }
    }
    let params_str = &sig[lparen_idx + 1..rparen_idx?];
    let params = split_top_level_csv(params_str);
    let target_param = params.get(param_index).or_else(|| params.last())?;
    let colon_idx = super::params::find_param_colon(target_param)?;
    extract_type_from_after_colon(&target_param[colon_idx + 1..])
}
