use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub struct LambdaParsed<'a> {
    pub params_doc: Option<Doc<'a>>,
    pub has_param_trailing_comma: bool,
    pub has_arrow: bool,
    pub arrow_has_newline: bool,
    pub has_newline_after_brace: bool,
    pub comments_before_params: Vec<Doc<'a>>,
    pub comments_after_params: Vec<Doc<'a>>,
    pub body_docs: Vec<Doc<'a>>,
}

pub fn lower_lambda_parameters<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (Doc<'a>, bool) {
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
                if next_is_comment && !ctx.has_user_newline_between(child, next_node) {
                    docs.push(Doc::space());
                } else {
                    docs.push(Doc::Line);
                }
            }
        } else if kind == ":" {
            docs.push(Doc::text(": "));
        } else {
            docs.push(lower_fn(*child, ctx));
        }
    }
    let has_trailing = !remove_trailing && children.last().is_some_and(|c| c.kind() == ",");
    (Doc::concat(docs), has_trailing)
}

pub fn format_lambda_params_group<'a>(
    node: &Node<'a>,
    parsed: &LambdaParsed<'a>,
    p: Doc<'a>,
    max_width: usize,
) -> Vec<Doc<'a>> {
    let mut p_group = Vec::new();
    let params_node = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "lambda_parameters");
    let params_overflow = params_node.is_some_and(|pn| {
        let has_multi = pn.children(&mut pn.walk()).any(|c| {
            c.kind() == ","
                || c.kind() == "multi_variable_declaration"
                || c.children(&mut c.walk())
                    .any(|gc| gc.kind() == "multi_variable_declaration")
        });
        has_multi && pn.start_position().column + pn.byte_range().len() > max_width
    });
    let has_line_comment = params_node.is_some_and(|pn| {
        pn.children(&mut pn.walk())
            .any(|c| c.kind() == "line_comment")
    }) || parsed
        .comments_before_params
        .iter()
        .any(|c| c.ends_with_newline())
        || parsed
            .comments_after_params
            .iter()
            .any(|c| c.ends_with_newline());

    if !parsed.comments_before_params.is_empty() {
        for c in &parsed.comments_before_params {
            p_group.push(Doc::HardLine);
            p_group.push(c.clone());
        }
        if !p_group.last().is_some_and(|d| d.ends_with_newline()) {
            p_group.push(Doc::HardLine);
        }
    } else if parsed.has_newline_after_brace
        || has_line_comment
        || parsed.has_param_trailing_comma
        || params_overflow
    {
        p_group.push(Doc::HardLine);
    } else {
        p_group.push(Doc::Line);
    }
    p_group.push(p);

    if !parsed.comments_after_params.is_empty() {
        p_group.push(Doc::space());
        for (idx, c) in parsed.comments_after_params.iter().enumerate() {
            if idx > 0 {
                p_group.push(Doc::space());
            }
            p_group.push(c.clone());
        }
    }

    let mut header = Vec::new();
    let params_part = if parsed.has_newline_after_brace || has_line_comment || params_overflow {
        Doc::continuation_indent(Doc::concat(p_group))
    } else {
        Doc::group(Doc::continuation_indent(Doc::concat(p_group)))
    };
    header.push(params_part);

    if parsed.has_arrow {
        append_lambda_arrow_doc(&mut header, parsed);
    }
    header
}

fn append_lambda_arrow_doc<'a>(header: &mut Vec<Doc<'a>>, parsed: &LambdaParsed<'a>) {
    if !parsed.comments_after_params.is_empty() {
        header.push(Doc::continuation_indent(Doc::concat(vec![
            Doc::HardLine,
            Doc::text(" ->"),
        ])));
    } else if parsed.arrow_has_newline || parsed.has_param_trailing_comma {
        header.push(Doc::indent(Doc::concat(vec![
            Doc::HardLine,
            Doc::text("->"),
        ])));
    } else {
        header.push(Doc::group(Doc::concat(vec![
            Doc::indent(Doc::Line),
            Doc::text("->"),
        ])));
    }
}

pub fn format_lambda_arrow_only<'a>(parsed: &LambdaParsed<'a>) -> Vec<Doc<'a>> {
    let mut header = Vec::new();
    if !parsed.comments_before_params.is_empty() {
        let mut h_group = Vec::new();
        if parsed.has_newline_after_brace {
            h_group.push(Doc::HardLine);
        } else {
            h_group.push(Doc::space());
        }
        for c in &parsed.comments_before_params {
            h_group.push(c.clone());
        }
        if parsed.arrow_has_newline {
            h_group.push(Doc::HardLine);
        } else if !h_group
            .last()
            .is_some_and(|d| d.ends_with_newline() || d.ends_with_space())
        {
            h_group.push(Doc::space());
        }
        h_group.push(Doc::text("->"));
        header.push(Doc::concat(h_group));
    } else {
        header.push(Doc::text(" ->"));
    }
    header
}
