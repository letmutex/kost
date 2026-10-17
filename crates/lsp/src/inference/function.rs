use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::call::{CallInfo, parse_call_expression, solve_call_decl_return_type_traced};
use super::constructor::find_constructor_return_type;
use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
use super::overload::score_overload_candidate_parsed;
use super::qualification::qualify_type;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};
use crate::parser::{
    extract_single_expression_body, find_enclosing_declaration_node, find_node_at_position,
    parse_source,
};
use crate::syntax::ast_call_view::parse_call_arg;
use crate::syntax::signature::extract_return_type_from_signature;

/// Infer the return type of a function given its name and calling context
pub fn infer_function_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    fn_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    find_function_return_type_depth(index, current_url, fn_name, pos, source, root, 0)
}

/// Find function return type from uppercase constructor heuristic, declaration index, or definition lookup
pub fn find_function_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    fn_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    find_function_return_type_depth(index, current_url, fn_name, pos, source, root, 0)
}

/// Find function return type with recursion depth guard
pub fn find_function_return_type_depth(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_expr: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let call_info = parse_call_expression(call_expr);
    let fn_name = call_info.callee;

    let mut tracer = Tracer::Noop;
    let mut ctx = InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);

    if let Some(ctor_type) = find_constructor_return_type(ctx.reborrow(), fn_name, &call_info) {
        return Some(ctor_type);
    }

    if let Some(inferred) = find_top_level_function_return_type(ctx.reborrow(), fn_name, &call_info)
    {
        return Some(inferred);
    }

    find_definition_function_return_type(ctx, fn_name, &call_info)
}

fn find_top_level_function_return_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    let decls = ctx.index.name_to_decls.get(fn_name)?;
    let inferred_arg_types: Vec<Option<KotlinType>> = call_info
        .value_args
        .iter()
        .map(|arg_expr| {
            let parsed = parse_call_arg(arg_expr);
            infer_receiver_type_depth(
                ctx.index,
                ctx.current_url,
                &parsed.expr,
                ctx.pos,
                ctx.source,
                ctx.root,
                ctx.depth + 1,
            )
        })
        .collect();

    let parsed_args: Vec<_> = call_info
        .value_args
        .iter()
        .map(|a| parse_call_arg(a))
        .collect();

    let current_url = ctx.current_url;
    let mut scored_decls: Vec<_> = decls
        .iter()
        .filter(|(url, decl)| !decl.is_private || url.as_ref() == current_url)
        .filter_map(|cand| {
            let is_src = crate::indexer::is_source_url(&cand.0);
            let score = score_overload_candidate_parsed(
                cand.1.as_ref(),
                is_src,
                &parsed_args,
                &inferred_arg_types,
                ctx.index,
            );
            score.map(|s| (cand, s))
        })
        .collect();

    scored_decls.sort_by_key(|a| std::cmp::Reverse(a.1));

    let candidate_iter: Box<dyn Iterator<Item = _>> = if !scored_decls.is_empty() {
        Box::new(scored_decls.into_iter().map(|(cand, _)| cand))
    } else {
        Box::new(
            decls
                .iter()
                .filter(|(url, decl)| !decl.is_private || url.as_ref() == current_url),
        )
    };

    for (url, decl) in candidate_iter {
        if let Some(inferred) = resolve_candidate_decl_return_type(
            ctx.sub(),
            url.as_ref(),
            decl.as_ref(),
            call_info,
            Some(&inferred_arg_types),
        ) {
            return Some(inferred);
        }
    }
    None
}

fn find_definition_function_return_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    let cf = ctx.index.files.get(ctx.current_url)?;
    if let Some(decl) = cf.declarations.iter().find(|d| d.name == fn_name) {
        let current_url = ctx.current_url.clone();
        return resolve_candidate_decl_return_type(ctx, &current_url, decl, call_info, None);
    }
    for imp in &cf.imports {
        if imp.alias.as_deref() == Some(fn_name) {
            let real_name = imp.path.rsplit('.').next().unwrap_or(fn_name);
            if let Some(decls) = ctx.index.name_to_decls.get(real_name) {
                for (url, decl) in decls {
                    if let Some(inferred) = resolve_candidate_decl_return_type(
                        ctx.sub(),
                        url.as_ref(),
                        decl.as_ref(),
                        call_info,
                        None,
                    ) {
                        return Some(inferred);
                    }
                }
            }
        }
    }
    None
}

fn resolve_candidate_decl_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    decl_url: &Url,
    decl: &DeclarationItem,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> Option<KotlinType> {
    if let Some(sig) = decl.signature.as_deref() {
        let ret_opt = super::signature::SignatureInfo::from_decl(decl)
            .and_then(|info| info.return_type)
            .or_else(|| {
                extract_return_type_from_signature(sig).and_then(|s| KotlinType::parse(&s))
            });
        if let Some(ret) = ret_opt {
            let index = ctx.index;
            let solved = solve_call_decl_return_type_traced(
                ctx,
                decl,
                Some(&ret),
                None,
                call_info,
                inferred_arg_types,
            );
            return Some(qualify_type(index, decl_url, &solved));
        }
    }

    if let Some(decl_source) = ctx.index.get_file_source(decl_url)
        && let Some(inferred) = infer_single_expression_function_return_type(
            ctx.index,
            decl_url,
            decl,
            &decl_source,
            ctx.depth + 1,
        )
    {
        return Some(inferred);
    }

    if decl.kind == SymbolKind::FUNCTION {
        return Some(KotlinType::UNIT);
    }
    None
}

/// Infer return type for single-expression functions (`fun foo() = expr`)
pub fn infer_single_expression_function_return_type(
    index: &WorkspaceIndex,
    decl_url: &Url,
    decl: &DeclarationItem,
    decl_source: &str,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let tree = parse_source(decl_source)?;
    let root_node = tree.root_node();
    let id_node = find_node_at_position(root_node, decl.selection_range.start)?;
    let fn_node = find_enclosing_declaration_node(id_node);
    let rhs_expr = extract_single_expression_body(&fn_node, decl_source)?;
    infer_receiver_type_depth(
        index,
        decl_url,
        rhs_expr,
        decl.selection_range.start,
        decl_source,
        Some(root_node),
        depth + 1,
    )
}
