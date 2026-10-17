use tower_lsp::lsp_types::SymbolKind;
use url::Url;

use crate::indexer::WorkspaceIndex;
use crate::inference::signature::SignatureInfo;
use crate::inference::types::KotlinType;
use crate::syntax::ast_call_view::parse_call_arg;

fn find_enclosing_call_node(call_expr: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = Some(call_expr);
    while let Some(n) = current {
        if matches!(n.kind(), "call_expression" | "constructor_invocation") {
            return Some(n);
        } else if matches!(
            n.kind(),
            "function_declaration" | "class_declaration" | "source_file"
        ) {
            break;
        }
        current = n.parent();
    }
    None
}

fn find_lambda_argument<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    if let Some(unwrapped) = crate::syntax::ast_call_view::AstCallView::unwrap_lambda(*node) {
        return Some(unwrapped);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(unwrapped) = crate::syntax::ast_call_view::AstCallView::unwrap_lambda(child) {
            return Some(unwrapped);
        }
        if child.kind() == "parenthesized_expression"
            && let Some(inner) = find_lambda_argument(&child)
        {
            return Some(inner);
        }
    }
    None
}

fn extract_callee_identifier<'a>(
    call_node: tree_sitter::Node<'a>,
    call_expr: tree_sitter::Node<'a>,
    source: &'a str,
) -> Option<&'a str> {
    if call_expr.kind() == "simple_identifier" {
        return call_expr.utf8_text(source.as_bytes()).ok().map(str::trim);
    }
    let view = crate::syntax::ast_call_view::AstCallView::from_node(call_node)?;
    let callee = view.callee_node?;
    if callee.kind() == "navigation_expression" {
        let suffix = callee
            .children(&mut callee.walk())
            .find(|child| child.kind() == "navigation_suffix")?;
        let ident = suffix
            .children(&mut suffix.walk())
            .find(|child| child.kind() == "simple_identifier")?;
        ident.utf8_text(source.as_bytes()).ok().map(str::trim)
    } else if callee.kind() == "simple_identifier" {
        callee.utf8_text(source.as_bytes()).ok().map(str::trim)
    } else {
        view.callee_text(source)
    }
}

fn has_conflicting_overloads(index: &WorkspaceIndex, callee_name: &str, arg_count: usize) -> bool {
    let Some(decls) = index.name_to_decls.get(callee_name) else {
        return false;
    };
    let mut matching_overloads = 0;
    for (_, decl) in decls {
        if decl.kind != SymbolKind::FUNCTION && decl.kind != SymbolKind::CONSTRUCTOR {
            continue;
        }
        let Some(sig) = SignatureInfo::from_decl(decl) else {
            matching_overloads += 1;
            if matching_overloads > 1 {
                return true;
            }
            continue;
        };
        let has_vararg = sig.params.iter().any(|param| param.is_vararg);
        let min_params = sig
            .params
            .iter()
            .filter(|param| !param.has_default && !param.is_vararg)
            .count();
        let max_params = if has_vararg {
            usize::MAX
        } else {
            sig.params.len()
        };
        if arg_count >= min_params && arg_count <= max_params {
            matching_overloads += 1;
            if matching_overloads > 1 {
                return true;
            }
        }
    }
    false
}

fn infer_argument_node_type(
    arg: &tree_sitter::Node,
    expr: &str,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
    root: Option<tree_sitter::Node>,
    skip_lambda_return_type: bool,
) -> Option<KotlinType> {
    if let Some(lambda_node) = find_lambda_argument(arg) {
        let params = crate::inference::lambda_ast::extract_lambda_param_types(lambda_node, source);
        // In Kotlin, lambda bodies can coerce expressions to `Unit` (statement lambdas).
        // During reference search, `skip_lambda_return_type` treats lambda arguments as
        // `DynamicOrAny` so coerced returns or imperfect inference do not drop valid references.
        let return_type = if skip_lambda_return_type {
            KotlinType::DynamicOrAny
        } else {
            infer_fast_lambda_return_type(index, current_url, lambda_node, source, root)
        };
        Some(KotlinType::Function {
            receiver: None,
            params,
            return_type: Box::new(return_type),
            is_suspend: false,
        })
    } else {
        let arg_pos = crate::position::node_to_range(arg).start;
        crate::inference::infer_receiver_type_depth(
            index,
            current_url,
            expr,
            arg_pos,
            source,
            root,
            1,
        )
    }
}

fn infer_fast_lambda_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    lambda_node: tree_sitter::Node,
    source: &str,
    root: Option<tree_sitter::Node>,
) -> KotlinType {
    let Some((expr_node, _)) =
        crate::inference::lambda_ast::find_last_expression_in_lambda(lambda_node, source)
    else {
        return KotlinType::UNIT;
    };

    match expr_node.kind() {
        "string_literal" => return KotlinType::STRING,
        "integer_literal" => return KotlinType::INT,
        "boolean_literal" => return KotlinType::BOOLEAN,
        "character_literal" => return KotlinType::CHAR,
        "float_literal" | "real_literal" => return KotlinType::FLOAT,
        _ => {}
    }

    crate::inference::lambda::infer_lambda_return_type(
        index,
        current_url,
        lambda_node,
        source,
        root,
        1,
    )
    .unwrap_or(KotlinType::UNIT)
}

pub fn extract_call_args_and_inferred_types(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_expr: tree_sitter::Node,
    source: &str,
    root: Option<tree_sitter::Node>,
) -> (Vec<String>, Vec<Option<KotlinType>>) {
    extract_call_args_and_inferred_types_opts(index, current_url, call_expr, source, root, false)
}

pub fn extract_call_args_and_inferred_types_opts(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_expr: tree_sitter::Node,
    source: &str,
    root: Option<tree_sitter::Node>,
    skip_lambda_return_type: bool,
) -> (Vec<String>, Vec<Option<KotlinType>>) {
    let mut raw_args = Vec::new();
    let Some(call) = find_enclosing_call_node(call_expr) else {
        return (raw_args, Vec::new());
    };

    let mut arg_nodes = Vec::new();
    collect_call_value_arguments(call, &mut arg_nodes);

    let mut valid_args = Vec::with_capacity(arg_nodes.len());
    for arg in arg_nodes {
        if let Ok(text) = arg.utf8_text(source.as_bytes()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                raw_args.push(trimmed.to_string());
                valid_args.push((arg, trimmed));
            }
        }
    }

    let callee_name = extract_callee_identifier(call, call_expr, source);
    let needs_typed_scoring =
        callee_name.is_none_or(|name| has_conflicting_overloads(index, name, raw_args.len()));

    if !needs_typed_scoring {
        let inferred = vec![None; raw_args.len()];
        return (raw_args, inferred);
    }

    let mut inferred = Vec::with_capacity(valid_args.len());
    for (arg, trimmed) in valid_args {
        let parsed = parse_call_arg(trimmed);
        let inferred_type = infer_argument_node_type(
            &arg,
            &parsed.expr,
            source,
            index,
            current_url,
            root,
            skip_lambda_return_type,
        );
        inferred.push(inferred_type);
    }

    (raw_args, inferred)
}

fn collect_call_value_arguments<'a>(
    node: tree_sitter::Node<'a>,
    out: &mut Vec<tree_sitter::Node<'a>>,
) {
    if node.kind() == "value_argument" || node.kind() == "lambda_literal" {
        out.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "call_expression" | "constructor_invocation") {
            continue;
        }
        collect_call_value_arguments(child, out);
    }
}

pub fn extract_call_site_arguments(
    target_node: Option<tree_sitter::Node>,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
    _pos: tower_lsp::lsp_types::Position,
    root: Option<tree_sitter::Node>,
) -> (Vec<String>, Vec<Option<KotlinType>>) {
    extract_call_site_arguments_opts(target_node, source, index, current_url, _pos, root, false)
}

pub fn extract_call_site_arguments_opts(
    target_node: Option<tree_sitter::Node>,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
    _pos: tower_lsp::lsp_types::Position,
    root: Option<tree_sitter::Node>,
    skip_lambda_return_type: bool,
) -> (Vec<String>, Vec<Option<KotlinType>>) {
    if let Some(node) = target_node {
        extract_call_args_and_inferred_types_opts(
            index,
            current_url,
            node,
            source,
            root,
            skip_lambda_return_type,
        )
    } else {
        (Vec::new(), Vec::new())
    }
}
