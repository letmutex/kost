use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
use super::lambda_return::find_matching_lambda_in_ast;
use super::qualification::qualify_type;
use super::signature::{SignatureInfo, SignatureParam};
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

pub fn get_builtin_scope_function(callee: &str) -> Option<&'static str> {
    match callee {
        "apply" => Some("fun <T> T.apply(block: T.() -> Unit): T"),
        "run" => Some("fun <T, R> T.run(block: T.() -> R): R"),
        "with" => Some("fun <T, R> with(receiver: T, block: T.() -> R): R"),
        "let" => Some("fun <T, R> T.let(block: (T) -> R): R"),
        "also" => Some("fun <T> T.also(block: (T) -> Unit): T"),
        "takeIf" => Some("fun <T> T.takeIf(predicate: (T) -> Boolean): T?"),
        "takeUnless" => Some("fun <T> T.takeUnless(predicate: (T) -> Boolean): T?"),
        "repeat" => Some("fun repeat(times: Int, action: (Int) -> Unit)"),
        "forEach" => Some("fun <T> Iterable<T>.forEach(action: (T) -> Unit): Unit"),
        _ => None,
    }
}

pub fn is_explicit_type_arg_count_valid(type_param_count: usize, explicit_count: usize) -> bool {
    explicit_count == 0 || explicit_count == type_param_count
}

pub fn extract_explicit_type_args_from_call(
    call_node: Node,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
) -> Vec<KotlinType> {
    let mut raw_args = Vec::new();
    if let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(call_node) {
        for ta in view.type_arguments {
            if let Ok(text) = ta.utf8_text(source.as_bytes()) {
                raw_args.push(text.trim().to_string());
            }
        }
        if raw_args.is_empty()
            && let Some(callee_node) = view.callee_node
            && let Ok(callee_text) = callee_node.utf8_text(source.as_bytes())
        {
            let call_info = super::call_parser::parse_call_expression(callee_text.trim());
            for ta in call_info.explicit_type_args {
                raw_args.push(ta.to_string());
            }
        }
    }
    raw_args
        .into_iter()
        .filter_map(|s| {
            let ty = super::types::parse_type_signature(&s)?;
            Some(qualify_type(index, current_url, &ty))
        })
        .collect()
}

pub fn resolve_navigation_callee_node<'a>(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &'a str,
    root: Option<Node<'a>>,
    depth: usize,
    callee_node: Node<'a>,
) -> Option<(KotlinType, &'a str)> {
    let lhs_node = callee_node.named_child(0)?;
    let suffix_node = callee_node
        .children(&mut callee_node.walk())
        .find(|c| c.kind() == "navigation_suffix")?;
    let method_name = suffix_node
        .children(&mut suffix_node.walk())
        .find(|c| c.kind() == "simple_identifier")
        .and_then(|c| c.utf8_text(source.as_bytes()).ok())?
        .trim();
    let mut tracer = super::trace::Tracer::Noop;
    let mut inf_ctx = super::InferenceContext::new(
        index,
        current_url,
        pos,
        source,
        root,
        depth + 1,
        &mut tracer,
    );
    let mut eval_ctx = super::ast_eval::AstEvalContext::new(inf_ctx.reborrow(), source);
    let rec_ty = super::ast_eval::infer_expression_node(&mut eval_ctx, lhs_node).or_else(|| {
        let lhs_text = lhs_node.utf8_text(source.as_bytes()).ok()?.trim();
        infer_receiver_type_depth(index, current_url, lhs_text, pos, source, root, depth + 1)
    })?;
    Some((rec_ty, method_name))
}

fn find_matching_builder_call<'a, 'd>(
    node: Node<'a>,
    source: &'a str,
    candidate_containers: &[String],
    index: &'d WorkspaceIndex,
) -> Option<(&'d crate::indexer::DeclarationItem, Node<'a>)> {
    if node.kind() == "call_expression"
        && let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(node)
        && view.value_arguments.len() == 1
        && let Some(callee) = view.callee_text(source)
        && !callee.contains('.')
        && let Some(arg_expr) =
            crate::syntax::ast_call_view::AstCallView::argument_expression(view.value_arguments[0])
    {
        for container in candidate_containers {
            if let Some(decls) = index.container_to_decls.get(container.as_str())
                && let Some((_, decl)) = decls.iter().find(|(_, d)| d.name == callee)
            {
                return Some((decl.as_ref(), arg_expr));
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "lambda_literal"
            && let Some(found) =
                find_matching_builder_call(child, source, candidate_containers, index)
        {
            return Some(found);
        }
    }
    let mut cursor2 = node.walk();
    for child in node.children(&mut cursor2) {
        if child.kind() == "lambda_literal"
            && let Some(found) =
                find_matching_builder_call(child, source, candidate_containers, index)
        {
            return Some(found);
        }
    }
    None
}

fn find_builder_clause_lambda<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if node.kind() == "call_expression"
        && let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(node)
        && let Some(trailing) = view.trailing_lambda
    {
        return crate::syntax::ast_call_view::AstCallView::unwrap_lambda(trailing)
            .or(Some(trailing));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "lambda_literal"
            && let Some(found) = find_builder_clause_lambda(child)
        {
            return Some(found);
        }
    }
    None
}

fn solve_builder_method_receiver(
    index: &WorkspaceIndex,
    rec_ty: &KotlinType,
    container_base: &str,
    method_decl: &crate::indexer::DeclarationItem,
    arg_ty: KotlinType,
) -> Option<KotlinType> {
    let rec_args = rec_ty.type_arguments();
    if let Some(sig_info) = SignatureInfo::from_decl(method_decl)
        && let Some(param) = sig_info.params.first()
    {
        let mut type_params: Vec<_> = rec_args
            .iter()
            .filter_map(|a| match &a.type_ref {
                KotlinType::TypeParam(tp) => Some(tp.clone()),
                KotlinType::Class { name, args } if args.is_empty() => Some(name.clone()),
                _ => None,
            })
            .collect();
        for tp in &sig_info.type_params {
            if !type_params.contains(&tp.name) {
                type_params.push(tp.name.clone());
            }
        }
        let mut system = ConstraintSystem::new(index, type_params);
        let expected_param = system
            .parse_with_params(&param.type_name)
            .or_else(|| param.kotlin_type.clone())?;
        if !system.add_subtype_constraint(&arg_ty, &expected_param) {
            return None;
        }
        Some(system.solve_and_substitute(rec_ty))
    } else {
        Some(KotlinType::class(
            container_base,
            vec![super::types::TypeArg::invariant(arg_ty)],
        ))
    }
}

pub fn infer_builder_lambda_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    lambda_node: Node,
    call_expr: &str,
    fn_rec: Option<&KotlinType>,
) -> Option<KotlinType> {
    let rec_ty = fn_rec?;
    if rec_ty.type_arguments().is_empty() {
        return None;
    }
    let parsed_tree = ctx
        .root
        .is_none()
        .then(|| parse_source(ctx.source))
        .flatten();
    let outer_root = ctx
        .root
        .or_else(|| parsed_tree.as_ref().map(|t| t.root_node()))?;
    let lambda_text = lambda_node.utf8_text(call_expr.as_bytes()).ok()?;
    let matched_lambda = find_matching_lambda_in_ast(outer_root, ctx.source, lambda_text, ctx.pos)?;

    let container_base = rec_ty.base_name();
    let candidate_containers =
        super::member_ranking::collect_candidate_containers(container_base, ctx.index);
    if let Some((method_decl, arg_node)) =
        find_matching_builder_call(matched_lambda, ctx.source, &candidate_containers, ctx.index)
    {
        let arg_pos = crate::position::point_to_position(arg_node.start_position());
        let mut tracer = super::trace::Tracer::Noop;
        let inf_ctx = InferenceContext::new(
            ctx.index,
            ctx.current_url,
            arg_pos,
            ctx.source,
            Some(outer_root),
            ctx.depth + 1,
            &mut tracer,
        );
        let mut eval_ctx = super::ast_eval::AstEvalContext::from_ctx(inf_ctx);
        let arg_ty = super::ast_eval::infer_expression_node(&mut eval_ctx, arg_node)?;
        let instantiated_rec =
            solve_builder_method_receiver(ctx.index, rec_ty, container_base, method_decl, arg_ty)?;
        return Some(KotlinType::function(
            Some(instantiated_rec),
            Vec::new(),
            KotlinType::UNIT,
            false,
        ));
    }

    let clause_lambda = find_builder_clause_lambda(matched_lambda)?;
    let clause_ret = super::lambda_return::infer_lambda_return_type(
        ctx.index,
        ctx.current_url,
        clause_lambda,
        ctx.source,
        Some(outer_root),
        ctx.depth + 1,
    )?;
    if clause_ret.is_unit() {
        return None;
    }
    let instantiated_rec = KotlinType::class(
        container_base,
        vec![super::types::TypeArg::invariant(clause_ret)],
    );
    Some(KotlinType::function(
        Some(instantiated_rec),
        Vec::new(),
        KotlinType::UNIT,
        false,
    ))
}

pub fn map_trailing_lambda_param_index(
    index: &WorkspaceIndex,
    params: &[SignatureParam],
    preceding_args: usize,
) -> usize {
    if params.is_empty() {
        return preceding_args;
    }
    let is_functional = |param: &SignatureParam| {
        param
            .kotlin_type
            .as_ref()
            .is_some_and(|kt| super::types::is_functional_or_sam(kt, index))
    };
    params
        .iter()
        .rposition(&is_functional)
        .unwrap_or(preceding_args)
}

pub fn resolve_fallback_type_param(
    solved: KotlinType,
    receiver_type: Option<&KotlinType>,
) -> KotlinType {
    if matches!(solved, KotlinType::TypeParam(_))
        && let Some(rec) = receiver_type
        && let KotlinType::Class { args, .. } = rec.unwrap_nullable_ref()
        && args.len() == 1
    {
        return args[0].type_ref.clone();
    }
    solved
}

pub fn find_matching_functional_param(
    info: &SignatureInfo,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let extract = |p: &SignatureParam| {
        let kt = p.kotlin_type.as_ref()?;
        if let KotlinType::Function { params, .. } = kt.unwrap_nullable_ref() {
            params.get(param_idx).cloned()
        } else {
            None
        }
    };
    if let Some(param) = info.params.get(arg_idx)
        && param
            .kotlin_type
            .as_ref()
            .is_some_and(|kt| kt.is_function())
    {
        return extract(param);
    }
    info.params
        .iter()
        .rev()
        .find(|p| p.kotlin_type.as_ref().is_some_and(|kt| kt.is_function()))
        .and_then(extract)
}
