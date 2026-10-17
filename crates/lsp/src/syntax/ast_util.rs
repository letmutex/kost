use tree_sitter::Node;

use crate::parser::extract_extension_receiver;

/// Check if an identifier node is the callee of a function invocation (e.g. `fib(5)` or `obj.fib(5)`)
pub fn is_call_callee(node: &Node, source: &str) -> bool {
    if let Some(parent) = node.parent() {
        let pk = parent.kind();
        if pk == "call_expression"
            && let Some(callee) = parent.child(0)
        {
            return callee.id() == node.id();
        }
        // In tree-sitter-kotlin 0.4.0:
        // call_expression -> navigation_expression -> navigation_suffix -> simple_identifier
        if pk == "navigation_suffix"
            && let Some(nav_expr) = parent.parent()
            && nav_expr.kind() == "navigation_expression"
            && let Some(call_expr) = nav_expr.parent()
            && call_expr.kind() == "call_expression"
        {
            let is_callee = call_expr.child(0).is_some_and(|c| c.id() == nav_expr.id());
            let is_suffix_member = parent
                .children(&mut parent.walk())
                .any(|c| c.id() == node.id() && c.kind() == "simple_identifier");
            if is_callee && is_suffix_member {
                return true;
            }
        }
        if pk == "navigation_expression"
            && let Some(gp) = parent.parent()
            && gp.kind() == "call_expression"
        {
            return gp.child(0).is_some_and(|c| c.id() == parent.id())
                && parent.child(2).is_some_and(|c| c.id() == node.id());
        }
        if pk == "constructor_invocation"
            || (pk == "user_type"
                && parent
                    .parent()
                    .is_some_and(|p| p.kind() == "constructor_invocation"))
        {
            return true;
        }
        if pk == "infix_expression" && parent.child(1).is_some_and(|c| c.id() == node.id()) {
            return true;
        }
    }
    let end_byte = node.end_byte();
    if end_byte < source.len() {
        let rest = source[end_byte..].trim_start();
        if rest.starts_with('(')
            && let Some(parent) = node.parent()
            && !matches!(
                parent.kind(),
                "function_declaration"
                    | "class_declaration"
                    | "primary_constructor"
                    | "secondary_constructor"
            )
        {
            return true;
        }
    }
    false
}

/// Finds the extension receiver type of an enclosing function or property declaration
pub fn find_enclosing_extension_receiver(node: Node, source: &str) -> Option<String> {
    let mut current = Some(node);
    while let Some(curr) = current {
        let k = curr.kind();
        if matches!(
            k,
            "class_declaration" | "object_declaration" | "companion_object"
        ) {
            return None;
        }
        if matches!(k, "function_declaration" | "property_declaration")
            && let Some(ext) = extract_extension_receiver(&curr, source)
        {
            let clean = ext
                .split('<')
                .next()
                .unwrap_or(ext)
                .trim_end_matches('?')
                .trim();
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
        current = curr.parent();
    }
    None
}

/// Extract the initializer expression (the RHS of `=`) for a variable or property declaration node
pub fn extract_initializer_expression<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let target = if node.kind() == "variable_declaration"
        && node
            .parent()
            .is_some_and(|p| p.kind() == "property_declaration")
    {
        node.parent()?
    } else {
        *node
    };
    let mut cursor = target.walk();
    for child in target.children(&mut cursor) {
        if child.kind() == "=" {
            let start = child.end_byte();
            let end = target.end_byte();
            if start < end && end <= source.len() {
                let text = source[start..end].trim();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Extract the initializer AST node (the RHS of `=`) for a variable or property declaration node
pub fn extract_initializer_node<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let target = if node.kind() == "variable_declaration"
        && node
            .parent()
            .is_some_and(|p| p.kind() == "property_declaration")
    {
        node.parent()?
    } else {
        *node
    };
    let mut cursor = target.walk();
    let mut found_eq = false;
    for child in target.children(&mut cursor) {
        if found_eq {
            if child.is_named() && !child.kind().contains("comment") {
                return Some(child);
            }
        } else if child.kind() == "=" {
            found_eq = true;
        }
    }
    None
}

/// Check if an AST node is a navigation_suffix using the "::" operator
pub fn is_double_colon_navigation_suffix(suffix_node: &Node) -> bool {
    if suffix_node.kind() != "navigation_suffix" {
        return false;
    }
    let mut cursor = suffix_node.walk();
    suffix_node
        .children(&mut cursor)
        .any(|child| child.kind() == "::")
}

/// Check if an AST node is the target member in a callable reference expression
pub fn is_callable_reference_target(node: &Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let pk = parent.kind();
    if pk == "callable_reference" {
        let mut cursor = parent.walk();
        let mut seen_colons = false;
        for child in parent.children(&mut cursor) {
            if child.kind() == "::" {
                seen_colons = true;
            } else if seen_colons && child.id() == node.id() {
                return node.kind() == "simple_identifier";
            }
        }
        return false;
    }
    if pk == "navigation_suffix" && is_double_colon_navigation_suffix(&parent) {
        return node.kind() == "simple_identifier";
    }
    false
}

/// Extract the AST receiver node for a member access, navigation, infix, or callable reference
pub fn extract_receiver_node<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let parent = node.parent()?;
    let pk = parent.kind();
    if pk == "navigation_suffix" {
        let nav_expr = parent.parent()?;
        if nav_expr.kind() == "navigation_expression" {
            return nav_expr.child(0);
        }
    } else if pk == "callable_reference" {
        if parent.named_child_count() > 1 {
            let receiver_child = parent.named_child(0)?;
            if receiver_child.id() != node.id() {
                return Some(receiver_child);
            }
        }
    } else if matches!(pk, "navigation_expression" | "infix_expression") {
        let lhs = parent.child(0)?;
        if lhs.id() != node.id() {
            return Some(lhs);
        }
    }
    None
}

/// Extract the receiver text for a callable reference (if any)
pub fn extract_callable_reference_receiver(node: &Node, source: &str) -> Option<String> {
    if is_callable_reference_target(node) {
        let rec_node = extract_receiver_node(node)?;
        return source
            .get(rec_node.start_byte()..rec_node.end_byte())
            .map(|s| s.trim().to_string());
    }
    None
}
