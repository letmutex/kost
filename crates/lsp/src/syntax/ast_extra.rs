use tree_sitter::Node;

/// Extract instantiated type name from a constructor call in declaration
pub fn extract_instantiated_type<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        if k == "call_expression" {
            if let Some(callee) = child.child(0)
                && let Some(text) = source.get(callee.start_byte()..callee.end_byte())
            {
                let trimmed = text.trim();
                if !trimmed.contains(['(', ')', '{', '}', '.']) {
                    let base = trimmed.split('<').next().unwrap_or(trimmed);
                    let name = base.trim();
                    if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                        return Some(name);
                    }
                }
            }
            continue;
        }
        if matches!(
            k,
            "lambda_literal" | "annotated_lambda" | "anonymous_function" | "property_delegate"
        ) {
            continue;
        }
        if child.child_count() > 0
            && let Some(t) = extract_instantiated_type(&child, source)
        {
            return Some(t);
        }
    }
    None
}

/// Extract receiver type for extension functions/properties
pub fn extract_extension_receiver<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let nk = node.kind();
    if nk != "function_declaration" && nk != "property_declaration" {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        if matches!(
            k,
            "receiver_type" | "user_type" | "type_identifier" | "nullable_type" | "function_type"
        ) {
            let text = source
                .get(child.start_byte()..child.end_byte())?
                .trim()
                .trim_end_matches('.')
                .trim();
            if !text.is_empty() {
                return Some(text);
            }
        }
        if matches!(k, "simple_identifier" | "variable_declaration") {
            return None;
        }
    }
    None
}

/// Extract single-expression body RHS text from a declaration node (e.g. `fun get() = 42` -> `42`)
pub fn extract_single_expression_body<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    if let Some(expr) = extract_eq_rhs_from_node(node, source) {
        return Some(expr);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_body"
            && let Some(expr) = extract_eq_rhs_from_node(&child, source)
        {
            return Some(expr);
        }
    }
    None
}

fn extract_eq_rhs_from_node<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    let mut saw_eq = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "=" {
            saw_eq = true;
        } else if saw_eq {
            let text = child.utf8_text(source.as_bytes()).ok()?.trim();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Strip outermost matching balanced parentheses from an expression string, e.g. `(u)` -> `u`
pub fn strip_enclosing_parens(s: &str) -> &str {
    let mut trimmed = s.trim();
    while trimmed.starts_with('(') && trimmed.ends_with(')') {
        let mut depth = 0;
        let mut fully_enclosed = true;
        for (i, c) in trimmed.char_indices() {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 && i < trimmed.len() - 1 {
                    fully_enclosed = false;
                    break;
                }
            }
        }
        if fully_enclosed && depth == 0 {
            trimmed = trimmed[1..trimmed.len() - 1].trim();
        } else {
            break;
        }
    }
    trimmed
}
