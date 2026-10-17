use smol_str::SmolStr;

use super::params::{find_param_colon, find_parameter_list_open};
use super::raw::RawGenericParamInfo;
use super::type_args::split_type_arguments;

fn compute_string_ranges(s: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut in_string = false;
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                ranges.push((start, i));
            }
        } else if c == '"' {
            in_string = true;
            start = i;
        }
    }
    ranges
}

fn find_all_keyword_tokens(s: &str, kw: &str) -> Vec<usize> {
    let ranges = compute_string_ranges(s);
    let kw_len = kw.len();
    s.match_indices(kw)
        .filter_map(|(idx, _)| {
            if ranges
                .iter()
                .any(|&(start, end)| idx >= start && idx <= end)
            {
                return None;
            }
            let prev_ok = idx == 0
                || !(s[..idx].chars().next_back().unwrap().is_alphanumeric()
                    || s[..idx].ends_with('_')
                    || s[..idx].ends_with('$')
                    || (kw == "class" && s[..idx].ends_with(':')));
            let next_idx = idx + kw_len;
            let next_ok = next_idx >= s.len()
                || s[next_idx..]
                    .chars()
                    .next()
                    .is_some_and(|c| !c.is_alphanumeric() && c != '_' && c != '$');
            if prev_ok && next_ok { Some(idx) } else { None }
        })
        .collect()
}

fn match_angle_bracket(s: &str, start_idx: usize) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in s[start_idx..].char_indices() {
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
                    return Some((start_idx, start_idx + i));
                }
            }
            _ => {}
        }
    }
    None
}

fn find_type_param_bracket(sig: &str) -> Option<(usize, usize)> {
    for kw in &["fun", "val", "var"] {
        for idx in find_all_keyword_tokens(sig, kw) {
            let after = sig[idx + kw.len()..].trim_start();
            if after.starts_with('<')
                && let Some(p) = sig[idx + kw.len()..].find('<')
                && let Some(bracket) = match_angle_bracket(sig, idx + kw.len() + p)
            {
                return Some(bracket);
            }
        }
    }

    for kw in &["class", "interface", "typealias"] {
        for idx in find_all_keyword_tokens(sig, kw) {
            let after_kw = &sig[idx + kw.len()..];
            for (rel_idx, c) in after_kw.char_indices() {
                match c {
                    '<' => {
                        let start_idx = idx + kw.len() + rel_idx;
                        if let Some(bracket) = match_angle_bracket(sig, start_idx) {
                            return Some(bracket);
                        }
                        break;
                    }
                    ':' | '{' | '(' | '=' => break,
                    _ => {}
                }
            }
        }
    }

    let trimmed = sig.trim_start();
    if trimmed.starts_with('<') {
        let start_idx = sig.find('<')?;
        return match_angle_bracket(sig, start_idx);
    }

    None
}

pub fn extract_param_name(raw: &str) -> &str {
    raw.split_whitespace()
        .next_back()
        .unwrap_or(raw)
        .trim()
        .trim_matches('`')
}

fn parse_single_type_param(p: &str) -> Option<RawGenericParamInfo> {
    let trimmed = p.trim();
    if trimmed.is_empty() {
        return None;
    }
    let is_out = trimmed.starts_with("out ") || trimmed.contains(" out ");
    if let Some(colon_idx) = find_param_colon(trimmed) {
        let name = extract_param_name(&trimmed[..colon_idx]);
        let bound_part = trimmed[colon_idx + 1..].trim();
        if !name.is_empty() {
            let bounds = if bound_part.is_empty() {
                Vec::new()
            } else {
                vec![SmolStr::new(bound_part)]
            };
            return Some(RawGenericParamInfo {
                name: SmolStr::new(name),
                bounds,
                is_out,
            });
        }
    } else {
        let name = extract_param_name(trimmed);
        if !name.is_empty() {
            return Some(RawGenericParamInfo {
                name: SmolStr::new(name),
                bounds: Vec::new(),
                is_out,
            });
        }
    }
    None
}

fn find_header_end(clean: &str) -> usize {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    let bytes = clean.as_bytes();

    for (i, c) in clean.char_indices() {
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
            '(' | '[' | '<' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '>' if depth > 0 && (i == 0 || bytes[i - 1] != b'-') => {
                depth = depth.saturating_sub(1);
            }
            '{' | '=' if depth == 0 => return i,
            _ => {}
        }
    }
    clean.len()
}

fn find_where_search_start(clean: &str) -> usize {
    if let Some(open) = find_parameter_list_open(clean) {
        let mut depth = 0;
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in clean[open..].char_indices() {
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
                '(' => depth += 1,
                ')' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        return open + i + 1;
                    }
                }
                _ => {}
            }
        }
    }
    0
}

fn apply_where_clauses(clean: &str, params: &mut [RawGenericParamInfo]) {
    let header_end = find_header_end(clean);
    let header = &clean[..header_end];
    let search_start = find_where_search_start(header);
    for rel_where_idx in find_all_keyword_tokens(&header[search_start..], "where") {
        let where_idx = search_start + rel_where_idx;
        let after_where = &header[where_idx + 5..];
        for clause in split_type_arguments(after_where.trim()) {
            let trimmed = clause.trim();
            if let Some(colon_idx) = find_param_colon(trimmed) {
                let name = extract_param_name(&trimmed[..colon_idx]);
                let bound_part = trimmed[colon_idx + 1..].trim();
                if let Some(param) = params.iter_mut().find(|p| p.name == name) {
                    let bound_smol = SmolStr::new(bound_part);
                    if !param.bounds.contains(&bound_smol) {
                        param.bounds.push(bound_smol);
                    }
                }
            }
        }
    }
}

/// Parse type parameters from signature string returning RawGenericParamInfo
pub fn parse_signature_type_params(sig: &str) -> Vec<RawGenericParamInfo> {
    let clean = sig.trim();
    if clean.is_empty() {
        return Vec::new();
    }

    let mut params = Vec::new();
    if let Some((start_idx, end_idx)) = find_type_param_bracket(clean) {
        let inside = &clean[start_idx + 1..end_idx];
        for p in split_type_arguments(inside) {
            if let Some(info) = parse_single_type_param(&p) {
                params.push(info);
            }
        }
    }

    apply_where_clauses(clean, &mut params);
    params
}
