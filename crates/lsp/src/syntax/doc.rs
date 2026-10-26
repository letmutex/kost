use tree_sitter::Node;

use super::doc_link::render_kdoc_links;
use crate::signature::is_modifier_line;

pub use super::doc_link::render_kdoc_links as render_doc_links;

/// Clean up KDoc markdown format and format tags
pub fn clean_kdoc(raw: &str) -> String {
    let trimmed = raw.trim();
    let content = if let Some(stripped) = trimmed.strip_prefix("/**") {
        stripped.strip_suffix("*/").unwrap_or(stripped)
    } else if let Some(stripped) = trimmed.strip_prefix("/*") {
        stripped.strip_suffix("*/").unwrap_or(stripped)
    } else {
        trimmed
    };

    let mut lines = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed_start = line.trim_start();
        let stripped = if let Some(s) = trimmed_start.strip_prefix("///") {
            s.strip_prefix(' ').unwrap_or(s)
        } else if let Some(s) = trimmed_start.strip_prefix("//") {
            s.strip_prefix(' ').unwrap_or(s)
        } else if let Some(s) = trimmed_start.strip_prefix('*') {
            s.strip_prefix(' ').unwrap_or(s)
        } else {
            line
        };

        let trimmed_content = stripped.trim_start();
        if trimmed_content.starts_with("```") || trimmed_content.starts_with("~~~") {
            in_code_block = !in_code_block;
            lines.push(stripped.trim_end().to_string());
            continue;
        }

        if in_code_block {
            lines.push(stripped.trim_end().to_string());
        } else {
            let formatted = format_kdoc_tag_line(stripped.trim_end());
            lines.push(formatted);
        }
    }

    let start_idx = lines
        .iter()
        .position(|l| !l.is_empty())
        .unwrap_or(lines.len());
    let end_idx = lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map(|i| i + 1)
        .unwrap_or(start_idx);
    lines[start_idx..end_idx].join("\n")
}

fn format_kdoc_tag_line(cleaned: &str) -> String {
    let trimmed_start = cleaned.trim_start();
    if let Some(rest) = trimmed_start.strip_prefix("@param") {
        let (param, desc) = rest
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((rest.trim(), ""));
        let param = param.trim_matches(|c| c == '[' || c == ']');
        let desc = render_kdoc_links(desc.trim());
        if desc.is_empty() {
            common::concat_str!("* **@param** `", param, "`")
        } else {
            common::concat_str!("* **@param** `", param, "` — ", &desc)
        }
    } else if let Some(rest) = trimmed_start.strip_prefix("@return") {
        let desc = render_kdoc_links(rest.trim());
        common::concat_str!("* **@return** — ", &desc)
    } else if let Some(rest) = trimmed_start.strip_prefix("@throws") {
        let (exc, desc) = rest
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((rest.trim(), ""));
        let exc = exc.trim_matches(|c| c == '[' || c == ']');
        let desc = render_kdoc_links(desc.trim());
        if desc.is_empty() {
            common::concat_str!("* **@throws** `", exc, "`")
        } else {
            common::concat_str!("* **@throws** `", exc, "` — ", &desc)
        }
    } else if let Some(rest) = trimmed_start.strip_prefix("@see") {
        let rendered = render_kdoc_links(rest.trim());
        let final_val = if rendered.starts_with('`') && rendered.ends_with('`') {
            rendered
        } else {
            common::concat_str!("`", &rendered, "`")
        };
        common::concat_str!("* **@see** ", &final_val)
    } else {
        render_kdoc_links(cleaned)
    }
}

/// Extract raw preceding KDoc or comment text for a node
pub fn extract_raw_doc_comment(node: &Node, source: &str) -> Option<String> {
    let mut prev = node.prev_sibling();
    let mut line_comments = Vec::new();

    while let Some(sibling) = prev {
        let kind = sibling.kind();
        if kind.contains("comment")
            && let Some(text) = source.get(sibling.start_byte()..sibling.end_byte())
        {
            if text.starts_with("/*") {
                return Some(text.to_string());
            }
            if text.starts_with("//") {
                line_comments.push(text.to_string());
                prev = sibling.prev_sibling();
                continue;
            }
        }
        if !line_comments.is_empty() {
            line_comments.reverse();
            return Some(line_comments.join("\n"));
        }
        if sibling.child_count() > 0
            && let Some(last_child) = sibling.child(sibling.child_count() - 1)
            && last_child.kind().contains("comment")
            && let Some(text) = source.get(last_child.start_byte()..last_child.end_byte())
            && text.starts_with("/*")
        {
            return Some(text.to_string());
        }
        if matches!(kind, "," | "modifiers" | "annotation" | "annotated_lambda") {
            prev = sibling.prev_sibling();
        } else {
            break;
        }
    }

    if !line_comments.is_empty() {
        line_comments.reverse();
        return Some(line_comments.join("\n"));
    }

    let start = node.start_byte().min(source.len());
    extract_doc_from_prefix(&source[..start])
}

fn extract_doc_from_prefix(prefix: &str) -> Option<String> {
    let mut comment_lines = Vec::new();
    let mut block_lines = Vec::new();
    let mut in_block = false;

    for line in prefix.lines().rev() {
        let trimmed = line.trim();
        let trimmed_start = line.trim_start();
        if trimmed.is_empty() {
            if in_block {
                block_lines.push(line);
            } else if !comment_lines.is_empty() {
                break;
            }
            continue;
        }
        if in_block {
            block_lines.push(line);
            if trimmed.starts_with("/*") {
                block_lines.reverse();
                return Some(block_lines.join("\n"));
            }
            continue;
        }
        if trimmed.ends_with("*/") {
            in_block = true;
            block_lines.push(line);
            if trimmed.starts_with("/*") {
                return Some(trimmed.to_string());
            }
            continue;
        }
        if trimmed_start.starts_with("//") {
            comment_lines.push(trimmed_start.to_string());
            continue;
        }
        if trimmed.starts_with('@') || is_modifier_line(trimmed) || trimmed == "," {
            continue;
        }
        break;
    }

    if !comment_lines.is_empty() {
        comment_lines.reverse();
        return Some(comment_lines.join("\n"));
    }
    None
}

/// Extract preceding KDoc or comment text for a node
pub fn extract_doc_comment(node: &Node, source: &str) -> Option<String> {
    let raw = extract_raw_doc_comment(node, source)?;
    let cleaned = clean_kdoc(&raw);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Extract @param doc comment description for a parameter from enclosing function/class
pub fn extract_param_doc_from_function(
    func_node: &Node,
    param_name: &str,
    source: &str,
) -> Option<String> {
    let doc_text = extract_raw_doc_comment(func_node, source)?;
    let mut collecting = false;
    let mut desc_lines = Vec::new();
    let mut in_code_block = false;

    for line in doc_text.lines() {
        let trimmed_start = line.trim_start();
        let stripped = if let Some(s) = trimmed_start.strip_prefix("/**") {
            s.strip_prefix(' ').unwrap_or(s)
        } else if let Some(s) = trimmed_start.strip_prefix("*/") {
            s.strip_prefix(' ').unwrap_or(s)
        } else if let Some(s) = trimmed_start.strip_prefix('*') {
            s.strip_prefix(' ').unwrap_or(s)
        } else if let Some(s) = trimmed_start.strip_prefix("//") {
            s.strip_prefix(' ').unwrap_or(s)
        } else {
            line
        };

        let trimmed_content = stripped.trim_start();
        if trimmed_content.starts_with("```") || trimmed_content.starts_with("~~~") {
            in_code_block = !in_code_block;
            if collecting {
                desc_lines.push(stripped.trim_end().to_string());
            }
            continue;
        }

        if !in_code_block {
            let cleaned = stripped.trim();
            if let Some(rest) = cleaned.strip_prefix("@param") {
                let rest = rest.trim();
                if let Some(after_param) = rest.strip_prefix(param_name)
                    && (after_param.is_empty() || after_param.starts_with(char::is_whitespace))
                {
                    collecting = true;
                    let desc = after_param.trim();
                    if !desc.is_empty() {
                        desc_lines.push(render_kdoc_links(desc));
                    }
                    continue;
                }
                if collecting {
                    break;
                }
            } else if cleaned.starts_with('@') {
                if collecting {
                    break;
                }
            } else if collecting && !cleaned.is_empty() {
                desc_lines.push(render_kdoc_links(stripped.trim_end()));
            }
        } else if collecting {
            desc_lines.push(stripped.trim_end().to_string());
        }
    }

    if desc_lines.is_empty() {
        None
    } else {
        Some(desc_lines.join("\n"))
    }
}

/// Merge extra inline/block comments into an optional doc comment
pub fn merge_extra_comments(doc: Option<String>, extra: &[String]) -> Option<String> {
    if extra.is_empty() {
        return doc;
    }
    let mut d = doc.unwrap_or_default();
    for comment in extra {
        if !d.contains(comment) {
            if !d.is_empty() {
                d.push_str("\n\n");
            }
            d.push_str(comment);
        }
    }
    if d.trim().is_empty() { None } else { Some(d) }
}
