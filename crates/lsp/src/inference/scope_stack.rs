use ahash::AHashSet;
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
use super::qualification::{qualify_type, resolve_full_type_name};
use super::scope_lambda::resolve_enclosing_lambda_type;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};
use crate::parser::{
    extract_identifier_text, extract_type_annotation, find_enclosing_declaration_node,
    find_node_at_position, parse_source,
};
use crate::syntax::ast_extra::extract_single_expression_body;
use crate::syntax::ast_util::extract_initializer_node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeReceiverKind {
    ClassBody,
    ExtensionFunction,
    LambdaReceiver,
}

#[derive(Debug, Clone)]
pub struct ScopeReceiver {
    pub type_ref: KotlinType,
    pub kind: ScopeReceiverKind,
    pub label: Option<SmolStr>,
    pub node_start_byte: usize,
}

pub struct ScopeContext<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub receivers: Vec<ScopeReceiver>,
}

impl<'a> ScopeContext<'a> {
    pub fn new(index: &'a WorkspaceIndex, current_url: &'a Url) -> Self {
        Self {
            index,
            current_url,
            receivers: Vec::new(),
        }
    }

    pub fn push_receiver(
        &mut self,
        type_ref: KotlinType,
        kind: ScopeReceiverKind,
        label: Option<SmolStr>,
        node_start_byte: usize,
    ) {
        self.receivers.push(ScopeReceiver {
            type_ref,
            kind,
            label,
            node_start_byte,
        });
    }

    pub fn current_this(&self) -> Option<&KotlinType> {
        self.receivers.first().map(|r| &r.type_ref)
    }

    pub fn find_this_by_label(&self, label: &str) -> Option<&KotlinType> {
        self.receivers
            .iter()
            .find(|r| r.label.as_deref() == Some(label))
            .map(|r| &r.type_ref)
    }

    pub fn collect_visible_container_names(&self) -> AHashSet<String> {
        let mut names = AHashSet::default();
        for receiver in &self.receivers {
            let base = receiver.type_ref.base_name();
            let short = base.rsplit('.').next().unwrap_or(base);
            names.insert(base.to_string());
            names.insert(short.to_string());
            for sup in self.index.get_all_supertypes(base) {
                let sup_bare = sup.split('<').next().unwrap_or(&sup).trim();
                let sup_short = sup_bare.rsplit('.').next().unwrap_or(sup_bare);
                names.insert(sup.clone());
                names.insert(sup_bare.to_string());
                names.insert(sup_short.to_string());
            }
        }
        names
    }
}

fn push_enclosing_receiver<'a>(
    ctx: &mut ScopeContext<'a>,
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    curr: Node<'a>,
    pos: Position,
    source: &str,
    root: Node<'a>,
    depth: usize,
) {
    let kind = curr.kind();
    if kind == "lambda_literal" {
        if curr.start_position() >= crate::position::position_to_point(pos) {
            return;
        }
        let lambda_pos = crate::position::point_to_position(curr.start_position());
        if let Some((
            KotlinType::Function {
                receiver: Some(rec),
                ..
            },
            label,
        )) = resolve_enclosing_lambda_type(
            index,
            current_url,
            curr,
            lambda_pos,
            source,
            Some(root),
            depth + 1,
        ) {
            ctx.push_receiver(
                *rec,
                ScopeReceiverKind::LambdaReceiver,
                label,
                curr.start_byte(),
            );
        }
    } else if kind == "function_declaration" {
        if let Some(ext_rec) =
            crate::syntax::ast_util::find_enclosing_extension_receiver(curr, source)
        {
            let full = resolve_full_type_name(index, current_url, &ext_rec);
            if let Some(k_ty) = super::types::parse_type_signature(&full) {
                let qualified = qualify_type(index, current_url, &k_ty);
                let fn_name = extract_identifier_text(&curr, source).map(SmolStr::new);
                ctx.push_receiver(
                    qualified,
                    ScopeReceiverKind::ExtensionFunction,
                    fn_name,
                    curr.start_byte(),
                );
            }
        }
    } else if matches!(
        kind,
        "class_declaration" | "object_declaration" | "companion_object"
    ) {
        let class_name =
            extract_identifier_text(&curr, source).unwrap_or(if kind == "companion_object" {
                "Companion"
            } else {
                ""
            });
        if !class_name.is_empty() {
            let full = resolve_full_type_name(index, current_url, class_name);
            if let Some(k_ty) = super::types::parse_type_signature(&full) {
                let qualified = qualify_type(index, current_url, &k_ty);
                ctx.push_receiver(
                    qualified,
                    ScopeReceiverKind::ClassBody,
                    Some(SmolStr::new(class_name)),
                    curr.start_byte(),
                );
            }
        }
    }
}

pub fn build_scope_context<'a>(
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    pos: Position,
    source: &str,
    root: Node<'a>,
    depth: usize,
) -> ScopeContext<'a> {
    let mut ctx = ScopeContext::new(index, current_url);
    let target = match find_node_at_position(root, pos) {
        Some(node) => node,
        None => return ctx,
    };

    let mut current = Some(target);
    while let Some(curr) = current {
        push_enclosing_receiver(&mut ctx, index, current_url, curr, pos, source, root, depth);
        current = curr.parent();
    }

    ctx
}

pub fn collect_implicit_receivers(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Vec<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return Vec::new();
    }
    let Some(root_node) = root else {
        return Vec::new();
    };
    let scope_ctx = build_scope_context(index, current_url, pos, source, root_node, depth);
    scope_ctx
        .receivers
        .into_iter()
        .map(|r| r.type_ref)
        .collect()
}

fn eval_function_expr_with_scope(
    index: &WorkspaceIndex,
    decl_url: &Url,
    decl_source: &str,
    root_node: Node,
    expr_node: Node,
    depth: usize,
) -> Option<KotlinType> {
    let expr_pos = crate::position::point_to_position(expr_node.start_position());
    let mut tracer = Tracer::Noop;
    let inf_ctx = InferenceContext::new(
        index,
        decl_url,
        expr_pos,
        decl_source,
        Some(root_node),
        depth + 1,
        &mut tracer,
    );
    let mut eval_ctx = AstEvalContext::from_ctx(inf_ctx);
    let scope_ctx = build_scope_context(index, decl_url, expr_pos, decl_source, root_node, depth);
    eval_ctx.scope_stack = scope_ctx
        .receivers
        .into_iter()
        .rev()
        .map(|r| r.type_ref)
        .collect();
    infer_expression_node(&mut eval_ctx, expr_node)
}

pub fn find_local_function_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
) -> Option<KotlinType> {
    let parsed_tree = ctx
        .root
        .is_none()
        .then(|| parse_source(ctx.source))
        .flatten();
    let root_node = ctx
        .root
        .or_else(|| parsed_tree.as_ref().map(|t| t.root_node()))?;
    let target_node = find_node_at_position(root_node, ctx.pos)?;
    let offset = crate::position::position_to_byte_offset(ctx.pos, ctx.source);
    let found_node = crate::syntax::scope::find_lexical_binding_node_before(
        target_node,
        fn_name,
        ctx.source,
        offset,
        crate::syntax::scope::BindingRole::DirectCall,
    )?;
    if found_node.kind() != "function_declaration" {
        return None;
    }
    if let Some(ret_str) = extract_type_annotation(&found_node, ctx.source)
        && let Some(ret_ty) = KotlinType::parse(ret_str)
    {
        return Some(qualify_type(ctx.index, ctx.current_url, &ret_ty));
    }
    let Some(expr_node) = extract_initializer_node(&found_node).or_else(|| {
        found_node
            .children(&mut found_node.walk())
            .find(|c| c.kind() == "function_body")
            .and_then(|body| extract_initializer_node(&body))
    }) else {
        return Some(KotlinType::UNIT);
    };
    eval_function_expr_with_scope(
        ctx.index,
        ctx.current_url,
        ctx.source,
        root_node,
        expr_node,
        ctx.depth,
    )
}

pub fn infer_single_expression_with_root(
    index: &WorkspaceIndex,
    decl_url: &Url,
    decl: &DeclarationItem,
    decl_source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let parsed_tree = root.is_none().then(|| parse_source(decl_source)).flatten();
    let root_node = root.or_else(|| parsed_tree.as_ref().map(|t| t.root_node()))?;
    let id_node = find_node_at_position(root_node, decl.selection_range.start)?;
    let fn_node = find_enclosing_declaration_node(id_node);

    if let Some(expr_node) = extract_initializer_node(&fn_node).or_else(|| {
        fn_node
            .children(&mut fn_node.walk())
            .find(|c| c.kind() == "function_body")
            .and_then(|body| extract_initializer_node(&body))
    }) && let Some(inferred) =
        eval_function_expr_with_scope(index, decl_url, decl_source, root_node, expr_node, depth)
    {
        return Some(inferred);
    }

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

/// Infer return type for single-expression functions (`fun foo() = expr`)
pub fn infer_single_expression_function_return_type(
    index: &WorkspaceIndex,
    decl_url: &Url,
    decl: &DeclarationItem,
    decl_source: &str,
    depth: usize,
) -> Option<KotlinType> {
    infer_single_expression_with_root(index, decl_url, decl, decl_source, None, depth)
}
