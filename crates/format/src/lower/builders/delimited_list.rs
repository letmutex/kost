use crate::doc::Doc;
use crate::options::TrailingCommaStrategy;

#[derive(Debug, Clone)]
pub struct ListItem<'a> {
    pub doc: Doc<'a>,
    pub same_line_comment: Option<Doc<'a>>,
    pub force_newline_after: bool,
}

impl<'a> ListItem<'a> {
    pub fn new(doc: Doc<'a>) -> Self {
        Self {
            doc,
            same_line_comment: None,
            force_newline_after: false,
        }
    }

    pub fn with_comment(mut self, comment: Doc<'a>) -> Self {
        self.same_line_comment = Some(comment);
        self
    }
}

pub fn strategy_trailing_comma_doc(
    strategy: TrailingCommaStrategy,
    has_trailing_comma: bool,
) -> Doc<'static> {
    match strategy {
        TrailingCommaStrategy::Complete => Doc::if_break(Doc::text(","), Doc::empty()),
        TrailingCommaStrategy::OnlyAdd => {
            if has_trailing_comma {
                Doc::text(",")
            } else {
                Doc::if_break(Doc::text(","), Doc::empty())
            }
        }
        TrailingCommaStrategy::None => {
            if has_trailing_comma {
                Doc::text(",")
            } else {
                Doc::empty()
            }
        }
    }
}

pub fn format_delimited_list<'a>(
    open_delim: &'static str,
    close_delim: &'static str,
    items: Vec<ListItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
    has_user_trailing_comma: bool,
    strategy: TrailingCommaStrategy,
    use_continuation_indent: bool,
    omit_closing_break: bool,
    wrap_in_group: bool,
) -> Doc<'a> {
    format_delimited_list_ext(
        open_delim,
        close_delim,
        items,
        dangling_comments,
        has_user_trailing_comma,
        false,
        false,
        strategy,
        use_continuation_indent,
        omit_closing_break,
        wrap_in_group,
    )
}

fn build_delimited_list_items<'a>(
    items: Vec<ListItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
    sep_line: &Doc<'a>,
    trailing_comma: Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut inner = Vec::new();
    let total = items.len();
    for (i, item) in items.into_iter().enumerate() {
        let is_last = i + 1 == total;
        inner.push(item.doc);

        if !is_last {
            inner.push(Doc::text(","));
            if let Some(comment) = item.same_line_comment {
                inner.push(comment);
            }
            if item.force_newline_after {
                inner.push(Doc::HardLine);
            } else {
                inner.push(sep_line.clone());
            }
        } else {
            inner.push(trailing_comma.clone());
            if let Some(comment) = item.same_line_comment {
                inner.push(comment);
            }
        }
    }

    for (idx, comment) in dangling_comments.into_iter().enumerate() {
        if !inner.is_empty() || idx > 0 {
            inner.push(Doc::HardLine);
        }
        inner.push(comment);
    }
    inner
}

fn resolve_list_break_lines<'a>(
    items: &[ListItem<'a>],
    dangling_comments: &[Doc<'a>],
    strategy: TrailingCommaStrategy,
    has_user_trailing_comma: bool,
    has_user_newline: bool,
    is_declaration: bool,
    omit_closing_break: bool,
) -> (Doc<'static>, Doc<'static>, Doc<'static>) {
    let has_line_comment = items.iter().any(|item| {
        item.force_newline_after
            || item.doc.has_hardline()
            || item
                .same_line_comment
                .as_ref()
                .is_some_and(|c| c.has_hardline() || c.ends_with_newline())
    });

    let forces_multiline = has_line_comment
        || !dangling_comments.is_empty()
        || (strategy == TrailingCommaStrategy::Complete && has_user_newline)
        || (strategy != TrailingCommaStrategy::None && is_declaration && has_user_newline)
        || (strategy != TrailingCommaStrategy::Complete && has_user_trailing_comma);

    if forces_multiline {
        (
            Doc::HardLine,
            Doc::HardLine,
            if omit_closing_break {
                Doc::empty()
            } else {
                Doc::HardLine
            },
        )
    } else {
        (
            Doc::SoftLine,
            Doc::Line,
            if omit_closing_break {
                Doc::empty()
            } else {
                Doc::SoftLine
            },
        )
    }
}

pub fn format_delimited_list_ext<'a>(
    open_delim: &'static str,
    close_delim: &'static str,
    items: Vec<ListItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
    has_user_trailing_comma: bool,
    has_user_newline: bool,
    is_declaration: bool,
    strategy: TrailingCommaStrategy,
    use_continuation_indent: bool,
    omit_closing_break: bool,
    wrap_in_group: bool,
) -> Doc<'a> {
    if items.is_empty() && dangling_comments.is_empty() {
        return Doc::concat(vec![Doc::text(open_delim), Doc::text(close_delim)]);
    }

    let (open_line, sep_line, closing_line) = resolve_list_break_lines(
        &items,
        &dangling_comments,
        strategy,
        has_user_trailing_comma,
        has_user_newline,
        is_declaration,
        omit_closing_break,
    );

    let trailing_comma = strategy_trailing_comma_doc(strategy, has_user_trailing_comma);
    let inner = build_delimited_list_items(items, dangling_comments, &sep_line, trailing_comma);

    let (inner_body, close_doc) = if omit_closing_break {
        (
            Doc::group(Doc::concat(vec![
                Doc::concat(inner),
                Doc::text(close_delim),
            ])),
            Doc::empty(),
        )
    } else {
        (
            Doc::concat(inner),
            Doc::concat(vec![closing_line, Doc::text(close_delim)]),
        )
    };

    let indented_body = if use_continuation_indent {
        Doc::continuation_indent(Doc::concat(vec![open_line, inner_body]))
    } else {
        Doc::indent(Doc::concat(vec![open_line, inner_body]))
    };

    let list_doc = Doc::concat(vec![Doc::text(open_delim), indented_body, close_doc]);
    if wrap_in_group {
        Doc::group(list_doc)
    } else {
        list_doc
    }
}

pub use super::delimited_builder::DelimitedListBuilder;
