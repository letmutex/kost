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
        && start_col + normalized.chars().count() > max_width
        && let Some(content) = normalized.strip_prefix("// ")
    {
        let mut lines = Vec::new();
        let mut cur_line = String::from("//");
        let mut cur_chars = 2usize;
        let first_budget = max_width.saturating_sub(start_col);
        let cont_budget = max_width.saturating_sub(base_indent);
        for word in content.split_whitespace() {
            let limit = if lines.is_empty() {
                first_budget
            } else {
                cont_budget
            };
            let word_chars = word.chars().count();
            if cur_chars == 2 || cur_chars + 1 + word_chars <= limit {
                cur_line.push(' ');
                cur_line.push_str(word);
                cur_chars += 1 + word_chars;
            } else {
                lines.push(SmolStr::new(&cur_line));
                cur_line = crate::concat_str!("// ", word);
                cur_chars = 3 + word_chars;
            }
        }
        if cur_chars > 2 {
            lines.push(SmolStr::new(&cur_line));
        }
        if lines.len() > 1 && lines[0].chars().count() <= first_budget {
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

fn node_char_column(node: &tree_sitter::Node<'_>, source: &str) -> usize {
    let start_byte = node.start_byte();
    let col_bytes = node.start_position().column;
    if start_byte >= col_bytes
        && let Some(line_prefix) = source.get(start_byte - col_bytes..start_byte)
    {
        if line_prefix.is_ascii() {
            return col_bytes;
        }
        return line_prefix.chars().count();
    }
    col_bytes
}

pub fn should_merge_wrapped_comment(
    prev: &tree_sitter::Node<'_>,
    next: &tree_sitter::Node<'_>,
    ctx: &crate::lower::context::LowerContext<'_, '_>,
) -> bool {
    if prev.end_position().row + 1 != next.start_position().row {
        return false;
    }
    let prev_col = node_char_column(prev, ctx.source);
    let next_col = node_char_column(next, ctx.source);
    if prev_col != next_col {
        return false;
    }
    let prev_text = ctx.node_text(prev);
    let next_text = ctx.node_text(next);
    let stripped = next_text.trim_start_matches('/').trim_start();
    let first_word = stripped.split_whitespace().next().unwrap_or("");
    prev_col + prev_text.chars().count() + 1 + first_word.chars().count() > ctx.options.max_width
}

pub fn push_modifier_comment<'a>(
    child: tree_sitter::Node<'a>,
    prev_child: Option<tree_sitter::Node<'a>>,
    modifier_docs: &mut Vec<crate::doc::Doc<'a>>,
    ctx: &crate::lower::context::LowerContext<'a, '_>,
    lower_fn: &dyn Fn(
        tree_sitter::Node<'a>,
        &crate::lower::context::LowerContext<'a, '_>,
    ) -> crate::doc::Doc<'a>,
) {
    if try_merge_continuation_comment(prev_child, &child, modifier_docs, ctx) {
        return;
    }
    let kind = child.kind();
    let is_same_line = prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
    if is_same_line
        && !modifier_docs.is_empty()
        && !modifier_docs
            .last()
            .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
    {
        modifier_docs.push(crate::doc::Doc::space());
    } else if !is_same_line
        && !modifier_docs.is_empty()
        && !modifier_docs.last().is_some_and(|d| d.ends_with_newline())
    {
        modifier_docs.push(crate::doc::Doc::HardLine);
    }
    let c_doc = if kind == "line_comment" && is_same_line {
        crate::doc::Doc::line_comment(
            ctx.node_text(&child),
            true,
            ctx.style.align_wrapped_trailing_comments,
        )
    } else {
        lower_fn(child, ctx)
    };
    modifier_docs.push(c_doc);
    let next_has_newline = child
        .next_sibling()
        .is_some_and(|n| ctx.has_user_newline_between(&child, &n));
    if kind == "line_comment" || next_has_newline {
        if !modifier_docs.last().is_some_and(|d| d.ends_with_newline()) {
            modifier_docs.push(crate::doc::Doc::HardLine);
        }
    } else {
        modifier_docs.push(crate::doc::Doc::space());
    }
}

pub fn push_context_receiver_doc<'a>(
    child: tree_sitter::Node<'a>,
    modifier_docs: &mut Vec<crate::doc::Doc<'a>>,
    ctx: &crate::lower::context::LowerContext<'a, '_>,
    lower_fn: &dyn Fn(
        tree_sitter::Node<'a>,
        &crate::lower::context::LowerContext<'a, '_>,
    ) -> crate::doc::Doc<'a>,
) {
    if !modifier_docs.is_empty() && !modifier_docs.last().is_some_and(|d| d.ends_with_newline()) {
        modifier_docs.push(crate::doc::Doc::HardLine);
    }
    modifier_docs.push(lower_fn(child, ctx));
    modifier_docs.push(crate::doc::Doc::HardLine);
}

pub use crate::rules::kdoc::format_kdoc;
