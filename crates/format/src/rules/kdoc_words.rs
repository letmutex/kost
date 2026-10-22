fn is_list_item_token(word: &str) -> bool {
    if matches!(word, "-" | "*" | "+") {
        return true;
    }
    if let Some(first) = word.chars().next()
        && first.is_ascii_digit()
        && (word.ends_with('.') || word.ends_with(')'))
    {
        return true;
    }
    false
}

fn is_kdoc_tag(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('@') else {
        return false;
    };
    let tag = rest.split_whitespace().next().unwrap_or(rest);
    let tag_name = tag.split_once('[').map_or(tag, |(b, _)| b);
    tag_name.len() >= 2 && !tag.contains('@') && tag_name.chars().all(|c| c.is_ascii_lowercase())
}

fn can_break_at(prev: &str, word: &str) -> bool {
    if word.starts_with('#')
        || word.starts_with("```")
        || word.starts_with("<!--")
        || word.starts_with("-->")
        || is_kdoc_tag(word)
        || word.starts_with("TODO:")
        || word.starts_with("TODO(")
        || word.starts_with('>')
        || prev == "@sample"
    {
        return false;
    }
    if let Some(first) = word.chars().next()
        && !first.is_alphabetic()
        && is_list_item_token(word)
    {
        return false;
    }
    true
}

pub(crate) fn split_kdoc_words(text: &str, is_hanging: bool) -> Vec<String> {
    let raw: Vec<&str> = text.split_whitespace().collect();
    if raw.len() <= 1 {
        return raw.into_iter().map(String::from).collect();
    }
    let mut combined = Vec::new();
    let mut from = 0;
    while from < raw.len() {
        let start = if from == 0 && is_hanging {
            (from + 2).min(raw.len())
        } else {
            from + 1
        };
        let mut to = raw.len();
        for i in start..raw.len() {
            let next = raw[i];
            if next.starts_with('[') && !next.starts_with("[[") {
                let mut j = None;
                for (k, item) in raw.iter().enumerate().skip(i) {
                    if item.contains(']') {
                        j = Some(k);
                        break;
                    }
                }
                if let Some(j_idx) = j {
                    to = j_idx + 1;
                    while to < raw.len() && !can_break_at(raw[to - 1], raw[to]) {
                        to += 1;
                    }
                    break;
                }
            } else if can_break_at(raw[i - 1], next) {
                to = i;
                break;
            }
        }
        if to == from + 1 {
            combined.push(raw[from].to_string());
        } else if to > from {
            combined.push(raw[from..to].join(" "));
        }
        from = to;
    }
    combined
}

pub(crate) fn reflow_words(words: &[String], first_max: usize, cont_max: usize) -> Vec<String> {
    if words.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut is_first = true;
    for word in words {
        let limit = if is_first { first_max } else { cont_max };
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.len() + 1 + word.len() <= limit {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(cur);
            cur = word.clone();
            is_first = false;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
