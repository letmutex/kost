use tree_sitter::Node;

use crate::lower::context::LowerContext;

/// Determines if an expression is block-like and can anchor its first line on the `=` line.
pub fn is_block_like_rhs(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    is_block_like_rhs_ext(node, false, ctx)
}

pub fn is_block_like_rhs_ext(
    node: &Node<'_>,
    is_destructuring: bool,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let kind = node.kind();
    if kind == "control_structure_body" {
        return node
            .children(&mut node.walk())
            .any(|c| c.kind() == "{" || is_block_like_rhs_ext(&c, is_destructuring, ctx));
    }

    if matches!(
        kind,
        "block" | "lambda_literal" | "annotated_lambda" | "anonymous_function"
    ) {
        return true;
    }

    if is_destructuring
        && kind == "call_expression"
        && ctx.style.preset != crate::options::StylePreset::Kotlinlang
    {
        let is_chained = node
            .child(0)
            .is_some_and(|c| c.kind() == "navigation_expression");
        if !is_chained {
            return true;
        }
    }

    if kind == "call_expression" {
        if is_scoping_call(node) {
            return true;
        }

        if ctx.options.use_experimental_engine {
            let (receiver, segments) = crate::lower::chain_step::flatten_spine(*node, ctx);
            if receiver.kind() == "simple_identifier"
                && let Some(crate::lower::chain_step::SpineSegment::Call {
                    val_args: Some(va),
                    trailing_lambda: None,
                    ..
                }) = segments.first()
                && va.start_position().row != va.end_position().row
            {
                return true;
            }
        }
    }

    let root = get_chain_root(*node);
    if root.id() != node.id() && is_block_like_rhs_ext(&root, is_destructuring, ctx) {
        return true;
    }

    false
}

fn inspect_call_child(
    child: &Node<'_>,
    has_type_args: &mut bool,
    has_parens: &mut bool,
    has_lambda: &mut bool,
) {
    match child.kind() {
        "type_arguments" => *has_type_args = true,
        "value_arguments" | "(" => *has_parens = true,
        "lambda_literal" => *has_lambda = true,
        "annotated_lambda" => {
            if !child
                .children(&mut child.walk())
                .any(|c| c.kind() == "annotation")
            {
                *has_lambda = true;
            }
        }
        "call_suffix" => {
            let mut sc = child.walk();
            for suffix_child in child.children(&mut sc) {
                inspect_call_child(&suffix_child, has_type_args, has_parens, has_lambda);
            }
        }
        _ => {}
    }
}

/// Determines if an expression is a scoping function call without chained calls (e.g. `lazy { ... }`, `run { ... }`).
pub fn is_scoping_call(node: &Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "lambda_literal" {
        return true;
    }
    if kind == "annotated_lambda" {
        let has_annotation = node
            .children(&mut node.walk())
            .any(|c| c.kind() == "annotation");
        return !has_annotation;
    }
    if kind != "call_expression" {
        return false;
    }
    let Some(callee) = node.child(0) else {
        return false;
    };
    let is_ident_or_nav = callee.kind() == "simple_identifier"
        || (callee.kind() == "navigation_expression"
            && callee
                .child(0)
                .is_some_and(|c| c.kind() == "simple_identifier"));
    if !is_ident_or_nav {
        return false;
    }
    let mut has_parens = false;
    let mut has_type_args = false;
    let mut has_lambda = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        inspect_call_child(&child, &mut has_type_args, &mut has_parens, &mut has_lambda);
    }
    !has_parens && !has_type_args && has_lambda
}

fn get_chain_root<'a>(node: Node<'a>) -> Node<'a> {
    let mut curr = node;
    loop {
        if curr.kind() == "call_expression"
            && let Some(callee) = curr.child(0)
            && callee.kind() == "navigation_expression"
            && let Some(rcv) = callee.child(0)
        {
            curr = rcv;
            continue;
        } else if curr.kind() == "navigation_expression"
            && let Some(rcv) = curr.child(0)
        {
            curr = rcv;
            continue;
        }
        break;
    }
    curr
}
