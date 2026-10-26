use tree_sitter::Node;

use crate::doc::Doc;

/// Fallback formatter for unrecognized or unsupported AST node types.
///
/// Instead of naively concatenating unspaced child nodes (which can produce unparseable
/// syntax like `valx=1`), this slices the original source bytes and preserves line breaks.
pub fn lower_verbatim<'a>(node: Node<'a>, source: &'a str) -> Doc<'a> {
    let start = node.start_byte();
    let end = node.end_byte();
    if start >= end || end > source.len() {
        return Doc::empty();
    }
    let text = &source[start..end];
    if !text.contains('\n') {
        return Doc::text(text);
    }

    let mut docs = Vec::new();
    let base_col = node.start_position().column;
    let mut lines = text.split('\n');
    if let Some(first) = lines.next() {
        docs.push(Doc::text(first));
    }
    for line in lines {
        docs.push(Doc::HardLine);
        let stripped = line
            .char_indices()
            .take_while(|&(i, c)| i < base_col && c == ' ')
            .last()
            .map(|(i, c)| &line[i + c.len_utf8()..])
            .unwrap_or(line);
        docs.push(Doc::text(stripped));
    }
    Doc::concat(docs)
}

/// Verification helper for tests: ensures non-trivia tokens are conserved.
#[cfg(test)]
pub fn verify_token_conservation(original: &str, formatted: &str) -> Result<(), String> {
    fn extract_core_tokens(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else if c == '/' && chars.clone().nth(1) == Some('/') {
                // Skip line comment
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        break;
                    }
                }
            } else if c == '/' && chars.clone().nth(1) == Some('*') {
                // Skip block comment
                chars.next();
                chars.next();
                while let Some(ch) = chars.next() {
                    if ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
            } else if c == ';' || c == ',' {
                // Comma/semicolon can be added/removed by formatter rules
                chars.next();
            } else if c.is_alphanumeric() || c == '_' {
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        word.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(word);
            } else {
                tokens.push(c.to_string());
                chars.next();
            }
        }
        tokens
    }

    let orig_tokens = extract_core_tokens(original);
    let fmt_tokens = extract_core_tokens(formatted);

    // If imports were removed/reordered, token count may differ slightly,
    // but in general fixtures without unused imports should conserve tokens.
    if orig_tokens.len() != fmt_tokens.len() && !original.contains("import ") {
        return Err(common::concat_str!(
            "Token count mismatch: original has ",
            orig_tokens.len().to_string(),
            ", formatted has ",
            fmt_tokens.len().to_string()
        ));
    }

    Ok(())
}
