use smol_str::SmolStr;

use crate::doc::Doc;

fn has_template_expr(s: &str, is_dollar: bool) -> bool {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if is_dollar {
            if i + 2 < n && bytes[i] == b'$' && bytes[i + 1] == b'$' {
                let next = bytes[i + 2];
                if next == b'{' || next.is_ascii_alphabetic() || next == b'_' {
                    return true;
                }
                i += 2;
                continue;
            }
        } else if bytes[i] == b'$' {
            if i > 0 && bytes[i - 1] == b'\\' {
                i += 1;
                continue;
            }
            if i + 1 < n {
                let next = bytes[i + 1];
                if next == b'{' || next.is_ascii_alphabetic() || next == b'_' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn process_trim_margin_lines(
    raw_lines: &[&str],
    last_before_quote: &str,
) -> (Vec<SmolStr>, &'static str) {
    let mut lines = raw_lines.to_vec();
    let mut closing_quote = "\"\"\"";
    if last_before_quote.trim_start() == "|" {
        closing_quote = "|\"\"\"";
    } else if lines
        .last()
        .is_some_and(|l| l.trim_start() == "|" || l.is_empty())
    {
        lines.pop();
        closing_quote = "|\"\"\"";
    }

    let processed = lines
        .into_iter()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('|') {
                SmolStr::new(trimmed)
            } else {
                SmolStr::new(common::concat_str!("|", line))
            }
        })
        .collect();
    (processed, closing_quote)
}

fn process_trim_indent_lines(raw_lines: &[&str]) -> (Vec<SmolStr>, &'static str) {
    let min_indent = raw_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let processed = raw_lines
        .iter()
        .map(|&line| {
            if line.trim().is_empty() {
                SmolStr::default()
            } else if line.len() >= min_indent {
                SmolStr::new(&line[min_indent..])
            } else {
                SmolStr::new(line.trim_start())
            }
        })
        .collect();
    (processed, "\"\"\"")
}

pub fn format_multiline_raw_string(raw_str: &str, is_trim_margin: bool) -> Option<Doc<'static>> {
    let is_dollar = raw_str.starts_with("$$\"\"\"");
    let quote_prefix = if is_dollar { "$$\"\"\"" } else { "\"\"\"" };

    if !raw_str.starts_with(quote_prefix) || !raw_str.ends_with("\"\"\"") {
        return None;
    }

    if has_template_expr(raw_str, is_dollar) {
        return None;
    }

    let lines: Vec<&str> = raw_str.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    let first_after_quote = lines[0].strip_prefix(quote_prefix).unwrap_or("");
    let last_before_quote = lines[lines.len() - 1].strip_suffix("\"\"\"").unwrap_or("");

    let mut content_lines = Vec::new();
    if !first_after_quote.trim().is_empty() {
        content_lines.push(first_after_quote);
    }
    if lines.len() > 2 {
        for &line in &lines[1..lines.len() - 1] {
            content_lines.push(line);
        }
    }
    if !last_before_quote.trim().is_empty() && last_before_quote.trim_start() != "|" {
        content_lines.push(last_before_quote);
    }

    let (processed_lines, closing_quote) = if is_trim_margin {
        process_trim_margin_lines(&content_lines, last_before_quote)
    } else {
        process_trim_indent_lines(&content_lines)
    };

    let mut docs = Vec::with_capacity(processed_lines.len() * 2 + 2);
    docs.push(Doc::text(quote_prefix));
    for line in processed_lines {
        if line.is_empty() {
            docs.push(Doc::EmptyLine);
        } else {
            docs.push(Doc::HardLine);
            docs.push(Doc::smol(line));
        }
    }
    docs.push(Doc::HardLine);
    docs.push(Doc::text(closing_quote));

    Some(Doc::preserve_trailing(Doc::Concat(docs)))
}
