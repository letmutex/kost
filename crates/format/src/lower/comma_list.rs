use crate::doc::Doc;
use crate::options::TrailingCommaStrategy;

fn strategy_trailing_comma_doc(
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

#[derive(Debug, Clone)]
pub struct CommaListItem<'a> {
    pub doc: Doc<'a>,
    pub trailing_comment: Option<Doc<'a>>,
}

impl<'a> From<Doc<'a>> for CommaListItem<'a> {
    fn from(doc: Doc<'a>) -> Self {
        Self {
            doc,
            trailing_comment: None,
        }
    }
}

fn assemble_comma_list_items<'a>(
    items: Vec<CommaListItem<'a>>,
    trailing_comma_doc: Doc<'a>,
    line_doc: Doc<'a>,
    omit_closing_break: bool,
    last_non_comment_idx: Option<usize>,
) -> Vec<Doc<'a>> {
    let total_items = items.len();
    let mut inner_docs = Vec::new();
    for (idx, item) in items.into_iter().enumerate() {
        let is_last = idx + 1 == total_items;
        let is_comment = item.doc.ends_with_newline();
        let is_last_non_comment = Some(idx) == last_non_comment_idx;

        inner_docs.push(item.doc);
        if is_last_non_comment && !omit_closing_break {
            inner_docs.push(trailing_comma_doc.clone());
        }
        if let Some(comment) = item.trailing_comment {
            if !is_last_non_comment && !is_comment {
                inner_docs.push(Doc::text(","));
            }
            inner_docs.push(Doc::space());
            inner_docs.push(comment);
        } else if !is_last && !is_comment && !is_last_non_comment {
            inner_docs.push(Doc::text(","));
            inner_docs.push(line_doc.clone());
        } else if !is_last && is_last_non_comment {
            inner_docs.push(line_doc.clone());
        }
    }
    inner_docs
}

fn wrap_comma_list_docs<'a>(
    inner_docs: Vec<Doc<'a>>,
    open_token: &'static str,
    close_token: &'static str,
    item_forces_hardline: bool,
    omit_closing_break: bool,
    has_break_after_open: bool,
    is_declaration: bool,
) -> Doc<'a> {
    if item_forces_hardline {
        let closing = if omit_closing_break {
            Doc::empty()
        } else {
            Doc::HardLine
        };
        Doc::Concat(vec![
            Doc::text(open_token),
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(inner_docs)])),
            closing,
            Doc::text(close_token),
        ])
    } else {
        let closing_line = if omit_closing_break {
            Doc::empty()
        } else {
            Doc::SoftLine
        };
        let (inner_doc, close_doc) = if is_declaration {
            (Doc::Concat(inner_docs), Doc::text(close_token))
        } else if omit_closing_break {
            (
                Doc::Group(Box::new(Doc::Concat(vec![
                    Doc::Concat(inner_docs),
                    Doc::text(close_token),
                ]))),
                Doc::empty(),
            )
        } else {
            (Doc::Concat(inner_docs), Doc::text(close_token))
        };
        let open_break = if has_break_after_open {
            Doc::HardLine
        } else {
            Doc::SoftLine
        };
        let list_doc = Doc::Concat(vec![
            Doc::text(open_token),
            Doc::continuation_indent(Doc::Concat(vec![open_break, inner_doc])),
            closing_line,
            close_doc,
        ]);
        if is_declaration {
            list_doc
        } else {
            Doc::Group(Box::new(list_doc))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommaListOptions {
    pub strategy: TrailingCommaStrategy,
    pub is_declaration: bool,
    pub has_user_newline: bool,
    pub has_break_after_open: bool,
    pub is_single_unnamed_lambda: bool,
    pub is_kotlinlang: bool,
}

pub fn format_comma_list<'a>(
    items: Vec<CommaListItem<'a>>,
    has_trailing_comma: bool,
    open_token: &'static str,
    close_token: &'static str,
    opts: CommaListOptions,
) -> Doc<'a> {
    if items.is_empty() {
        return Doc::Concat(vec![Doc::text(open_token), Doc::text(close_token)]);
    }

    let has_line_comment = items
        .iter()
        .any(|item| item.doc.ends_with_newline() || item.trailing_comment.is_some());
    let last_non_comment_idx = items.iter().rposition(|it| !it.doc.ends_with_newline());
    let omit_closing_break = !opts.is_declaration
        && !has_trailing_comma
        && !opts.is_kotlinlang
        && opts.strategy != TrailingCommaStrategy::Complete
        && (opts.strategy == TrailingCommaStrategy::None
            || (!has_line_comment && items.last().is_some_and(|d| d.doc.ends_with_brace())));

    let trailing_comma_doc = strategy_trailing_comma_doc(opts.strategy, has_trailing_comma);
    let item_forces_hardline = has_line_comment
        || (opts.strategy == TrailingCommaStrategy::Complete && opts.has_user_newline)
        || (opts.strategy != TrailingCommaStrategy::None
            && opts.is_declaration
            && opts.has_user_newline)
        || (opts.strategy != TrailingCommaStrategy::Complete && has_trailing_comma);
    let line_doc = if item_forces_hardline {
        Doc::HardLine
    } else {
        Doc::Line
    };

    let inner_docs = assemble_comma_list_items(
        items,
        trailing_comma_doc,
        line_doc,
        omit_closing_break,
        last_non_comment_idx,
    );

    if opts.is_single_unnamed_lambda && !has_trailing_comma && inner_docs.len() == 1 {
        return Doc::Concat(vec![
            Doc::text(open_token),
            inner_docs.into_iter().next().unwrap(),
            Doc::text(close_token),
        ]);
    }

    wrap_comma_list_docs(
        inner_docs,
        open_token,
        close_token,
        item_forces_hardline,
        omit_closing_break,
        opts.has_break_after_open,
        opts.is_declaration,
    )
}
