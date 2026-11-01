use ahash::AHashMap;

use crate::coordinate::DependencyCoordinate;

pub(crate) fn parse_script_dependencies(
    content: &str,
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    let cleaned = strip_script_comments(content);
    let mut active_props = properties.clone();
    extract_script_variables(&cleaned, &mut active_props);

    static SCRIPT_DEP_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let dep_regex = SCRIPT_DEP_RE.get_or_init(|| {
        regex::Regex::new(r#"(?:[a-zA-Z0-9_]*(?:[iI]mplementation|[aA]pi|[cC]ompileOnly|[rR]untimeOnly)|ksp|annotationProcessor)\s*\(?\s*(?:(?:platform|enforcedPlatform)\s*\(\s*)?["']([^"'\r\n]+)["']"#).unwrap()
    });

    for capture in dep_regex.captures_iter(&cleaned) {
        if let Some(coord_str) = capture.get(1) {
            let substituted = substitute_script_variables(coord_str.as_str(), &active_props);
            if let Some(coord) = DependencyCoordinate::parse(&substituted) {
                deps.push(coord);
            }
        }
    }

    parse_named_script_dependencies(&cleaned, &active_props, deps);
}

fn extract_script_variables(cleaned: &str, vars: &mut AHashMap<String, String>) {
    static SCRIPT_VAR_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let var_regex = SCRIPT_VAR_RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?:(?:def|val|var)\s+|ext\.)([a-zA-Z0-9_]+)\s*(?::\s*[a-zA-Z0-9_<>]+)?\s*=\s*["']([^"'\r\n]+)["']"#,
        )
        .unwrap()
    });
    for capture in var_regex.captures_iter(cleaned) {
        if let (Some(key_match), Some(val_match)) = (capture.get(1), capture.get(2)) {
            vars.insert(
                key_match.as_str().to_string(),
                val_match.as_str().to_string(),
            );
        }
    }
}

fn parse_named_script_dependencies(
    cleaned: &str,
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    static SCRIPT_NAMED_DEP_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let named_re = SCRIPT_NAMED_DEP_RE.get_or_init(|| {
        regex::Regex::new(r#"(?:group\s*[:=]\s*["']([^"'\r\n]+)["']\s*,\s*name\s*[:=]\s*["']([^"'\r\n]+)["'](?:\s*,\s*version\s*[:=]\s*["']([^"'\r\n]+)["'])?|name\s*[:=]\s*["']([^"'\r\n]+)["']\s*,\s*group\s*[:=]\s*["']([^"'\r\n]+)["'](?:\s*,\s*version\s*[:=]\s*["']([^"'\r\n]+)["'])?)"#).unwrap()
    });

    for capture in named_re.captures_iter(cleaned) {
        let (group_raw, name_raw, ver_raw) = if let Some(group_match) = capture.get(1) {
            (
                group_match.as_str(),
                capture.get(2).map_or("", |mat| mat.as_str()),
                capture.get(3),
            )
        } else if let Some(name_match) = capture.get(4) {
            (
                capture.get(5).map_or("", |mat| mat.as_str()),
                name_match.as_str(),
                capture.get(6),
            )
        } else {
            continue;
        };
        let group = substitute_script_variables(group_raw, properties);
        let name = substitute_script_variables(name_raw, properties);
        let ver = ver_raw.map(|mat| substitute_script_variables(mat.as_str(), properties));
        if !group.is_empty() && !name.is_empty() {
            deps.push(DependencyCoordinate::new(group, name, ver.as_deref()));
        }
    }
}

fn substitute_script_variables(raw: &str, properties: &AHashMap<String, String>) -> String {
    if !raw.contains('$') || properties.is_empty() {
        return raw.to_string();
    }
    let mut output = String::with_capacity(raw.len());
    let mut rest = raw;

    while let Some(dollar_idx) = rest.find('$') {
        output.push_str(&rest[..dollar_idx]);
        let after = &rest[dollar_idx + 1..];

        if let Some(braced) = after.strip_prefix('{')
            && let Some(end_idx) = braced.find('}')
        {
            match properties.get(&braced[..end_idx]) {
                Some(val) => output.push_str(val),
                None => output.push_str(&rest[dollar_idx..dollar_idx + 3 + end_idx]),
            }
            rest = &braced[end_idx + 1..];
            continue;
        }

        let ident_len = after
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            .count();
        if ident_len > 0 {
            match properties.get(&after[..ident_len]) {
                Some(val) => output.push_str(val),
                None => output.push_str(&rest[dollar_idx..dollar_idx + 1 + ident_len]),
            }
            rest = &after[ident_len..];
            continue;
        }
        output.push('$');
        rest = after;
    }
    output.push_str(rest);
    output
}

pub(crate) fn strip_script_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut in_string: Option<char> = None;
    let mut is_escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                result.push('\n');
            }
        } else if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                result.push(' ');
            } else if ch == '\n' {
                result.push('\n');
            }
        } else if let Some(quote) = in_string {
            result.push(ch);
            if is_escaped {
                is_escaped = false;
            } else if ch == '\\' {
                is_escaped = true;
            } else if ch == quote {
                in_string = None;
            }
        } else if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            result.push(ch);
        } else if ch == '/' && chars.next_if_eq(&'/').is_some() {
            in_line_comment = true;
        } else if ch == '/' && chars.next_if_eq(&'*').is_some() {
            in_block_comment = true;
        } else {
            result.push(ch);
        }
    }
    result
}
