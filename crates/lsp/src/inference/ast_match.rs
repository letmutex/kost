use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

fn match_in_receiver_chain<'a>(
    node: Node<'a>,
    clean_trimmed: &str,
    source: &str,
) -> Option<Node<'a>> {
    let mut current = node;
    loop {
        if is_ast_evaluable_kind(current.kind())
            && source.get(current.byte_range()).map(str::trim) == Some(clean_trimmed)
        {
            return Some(current);
        }
        match current.kind() {
            "navigation_expression" => {
                let Some(lhs) = current.child(0) else { break };
                current = lhs;
            }
            "call_expression" => {
                let Some(callee) = current.child(0) else {
                    break;
                };
                current = callee;
            }
            "parenthesized_expression" => {
                let Some(inner) = current.named_child(0) else {
                    break;
                };
                current = inner;
            }
            "postfix_expression" => {
                let Some(base) = current.child(0) else { break };
                current = base;
            }
            _ => break,
        }
    }
    None
}

fn find_matching_in_ancestors<'a>(
    at_pos: Option<Node<'a>>,
    clean_trimmed: &str,
    source: &str,
) -> Option<Node<'a>> {
    if let Some(pos_node) = at_pos
        && let Some(rec) = crate::syntax::ast_util::extract_receiver_node(&pos_node)
        && let Some(matched) = match_in_receiver_chain(rec, clean_trimmed, source)
    {
        return Some(matched);
    }

    let mut current = at_pos;
    while let Some(node) = current {
        if let Some(matched) = match_in_receiver_chain(node, clean_trimmed, source) {
            return Some(matched);
        }
        if matches!(
            node.kind(),
            "value_arguments"
                | "value_argument"
                | "call_expression"
                | "navigation_expression"
                | "infix_expression"
                | "binary_expression"
                | "property_declaration"
                | "variable_declaration"
        ) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.is_named()
                    && let Some(matched) = match_in_receiver_chain(child, clean_trimmed, source)
                {
                    return Some(matched);
                }
            }
        }
        if matches!(
            node.kind(),
            "statements" | "source_file" | "class_body" | "function_body"
        ) {
            break;
        }
        current = node.parent();
    }
    None
}

fn find_enclosing_statement<'a>(at_pos: Option<Node<'a>>) -> Option<Node<'a>> {
    let mut curr = at_pos;
    while let Some(node) = curr {
        if let Some(parent) = node.parent()
            && matches!(
                parent.kind(),
                "statements" | "source_file" | "class_body" | "function_body"
            )
        {
            return Some(node);
        }
        curr = node.parent();
    }
    None
}

/// Find an AST expression node in root at pos using byte spans and syntax hierarchy
pub fn find_matching_ast_node<'a>(
    root: Node<'a>,
    pos: Position,
    clean: &str,
    source: &str,
) -> Option<Node<'a>> {
    let clean_trimmed = clean.trim();
    if clean_trimmed.is_empty() {
        return None;
    }
    let offset = crate::position::position_to_byte_offset(pos, source);
    let at_pos = root
        .named_descendant_for_byte_range(offset, offset)
        .or_else(|| crate::parser::find_node_at_position(root, pos));

    if let Some(node) = find_matching_in_ancestors(at_pos, clean_trimmed, source) {
        return Some(node);
    }

    let enclosing = find_enclosing_statement(at_pos)?;
    find_matching_in_descendants(enclosing, clean_trimmed, source)
}

fn find_matching_in_descendants<'a>(
    node: Node<'a>,
    clean_trimmed: &str,
    source: &str,
) -> Option<Node<'a>> {
    for idx in (0..node.child_count()).rev() {
        if let Some(child) = node.child(idx)
            && let Some(found) = find_matching_in_descendants(child, clean_trimmed, source)
        {
            return Some(found);
        }
    }
    if is_ast_evaluable_kind(node.kind())
        && source.get(node.byte_range()).map(str::trim) == Some(clean_trimmed)
    {
        return Some(node);
    }
    None
}

/// Find the primary expression node within a parsed snippet or subtree
pub fn find_root_expression_node(node: Node) -> Option<Node> {
    let mut current = node;
    loop {
        match current.kind() {
            "source_file" | "statements" | "statement" => {
                let mut named_child = None;
                let mut c = current.walk();
                for child in current.children(&mut c) {
                    if child.is_named()
                        && !matches!(
                            child.kind(),
                            "comment" | "line_comment" | "multiline_comment"
                        )
                    {
                        named_child = Some(child);
                        break;
                    }
                }
                current = named_child?;
            }
            _ => return Some(current),
        }
    }
}

/// Check if an AST node kind can be directly evaluated as an expression
pub fn is_ast_evaluable_kind(kind: &str) -> bool {
    matches!(
        kind,
        "parenthesized_expression"
            | "integer_literal"
            | "long_literal"
            | "hex_literal"
            | "bin_literal"
            | "unsigned_literal"
            | "real_literal"
            | "float_literal"
            | "boolean_literal"
            | "character_literal"
            | "string_literal"
            | "line_string_expression"
            | "multi_line_string_expression"
            | "null"
            | "null_literal"
            | "elvis_expression"
            | "is_expression"
            | "as_expression"
            | "navigation_expression"
            | "call_expression"
            | "lambda_literal"
            | "if_expression"
            | "when_expression"
            | "try_expression"
            | "infix_expression"
            | "simple_identifier"
            | "this_expression"
            | "super_expression"
            | "while_statement"
            | "for_statement"
            | "do_while_statement"
            | "jump_expression"
            | "return_expression"
    )
}
