fn skip_backticks(text: &str, start: usize, len: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start;
    let mut backtick_count = 0;
    while i < len && bytes[i] == b'`' {
        backtick_count += 1;
        i += 1;
    }
    let pattern = &text[start..start + backtick_count];
    if let Some(end_rel) = text[i..].find(pattern) {
        i + end_rel + backtick_count
    } else {
        len
    }
}

fn find_closing_bracket(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut i = start;
    let len = bytes.len();
    while i < len {
        if bytes[i] == b'[' {
            depth += 1;
        } else if bytes[i] == b']' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn try_render_image(text: &str, i: usize, len: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes[i] != b'!' || i + 1 >= len || bytes[i + 1] != b'[' {
        return None;
    }
    let close_idx = find_closing_bracket(text, i + 1)?;
    let after = close_idx + 1;
    if after < len && (bytes[after] == b'(' || bytes[after] == b'[') {
        let closing_char = if bytes[after] == b'(' { ')' } else { ']' };
        if let Some(close_second) = text[after + 1..].find(closing_char) {
            let end = after + 1 + close_second + 1;
            return Some((text[i..end].to_string(), end));
        }
    }
    None
}

fn try_render_bracket_link(text: &str, i: usize, len: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let close_idx = find_closing_bracket(text, i)?;
    let content = &text[i + 1..close_idx];
    let after = close_idx + 1;

    // Standard markdown link: [text](url) -> keep untouched
    if after < len && bytes[after] == b'(' {
        let close_paren = text[after..].find(')')?;
        let end = after + close_paren + 1;
        return Some((text[i..end].to_string(), end));
    }

    // Reference link: [label][target] -> render label as inline code
    if after < len
        && bytes[after] == b'['
        && let Some(close_second) = find_closing_bracket(text, after)
    {
        let end = close_second + 1;
        let target = &text[after + 1..close_second];
        let label = if content.is_empty() { target } else { content };
        if !label.is_empty() && label != " " && !label.eq_ignore_ascii_case("x") {
            let rendered = if label.starts_with('`') && label.ends_with('`') && label.len() >= 2 {
                label.to_string()
            } else {
                common::concat_str!("`", label, "`")
            };
            return Some((rendered, end));
        }
    }

    // Task list checkboxes: [ ] or [x] or [X] or empty [] -> keep untouched
    if content == " " || content.eq_ignore_ascii_case("x") || content.is_empty() {
        return Some((text[i..=close_idx].to_string(), close_idx + 1));
    }

    // Code link: [content] -> `content`
    let rendered = if content.starts_with('`') && content.ends_with('`') && content.len() >= 2 {
        content.to_string()
    } else {
        common::concat_str!("`", content, "`")
    };
    Some((rendered, close_idx + 1))
}

/// Render KDoc code links `[Symbol]` or `[label][target]` to inline code `` `Symbol` ``
pub fn render_kdoc_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 16);
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'`' {
            let end = skip_backticks(text, i, len);
            result.push_str(&text[i..end]);
            i = end;
            continue;
        }

        if let Some((rendered, end)) = try_render_image(text, i, len) {
            result.push_str(&rendered);
            i = end;
            continue;
        }

        if bytes[i] == b'['
            && let Some((rendered, end)) = try_render_bracket_link(text, i, len)
        {
            result.push_str(&rendered);
            i = end;
            continue;
        }

        let ch = text[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }

    result
}
