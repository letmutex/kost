use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
use super::lambda_call_solve::get_builtin_scope_function;
use super::scope_lambda::LambdaSolveCtx;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

impl<'a> LambdaSolveCtx<'a> {
    pub(crate) fn find_in_containers(
        &self,
        containers: &[String],
        method_name: &str,
    ) -> Option<KotlinType> {
        for container_name in containers {
            if let Some(decls) = self.index.container_to_decls.get(container_name.as_str()) {
                let mut candidates = Vec::new();
                for (decl_url, decl) in decls.iter().filter(|(_, d)| d.name == method_name) {
                    let is_ext = super::signature::SignatureInfo::from_decl(decl)
                        .is_some_and(|info| info.receiver.is_some());
                    let is_visible = if is_ext {
                        super::qualification::is_declaration_visible(
                            self.index,
                            self.current_url,
                            decl_url.as_ref(),
                            decl,
                        )
                    } else {
                        !decl.is_private || decl_url.as_ref() == self.current_url
                    };
                    if is_visible {
                        candidates.push((decl_url.as_ref(), decl.as_ref()));
                    }
                }
                if let Some(k_ty) = self.sort_and_solve_candidates(candidates) {
                    return Some(k_ty);
                }
            }
        }
        None
    }

    pub(crate) fn find_in_declarations_or_builtin(
        &self,
        name: &str,
        require_function: bool,
        supertypes: &[String],
    ) -> Option<KotlinType> {
        if let Some(decls) = self.index.name_to_decls.get(name) {
            let mut candidates = Vec::new();
            for (decl_url, decl) in decls {
                let is_ok = matches!(
                    decl.kind,
                    SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
                ) && (!require_function
                    || (decl.kind == SymbolKind::FUNCTION && decl.container_name.is_none()))
                    && self.receiver_type.is_none_or(|rec| {
                        super::lambda_call_extension::is_extension_container_plausible(
                            self.index, rec, supertypes, decl,
                        )
                    })
                    && super::qualification::is_declaration_visible(
                        self.index,
                        self.current_url,
                        decl_url.as_ref(),
                        decl,
                    );
                if is_ok {
                    candidates.push((decl_url.as_ref(), decl.as_ref()));
                }
            }
            if let Some(k_ty) = self.sort_and_solve_candidates(candidates) {
                return Some(k_ty);
            }
        }
        let builtin_sig = get_builtin_scope_function(name)?;
        self.solve_for_signature(self.current_url, builtin_sig)
    }
}

pub(crate) fn resolve_member_lambda_type(
    ctx: &LambdaSolveCtx,
    receiver_type: &KotlinType,
    method_name: &str,
) -> Option<KotlinType> {
    let clean_container = receiver_type.base_name();
    let supertypes = ctx.index.get_all_supertypes(clean_container);
    let all_containers =
        super::member_ranking::collect_candidate_containers(clean_container, ctx.index);

    ctx.find_in_containers(&all_containers, method_name)
        .or_else(|| ctx.find_in_declarations_or_builtin(method_name, false, &supertypes))
}

pub(crate) fn resolve_toplevel_lambda_type(
    ctx: &LambdaSolveCtx,
    callee_name: &str,
) -> Option<KotlinType> {
    if let Some(res) = ctx.find_in_declarations_or_builtin(callee_name, true, &[]) {
        return Some(res);
    }
    for implicit_rec in super::scope_stack::collect_implicit_receivers(
        ctx.index,
        ctx.current_url,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    ) {
        let member_ctx = ctx.with_receiver(&implicit_rec);
        if let Some(res) = resolve_member_lambda_type(&member_ctx, &implicit_rec, callee_name) {
            return Some(res);
        }
    }
    None
}

pub fn resolve_lambda_signature_for_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    callee: &str,
    explicit_type_args: &[KotlinType],
    value_args: &[&str],
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    let solve_ctx = LambdaSolveCtx::new(
        ctx.index,
        ctx.current_url,
        receiver_type,
        explicit_type_args,
        value_args,
        arg_idx,
        is_trailing,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    );
    if let Some(rec) = receiver_type {
        resolve_member_lambda_type(&solve_ctx, rec, callee)
    } else {
        resolve_toplevel_lambda_type(&solve_ctx, callee)
    }
}

fn dispatch_enclosing_lambda_callee<'a>(
    ctx: LambdaSolveCtx<'a>,
    callee_node: Node<'a>,
    callee_clean: &'a str,
) -> Option<(KotlinType, Option<SmolStr>)> {
    if callee_node.kind() == "navigation_expression" {
        let (rec_ty, method_name) = super::lambda_call_solve::resolve_navigation_callee_node(
            ctx.index,
            ctx.current_url,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth,
            callee_node,
        )?;
        let member_ctx = LambdaSolveCtx {
            receiver_type: Some(&rec_ty),
            ..ctx
        };
        resolve_member_lambda_type(&member_ctx, &rec_ty, method_name)
            .map(|kty| (kty, Some(SmolStr::new(method_name))))
    } else if let Some((rec_expr, method_name)) = callee_clean.rsplit_once('.') {
        let rec_ty = infer_receiver_type_depth(
            ctx.index,
            ctx.current_url,
            rec_expr,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        )?;
        let member_ctx = LambdaSolveCtx {
            receiver_type: Some(&rec_ty),
            ..ctx
        };
        resolve_member_lambda_type(&member_ctx, &rec_ty, method_name)
            .map(|kty| (kty, Some(SmolStr::new(method_name))))
    } else {
        resolve_toplevel_lambda_type(&ctx, callee_clean)
            .map(|kty| (kty, Some(SmolStr::new(callee_clean))))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_lambda_type_for_call_and_lambda(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_node: Node,
    l_node: Node,
    _pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<(KotlinType, Option<SmolStr>)> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let arg_idx = super::lambda_ast::find_lambda_argument_index_in_call(call_node, l_node);
    let explicit_type_args = super::lambda_call_solve::extract_explicit_type_args_from_call(
        call_node,
        source,
        index,
        current_url,
    );

    let view = crate::syntax::ast_call_view::AstCallView::from_node(call_node);
    let callee_node = view
        .as_ref()
        .and_then(|v| v.callee_node)
        .or_else(|| call_node.child(0))?;
    let callee_raw = callee_node.utf8_text(source.as_bytes()).ok()?.trim();
    let call_info = super::call_parser::parse_call_expression(callee_raw);
    let callee_clean = call_info.callee.trim();
    let ast_args = super::lambda_ast::extract_call_argument_expressions(call_node, source);
    let (effective_value_args, effective_arg_idx) = if !call_info.value_args.is_empty() {
        (&call_info.value_args, call_info.value_args.len())
    } else {
        (&ast_args, arg_idx)
    };
    let is_trailing = view.as_ref().is_some_and(|v| {
        v.trailing_lambda.is_some_and(|tl| {
            tl.start_byte() <= l_node.start_byte() && l_node.end_byte() <= tl.end_byte()
        }) || !v.value_arguments.iter().any(|arg| {
            arg.start_byte() <= l_node.start_byte() && l_node.end_byte() <= arg.end_byte()
        })
    });
    let call_pos = crate::position::point_to_position(call_node.start_position());
    let ctx = LambdaSolveCtx::new(
        index,
        current_url,
        None,
        &explicit_type_args,
        effective_value_args,
        effective_arg_idx,
        is_trailing,
        call_pos,
        source,
        root,
        depth,
    );

    dispatch_enclosing_lambda_callee(ctx, callee_node, callee_clean)
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
