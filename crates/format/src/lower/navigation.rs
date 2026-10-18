use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::callable_reference::lower_double_colon_navigation;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn is_navigation_chain_root(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };
    if matches!(
        parent.kind(),
        "navigation_expression" | "postfix_expression"
    ) {
        return false;
    }
    if parent.kind() == "indexing_expression"
        && parent.child(0).is_some_and(|c| c.id() == node.id())
    {
        return false;
    }
    if parent.kind() == "call_expression"
        && let Some(grandparent) = parent.parent()
    {
        if matches!(
            grandparent.kind(),
            "navigation_expression" | "postfix_expression"
        ) {
            return false;
        }
        if grandparent.kind() == "indexing_expression"
            && grandparent.child(0).is_some_and(|c| c.id() == parent.id())
        {
            return false;
        }
    }
    true
}

pub fn is_call_receiver(node: &Node<'_>) -> bool {
    matches!(node.kind(), "call_expression" | "indexing_expression")
}

pub fn is_super_or_this(node: &Node<'_>) -> bool {
    matches!(node.kind(), "super_expression" | "this_expression")
}

pub fn is_part_of_call_chain(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    parent.kind() == "navigation_expression"
}

pub fn is_type_name_receiver(node: &Node<'_>, source: &str) -> bool {
    let name = match node.kind() {
        "simple_identifier" => source.get(node.start_byte()..node.end_byte()),
        _ => None,
    };
    name.is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
}

pub fn count_chain_lambdas(node: &Node<'_>) -> usize {
    let mut count = 0;
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            let mut c = n.walk();
            for child in n.children(&mut c) {
                if child.kind() == "call_suffix" {
                    let mut c2 = child.walk();
                    for grand in child.children(&mut c2) {
                        if matches!(grand.kind(), "lambda_literal" | "annotated_lambda") {
                            count += 1;
                        }
                    }
                }
            }
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            curr = n.child(0);
        } else {
            break;
        }
    }
    count
}

fn has_multiline_trailing_lambda(node: &Node<'_>) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut suffix_cursor = child.walk();
            for grandchild in child.children(&mut suffix_cursor) {
                if matches!(grandchild.kind(), "lambda_literal" | "annotated_lambda")
                    && grandchild.start_position().row != grandchild.end_position().row
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn is_chain_broken(node: &Node<'_>, ctx: &LowerContext<'_, '_>, chain_len: usize) -> bool {
    if chain_len > ctx.options.max_width {
        return true;
    }
    if count_chain_lambdas(node) > 1 {
        return true;
    }
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            let mut nav_c = n.walk();
            let mut chs = n.children(&mut nav_c);
            let first = chs.next();
            let second = chs.next();
            if let (Some(f), Some(s)) = (first, second)
                && (ctx.has_newline_between(&f, &s) || has_multiline_trailing_lambda(&f))
            {
                return true;
            }
            curr = n.child(0);
        } else {
            break;
        }
    }
    false
}

pub fn lower_navigation_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut operand = None;
    let mut comments = Vec::new();
    let mut suffix = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if operand.is_none() {
            operand = Some(child);
        } else if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            comments.push(child);
        } else {
            suffix = Some(child);
        }
    }

    let Some(op_node) = operand else {
        return Doc::empty();
    };

    let op_doc = lower_node(op_node, ctx);

    if let Some(suf_node) = suffix {
        let has_double_colon = suf_node.child(0).is_some_and(|c| c.kind() == "::");
        if has_double_colon {
            return lower_double_colon_navigation(&node, op_doc, suf_node, ctx);
        }

        let is_breakable = !comments.is_empty()
            || (!is_super_or_this(&op_node)
                && (is_call_receiver(&op_node) || ctx.has_newline_between(&op_node, &suf_node)));
        let is_raw_string = matches!(op_node.kind(), "multiline_string_literal")
            || (op_node.kind() == "string_literal"
                && op_node.start_position().row != op_node.end_position().row);
        let suf_doc =
            lower_navigation_suffix_with_break(suf_node, ctx, is_breakable, is_raw_string);
        let mut parts = Vec::new();
        for c in comments {
            parts.push(Doc::HardLine);
            parts.push(lower_node(c, ctx));
        }
        parts.push(suf_doc);
        let chain_doc = if is_breakable || is_raw_string {
            Doc::Concat(vec![op_doc, Doc::continuation_indent(Doc::Concat(parts))])
        } else {
            Doc::Concat(vec![op_doc, Doc::Concat(parts)])
        };
        if is_navigation_chain_root(&node) {
            Doc::Group(Box::new(chain_doc))
        } else {
            chain_doc
        }
    } else {
        op_doc
    }
}

pub fn lower_navigation_suffix_with_break<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    is_breakable: bool,
    force_hardline: bool,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "." || kind == "?." {
            if force_hardline {
                docs.push(Doc::HardLine);
            } else if is_breakable {
                docs.push(Doc::SoftLine);
            }
            docs.push(Doc::text(ctx.node_text(&child)));
        } else {
            docs.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_navigation_suffix<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    lower_navigation_suffix_with_break(node, ctx, true, false)
}

pub fn is_call_receiver_in_chain(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    if ctx.options.block_indent == 4 {
        return false;
    }
    let mut parent = node.parent();
    if parent.is_some_and(|p| p.kind() == "annotated_lambda") {
        parent = parent.unwrap().parent();
    }
    let Some(suffix) = parent else {
        return false;
    };
    if suffix.kind() == "indexing_expression" {
        if suffix.child(0).is_some_and(|operand| {
            operand.kind() == "navigation_expression" && ctx.node_text(&operand).contains('\n')
        }) {
            return true;
        }
        return suffix
            .parent()
            .is_some_and(|p| p.kind() == "navigation_expression");
    }
    if suffix.kind() != "call_suffix" {
        return false;
    }
    let Some(call) = suffix.parent() else {
        return false;
    };
    if call.kind() != "call_expression" {
        return false;
    }
    let Some(parent) = call.parent() else {
        return false;
    };
    if parent.kind() != "navigation_expression" {
        return false;
    }
    if let Some(callee) = call.child(0)
        && callee.kind() == "navigation_expression"
    {
        let mut c = callee.walk();
        let mut chs = callee.children(&mut c);
        if let (Some(op), Some(suf)) = (chs.next(), chs.next())
            && ctx.has_newline_between(&op, &suf)
        {
            return false;
        }
    }
    true
}

pub fn is_spine_root(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };
    if matches!(
        parent.kind(),
        "call_expression" | "indexing_expression" | "navigation_expression" | "postfix_expression"
    ) && parent.child(0).is_some_and(|c| c.id() == node.id())
    {
        return false;
    }
    true
}
