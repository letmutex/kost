use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{BlockItem, format_braced_block, strategy_trailing_comma_doc};
use crate::lower::context::LowerContext;

fn push_try_body_child<'a>(
    child: Node<'a>,
    prev_child: &mut Option<Node<'a>>,
    items: &mut Vec<BlockItem<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    let is_comment = matches!(kind, "comment" | "line_comment" | "multiline_comment");
    if is_comment
        && prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child))
        && let Some(last) = items.last_mut()
    {
        let c_doc = if kind == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        last.doc = Doc::concat(vec![last.doc.clone(), Doc::space(), c_doc]);
        *prev_child = Some(child);
        return;
    }
    if ctx.style.preset == crate::options::StylePreset::Kotlinlang
        && ctx.style.align_wrapped_trailing_comments
        && kind == "line_comment"
        && let Some(prev) = prev_child.and_then(crate::rules::comments::find_trailing_line_comment)
        && crate::rules::comments::should_merge_wrapped_comment(&prev, &child, ctx)
        && let Some(last) = items.last_mut()
        && crate::rules::comments::try_append_to_trailing_line_comment(
            &mut last.doc,
            ctx.node_text(&child),
        )
    {
        *prev_child = Some(child);
        return;
    }
    let blank = prev_child.is_some_and(|p| ctx.has_blank_line_between(&p, &child));
    items.push(BlockItem::new(lower_fn(child, ctx)).with_blank_line_before(blank));
    *prev_child = Some(child);
}

pub fn lower_try_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut try_items = Vec::new();
    let mut handlers = Vec::new();
    let mut leading_comments = Vec::new();
    let mut in_try = false;
    let mut prev_try_child = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "try" => in_try = true,
            "catch_block" => {
                in_try = false;
                handlers.push((
                    std::mem::take(&mut leading_comments),
                    lower_catch_block(child, ctx, lower_fn),
                ));
            }
            "finally_block" => {
                in_try = false;
                handlers.push((
                    std::mem::take(&mut leading_comments),
                    lower_finally_block(child, ctx, lower_fn),
                ));
            }
            "{" => {}
            "}" => {
                if in_try {
                    in_try = false;
                }
            }
            "comment" | "line_comment" | "multiline_comment" => {
                if in_try {
                    push_try_body_child(child, &mut prev_try_child, &mut try_items, ctx, lower_fn);
                } else {
                    leading_comments.push(lower_fn(child, ctx));
                }
            }
            _ => {
                if in_try {
                    push_try_body_child(child, &mut prev_try_child, &mut try_items, ctx, lower_fn);
                }
            }
        }
    }

    let try_block = format_braced_block(try_items, Vec::new());
    let mut full = vec![Doc::text("try "), try_block];
    for (comments, h) in handlers {
        if comments.is_empty() {
            full.push(Doc::space());
        } else {
            for c in comments {
                full.push(Doc::HardLine);
                full.push(c);
            }
            full.push(Doc::HardLine);
        }
        full.push(h);
    }
    Doc::concat(full)
}

fn push_catch_param_child<'a>(
    child: Node<'a>,
    open_paren: Option<Node<'a>>,
    prev_param_child: &mut Option<Node<'a>>,
    has_param_newline: &mut bool,
    param_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    if prev_param_child.is_none()
        && let Some(op) = open_paren
    {
        if ctx.has_user_newline_between(&op, &child) {
            *has_param_newline = true;
        }
    } else if let Some(prev) = *prev_param_child {
        if ctx.has_user_newline_between(&prev, &child) {
            *has_param_newline = true;
            if !param_docs.last().is_some_and(|d| d.ends_with_newline()) {
                param_docs.push(Doc::HardLine);
            }
        } else if !param_docs.is_empty()
            && !param_docs
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
        {
            param_docs.push(Doc::space());
        }
    }
    param_docs.push(lower_fn(child, ctx));
    if matches!(kind, "annotation" | "modifiers") {
        param_docs.push(Doc::space());
    }
    *prev_param_child = Some(child);
}

fn format_catch_params_doc<'a>(
    param_docs: Vec<Doc<'a>>,
    has_param_newline: bool,
    has_trailing_comma: bool,
    strategy: crate::options::TrailingCommaStrategy,
) -> Doc<'a> {
    let comma_doc = strategy_trailing_comma_doc(strategy, has_trailing_comma);
    if has_param_newline {
        Doc::concat(vec![
            Doc::text("("),
            Doc::continuation_indent(Doc::concat(vec![
                Doc::HardLine,
                Doc::concat(param_docs),
                comma_doc,
            ])),
            Doc::text(")"),
        ])
    } else {
        Doc::group(Doc::concat(vec![
            Doc::text("("),
            Doc::continuation_indent(Doc::concat(vec![
                Doc::SoftLine,
                Doc::concat(param_docs),
                comma_doc,
            ])),
            Doc::text(")"),
        ]))
    }
}

pub fn lower_catch_block<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut param_docs = Vec::new();
    let mut body_items = Vec::new();
    let mut in_params = false;
    let mut in_body = false;
    let mut prev_body_child = None;
    let mut open_paren = None;
    let mut prev_param_child = None;
    let mut has_param_newline = false;
    let mut has_trailing_comma = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "(" => {
                in_params = true;
                open_paren = Some(child);
            }
            ")" => in_params = false,
            "{" => in_body = true,
            "}" => in_body = false,
            ":" if in_params => param_docs.push(Doc::text(": ")),
            "," if in_params => {
                has_trailing_comma = true;
            }
            "comment" | "line_comment" | "multiline_comment" if in_params => {
                has_param_newline = true;
                if !param_docs.is_empty()
                    && !param_docs.last().is_some_and(|d| d.ends_with_newline())
                {
                    param_docs.push(Doc::HardLine);
                }
                param_docs.push(lower_fn(child, ctx));
                param_docs.push(Doc::HardLine);
                prev_param_child = Some(child);
            }
            _ if in_params => {
                push_catch_param_child(
                    child,
                    open_paren,
                    &mut prev_param_child,
                    &mut has_param_newline,
                    &mut param_docs,
                    ctx,
                    lower_fn,
                );
            }
            _ if in_body => {
                push_try_body_child(child, &mut prev_body_child, &mut body_items, ctx, lower_fn);
            }
            _ => {}
        }
    }

    let params_doc = format_catch_params_doc(
        param_docs,
        has_param_newline,
        has_trailing_comma,
        ctx.options.trailing_comma_strategy,
    );
    let body_doc = format_braced_block(body_items, Vec::new());

    Doc::concat(vec![
        Doc::text("catch "),
        params_doc,
        Doc::space(),
        body_doc,
    ])
}

pub fn lower_finally_block<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut body_items = Vec::new();
    let mut prev_child = None;
    let mut in_body = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "{" => in_body = true,
            "}" => in_body = false,
            _ if in_body => {
                push_try_body_child(child, &mut prev_child, &mut body_items, ctx, lower_fn);
            }
            _ => {}
        }
    }

    let body_doc = format_braced_block(body_items, Vec::new());
    Doc::concat(vec![Doc::text("finally "), body_doc])
}
