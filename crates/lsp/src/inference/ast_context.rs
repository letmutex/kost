use super::ast_eval::AstEvalContext;
use super::types::KotlinType;
use tree_sitter::Node;

/// Infer statement or control node (loops, jumps, casts, type tests)
pub fn infer_statement_or_control(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    match node.kind() {
        "while_statement" | "for_statement" | "do_while_statement" | "assignment" => {
            Some(KotlinType::UNIT)
        }
        "call_expression" => {
            let first = node.child(0)?;
            if first.kind() == "simple_identifier" {
                let name = ctx.node_text(first)?;
                if (name == "while" || name == "for")
                    && node.child(1).is_some_and(|c| c.kind() == "call_suffix")
                {
                    return Some(KotlinType::UNIT);
                }
            }
            None
        }
        "jump_expression" | "return_expression" => Some(KotlinType::NOTHING),
        "is_expression" | "type_test" => Some(KotlinType::BOOLEAN),
        "as_expression" => {
            super::ast_binary::infer_as_cast(ctx.index, ctx.current_url, node, ctx.node_source)
        }
        _ => None,
    }
}

/// Find expected type from an enclosing property, variable, function, or return expression
pub fn find_enclosing_initializer_expected_type(
    ctx: &AstEvalContext,
    node: Node,
) -> Option<KotlinType> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        let pk = parent.kind();
        if pk == "property_declaration" || pk == "variable_declaration" {
            if let Some(init) = super::variable::extract_initializer_node(&parent)
                && (init == current || init == node)
            {
                return resolve_decl_annotation_type(ctx, &parent);
            }
            return None;
        } else if pk == "function_declaration" {
            return resolve_decl_annotation_type(ctx, &parent);
        } else if pk == "jump_expression" || pk == "return_expression" {
            return find_innermost_function_return_type(ctx, parent);
        } else if pk == "function_body" {
            let mut cursor = parent.walk();
            let is_block = parent
                .children(&mut cursor)
                .any(|c| matches!(c.kind(), "{" | "statements"));
            if !is_block {
                current = parent;
            } else {
                return None;
            }
        } else if pk == "parenthesized_expression" {
            current = parent;
        } else {
            return None;
        }
    }
    None
}

pub(crate) fn resolve_decl_annotation_type(
    ctx: &AstEvalContext,
    node: &Node,
) -> Option<KotlinType> {
    let annot = crate::parser::extract_type_annotation(node, ctx.node_source)?;
    let raw = super::typealias::resolve_type_alias_or_import(ctx.index, ctx.current_url, annot);
    let parsed = super::types::parse_type_signature(&raw)?;
    Some(super::qualification::qualify_type(
        ctx.index,
        ctx.current_url,
        &parsed,
    ))
}

fn extract_jump_label<'a>(jump_node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = jump_node.walk();
    for child in jump_node.children(&mut cursor) {
        if child.kind() == "label" {
            return source
                .get(child.start_byte()..child.end_byte())
                .map(|s| s.trim_end_matches('@'));
        }
    }
    None
}

fn find_innermost_function_return_type(
    ctx: &AstEvalContext,
    jump_node: Node,
) -> Option<KotlinType> {
    let return_label = extract_jump_label(jump_node, ctx.node_source);
    let mut current = jump_node;
    while let Some(parent) = current.parent() {
        let pk = parent.kind();
        if pk == "function_declaration" {
            if let Some(target_label) = return_label {
                let fn_name = crate::parser::extract_identifier_text(&parent, ctx.node_source);
                if fn_name != Some(target_label) {
                    current = parent;
                    continue;
                }
            }
            return resolve_decl_annotation_type(ctx, &parent);
        } else if pk == "anonymous_function" && return_label.is_none() {
            return resolve_decl_annotation_type(ctx, &parent);
        }
        current = parent;
    }
    None
}
