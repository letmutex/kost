use tree_sitter::Node;

/// Find enclosing call expression and lambda literal node from a parameter node
pub fn find_enclosing_call_and_lambda(start_node: Node) -> Option<(Node, Node)> {
    let mut current = Some(start_node);
    let mut lambda_node = None;

    while let Some(node) = current {
        let k = node.kind();
        if k == "lambda_literal" && lambda_node.is_none() {
            lambda_node = Some(node);
        } else if k == "call_expression" {
            if let Some(l) = lambda_node {
                return Some((node, l));
            }
        } else if matches!(k, "function_declaration" | "class_declaration") {
            break;
        }
        current = node.parent();
    }
    None
}

/// Collect enclosing lambda literals from a node walking outwards
pub fn collect_enclosing_lambdas(start_node: Node) -> Vec<Node> {
    let mut lambdas = Vec::new();
    let mut current = Some(start_node);
    while let Some(node) = current {
        let k = node.kind();
        if k == "lambda_literal" {
            lambdas.push(node);
        } else if matches!(
            k,
            "function_declaration"
                | "anonymous_function"
                | "secondary_constructor"
                | "class_declaration"
                | "source_file"
        ) {
            break;
        }
        current = node.parent();
    }
    lambdas
}

/// Find enclosing call expression for a lambda literal node
pub fn find_call_for_lambda(lambda_node: Node) -> Option<Node> {
    let mut current = lambda_node.parent();
    while let Some(node) = current {
        let k = node.kind();
        if k == "call_expression" {
            return Some(node);
        }
        if matches!(
            k,
            "function_declaration"
                | "class_declaration"
                | "source_file"
                | "property_declaration"
                | "variable_declaration"
        ) {
            break;
        }
        current = node.parent();
    }
    None
}

/// Find enclosing typed declaration for a lambda node
pub fn find_enclosing_typed_decl_for_lambda<'a>(lambda_node: Node<'a>) -> Option<Node<'a>> {
    let mut current = lambda_node.parent();
    while let Some(node) = current {
        let k = node.kind();
        if matches!(
            k,
            "property_declaration"
                | "variable_declaration"
                | "parameter"
                | "class_parameter"
                | "function_declaration"
        ) {
            return Some(node);
        }
        if matches!(k, "class_declaration" | "source_file") {
            break;
        }
        current = node.parent();
    }
    None
}

/// Find index of parameter node inside lambda parameters
pub fn find_param_index_in_lambda(param_node: Node, lambda_node: Node) -> usize {
    let mut cursor = lambda_node.walk();
    for child in lambda_node.children(&mut cursor) {
        if child.kind() == "lambda_parameters" {
            let mut p_cursor = child.walk();
            let mut idx = 0;
            for p in child.children(&mut p_cursor) {
                let k = p.kind();
                if matches!(
                    k,
                    "variable_declaration"
                        | "parameter"
                        | "simple_identifier"
                        | "multi_variable_declaration"
                ) {
                    if p.start_byte() == param_node.start_byte()
                        || (p.start_byte() <= param_node.start_byte()
                            && param_node.end_byte() <= p.end_byte())
                        || (param_node.start_byte() <= p.start_byte()
                            && p.end_byte() <= param_node.end_byte())
                    {
                        return idx;
                    }
                    idx += 1;
                }
            }
        }
    }
    0
}

/// Find index of the lambda argument in its enclosing call expression
pub fn find_lambda_argument_index_in_call(call_node: Node, lambda_node: Node) -> usize {
    if let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(call_node) {
        for (idx, arg) in view.value_arguments.iter().enumerate() {
            if arg.start_byte() <= lambda_node.start_byte()
                && lambda_node.end_byte() <= arg.end_byte()
            {
                return idx;
            }
        }
        return view.value_arguments.len();
    }
    0
}

/// Extract argument expressions from a call node
pub fn extract_call_argument_expressions<'a>(call_node: Node<'a>, source: &'a str) -> Vec<&'a str> {
    if let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(call_node) {
        view.value_arguments
            .iter()
            .filter_map(|arg| {
                crate::syntax::ast_call_view::AstCallView::argument_expression(*arg)
                    .or(Some(*arg))
                    .and_then(|e| source.get(e.start_byte()..e.end_byte()).map(str::trim))
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// Check if lambda literal has explicitly declared parameters
pub fn lambda_has_explicit_params(lambda_node: Node) -> bool {
    let mut cursor = lambda_node.walk();
    for child in lambda_node.children(&mut cursor) {
        if child.kind() == "lambda_parameters" && child.named_child_count() > 0 {
            return true;
        }
    }
    false
}

/// Extract explicitly declared parameter types of a lambda literal
pub fn extract_lambda_param_types(
    lambda_node: Node,
    source: &str,
) -> Vec<crate::inference::types::KotlinType> {
    let mut params = Vec::new();
    let mut cursor = lambda_node.walk();
    for child in lambda_node.children(&mut cursor) {
        if child.kind() == "lambda_parameters" {
            let mut p_cursor = child.walk();
            for p in child.children(&mut p_cursor) {
                if matches!(p.kind(), "parameter" | "variable_declaration") {
                    if let Some(annot) = crate::parser::extract_type_annotation(&p, source)
                        && let Some(ty) = crate::inference::types::parse_type_signature(annot)
                    {
                        params.push(ty);
                        continue;
                    }
                    params.push(crate::inference::types::KotlinType::DynamicOrAny);
                }
            }
        }
    }
    params
}

/// Extract explicitly declared parameter names of a lambda literal
pub fn extract_lambda_param_names<'a>(lambda_node: Node<'a>, source: &'a str) -> Vec<&'a str> {
    let mut names = Vec::new();
    let mut cursor = lambda_node.walk();
    for child in lambda_node.children(&mut cursor) {
        if child.kind() == "lambda_parameters" {
            let mut p_cursor = child.walk();
            for p in child.children(&mut p_cursor) {
                if matches!(
                    p.kind(),
                    "parameter" | "variable_declaration" | "simple_identifier"
                ) {
                    let ident = if p.kind() == "simple_identifier" {
                        Some(p)
                    } else {
                        let mut found = None;
                        let mut c = p.walk();
                        for n in p.children(&mut c) {
                            if n.kind() == "simple_identifier" {
                                found = Some(n);
                                break;
                            }
                        }
                        found
                    };
                    if let Some(id_node) = ident
                        && let Ok(name) = id_node.utf8_text(source.as_bytes())
                    {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            names.push(trimmed);
                        }
                    }
                }
            }
        }
    }
    names
}

/// Find the last expression node and text inside a lambda literal
pub fn find_last_expression_in_lambda<'a>(
    lambda_node: Node<'a>,
    source: &'a str,
) -> Option<(Node<'a>, &'a str)> {
    let mut cursor = lambda_node.walk();
    let mut stmts_node = None;
    let mut last_direct_child = None;

    for child in lambda_node.children(&mut cursor) {
        if child.kind() == "statements" {
            stmts_node = Some(child);
            break;
        }
        if child.is_named()
            && !matches!(
                child.kind(),
                "lambda_parameters" | "comment" | "line_comment" | "multiline_comment"
            )
        {
            last_direct_child = Some(child);
        }
    }

    let target_node = if let Some(stmts) = stmts_node {
        let mut s_cursor = stmts.walk();
        let mut last_stmt = None;
        for child in stmts.children(&mut s_cursor) {
            if child.is_named()
                && !matches!(
                    child.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                )
            {
                last_stmt = Some(child);
            }
        }
        last_stmt
    } else {
        last_direct_child
    }?;

    let (expr_node, expr_text) =
        if matches!(target_node.kind(), "jump_expression" | "return_expression") {
            let mut expr_child = None;
            let mut c = target_node.walk();
            for child in target_node.children(&mut c) {
                if child.is_named() && !child.kind().contains("label") {
                    expr_child = Some(child);
                }
            }
            if let Some(child) = expr_child {
                let child_text = child.utf8_text(source.as_bytes()).ok()?.trim();
                (child, child_text)
            } else {
                let full_text = target_node.utf8_text(source.as_bytes()).ok()?.trim();
                let stripped = full_text.strip_prefix("return").unwrap_or(full_text).trim();
                (target_node, stripped)
            }
        } else {
            let text = target_node.utf8_text(source.as_bytes()).ok()?.trim();
            (target_node, text)
        };

    (!expr_text.is_empty()).then_some((expr_node, expr_text))
}
