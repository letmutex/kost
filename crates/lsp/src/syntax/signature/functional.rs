use tree_sitter::Node;

fn find_inner_type_node<'a>(node: Node<'a>) -> Option<Node<'a>> {
    let count = node.child_count();
    for i in 0..count {
        if let Some(child) = node.child(i)
            && !matches!(child.kind(), "(" | ")" | "?" | "quest" | "type_modifiers")
        {
            return Some(child);
        }
    }
    None
}

fn find_function_type_node<'a>(root: &'a Node<'a>) -> Option<Node<'a>> {
    let mut type_alias = None;
    for i in 0..root.child_count() {
        if let Some(child) = root.child(i)
            && child.kind() == "type_alias"
        {
            type_alias = Some(child);
            break;
        }
    }
    let type_alias = type_alias?;

    for i in 0..type_alias.child_count() {
        let Some(child) = type_alias.child(i) else {
            continue;
        };
        match child.kind() {
            "=" | "typealias" | "type_identifier" | "type_modifiers" => {}
            _ => {
                let mut current = child;
                while matches!(current.kind(), "nullable_type" | "parenthesized_type") {
                    if let Some(inner) = find_inner_type_node(current) {
                        current = inner;
                    } else {
                        break;
                    }
                }
                if current.kind() == "function_type" {
                    return Some(current);
                }
            }
        }
    }
    None
}

fn extract_parameter_type_text(child: &Node, wrapped: &str) -> Option<String> {
    let count = child.child_count();
    let mut colon_seen = false;
    let mut type_range = None;
    for i in 0..count {
        let Some(c) = child.child(i) else { continue };
        if c.kind() == ":" {
            colon_seen = true;
            continue;
        }
        if colon_seen {
            if c.kind() == "=" {
                break;
            }
            if matches!(
                c.kind(),
                "type_modifiers"
                    | "user_type"
                    | "nullable_type"
                    | "function_type"
                    | "parenthesized_type"
                    | "not_nullable_type"
                    | "dynamic"
                    | "type_identifier"
            ) {
                type_range = Some(match type_range {
                    Some((s, _)) => (s, c.end_byte()),
                    None => (c.start_byte(), c.end_byte()),
                });
            }
        }
    }
    let (s, e) = type_range?;
    let text = wrapped.get(s..e)?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Extract input parameter types from a functional type string (e.g. `(User, Int) -> Unit` -> `["User", "Int"]`)
pub fn extract_functional_input_types(func_type: &str) -> Vec<String> {
    let clean = func_type.trim();
    if clean.is_empty() {
        return Vec::new();
    }
    let wrapped = common::concat_str!("typealias _T = ", clean);
    let tree = match crate::syntax::parser::with_parser(|p| p.parse(&wrapped)) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let root = tree.root_node();
    let fn_node = match find_function_type_node(&root) {
        Some(n) => n,
        None => return Vec::new(),
    };

    let mut params_node = None;
    for i in 0..fn_node.child_count() {
        if let Some(child) = fn_node.child(i)
            && child.kind() == "function_type_parameters"
        {
            params_node = Some(child);
            break;
        }
    }
    let params_node = match params_node {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut results = Vec::new();
    let mut pending_modifier_start = None;
    for i in 0..params_node.child_count() {
        let Some(child) = params_node.child(i) else {
            continue;
        };
        if child.kind() == "type_modifiers" {
            if pending_modifier_start.is_none() {
                pending_modifier_start = Some(child.start_byte());
            }
            continue;
        }
        if child.kind() == "parameter" {
            pending_modifier_start = None;
            if let Some(ty) = extract_parameter_type_text(&child, &wrapped) {
                results.push(ty);
            }
        } else if matches!(
            child.kind(),
            "user_type"
                | "nullable_type"
                | "function_type"
                | "parenthesized_type"
                | "not_nullable_type"
                | "dynamic"
                | "type_identifier"
        ) {
            let start = pending_modifier_start.take().unwrap_or(child.start_byte());
            if let Some(text) = wrapped.get(start..child.end_byte()) {
                results.push(text.trim().to_string());
            }
        }
    }
    results
}

fn unwrap_parenthesized_type_range(mut node: Node) -> (usize, usize) {
    while node.kind() == "parenthesized_type" {
        let count = node.child_count();
        let mut first = None;
        let mut last = None;
        let mut non_paren_count = 0;
        for i in 0..count {
            if let Some(c) = node.child(i)
                && c.kind() != "("
                && c.kind() != ")"
            {
                if first.is_none() {
                    first = Some(c);
                }
                last = Some(c);
                non_paren_count += 1;
            }
        }
        if non_paren_count == 1 && first.is_some_and(|n| n.kind() == "parenthesized_type") {
            node = first.unwrap();
        } else if let (Some(f), Some(l)) = (first, last) {
            return (f.start_byte(), l.end_byte());
        } else {
            break;
        }
    }
    (node.start_byte(), node.end_byte())
}

/// Extract return type of a functional type (e.g. `() -> T`, `(T) -> R`, `T.() -> R`)
pub fn extract_functional_return_type(param_type: &str) -> Option<String> {
    let clean = param_type.trim();
    if clean.is_empty() {
        return None;
    }
    let wrapped = common::concat_str!("typealias _T = ", clean);
    let tree = crate::syntax::parser::with_parser(|p| p.parse(&wrapped))?;
    let root = tree.root_node();
    let fn_node = find_function_type_node(&root)?;

    let mut is_after_arrow = false;
    let mut modifier_start = None;
    for i in 0..fn_node.child_count() {
        let Some(child) = fn_node.child(i) else {
            continue;
        };
        if child.kind() == "->" {
            is_after_arrow = true;
            continue;
        }
        if is_after_arrow {
            if child.kind() == "type_modifiers" {
                if modifier_start.is_none() {
                    modifier_start = Some(child.start_byte());
                }
                continue;
            }
            if matches!(
                child.kind(),
                "user_type"
                    | "nullable_type"
                    | "function_type"
                    | "parenthesized_type"
                    | "not_nullable_type"
                    | "dynamic"
                    | "type_identifier"
            ) {
                let (start, end) = unwrap_parenthesized_type_range(child);
                let actual_start = modifier_start.unwrap_or(start);
                let text = wrapped.get(actual_start..end)?.trim();
                return (!text.is_empty()).then(|| text.to_string());
            }
        }
    }
    None
}
