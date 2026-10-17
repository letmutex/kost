use tree_sitter::Node;

use super::ast_binary::is_binary_kind;
use super::ast_context::{find_enclosing_initializer_expected_type, infer_statement_or_control};
use super::ast_control::{infer_control_flow, is_control_expression_kind};
use super::ast_literal::infer_literal;
pub use super::ast_match::{
    find_matching_ast_node, find_root_expression_node, is_ast_evaluable_kind,
};
use super::context::InferenceContext;
use super::infix::infer_infix_node;
use super::types::KotlinType;

pub struct AstEvalContext<'a, 't, 'tr> {
    pub ctx: InferenceContext<'a, 't, 'tr>,
    pub node_source: &'a str,
    pub expected_type: Option<KotlinType>,
    pub scope_stack: Vec<KotlinType>,
    pub local_bindings: Vec<(smol_str::SmolStr, KotlinType)>,
}

impl<'a, 't, 'tr> std::ops::Deref for AstEvalContext<'a, 't, 'tr> {
    type Target = InferenceContext<'a, 't, 'tr>;
    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl<'a, 't, 'tr> std::ops::DerefMut for AstEvalContext<'a, 't, 'tr> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl<'a, 't, 'tr> AstEvalContext<'a, 't, 'tr> {
    pub fn new(ctx: InferenceContext<'a, 't, 'tr>, node_source: &'a str) -> Self {
        Self {
            ctx,
            node_source,
            expected_type: None,
            scope_stack: Vec::new(),
            local_bindings: Vec::new(),
        }
    }

    pub fn from_ctx(ctx: InferenceContext<'a, 't, 'tr>) -> Self {
        let node_source = ctx.source;
        Self {
            ctx,
            node_source,
            expected_type: None,
            scope_stack: Vec::new(),
            local_bindings: Vec::new(),
        }
    }

    pub fn node_text(&self, node: Node) -> Option<&'a str> {
        let start = node.start_byte();
        let end = node.end_byte();
        if end <= self.node_source.len() && start <= end {
            self.node_source.get(start..end).map(str::trim)
        } else {
            None
        }
    }

    pub fn push_scope(&mut self, ty: KotlinType) {
        self.scope_stack.push(ty);
    }

    pub fn sub<'s>(&'s mut self) -> AstEvalContext<'a, 's, 'tr> {
        AstEvalContext {
            ctx: self.ctx.sub(),
            node_source: self.node_source,
            expected_type: self.expected_type.clone(),
            scope_stack: self.scope_stack.clone(),
            local_bindings: self.local_bindings.clone(),
        }
    }

    pub fn with_expected(mut self, expected: Option<KotlinType>) -> Self {
        self.expected_type = expected;
        self
    }

    pub fn apply_expected_function(&mut self, lambda_node: Node, fn_type: &KotlinType) {
        if let KotlinType::Function {
            receiver,
            params,
            return_type,
            ..
        } = fn_type
        {
            if let Some(rec) = receiver {
                self.push_scope((**rec).clone());
            }
            let param_names =
                super::lambda_ast::extract_lambda_param_names(lambda_node, self.node_source);
            if !param_names.is_empty() {
                for (name, ty) in param_names.into_iter().zip(params) {
                    self.local_bindings.push((name.into(), ty.clone()));
                }
            } else if params.len() == 1 {
                self.local_bindings.push(("it".into(), params[0].clone()));
            }
            if !matches!(**return_type, KotlinType::TypeParam(_)) {
                self.expected_type = Some((**return_type).clone());
            }
        }
    }
}

/// Try AST-based evaluation with top-down expected type propagation
pub fn try_ast_evaluation_expected(
    mut ctx: InferenceContext<'_, '_, '_>,
    clean: &str,
    expected: Option<&KotlinType>,
) -> Option<KotlinType> {
    if let Some(r) = ctx.root
        && let Some(matched) = find_matching_ast_node(r, ctx.pos, clean, ctx.source)
    {
        let mut eval_ctx = AstEvalContext::from_ctx(ctx.reborrow());
        if let Some(exp) = expected {
            eval_ctx.expected_type = Some(exp.clone());
        }
        return infer_expression_node(&mut eval_ctx, matched);
    }

    let tree = crate::parser::parse_source(clean)?;
    let expr_node = find_root_expression_node(tree.root_node())?;
    let kind = expr_node.kind();
    if is_binary_kind(kind) || is_ast_evaluable_kind(kind) || is_control_expression_kind(kind) {
        let mut eval_ctx = AstEvalContext::new(ctx, clean);
        if let Some(exp) = expected {
            eval_ctx.expected_type = Some(exp.clone());
        }
        infer_expression_node(&mut eval_ctx, expr_node)
    } else {
        None
    }
}

/// Infer type directly from an AST expression node
pub fn infer_expression_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }

    if ctx.expected_type.is_none()
        && let Some(expected) = find_enclosing_initializer_expected_type(ctx, node)
    {
        ctx.expected_type = Some(expected);
    }

    if let Some(ctrl) = infer_control_flow(ctx, node) {
        return Some(ctrl);
    }

    if let Some(stmt) = infer_statement_or_control(ctx, node) {
        return Some(stmt);
    }

    if let Some(inv) = super::ast_call::infer_invocation(ctx, node) {
        return Some(inv);
    }

    if is_binary_kind(node.kind()) {
        return super::ast_binary::infer_binary_node(ctx, node);
    }

    match node.kind() {
        "parenthesized_expression" => infer_parenthesized(ctx, node),
        "elvis_expression" => super::ast_binary::infer_elvis_node(ctx, node),
        "as_expression" => {
            super::ast_binary::infer_as_cast(ctx.index, ctx.current_url, node, ctx.node_source)
        }
        "simple_identifier" => infer_identifier(ctx, node),
        "this_expression" | "super_expression" => infer_this_or_super(ctx, node),
        "lambda_literal" => infer_lambda_ast_node(ctx, node),
        "infix_expression" => infer_infix_node(ctx, node),
        _ => infer_literal(ctx, node).or_else(|| {
            let text = ctx.node_text(node)?;
            super::qualification::match_qualified_class(ctx.index, text, ctx.tracer)
                .or_else(|| super::expression::resolve_single_receiver(ctx.ctx.sub(), text))
        }),
    }
}

fn infer_this_or_super(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let text = ctx.node_text(node)?;
    if text == "this"
        && let Some(rec) = ctx.scope_stack.last()
    {
        return Some(rec.clone());
    }
    super::scope_function::resolve_scope_keyword_receiver(
        ctx.index,
        ctx.current_url,
        text,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    )
}

fn infer_lambda_ast_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    if let Some(KotlinType::Function { return_type, .. }) = ctx.expected_type.as_ref()
        && return_type.is_unit()
    {
        return ctx.expected_type.clone();
    }
    if let Some(ref fn_ty) = ctx.expected_type.clone()
        && let KotlinType::Function {
            receiver,
            params,
            return_type,
            is_suspend,
        } = fn_ty
    {
        let actual_return = if let Some((last_expr, _)) =
            super::lambda_ast::find_last_expression_in_lambda(node, ctx.node_source)
        {
            let mut sub_ctx = ctx.sub();
            sub_ctx.apply_expected_function(node, fn_ty);
            infer_expression_node(&mut sub_ctx, last_expr)
                .unwrap_or_else(|| (**return_type).clone())
        } else {
            (**return_type).clone()
        };
        return Some(KotlinType::Function {
            receiver: receiver.clone(),
            params: params.clone(),
            return_type: Box::new(actual_return),
            is_suspend: *is_suspend,
        });
    }

    let (lambda_src, lambda_root) = if ctx.node_source == ctx.source {
        (ctx.source, ctx.root)
    } else {
        (ctx.node_source, None)
    };
    let body_ret = super::lambda::infer_lambda_return_type(
        ctx.index,
        ctx.current_url,
        node,
        lambda_src,
        lambda_root,
        ctx.depth + 1,
    )
    .unwrap_or(KotlinType::UNIT);
    let params = super::lambda_ast::extract_lambda_param_types(node, ctx.node_source);

    Some(KotlinType::Function {
        receiver: None,
        params,
        return_type: Box::new(body_ret),
        is_suspend: false,
    })
}

fn infer_identifier(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let name = ctx.node_text(node)?;
    if let Some((_, ty)) = ctx.local_bindings.iter().rev().find(|(k, _)| k == name) {
        return Some(ty.clone());
    }
    if name == "this"
        && let Some(rec) = ctx.scope_stack.last()
    {
        return Some(rec.clone());
    }
    if (matches!(name, "this" | "it" | "super")
        || name.starts_with("this@")
        || name.starts_with("super@"))
        && let Some(res) = super::scope_function::resolve_scope_keyword_receiver(
            ctx.index,
            ctx.current_url,
            name,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth,
        )
    {
        return Some(res);
    }
    for i in (0..ctx.scope_stack.len()).rev() {
        let rec = ctx.scope_stack[i].clone();
        if let Some(member_ty) =
            super::member::resolve_member_type_kt_depth_traced(ctx.ctx.sub(), &rec, name)
        {
            return Some(member_ty);
        }
    }
    super::symbol_receiver::resolve_named_symbol_receiver(ctx.reborrow(), name)
}

fn infer_parenthesized(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            return infer_expression_node(ctx, child);
        }
    }
    None
}
