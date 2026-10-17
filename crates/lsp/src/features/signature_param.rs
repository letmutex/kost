use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel,
};

pub fn parse_signature_parameters(sig: &str, doc: Option<&str>) -> Vec<ParameterInformation> {
    let Some(open_paren) = crate::syntax::signature::find_parameter_list_open(sig) else {
        return Vec::new();
    };
    let Some(close_paren) = find_matching_paren(sig, open_paren) else {
        return Vec::new();
    };

    let params_str = &sig[open_paren + 1..close_paren];
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    let raw_params = split_top_level_commas(params_str);

    raw_params
        .into_iter()
        .map(|param_text| {
            let trimmed = param_text.trim();
            let param_name = extract_param_name(trimmed);
            let param_doc = doc.and_then(|d| extract_param_doc_from_kdoc(d, param_name));
            ParameterInformation {
                label: ParameterLabel::Simple(trimmed.to_string()),
                documentation: param_doc.map(|d| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: d,
                    })
                }),
            }
        })
        .collect()
}

pub fn find_matching_paren(sig: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0;
    for (idx, c) in sig[open_idx..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + idx);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn split_top_level_commas(params_str: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut angle_depth = 0;
    let mut start = 0;

    for (idx, c) in params_str.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if depth == 0 && angle_depth == 0 => {
                result.push(&params_str[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start < params_str.len() {
        result.push(&params_str[start..]);
    }
    result
}

pub fn extract_param_name(param: &str) -> &str {
    let without_val = param
        .trim_start_matches("val ")
        .trim_start_matches("var ")
        .trim_start_matches("vararg ")
        .trim_start_matches("crossinline ")
        .trim_start_matches("noinline ")
        .trim();

    let name_part = without_val.split(':').next().unwrap_or(without_val).trim();
    name_part.split('=').next().unwrap_or(name_part).trim()
}

pub fn extract_param_doc_from_kdoc(doc: &str, param_name: &str) -> Option<String> {
    for line in doc.lines() {
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed
            .strip_prefix("* **@param** `")
            .or_else(|| trimmed.strip_prefix("**@param** `"))
        {
            if let Some((p, d)) = r.split_once('`') {
                if p == param_name { Some(d) } else { None }
            } else {
                None
            }
        } else if let Some(r) = trimmed
            .strip_prefix('*')
            .unwrap_or(trimmed)
            .trim_start()
            .strip_prefix("@param")
        {
            let (p, d) = r
                .trim()
                .split_once(char::is_whitespace)
                .unwrap_or((r.trim(), ""));
            let p = p.trim_matches(|c| c == '[' || c == ']' || c == '`');
            if p == param_name { Some(d) } else { None }
        } else {
            None
        };

        if let Some(desc_raw) = rest {
            let desc = desc_raw
                .trim()
                .trim_start_matches([' ', '—', '-', ':'])
                .trim();
            if !desc.is_empty() {
                return Some(desc.to_string());
            }
        }
    }
    None
}
