/// Normalizes a single-line comment, ensuring `// text` has a space after `//`
/// and wrapping lines that exceed `max_width`.
pub fn normalize_line_comment(text: &str, start_col: usize, max_width: usize) -> String {
    let trimmed = text.trim_end();
    if trimmed.starts_with("//noinspection") || trimmed.starts_with("//$NON-NLS") {
        return trimmed.to_string();
    }

    let normalized = if let Some(rest) = trimmed.strip_prefix("//")
        && !rest.starts_with(' ')
        && !rest.starts_with('/')
        && !rest.is_empty()
    {
        crate::concat_str!("// ", rest)
    } else {
        trimmed.to_string()
    };

    if max_width > 0
        && start_col + normalized.len() > max_width
        && let Some(content) = normalized.strip_prefix("// ")
    {
        let mut lines = Vec::new();
        let mut cur_line = String::from("//");
        let first_budget = max_width.saturating_sub(start_col);
        for word in content.split_whitespace() {
            let limit = if lines.is_empty() {
                first_budget
            } else {
                max_width.saturating_sub(start_col)
            };
            if cur_line.len() == 2 || cur_line.len() + 1 + word.len() <= limit {
                cur_line.push(' ');
                cur_line.push_str(word);
            } else {
                lines.push(cur_line);
                cur_line = crate::concat_str!("// ", word);
            }
        }
        if cur_line.len() > 2 {
            lines.push(cur_line);
        }
        if lines.len() > 1 && lines[0].len() <= first_budget {
            let indent_str = " ".repeat(start_col);
            let mut result = lines[0].clone();
            for line in &lines[1..] {
                result.push('\n');
                result.push_str(&indent_str);
                result.push_str(line);
            }
            return result;
        }
    }

    normalized
}

pub use crate::rules::kdoc::format_kdoc;
