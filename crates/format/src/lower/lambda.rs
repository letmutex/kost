use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
pub use crate::lower::lambda_params::lower_lambda_parameters;
use crate::lower::lambda_params::{
    LambdaParsed, format_lambda_arrow_only, format_lambda_params_group,
};

pub fn lower_lambda_literal<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    lower_lambda_literal_opt(node, ctx, lower_fn, false)
}

pub fn lower_lambda_literal_multiline<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    lower_lambda_literal_opt(node, ctx, lower_fn, true)
}

fn push_lambda_child<'a>(
    child: Node<'a>,
    prev_child: &mut Option<Node<'a>>,
    body_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    if ctx.style.align_wrapped_trailing_comments
        && child.kind() == "line_comment"
        && let Some(prev) = prev_child.and_then(crate::rules::comments::find_trailing_line_comment)
        && crate::rules::comments::should_merge_wrapped_comment(&prev, &child, ctx)
        && let Some(last_doc) = body_docs.last_mut()
        && crate::rules::comments::try_append_to_trailing_line_comment(
            last_doc,
            ctx.node_text(&child),
        )
    {
        *prev_child = Some(child);
        return;
    }
    if let Some(prev) = *prev_child {
        if ctx.has_blank_line_between(&prev, &child) {
            body_docs.push(Doc::EmptyLine);
        } else if !ctx.has_user_newline_between(&prev, &child) {
            body_docs.push(Doc::space());
        } else if !body_docs.last().is_some_and(|d| d.ends_with_newline()) {
            body_docs.push(Doc::HardLine);
        }
    }
    let child_doc = if child.kind() == "line_comment" {
        let is_trailing = prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
        Doc::line_comment(
            ctx.node_text(&child),
            is_trailing,
            ctx.style.align_wrapped_trailing_comments,
        )
    } else {
        lower_fn(child, ctx)
    };
    body_docs.push(child_doc);
    *prev_child = Some(child);
}

fn parse_lambda_children<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> LambdaParsed<'a> {
    let mut params_doc = None;
    let mut has_param_trailing_comma = false;
    let has_arrow = node.children(&mut node.walk()).any(|c| c.kind() == "->");
    let mut seen_arrow = false;
    let mut arrow_has_newline = false;
    let mut last_before_arrow: Option<Node<'a>> = None;
    let mut comments_before_params = Vec::new();
    let mut comments_after_params = Vec::new();
    let mut body_docs = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;

    let open_brace = node.children(&mut node.walk()).find(|c| c.kind() == "{");
    let first_inner = node
        .children(&mut node.walk())
        .find(|c| c.kind() != "{" && c.kind() != "}");
    let has_newline_after_brace = open_brace
        .is_some_and(|b| first_inner.is_some_and(|f| ctx.has_user_newline_between(&b, &f)));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" || kind == "}" {
            continue;
        }
        if kind == "->" {
            seen_arrow = true;
            arrow_has_newline =
                last_before_arrow.is_some_and(|l| ctx.has_user_newline_between(&l, &child));
            continue;
        }
        if has_arrow && !seen_arrow {
            last_before_arrow = Some(child);
            if kind == "lambda_parameters" {
                let (doc, has_trailing) = lower_lambda_parameters(child, ctx, lower_fn);
                params_doc = Some(doc);
                has_param_trailing_comma = has_trailing;
            } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                if params_doc.is_none() {
                    comments_before_params.push(lower_fn(child, ctx));
                } else {
                    comments_after_params.push(lower_fn(child, ctx));
                }
            }
            continue;
        }
        if kind == "lambda_parameters" {
            let (doc, has_trailing) = lower_lambda_parameters(child, ctx, lower_fn);
            params_doc = Some(doc);
            has_param_trailing_comma = has_trailing;
            continue;
        }
        push_lambda_child(child, &mut prev_child, &mut body_docs, ctx, lower_fn);
    }

    LambdaParsed {
        params_doc,
        has_param_trailing_comma,
        has_arrow,
        arrow_has_newline,
        has_newline_after_brace,
        comments_before_params,
        comments_after_params,
        body_docs,
    }
}

fn lower_lambda_literal_opt<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    force_multiline: bool,
) -> Doc<'a> {
    let parsed = parse_lambda_children(node, ctx, lower_fn);
    if parsed.params_doc.is_none()
        && parsed.comments_before_params.is_empty()
        && parsed.comments_after_params.is_empty()
        && !parsed.has_arrow
        && parsed.body_docs.is_empty()
    {
        return Doc::concat(vec![Doc::text("{"), Doc::text("}")]);
    }

    let header = if let Some(p) = parsed.params_doc.clone() {
        format_lambda_params_group(&node, &parsed, p, ctx.options.max_width)
    } else if parsed.has_arrow {
        format_lambda_arrow_only(&parsed)
    } else {
        Vec::new()
    };

    if parsed.body_docs.is_empty() {
        return Doc::concat(vec![
            Doc::text("{"),
            Doc::concat(header),
            Doc::space(),
            Doc::text("}"),
        ]);
    }

    let has_user_newline = ctx.node_text(&node).contains('\n');
    let should_break = force_multiline || (ctx.options.preserve_lambda_breaks && has_user_newline);

    if should_break {
        Doc::concat(vec![
            Doc::text("{"),
            Doc::concat(header),
            Doc::indent(Doc::concat(vec![
                Doc::HardLine,
                Doc::concat(parsed.body_docs),
            ])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    } else {
        Doc::group(Doc::concat(vec![
            Doc::text("{"),
            Doc::concat(header),
            Doc::indent(Doc::concat(vec![Doc::Line, Doc::concat(parsed.body_docs)])),
            Doc::Line,
            Doc::text("}"),
        ]))
    }
}

pub fn lower_lambda_node_multiline<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if node.kind() == "annotated_lambda" {
        let mut docs = Vec::new();
        let mut prev_kind: Option<&str> = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if !docs.is_empty() && prev_kind != Some("label") {
                docs.push(Doc::space());
            }
            if kind == "lambda_literal" {
                docs.push(lower_lambda_literal_multiline(child, ctx, lower_fn));
            } else {
                docs.push(lower_fn(child, ctx));
            }
            prev_kind = Some(kind);
        }
        Doc::concat(docs)
    } else {
        lower_lambda_literal_multiline(node, ctx, lower_fn)
    }
}

pub fn lower_annotated_lambda<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_kind: Option<&str> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if !docs.is_empty() && prev_kind != Some("label") {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
        prev_kind = Some(kind);
    }
    Doc::concat(docs)
}
