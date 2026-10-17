use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

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
        _ => None,
    }
}

fn find_last_functional_param_in_sig(sig: &str, index: &WorkspaceIndex) -> Option<KotlinType> {
    let params = crate::inference::signature::parse_signature_params(sig);
    params.into_iter().rev().find_map(|p| {
        p.kotlin_type
            .filter(|kt| kt.is_function() || super::types::is_functional_or_sam(kt, index))
    })
}

struct LambdaSolveCtx<'a> {
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    receiver_type: Option<&'a KotlinType>,
    value_args: &'a [&'a str],
    arg_idx: usize,
    is_trailing: bool,
    pos: Position,
    source: &'a str,
    root: Option<Node<'a>>,
    depth: usize,
}

impl<'a> LambdaSolveCtx<'a> {
    fn solve_for_decl(
        &self,
        decl_url: &Url,
        decl: &crate::indexer::DeclarationItem,
    ) -> Option<KotlinType> {
        let sig = decl.signature.as_deref()?;
        let sig_info = super::signature::SignatureInfo::from_decl(decl);
        self.solve_for_sig_and_info(decl_url, sig, sig_info.as_ref())
    }

    fn solve_for_signature(&self, decl_url: &Url, sig: &str) -> Option<KotlinType> {
        let sig_info = super::signature::SignatureInfo::parse(sig);
        self.solve_for_sig_and_info(decl_url, sig, sig_info.as_ref())
    }

    fn solve_for_sig_and_info(
        &self,
        decl_url: &Url,
        sig: &str,
        sig_info: Option<&super::signature::SignatureInfo>,
    ) -> Option<KotlinType> {
        let mut system = if let Some(info) = sig_info {
            ConstraintSystem::from_signature_info(self.index, info)
        } else {
            ConstraintSystem::from_signature(self.index, sig)
        };

        let target_ty = if self.is_trailing {
            sig_info
                .and_then(|info| {
                    let idx = super::lambda_call_solve::map_trailing_lambda_param_index(
                        self.index,
                        &info.params,
                        self.arg_idx,
                    );
                    info.params.get(idx).and_then(|p| {
                        p.kotlin_type
                            .clone()
                            .or_else(|| system.parse_with_params(&p.type_name))
                    })
                })
                .or_else(|| find_last_functional_param_in_sig(sig, self.index))?
        } else {
            crate::signature::extract_param_type_by_index(sig, self.arg_idx)
                .and_then(|p_str| system.parse_with_params(&p_str))
                .or_else(|| find_last_functional_param_in_sig(sig, self.index))?
        };

        if !target_ty.is_function() && !super::types::is_functional_or_sam(&target_ty, self.index) {
            return None;
        }

        if let Some(rec) = self.receiver_type {
            system.add_container_type_params_typed(rec);
            if let Some(decl_rec) = sig_info.and_then(|info| info.receiver.as_ref()) {
                system.add_receiver_constraint_typed(rec, decl_rec);
            } else {
                system.add_receiver_constraint_from_sig(sig, rec);
            }
        }

        for (i, arg_expr) in self.value_args.iter().enumerate() {
            if let Some(arg_ty) = infer_receiver_type_depth(
                self.index,
                self.current_url,
                arg_expr,
                self.pos,
                self.source,
                self.root,
                self.depth + 1,
            ) && let Some(p_str) = crate::signature::extract_param_type_by_index(sig, i)
                && let Some(expected) = system.parse_with_params(&p_str)
            {
                system.add_subtype_constraint(&arg_ty, &expected);
            }
        }

        let solved = system.solve_and_substitute(&target_ty);
        Some(super::qualification::qualify_type(
            self.index, decl_url, &solved,
        ))
    }

    fn find_in_containers(&self, containers: &[String], method_name: &str) -> Option<KotlinType> {
        for c in containers {
            if let Some(decls) = self.index.container_to_decls.get(c.as_str()) {
                for (decl_url, decl) in decls.iter().filter(|(_, d)| d.name == method_name) {
                    if let Some(k_ty) = self.solve_for_decl(decl_url.as_ref(), decl) {
                        return Some(k_ty);
                    }
                }
            }
        }
        None
    }

    fn find_in_declarations_or_builtin(
        &self,
        name: &str,
        require_function: bool,
    ) -> Option<KotlinType> {
        if let Some(decls) = self.index.name_to_decls.get(name) {
            for (decl_url, decl) in decls {
                let is_ok = !require_function
                    || (decl.kind == SymbolKind::FUNCTION
                        && (!decl.is_private || decl_url.as_ref() == self.current_url));
                if is_ok && let Some(k_ty) = self.solve_for_decl(decl_url.as_ref(), decl) {
                    return Some(k_ty);
                }
            }
        }
        if let Some(builtin_sig) = get_builtin_scope_function(name)
            && let Some(k_ty) = self.solve_for_signature(self.current_url, builtin_sig)
        {
            return Some(k_ty);
        }
        None
    }
}

fn resolve_member_lambda_type(
    ctx: &LambdaSolveCtx,
    receiver_type: &KotlinType,
    method_name: &str,
) -> Option<KotlinType> {
    let clean_container = receiver_type.base_name();
    let mut all_containers = vec![clean_container.to_string()];
    all_containers.extend(ctx.index.get_all_supertypes(clean_container));

    ctx.find_in_containers(&all_containers, method_name)
        .or_else(|| ctx.find_in_declarations_or_builtin(method_name, false))
}

fn resolve_toplevel_lambda_type(ctx: &LambdaSolveCtx, callee_name: &str) -> Option<KotlinType> {
    ctx.find_in_declarations_or_builtin(callee_name, true)
}

pub fn resolve_lambda_signature_for_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    callee: &str,
    value_args: &[&str],
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    let solve_ctx = LambdaSolveCtx {
        index: ctx.index,
        current_url: ctx.current_url,
        receiver_type,
        value_args,
        arg_idx,
        is_trailing,
        pos: ctx.pos,
        source: ctx.source,
        root: ctx.root,
        depth: ctx.depth,
    };
    if let Some(rec) = receiver_type {
        resolve_member_lambda_type(&solve_ctx, rec, callee)
    } else {
        resolve_toplevel_lambda_type(&solve_ctx, callee)
    }
}

fn resolve_navigation_callee<'a>(
    ctx: &LambdaSolveCtx<'a>,
    callee_node: Node<'a>,
) -> Option<(KotlinType, &'a str)> {
    let lhs_node = callee_node.named_child(0)?;
    let suffix_node = callee_node
        .children(&mut callee_node.walk())
        .find(|c| c.kind() == "navigation_suffix")?;
    let method_name = suffix_node
        .children(&mut suffix_node.walk())
        .find(|c| c.kind() == "simple_identifier")
        .and_then(|c| c.utf8_text(ctx.source.as_bytes()).ok())?
        .trim();
    let mut tracer = super::trace::Tracer::Noop;
    let inf_ctx = super::InferenceContext::new(
        ctx.index,
        ctx.current_url,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth + 1,
        &mut tracer,
    );
    let mut eval_ctx = super::ast_eval::AstEvalContext::new(inf_ctx, ctx.source);
    let rec_ty = super::ast_eval::infer_expression_node(&mut eval_ctx, lhs_node).or_else(|| {
        let lhs_text = lhs_node.utf8_text(ctx.source.as_bytes()).ok()?.trim();
        infer_receiver_type_depth(
            ctx.index,
            ctx.current_url,
            lhs_text,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        )
    })?;
    Some((rec_ty, method_name))
}

pub fn resolve_lambda_type_for_call_and_lambda(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_node: Node,
    l_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<(KotlinType, Option<SmolStr>)> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let arg_idx = super::lambda_ast::find_lambda_argument_index_in_call(call_node, l_node);

    let view = crate::syntax::ast_call_view::AstCallView::from_node(call_node);
    let callee_node = view
        .as_ref()
        .and_then(|v| v.callee_node)
        .or_else(|| call_node.child(0))?;
    let callee_raw = callee_node.utf8_text(source.as_bytes()).ok()?.trim();
    let call_info = super::call::parse_call_expression(callee_raw);
    let callee_clean = call_info.callee.trim();
    let ast_args = super::lambda_ast::extract_call_argument_expressions(call_node, source);
    let (effective_value_args, effective_arg_idx) = if !call_info.value_args.is_empty() {
        (&call_info.value_args, call_info.value_args.len())
    } else {
        (&ast_args, arg_idx)
    };
    let is_trailing = view
        .as_ref()
        .and_then(|v| v.trailing_lambda)
        .is_some_and(|tl| {
            tl.start_byte() <= l_node.start_byte() && l_node.end_byte() <= tl.end_byte()
        })
        || view.as_ref().is_some_and(|v| {
            !v.value_arguments.iter().any(|arg| {
                arg.start_byte() <= l_node.start_byte() && l_node.end_byte() <= arg.end_byte()
            })
        });

    let ctx = LambdaSolveCtx {
        index,
        current_url,
        receiver_type: None,
        value_args: effective_value_args,
        arg_idx: effective_arg_idx,
        is_trailing,
        pos,
        source,
        root,
        depth,
    };

    let (rec_type, method_name) = if callee_node.kind() == "navigation_expression" {
        let (rec_ty, method_name) = resolve_navigation_callee(&ctx, callee_node)?;
        (Some(rec_ty), method_name)
    } else {
        (None, callee_clean)
    };

    if let Some(rec_ty) = rec_type {
        let label = SmolStr::new(method_name);
        let mut member_ctx = ctx;
        member_ctx.receiver_type = Some(&rec_ty);
        resolve_member_lambda_type(&member_ctx, &rec_ty, method_name).map(|kty| (kty, Some(label)))
    } else {
        let label = SmolStr::new(method_name);
        resolve_toplevel_lambda_type(&ctx, method_name).map(|kty| (kty, Some(label)))
    }
}

pub fn resolve_enclosing_lambda_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    lambda_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<(KotlinType, Option<SmolStr>)> {
    let (call_node, l_node) = super::lambda_ast::find_enclosing_call_and_lambda(lambda_node)?;
    resolve_lambda_type_for_call_and_lambda(
        index,
        current_url,
        call_node,
        l_node,
        pos,
        source,
        root,
        depth,
    )
}
