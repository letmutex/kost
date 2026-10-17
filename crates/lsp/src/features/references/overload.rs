use tower_lsp::lsp_types::{SymbolKind, Url};
use tree_sitter::Node;

use super::candidate::{get_call_expression_for_callee, iter_overload_declarations};
use super::conformance::{is_functional_type_name, is_type_conforming};
use super::target::ReferenceTarget;
use crate::indexer::WorkspaceIndex;
use crate::syntax::ast_call_view::AstCallView;
use crate::syntax::signature::raw::RawSignatureParam;

struct CallContext<'a> {
    index: &'a WorkspaceIndex,
    target: &'a ReferenceTarget,
    cand_url: &'a Url,
    source: &'a str,
    file_root: Option<Node<'a>>,
    type_params: Vec<smol_str::SmolStr>,
}

pub fn verify_call_overload_signature(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    let Some(ref target_sig) = target.signature else {
        return true;
    };
    if crate::syntax::signature::params::find_parameter_list_bounds(target_sig).is_none() {
        return true;
    }
    let sig_params = crate::syntax::signature::params::parse_signature_params(target_sig);
    if crate::syntax::ast_util::is_callable_reference_target(cand_node) {
        return true;
    }
    let type_params =
        crate::syntax::signature::type_params::parse_signature_type_params(target_sig)
            .into_iter()
            .map(|p| p.name)
            .collect();
    let ctx = CallContext {
        index,
        target,
        cand_url,
        source,
        file_root,
        type_params,
    };
    if let Some(parent) = cand_node.parent()
        && parent.kind() == "infix_expression"
        && let Some(res) = verify_infix_overload(&ctx, parent, cand_node, &sig_params)
    {
        return res;
    }
    let Some(call_expr) = get_call_expression_for_callee(cand_node) else {
        if !matches!(
            target.kind,
            Some(SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR)
        ) {
            return true;
        }
        return sig_params.is_empty();
    };
    let Some(view) = AstCallView::from_node(call_expr) else {
        return true;
    };
    let arg_count = view.total_argument_count();
    if arg_count < sig_params.len() && has_better_exact_arity_overload(&ctx, &view, arg_count) {
        return false;
    }

    verify_call_arguments(&ctx, &view, &sig_params, arg_count)
}

fn collect_call_arguments<'a>(
    view: &AstCallView<'a>,
    source: &'a str,
) -> Vec<(Option<Node<'a>>, Option<&'a str>)> {
    view.value_arguments
        .iter()
        .map(|&arg| {
            (
                AstCallView::argument_expression(arg),
                AstCallView::argument_name(arg, source),
            )
        })
        .chain(
            view.trailing_lambda
                .map(|t| (AstCallView::unwrap_lambda(t).or(Some(t)), None)),
        )
        .collect()
}

fn verify_call_arguments(
    ctx: &CallContext<'_>,
    view: &AstCallView<'_>,
    sig_params: &[RawSignatureParam],
    arg_count: usize,
) -> bool {
    let mut used_params = vec![false; sig_params.len()];
    let mut positional_idx: usize = 0;
    for val_arg in &view.value_arguments {
        let named_opt = AstCallView::argument_name(*val_arg, ctx.source);
        let arg_expr_opt = AstCallView::argument_expression(*val_arg);
        let param_entry = if let Some(name) = named_opt {
            sig_params
                .iter()
                .enumerate()
                .find(|(idx, p)| !used_params[*idx] && p.name == name)
        } else {
            let next_unbound = (positional_idx..sig_params.len()).find(|idx| !used_params[*idx]);
            if let Some(idx) = next_unbound {
                positional_idx = idx + 1;
                Some((idx, &sig_params[idx]))
            } else {
                sig_params.iter().enumerate().find(|(_, p)| p.is_vararg)
            }
        };
        let Some((param_idx, param)) = param_entry else {
            return false;
        };
        used_params[param_idx] = true;
        if !verify_single_arg(
            ctx,
            arg_expr_opt,
            named_opt,
            param_idx,
            param,
            arg_count,
            false,
        ) {
            return false;
        }
    }

    if let Some(trailing) = view.trailing_lambda {
        let trailing_expr = AstCallView::unwrap_lambda(trailing).or(Some(trailing));
        let lambda_param = (0..sig_params.len())
            .rev()
            .find(|&idx| !used_params[idx] && is_functional_type_name(&sig_params[idx].type_name))
            .or_else(|| (0..sig_params.len()).rev().find(|&idx| !used_params[idx]));

        let Some(param_idx) = lambda_param else {
            return false;
        };
        used_params[param_idx] = true;
        let param = &sig_params[param_idx];
        if !verify_single_arg(ctx, trailing_expr, None, param_idx, param, arg_count, true) {
            return false;
        }
    }
    sig_params
        .iter()
        .enumerate()
        .all(|(idx, p)| used_params[idx] || p.has_default || p.is_vararg)
}

fn verify_single_arg(
    ctx: &CallContext<'_>,
    arg_expr_opt: Option<Node<'_>>,
    named_opt: Option<&str>,
    arg_idx: usize,
    param: &RawSignatureParam,
    arg_count: usize,
    is_trailing_lambda: bool,
) -> bool {
    let Some(arg_expr) = arg_expr_opt else {
        return true;
    };
    let Some(arg_type) = crate::inference::infer_node_type(
        ctx.index,
        ctx.cand_url,
        arg_expr,
        ctx.source,
        ctx.file_root,
    ) else {
        return true;
    };
    let tp_names: Vec<&str> = ctx.type_params.iter().map(|s| s.as_str()).collect();
    if !is_type_conforming(
        ctx.index,
        ctx.cand_url,
        &arg_type,
        &param.type_name,
        &tp_names,
    ) {
        if is_alternative_overload_better_match(
            ctx,
            named_opt,
            arg_idx,
            &arg_type,
            arg_count,
            is_trailing_lambda,
        ) {
            return false;
        }
        if crate::inference::types::KotlinType::parse(&param.type_name)
            .is_some_and(|p| p.is_primitive())
        {
            return false;
        }
    }
    true
}

fn verify_infix_overload(
    ctx: &CallContext<'_>,
    parent: Node<'_>,
    cand_node: &Node<'_>,
    sig_params: &[RawSignatureParam],
) -> Option<bool> {
    let non_comments: Vec<_> = parent
        .children(&mut parent.walk())
        .filter(|c| !matches!(c.kind(), "comment" | "line_comment" | "multiline_comment"))
        .collect();
    if non_comments.len() == 3 && non_comments[1].id() == cand_node.id() {
        if sig_params.len() != 1 {
            return Some(false);
        }
        let tp_names: Vec<&str> = ctx.type_params.iter().map(|s| s.as_str()).collect();
        if let Some(arg_type) = crate::inference::infer_node_type(
            ctx.index,
            ctx.cand_url,
            non_comments[2],
            ctx.source,
            ctx.file_root,
        ) && !is_type_conforming(
            ctx.index,
            ctx.cand_url,
            &arg_type,
            &sig_params[0].type_name,
            &tp_names,
        ) {
            return Some(false);
        }
        return Some(true);
    }
    None
}

fn is_alternative_overload_better_match(
    ctx: &CallContext<'_>,
    named_opt: Option<&str>,
    arg_idx: usize,
    arg_type: &crate::inference::types::KotlinType,
    arg_count: usize,
    is_trailing_lambda: bool,
) -> bool {
    iter_overload_declarations(ctx.index, ctx.target, ctx.cand_url).any(|decl| {
        decl.signature.as_ref().is_some_and(|other_sig| {
            let other_params = crate::syntax::signature::params::parse_signature_params(other_sig);
            let min_params = other_params
                .iter()
                .filter(|p| !p.has_default && !p.is_vararg)
                .count();
            let max_params = if other_params.iter().any(|p| p.is_vararg) {
                usize::MAX
            } else {
                other_params.len()
            };
            if arg_count < min_params || arg_count > max_params {
                return false;
            }
            let other_param = match named_opt {
                Some(name) => other_params.iter().find(|p| p.name == name),
                None if is_trailing_lambda => other_params
                    .iter()
                    .rfind(|p| is_functional_type_name(&p.type_name)),
                None => other_params.get(arg_idx),
            };
            let other_type_params =
                crate::syntax::signature::type_params::parse_signature_type_params(other_sig);
            let tp_names: Vec<&str> = other_type_params.iter().map(|p| p.name.as_str()).collect();
            other_param.is_some_and(|op| {
                is_type_conforming(ctx.index, ctx.cand_url, arg_type, &op.type_name, &tp_names)
            })
        })
    })
}

fn has_better_exact_arity_overload(
    ctx: &CallContext<'_>,
    view: &AstCallView<'_>,
    arg_count: usize,
) -> bool {
    let candidate_decls: Vec<_> = iter_overload_declarations(ctx.index, ctx.target, ctx.cand_url)
        .filter_map(|decl| {
            let sig = decl.signature.as_ref()?;
            let params = crate::syntax::signature::params::parse_signature_params(sig);
            if params.len() == arg_count && params.iter().all(|param| !param.has_default) {
                let type_params =
                    crate::syntax::signature::type_params::parse_signature_type_params(sig);
                let tp_names: Vec<smol_str::SmolStr> =
                    type_params.into_iter().map(|p| p.name).collect();
                Some((params, tp_names))
            } else {
                None
            }
        })
        .collect();

    if candidate_decls.is_empty() {
        return false;
    }

    let args = collect_call_arguments(view, ctx.source);
    let arg_types: Vec<_> = args
        .into_iter()
        .map(|(expr, named)| {
            let ty = expr.and_then(|n| {
                crate::inference::infer_node_type(
                    ctx.index,
                    ctx.cand_url,
                    n,
                    ctx.source,
                    ctx.file_root,
                )
            });
            (ty, named)
        })
        .collect();

    candidate_decls.into_iter().any(|(params, tp_names)| {
        let tp_strs: Vec<&str> = tp_names.iter().map(|s| s.as_str()).collect();
        arg_types
            .iter()
            .enumerate()
            .all(|(idx, (arg_type, named))| {
                let Some(arg_type) = arg_type else {
                    return false;
                };
                let param = match named {
                    Some(name) => params.iter().find(|p| p.name == *name),
                    None => params.get(idx),
                };
                param.is_some_and(|p| {
                    is_type_conforming(ctx.index, ctx.cand_url, arg_type, &p.type_name, &tp_strs)
                })
            })
    })
}
