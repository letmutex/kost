use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_annotated_lambda<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_kind: Option<&str> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if !docs.is_empty() && prev_kind != Some("label") {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
        prev_kind = Some(kind);
    }
    Doc::Concat(docs)
}

pub fn lower_lambda_parameters<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> (Doc<'a>, bool) {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    let remove_trailing =
        ctx.options.trailing_comma_strategy == crate::options::TrailingCommaStrategy::Complete;
    let last_param_idx = children.iter().rposition(|c| {
        c.kind() != "," && !matches!(c.kind(), "comment" | "line_comment" | "multiline_comment")
    });

    for (i, child) in children.iter().enumerate() {
        let kind = child.kind();
        if kind == "," {
            let is_trailing = last_param_idx.is_some_and(|lpi| i > lpi);
            if is_trailing && remove_trailing {
                continue;
            }
            docs.push(Doc::text(","));
            if let Some(next_node) = children.get(i + 1) {
                let next_is_comment = matches!(
                    next_node.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                );
                if next_is_comment && !ctx.has_newline_between(child, next_node) {
                    docs.push(Doc::space());
                } else {
                    docs.push(Doc::Line);
                }
            }
        } else if kind == ":" {
            docs.push(Doc::text(": "));
        } else {
            docs.push(lower_node(*child, ctx));
        }
    }
    let has_trailing = !remove_trailing && children.last().is_some_and(|c| c.kind() == ",");
    (Doc::Concat(docs), has_trailing)
}

fn format_lambda_params_header<'a>(
    params_doc: Option<Doc<'a>>,
    has_arrow: bool,
    has_trailing_comma: bool,
    has_newline_after_brace: bool,
    arrow_has_newline: bool,
    comments_before_arrow: Vec<Doc<'a>>,
) -> Vec<Doc<'a>> {
    let mut inner = Vec::new();
    let has_comments = !comments_before_arrow.is_empty();

    if let Some(params) = params_doc {
        let mut params_group = Vec::new();
        if has_newline_after_brace {
            params_group.push(Doc::HardLine);
        } else {
            params_group.push(Doc::Line);
        }
        params_group.push(params);

        if has_comments {
            params_group.push(Doc::space());
            params_group.extend(comments_before_arrow);
            if has_arrow {
                params_group.push(Doc::space());
                params_group.push(Doc::text("->"));
            }
            inner.push(Doc::continuation_indent(Doc::Concat(params_group)));
            return inner;
        }

        let params_part = if has_newline_after_brace {
            Doc::continuation_indent(Doc::Concat(params_group))
        } else {
            Doc::Group(Box::new(Doc::continuation_indent(Doc::Concat(
                params_group,
            ))))
        };
        inner.push(params_part);

        if has_arrow {
            if arrow_has_newline || has_trailing_comma {
                inner.push(Doc::indent(Doc::Concat(vec![
                    Doc::HardLine,
                    Doc::text("->"),
                ])));
            } else {
                inner.push(Doc::Group(Box::new(Doc::Concat(vec![
                    Doc::indent(Doc::Line),
                    Doc::text("->"),
                ]))));
            }
        }
    } else if has_arrow {
        if has_comments {
            inner.push(Doc::HardLine);
            inner.push(Doc::Concat(comments_before_arrow));
            inner.push(Doc::text("->"));
        } else {
            inner.push(Doc::text(" ->"));
        }
    }
    inner
}

fn assemble_lambda_doc<'a>(
    inner: Vec<Doc<'a>>,
    body_docs: Vec<Doc<'a>>,
    should_break: bool,
) -> Doc<'a> {
    if body_docs.is_empty() && inner.is_empty() {
        return Doc::text("{}");
    }

    if body_docs.is_empty() {
        return Doc::Concat(vec![
            Doc::text("{"),
            Doc::Concat(inner),
            Doc::space(),
            Doc::text("}"),
        ]);
    }

    if should_break {
        Doc::Concat(vec![
            Doc::text("{"),
            Doc::Concat(inner),
            Doc::indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(body_docs)])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    } else {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("{"),
            Doc::Concat(inner),
            Doc::indent(Doc::Concat(vec![Doc::Line, Doc::Concat(body_docs)])),
            Doc::Line,
            Doc::text("}"),
        ])))
    }
}

fn is_property_init_chain(node: &Node<'_>) -> bool {
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "property_declaration" {
            return true;
        }
        if matches!(n.kind(), "statements" | "function_body" | "class_body") {
            return false;
        }
        curr = n.parent();
    }
    false
}

struct LambdaParsed<'a> {
    params_doc: Option<Doc<'a>>,
    has_trailing_comma: bool,
    comments_before_arrow: Vec<Doc<'a>>,
    body_docs: Vec<Doc<'a>>,
    open_brace_node: Option<Node<'a>>,
    params_node: Option<Node<'a>>,
    arrow_node: Option<Node<'a>>,
    last_before_arrow: Option<Node<'a>>,
}

fn parse_lambda_children<'a>(node: &Node<'a>, ctx: &LowerContext<'a, '_>) -> LambdaParsed<'a> {
    let mut params_doc = None;
    let mut has_trailing_comma = false;
    let mut comments_before_arrow = Vec::new();
    let mut body_docs = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;
    let mut open_brace_node = None;
    let mut params_node = None;
    let mut arrow_node = None;
    let mut last_before_arrow: Option<Node<'a>> = None;
    let has_arrow = node.children(&mut node.walk()).any(|c| c.kind() == "->");
    let mut seen_arrow = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            open_brace_node = Some(child);
            continue;
        }
        if kind == "}" {
            continue;
        }
        if kind == "->" {
            seen_arrow = true;
            arrow_node = Some(child);
            continue;
        }
        if has_arrow && !seen_arrow {
            last_before_arrow = Some(child);
            if kind == "lambda_parameters" {
                params_node = Some(child);
                let (doc, trailing) = lower_lambda_parameters(child, ctx);
                params_doc = Some(doc);
                has_trailing_comma = trailing;
            } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                comments_before_arrow.push(lower_node(child, ctx));
            }
        } else if kind == "lambda_parameters" {
            params_node = Some(child);
            let (doc, trailing) = lower_lambda_parameters(child, ctx);
            params_doc = Some(doc);
            has_trailing_comma = trailing;
        } else {
            if let Some(prev) = prev_child {
                if ctx.has_blank_line_between(&prev, &child) {
                    body_docs.push(Doc::EmptyLine);
                } else if !ctx.has_newline_between(&prev, &child) {
                    body_docs.push(Doc::space());
                } else if !body_docs.last().is_some_and(|d| d.ends_with_newline()) {
                    body_docs.push(Doc::HardLine);
                }
            }
            body_docs.push(lower_node(child, ctx));
            prev_child = Some(child);
        }
    }

    LambdaParsed {
        params_doc,
        has_trailing_comma,
        comments_before_arrow,
        body_docs,
        open_brace_node,
        params_node,
        arrow_node,
        last_before_arrow,
    }
}

pub fn lower_lambda_literal<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let raw = ctx.node_text(&node);
    let has_user_newline = raw.contains('\n');

    let parsed = parse_lambda_children(&node, ctx);
    let has_newline_after_brace = match (parsed.open_brace_node, parsed.params_node) {
        (Some(b), Some(p)) => ctx.has_newline_between(&b, &p),
        _ => false,
    };
    let arrow_has_newline = match (parsed.last_before_arrow, parsed.arrow_node) {
        (Some(l), Some(a)) => ctx.has_newline_between(&l, &a),
        _ => false,
    };

    let inner = format_lambda_params_header(
        parsed.params_doc,
        parsed.arrow_node.is_some(),
        parsed.has_trailing_comma,
        has_newline_after_brace,
        arrow_has_newline,
        parsed.comments_before_arrow,
    );

    let should_break = ctx.options.preserve_lambda_breaks && has_user_newline;
    let res = assemble_lambda_doc(inner, parsed.body_docs, should_break);
    if ctx.options.continuation_indent == 2
        && !is_property_init_chain(&node)
        && crate::lower::navigation::is_call_receiver_in_chain(&node, ctx)
    {
        Doc::continuation_indent(res)
    } else {
        res
    }
}
