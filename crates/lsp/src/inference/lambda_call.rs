use tower_lsp::lsp_types::{SymbolKind, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::lambda_call_extension::{
    collect_ranked_extension_candidates, is_callable_function_decl,
};
use super::lambda_call_solve::{solve_decl_higher_order_call, solve_sig_info_higher_order_call};
use super::lambda_return::{
    find_lambda_node_and_arg_idx, find_matching_lambda_in_ast, infer_lambda_return_type_expected,
};
use super::overload::TypedCandidate;
use super::qualification::is_declaration_visible;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

/// Check if expression is a call containing a lambda (e.g. `remember { ... }` or `map { ... }`)
pub fn is_call_with_lambda(expr: &str) -> bool {
    let clean = expr.trim();
    if !clean.ends_with('}') {
        return false;
    }
    clean.contains('{')
}

fn find_call_node(node: Node) -> Option<Node> {
    if node.kind() == "call_expression" {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(call_expr) = find_call_node(child) {
            return Some(call_expr);
        }
    }
    None
}

fn find_call_with_lambda<'a>(node: Node<'a>) -> Option<(Node<'a>, Node<'a>, usize)> {
    if node.kind() == "call_expression"
        && let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(node)
    {
        if let Some(trailing) = view.trailing_lambda {
            let lambda = crate::syntax::ast_call_view::AstCallView::unwrap_lambda(trailing)
                .unwrap_or(trailing);
            let arg_idx = view.value_arguments.len();
            return Some((node, lambda, arg_idx));
        }
        for (idx, arg) in view.value_arguments.iter().enumerate() {
            if let Some(arg_expr) =
                crate::syntax::ast_call_view::AstCallView::argument_expression(*arg)
                && let Some(lambda) =
                    crate::syntax::ast_call_view::AstCallView::unwrap_lambda(arg_expr)
            {
                return Some((node, lambda, idx));
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_call_with_lambda(child) {
            return Some(found);
        }
    }
    None
}

fn extract_callee_from_ast<'a>(call_node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let view = crate::syntax::ast_call_view::AstCallView::from_node(call_node)?;
    let callee_node = view.callee_node?;
    if callee_node.kind() == "navigation_expression" {
        let suffix = callee_node
            .children(&mut callee_node.walk())
            .find(|child| child.kind() == "navigation_suffix")?;
        let ident = suffix
            .children(&mut suffix.walk())
            .find(|child| child.kind() == "simple_identifier")?;
        ident.utf8_text(source.as_bytes()).ok().map(str::trim)
    } else if callee_node.kind() == "simple_identifier" {
        callee_node.utf8_text(source.as_bytes()).ok().map(str::trim)
    } else {
        view.callee_text(source)
    }
}

/// Resolve return type of a higher-order function call with a lambda
pub fn resolve_higher_order_call(
    mut ctx: InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    call_expr: &str,
) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let call_tree = parse_source(call_expr)?;
    let call_root = call_tree.root_node();
    let (call_node, lambda_node, arg_idx) = find_call_with_lambda(call_root).or_else(|| {
        let call_node = find_call_node(call_root)?;
        let (lambda_node, arg_idx) = find_lambda_node_and_arg_idx(call_root)?;
        Some((call_node, lambda_node, arg_idx))
    })?;
    let callee = extract_callee_from_ast(call_node, call_expr)?;
    let value_args = super::lambda_ast::extract_call_argument_expressions(call_node, call_expr);
    let is_trailing = crate::syntax::ast_call_view::AstCallView::from_node(call_node)
        .and_then(|view| view.trailing_lambda)
        .is_some_and(|tl| {
            tl.start_byte() <= lambda_node.start_byte() && lambda_node.end_byte() <= tl.end_byte()
        })
        || arg_idx >= value_args.len();

    let expected_fn = super::scope_lambda::resolve_lambda_signature_for_receiver(
        ctx.reborrow(),
        receiver_type,
        callee,
        &value_args,
        arg_idx,
        is_trailing,
    );

    let lambda_ret = if let Some(KotlinType::Function { return_type, .. }) = expected_fn.as_ref()
        && return_type.is_unit()
    {
        Some(KotlinType::UNIT)
    } else {
        infer_call_lambda_return_type(
            ctx.reborrow(),
            lambda_node,
            call_expr,
            call_root,
            expected_fn.as_ref(),
        )
    };

    if let Some(rec) = receiver_type {
        resolve_member_higher_order(
            ctx.index,
            ctx.current_url,
            rec,
            callee,
            lambda_ret.as_ref(),
            arg_idx,
            is_trailing,
        )
    } else {
        resolve_toplevel_higher_order(
            ctx.index,
            ctx.current_url,
            callee,
            lambda_ret.as_ref(),
            arg_idx,
            is_trailing,
        )
    }
}

fn infer_call_lambda_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    lambda_node: Node,
    call_expr: &str,
    call_root: Node,
    expected_fn: Option<&KotlinType>,
) -> Option<KotlinType> {
    let parsed_tree = ctx
        .root
        .is_none()
        .then(|| parse_source(ctx.source))
        .flatten();
    let outer_tree = ctx
        .root
        .or_else(|| parsed_tree.as_ref().map(|t| t.root_node()));
    let lambda_text = lambda_node.utf8_text(call_expr.as_bytes()).ok();

    let res1 = outer_tree.and_then(|ot| {
        let matched_node = find_matching_lambda_in_ast(ot, ctx.source, lambda_text?, ctx.pos)?;
        infer_lambda_return_type_expected(
            ctx.index,
            ctx.current_url,
            matched_node,
            ctx.source,
            Some(ot),
            ctx.depth + 1,
            expected_fn,
        )
    });
    if res1.is_some() {
        return res1;
    }
    infer_lambda_return_type_expected(
        ctx.index,
        ctx.current_url,
        lambda_node,
        call_expr,
        Some(call_root),
        ctx.depth + 1,
        expected_fn,
    )
}

fn resolve_member_higher_order(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver_type: &KotlinType,
    callee: &str,
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    let clean_container = receiver_type.base_name();
    let supertypes = index.get_all_supertypes(clean_container);
    let container_iter = std::iter::once(clean_container).chain(
        supertypes
            .iter()
            .map(|s| s.split('<').next().unwrap_or(s).trim()),
    );

    for container_name in container_iter {
        if let Some(decls) = index.container_to_decls.get(container_name) {
            for (url, decl) in decls.iter().filter(|(_, d)| d.name == callee) {
                if !is_callable_function_decl(decl)
                    || !is_declaration_visible(index, current_url, url.as_ref(), decl)
                {
                    continue;
                }
                let candidate = TypedCandidate::new(decl);
                if candidate.sig_info.receiver.is_some() {
                    continue;
                }
                if let Some(res) = solve_decl_higher_order_call(
                    index,
                    decl,
                    url.as_ref(),
                    &candidate.sig_info,
                    Some(receiver_type),
                    lambda_ret,
                    arg_idx,
                    is_trailing,
                ) {
                    return Some(res);
                }
            }
        }
    }

    for candidate in collect_ranked_extension_candidates(index, current_url, receiver_type, callee)
    {
        if let Some(res) = solve_decl_higher_order_call(
            index,
            candidate.decl,
            candidate.url,
            &candidate.sig_info,
            Some(receiver_type),
            lambda_ret,
            arg_idx,
            is_trailing,
        ) {
            return Some(res);
        }
    }

    if let Some(builtin_sig) = super::scope_lambda::get_builtin_scope_function(callee)
        && let Some(sig_info) = super::signature::SignatureInfo::parse(builtin_sig)
        && let Some(res) = solve_sig_info_higher_order_call(
            index,
            current_url,
            &sig_info,
            Some(receiver_type),
            lambda_ret,
            arg_idx,
            false,
            is_trailing,
        )
    {
        return Some(res);
    }

    None
}

fn resolve_toplevel_higher_order(
    index: &WorkspaceIndex,
    current_url: &Url,
    callee: &str,
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    if let Some(decls) = index.name_to_decls.get(callee) {
        for (url, decl) in decls {
            if decl.container_name.is_none()
                && decl.kind == SymbolKind::FUNCTION
                && is_declaration_visible(index, current_url, url.as_ref(), decl)
            {
                let candidate = TypedCandidate::new(decl);
                if candidate.sig_info.receiver.is_none()
                    && let Some(res) = solve_decl_higher_order_call(
                        index,
                        decl,
                        url.as_ref(),
                        &candidate.sig_info,
                        None,
                        lambda_ret,
                        arg_idx,
                        is_trailing,
                    )
                {
                    return Some(res);
                }
            }
        }
    }

    if let Some(builtin_sig) = super::scope_lambda::get_builtin_scope_function(callee)
        && let Some(sig_info) = super::signature::SignatureInfo::parse(builtin_sig)
        && sig_info.receiver.is_none()
        && let Some(res) = solve_sig_info_higher_order_call(
            index,
            current_url,
            &sig_info,
            None,
            lambda_ret,
            arg_idx,
            false,
            is_trailing,
        )
    {
        return Some(res);
    }

    None
}
