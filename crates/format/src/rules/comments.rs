use smol_str::SmolStr;

/// Normalizes a single-line comment, ensuring `// text` has a space after `//`
/// and wrapping lines that exceed `max_width`.
pub fn normalize_line_comment(
    text: &str,
    start_col: usize,
    base_indent: usize,
    max_width: usize,
) -> Vec<SmolStr> {
    let trimmed = text.trim_end();
    if trimmed.starts_with("//noinspection") || trimmed.starts_with("//$NON-NLS") {
        return vec![SmolStr::new(trimmed)];
    }

    let normalized = if let Some(rest) = trimmed.strip_prefix("//")
        && !rest.starts_with(' ')
        && !rest.starts_with('/')
        && !rest.is_empty()
    {
        crate::concat_smol_str!("// ", rest)
    } else {
        SmolStr::new(trimmed)
    };

    if max_width > 0
        && start_col + normalized.len() > max_width
        && let Some(content) = normalized.strip_prefix("// ")
    {
        let mut lines = Vec::new();
        let mut cur_line = String::from("//");
        let first_budget = max_width.saturating_sub(start_col);
        let cont_budget = max_width.saturating_sub(base_indent);
        for word in content.split_whitespace() {
            let limit = if lines.is_empty() {
                first_budget
            } else {
                cont_budget
            };
            if cur_line.len() == 2 || cur_line.len() + 1 + word.len() <= limit {
                cur_line.push(' ');
                cur_line.push_str(word);
            } else {
                lines.push(SmolStr::new(&cur_line));
                cur_line = crate::concat_str!("// ", word);
            }
        }
        if cur_line.len() > 2 {
            lines.push(SmolStr::new(&cur_line));
        }
        if lines.len() > 1 && lines[0].len() <= first_budget {
            return lines;
        }
    }

    vec![normalized]
}

pub fn try_append_to_trailing_line_comment(
    doc: &mut crate::doc::Doc<'_>,
    additional_text: &str,
) -> bool {
    match doc {
        crate::doc::Doc::LineComment {
            text,
            is_trailing: true,
            ..
        } => {
            let stripped = additional_text.trim_start_matches('/').trim_start();
            *text = crate::concat_smol_str!(text.as_str(), " ", stripped);
            true
        }
        crate::doc::Doc::Concat(items) => {
            if let Some(last) = items
                .iter_mut()
                .rev()
                .find(|d| !d.is_empty() && !matches!(d, crate::doc::Doc::HardLine))
            {
                return try_append_to_trailing_line_comment(last, additional_text);
            }
            false
        }
        crate::doc::Doc::Group { doc: inner, .. }
        | crate::doc::Doc::Indent(inner)
        | crate::doc::Doc::ContinuationIndent(inner)
        | crate::doc::Doc::FitsFirstLine(inner)
        | crate::doc::Doc::PreserveTrailing(inner)
        | crate::doc::Doc::IndentIfBreak { doc: inner, .. } => {
            try_append_to_trailing_line_comment(inner, additional_text)
        }
        crate::doc::Doc::IfBreak { broken, flat, .. } => {
            let b = try_append_to_trailing_line_comment(broken, additional_text);
            let f = try_append_to_trailing_line_comment(flat, additional_text);
            b || f
        }
        _ => false,
    }
}

pub fn find_trailing_line_comment<'a>(
    node: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "line_comment" {
        return Some(node);
    }
    let last = node.child(node.child_count().checked_sub(1)?)?;
    find_trailing_line_comment(last)
}

pub fn try_merge_continuation_comment<'a>(
    prev_node: Option<tree_sitter::Node<'a>>,
    child: &tree_sitter::Node<'a>,
    docs: &mut [crate::doc::Doc<'a>],
    ctx: &crate::lower::context::LowerContext<'a, '_>,
) -> bool {
    if ctx.style.preset != crate::options::StylePreset::Kotlinlang
        || !ctx.style.align_wrapped_trailing_comments
        || child.kind() != "line_comment"
    {
        return false;
    }
    let Some(prev) = prev_node.and_then(find_trailing_line_comment) else {
        return false;
    };
    if !should_merge_wrapped_comment(&prev, child, ctx) {
        return false;
    }
    let text = ctx.node_text(child);
    if let Some(last) = docs
        .iter_mut()
        .rev()
        .find(|d| !d.is_empty() && !matches!(d, crate::doc::Doc::HardLine))
    {
        return try_append_to_trailing_line_comment(last, text);
    }
    false
}

pub fn should_merge_wrapped_comment(
    prev: &tree_sitter::Node<'_>,
    next: &tree_sitter::Node<'_>,
    ctx: &crate::lower::context::LowerContext<'_, '_>,
) -> bool {
    if prev.end_position().row + 1 != next.start_position().row {
        return false;
    }
    if prev.start_position().column != next.start_position().column {
        return false;
    }
    let prev_text = ctx.node_text(prev);
    let next_text = ctx.node_text(next);
    let stripped = next_text.trim_start_matches('/').trim_start();
    let first_word = stripped.split_whitespace().next().unwrap_or("");
    prev.start_position().column + prev_text.len() + 1 + first_word.len() > ctx.options.max_width
}

pub use crate::rules::kdoc::format_kdoc;
