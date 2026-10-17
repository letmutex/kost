use smol_str::SmolStr;

/// Helper to locate declaration keywords (`fun`, `val`, `var`) outside annotations and arguments
fn find_declaration_keyword_end(sig: &str) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut in_quote = false;
    let mut escaped = false;

    for (idx, c) in sig.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_quote = false;
            }
            continue;
        }
        if c == '"' {
            in_quote = true;
            continue;
        }
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ if paren_depth == 0 && bracket_depth == 0 => {
                for kw in &["fun", "val", "var"] {
                    if sig[idx..].starts_with(kw) {
                        let prev_ok = idx == 0
                            || sig[..idx]
                                .chars()
                                .next_back()
                                .is_some_and(|p| p.is_whitespace() || p == ')' || p == '>');
                        let next = idx + kw.len();
                        let next_ok = next < sig.len()
                            && sig[next..]
                                .chars()
                                .next()
                                .is_some_and(|n| n.is_whitespace() || n == '<');
                        if prev_ok && next_ok {
                            return Some(next);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Strip leading generic type parameters `<...>` on function or property
fn strip_leading_generic_params(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0usize;
    let bytes = s.as_bytes();
    for (idx, c) in s.char_indices() {
        if c == '<' {
            depth += 1;
        } else if c == '>' && (idx == 0 || bytes[idx - 1] != b'-') {
            depth -= 1;
            if depth == 0 {
                return s[idx + 1..].trim_start();
            }
        }
    }
    s
}

/// Find boundary ending the declaration header (before parameters or return type)
fn find_header_end(s: &str) -> usize {
    let mut paren_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut in_backtick = false;
    let mut saw_receiver_or_name = false;
    let bytes = s.as_bytes();

    for (idx, c) in s.char_indices() {
        if c == '`' {
            in_backtick = !in_backtick;
            saw_receiver_or_name = true;
            continue;
        }
        if in_backtick {
            continue;
        }
        match c {
            '(' => {
                if paren_depth == 0 && angle_depth == 0 && saw_receiver_or_name {
                    return idx;
                }
                paren_depth += 1;
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                if paren_depth == 0 && angle_depth == 0 {
                    saw_receiver_or_name = true;
                }
            }
            '<' => angle_depth += 1,
            '>' if idx == 0 || bytes[idx - 1] != b'-' => {
                angle_depth = angle_depth.saturating_sub(1);
                if paren_depth == 0 && angle_depth == 0 {
                    saw_receiver_or_name = true;
                }
            }
            ':' | '=' | '{' if paren_depth == 0 && angle_depth == 0 => {
                return idx;
            }
            c if !c.is_whitespace() && c != '.' && paren_depth == 0 && angle_depth == 0 => {
                saw_receiver_or_name = true;
            }
            _ => {}
        }
    }
    s.len()
}

/// Locate the extension dot preceding the member name at depth 0
fn find_extension_dot(header: &str) -> Option<usize> {
    let mut dot_idx = None;
    let mut p_depth = 0usize;
    let mut a_depth = 0usize;
    let mut in_bt = false;
    let bytes = header.as_bytes();

    for (idx, c) in header.char_indices() {
        if c == '`' {
            in_bt = !in_bt;
            continue;
        }
        if in_bt {
            continue;
        }
        match c {
            '(' => p_depth += 1,
            ')' => p_depth = p_depth.saturating_sub(1),
            '<' => a_depth += 1,
            '>' if idx == 0 || bytes[idx - 1] != b'-' => a_depth = a_depth.saturating_sub(1),
            '.' if p_depth == 0 && a_depth == 0 => dot_idx = Some(idx),
            _ => {}
        }
    }
    dot_idx
}

/// Extract extension receiver type from signature as String
pub fn extract_receiver_type_from_signature(sig: &str) -> Option<String> {
    let kw_end = find_declaration_keyword_end(sig)?;
    let after_kw = sig[kw_end..].trim_start();
    let after_gen = strip_leading_generic_params(after_kw);

    let header_end = find_header_end(after_gen);
    let header = after_gen[..header_end].trim();

    let dot_idx = find_extension_dot(header)?;
    let rec = header[..dot_idx].trim();
    (!rec.is_empty()).then_some(rec.to_string())
}

/// Extract extension receiver type from signature as SmolStr
pub fn extract_receiver_type(sig: &str) -> Option<SmolStr> {
    extract_receiver_type_from_signature(sig).map(SmolStr::new)
}
