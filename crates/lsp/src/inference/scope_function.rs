use std::collections::HashSet;
use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::resolve_full_type_name;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::find_node_at_position;
use crate::syntax::scope::{collect_enclosing_classes, find_enclosing_class_name};

/// Infer receiver type for scope keywords (`this`, `it`, `super`, `this@label`)
pub fn resolve_scope_keyword_receiver(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if clean == "it" {
        let target = root.and_then(|r| find_node_at_position(r, pos))?;
        return resolve_it_keyword(index, current_url, target, pos, source, root, depth);
    }
    if clean == "this" {
        let target = root.and_then(|r| find_node_at_position(r, pos))?;
        return resolve_this_keyword(index, current_url, target, pos, source, root, depth);
    }
    if let Some(label) = clean.strip_prefix("this@") {
        if let Some(r) = root {
            let ctx =
                super::scope_stack::build_scope_context(index, current_url, pos, source, r, depth);
            if let Some(this_ty) = ctx.find_this_by_label(label.trim()) {
                return Some(this_ty.clone());
            }
        }
        let full = resolve_full_type_name(index, current_url, label.trim());
        return super::types::parse_type_signature(&full);
    }
    if clean == "super" || clean.starts_with("super@") {
        return resolve_super_keyword(index, current_url, pos, source, root);
    }

    None
}

fn infer_lambda_param_from_decl(
    index: &WorkspaceIndex,
    current_url: &Url,
    decl_node: Node,
    source: &str,
) -> Option<KotlinType> {
    let type_str = crate::parser::extract_type_annotation(&decl_node, source)?;
    let raw_type = super::typealias::resolve_type_alias_or_import(index, current_url, type_str);
    let full = resolve_full_type_name(index, current_url, &raw_type);
    let parsed = super::types::parse_type_signature(&full)?;
    if let KotlinType::Function { params, .. } = parsed.unwrap_nullable_ref()
        && params.len() == 1
    {
        return Some(params[0].clone());
    }
    None
}

enum LambdaCallItResult {
    Resolved(KotlinType),
    ReceiverOnly,
    ExplicitParamsNeeded,
    Unresolved,
}

fn resolve_it_from_lambda_call(
    ctx: InferenceContext<'_, '_, '_>,
    call_node: Node,
    lambda_node: Node,
) -> LambdaCallItResult {
    if let Some((solved_fn_ty, label)) =
        super::scope_lambda::resolve_lambda_type_for_call_and_lambda(
            ctx.index,
            ctx.current_url,
            call_node,
            lambda_node,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        )
        && let super::types::KotlinType::Function { params, .. } =
            solved_fn_ty.unwrap_nullable_ref()
    {
        if params.len() == 1 {
            let res = params[0].clone();
            if let Some(lbl) = label {
                ctx.tracer.record_step(|| {
                    common::concat_str!("Solved enclosing call `", lbl.as_str(), "`")
                });
            }
            return LambdaCallItResult::Resolved(res);
        } else if params.is_empty() {
            ctx.tracer.record_step(|| {
                "Enclosing lambda has receiver-only block, searching outer scope".to_string()
            });
            return LambdaCallItResult::ReceiverOnly;
        } else {
            return LambdaCallItResult::ExplicitParamsNeeded;
        }
    }

    if let Some(t_inferred) = super::lambda::infer_lambda_it_type_for_call(
        ctx.index,
        ctx.current_url,
        call_node,
        lambda_node,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth + 1,
    ) {
        return LambdaCallItResult::Resolved(t_inferred);
    }

    LambdaCallItResult::Unresolved
}

pub fn resolve_it_keyword_traced(
    index: &WorkspaceIndex,
    current_url: &Url,
    t: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
    tracer: &mut super::trace::Tracer,
) -> Option<KotlinType> {
    tracer.enter_scope("LambdaParameter", "it");
    let lambdas = super::lambda_ast::collect_enclosing_lambdas(t);
    for lambda_node in lambdas {
        if super::lambda_ast::lambda_has_explicit_params(lambda_node) {
            tracer.record_step(|| {
                "Enclosing lambda has explicit parameters, continuing search to outer scope"
                    .to_string()
            });
            continue;
        }

        if let Some(call_node) = super::lambda_ast::find_call_for_lambda(lambda_node) {
            let inf_ctx =
                super::InferenceContext::new(index, current_url, pos, source, root, depth, tracer);
            match resolve_it_from_lambda_call(inf_ctx, call_node, lambda_node) {
                LambdaCallItResult::Resolved(res) => {
                    tracer.exit_scope_type(Some(&res));
                    return Some(res);
                }
                LambdaCallItResult::ReceiverOnly => continue,
                LambdaCallItResult::ExplicitParamsNeeded => {
                    tracer.exit_scope(None);
                    return None;
                }
                LambdaCallItResult::Unresolved => {}
            }
        }

        if let Some(decl_node) =
            super::lambda_ast::find_enclosing_typed_decl_for_lambda(lambda_node)
            && let Some(t_inferred) =
                infer_lambda_param_from_decl(index, current_url, decl_node, source)
        {
            tracer.record_step(|| "Resolved from enclosing typed declaration".to_string());
            tracer.exit_scope_type(Some(&t_inferred));
            return Some(t_inferred);
        }
    }

    tracer.exit_scope(None);
    None
}

pub fn resolve_it_keyword(
    index: &WorkspaceIndex,
    current_url: &Url,
    t: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    resolve_it_keyword_traced(
        index,
        current_url,
        t,
        pos,
        source,
        root,
        depth,
        &mut super::trace::Tracer::Noop,
    )
}

fn resolve_this_keyword(
    index: &WorkspaceIndex,
    current_url: &Url,
    t: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if let Some(r) = root {
        let ctx =
            super::scope_stack::build_scope_context(index, current_url, pos, source, r, depth);
        if let Some(this_ty) = ctx.current_this() {
            return Some(this_ty.clone());
        }
    }
    if let Some(ext_rec) = crate::syntax::ast_util::find_enclosing_extension_receiver(t, source) {
        let full = resolve_full_type_name(index, current_url, &ext_rec);
        return super::types::parse_type_signature(&full);
    }
    if let Some(name) = find_enclosing_class_name(t, source) {
        let full = resolve_full_type_name(index, current_url, &name);
        return super::types::parse_type_signature(&full);
    }
    None
}

fn resolve_super_keyword(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    let class_name = root
        .and_then(|r| find_node_at_position(r, pos))
        .and_then(|n| find_enclosing_class_name(n, source))?;
    index
        .class_supertypes
        .get(&class_name)
        .and_then(|s| s.first())
        .and_then(|s| {
            let full = resolve_full_type_name(index, current_url, s);
            super::types::parse_type_signature(&full)
        })
}

/// Infer the contextual receiver (`this` or `it`) inside a scope function, extension, or class body
pub fn infer_scope_receiver(
    index: &WorkspaceIndex,
    current_url: &Url,
    keyword: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    let tree = root
        .is_none()
        .then(|| crate::parser::parse_source(source))
        .flatten();
    let root_node = root.or_else(|| tree.as_ref().map(|t| t.root_node()));
    resolve_scope_keyword_receiver(index, current_url, keyword, pos, source, root_node, 0)
}

/// Collect all enclosing containers and their supertypes visible at the cursor
pub fn collect_visible_containers(
    index: &WorkspaceIndex,
    root: Option<Node>,
    pos: Position,
    source: &str,
) -> HashSet<String> {
    collect_visible_containers_depth(index, root, pos, source, 0)
}

pub fn collect_visible_containers_depth(
    index: &WorkspaceIndex,
    root: Option<Node>,
    pos: Position,
    source: &str,
    depth: usize,
) -> HashSet<String> {
    let dummy_url = Url::parse("file:///in_memory.kt").unwrap();
    collect_visible_containers_url_depth(index, &dummy_url, root, pos, source, depth)
}

pub fn collect_visible_containers_with_url(
    index: &WorkspaceIndex,
    current_url: &Url,
    root: Option<Node>,
    pos: Position,
    source: &str,
) -> HashSet<String> {
    collect_visible_containers_url_depth(index, current_url, root, pos, source, 0)
}

pub fn collect_visible_containers_url_depth(
    index: &WorkspaceIndex,
    current_url: &Url,
    root: Option<Node>,
    pos: Position,
    source: &str,
    depth: usize,
) -> HashSet<String> {
    let mut enclosing = collect_enclosing_classes(root, pos, source);
    if depth <= 2
        && let Some(root_node) = root
    {
        let ctx = super::scope_stack::build_scope_context(
            index,
            current_url,
            pos,
            source,
            root_node,
            depth,
        );
        for name in ctx.collect_visible_container_names() {
            enclosing.insert(name);
        }
    }
    let current: Vec<String> = enclosing.iter().cloned().collect();
    for container_name in current {
        for supertype in index.get_all_supertypes(&container_name) {
            enclosing.insert(supertype);
        }
    }
    enclosing
}
