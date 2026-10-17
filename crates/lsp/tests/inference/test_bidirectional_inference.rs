use tower_lsp::lsp_types::{Position, Range, SymbolKind, Url};

use lsp::indexer::{DeclarationItem, DeclarationMetadata, WorkspaceIndex};
use lsp::inference::ast_eval::{AstEvalContext, infer_expression_node};
use lsp::inference::solver::ConstraintSystem;
use lsp::inference::types::KotlinType;
use lsp::inference::variable::{extract_initializer_node, infer_variable_type};
use lsp::inference::{InferenceContext, SignatureInfo, Tracer};

#[test]
fn test_declaration_signature_info_on_demand_derivation() {
    let sig = "fun <T : CharSequence> process(input: T, count: Int): List<T>";
    let range = Range::default();
    let decl = DeclarationItem::new(
        "process",
        SymbolKind::FUNCTION,
        range,
        range,
        DeclarationMetadata::signature(sig),
    );

    // Verify signature_info is derived on demand without persistent cache fields
    let info = SignatureInfo::from_decl(&decl)
        .expect("signature_info() must derive parsed info on demand");
    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0].type_name.as_str(), "T");
    assert_eq!(info.params[1].type_name.as_str(), "Int");
    assert_eq!(
        info.return_type.as_ref().map(|t| t.to_string()),
        Some("List<T>".to_string())
    );
    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name.as_str(), "T");
    assert_eq!(
        info.type_params[0].bounds,
        vec![KotlinType::simple_class("CharSequence")]
    );
}

#[test]
fn test_constraint_system_from_signature_info_with_bounds() {
    let index = WorkspaceIndex::new();
    let sig = "fun <T : CharSequence> process(input: T): T";
    let range = Range::default();
    let decl = DeclarationItem::new(
        "process",
        SymbolKind::FUNCTION,
        range,
        range,
        DeclarationMetadata::signature(sig),
    );
    let sig_info = SignatureInfo::from_decl(&decl).expect("on-demand parsed sig");

    let mut system = ConstraintSystem::from_signature_info(&index, &sig_info);
    assert!(system.type_params.contains(&smol_str::SmolStr::new("T")));

    // Subtype constraint: String <: T (String satisfies CharSequence bound)
    let actual = KotlinType::STRING;
    let expected = KotlinType::TypeParam(smol_str::SmolStr::new("T"));
    system.add_subtype_constraint(&actual, &expected);

    let solution = system.solve();
    let solved_t = solution.get("T").expect("T must be solved");
    assert_eq!(*solved_t, KotlinType::STRING);
}

#[test]
fn test_infer_lambda_with_expected_functional_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BidirectionalLambda.kt").unwrap();
    let code = r#"
package com.example.bidi

fun test() {
    val block = { 42 }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();
    let pos = Position::new(4, 8);

    // Expecting functional type () -> Int
    let expected_func = KotlinType::Function {
        receiver: None,
        params: vec![],
        return_type: Box::new(KotlinType::INT),
        is_suspend: false,
    };

    let lambda_text = "{ 42 }";
    let lambda_tree = lsp::syntax::parser::parse_source(lambda_text).unwrap();
    let lambda_node = lsp::inference::ast_eval::find_root_expression_node(lambda_tree.root_node())
        .expect("lambda node");

    let mut tracer = Tracer::Noop;
    let inf_ctx = InferenceContext::new(&index, &url, pos, code, Some(root), 0, &mut tracer);
    let base_ctx = AstEvalContext::new(inf_ctx, lambda_text);
    let mut ctx = base_ctx.with_expected(Some(expected_func.clone()));

    let inferred = infer_expression_node(&mut ctx, lambda_node);
    assert_eq!(
        inferred,
        Some(expected_func),
        "Lambda evaluation under expected functional type should propagate expected type"
    );
}

#[test]
fn test_solve_call_decl_consumes_on_demand_signature_info() {
    let index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CallDecl.kt").unwrap();
    let sig = "fun <T : CharSequence> formatItem(item: T): List<T>";
    let range = Range::default();
    let decl = DeclarationItem::new(
        "formatItem",
        SymbolKind::FUNCTION,
        range,
        range,
        DeclarationMetadata::signature(sig),
    );

    let call_info = lsp::inference::call::CallInfo {
        callee: "formatItem",
        value_args: vec!["\"kotlin\""],
        explicit_type_args: vec![],
    };

    let mut tracer = Tracer::Noop;
    let sig_info = SignatureInfo::from_decl(&decl).unwrap();
    let ret_type = sig_info.return_type.as_ref();
    let ctx = lsp::inference::InferenceContext::new(
        &index,
        &url,
        Position::default(),
        "\"kotlin\"",
        None,
        0,
        &mut tracer,
    );
    let solved = lsp::inference::call::solve_call_decl_return_type_traced(
        ctx, &decl, ret_type, None, &call_info, None,
    );

    assert_eq!(solved, KotlinType::parse("List<String>").unwrap());
}

#[test]
fn test_variable_annotation_propagates_expected_type_to_lambda_initializer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AnnotatedLambda.kt").unwrap();
    let code = r#"
package com.example.annotated

fun test() {
    val action: () -> Int = { 10 + 20 }
}
"#;
    index.index_source(url.clone(), code);
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();
    let pos = Position::new(4, 8);

    let inferred = infer_variable_type(&index, &url, "action", pos, code, Some(root));
    assert_eq!(inferred, KotlinType::parse("() -> Int"));
}

#[test]
fn test_property_initializer_evaluates_under_enclosing_declared_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/EnclosingInit.kt").unwrap();
    let code = r#"
package com.example.annotated

fun test() {
    val handler: (Int) -> String = { it.toString() }
    val total: Long = 100 + 200
}
"#;
    index.index_source(url.clone(), code);
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();

    // 1. Unannotated lambda inside annotated property declaration
    let decl_lambda = find_decl_by_name(root, code, "handler").expect("handler decl node");
    let init_lambda = extract_initializer_node(&decl_lambda).expect("lambda init node");

    let mut tracer = Tracer::Noop;
    let inf_ctx_lambda = InferenceContext::new(
        &index,
        &url,
        Position::new(4, 8),
        code,
        Some(root),
        0,
        &mut tracer,
    );
    let mut ctx_lambda = AstEvalContext::new(inf_ctx_lambda, code);
    assert!(ctx_lambda.expected_type.is_none());
    let inferred_lambda = infer_expression_node(&mut ctx_lambda, init_lambda);
    assert_eq!(
        inferred_lambda,
        KotlinType::parse("(Int) -> String"),
        "Unannotated lambda initializer should inherit expected type from enclosing property declaration"
    );

    // 2. Expression inside annotated property declaration
    let decl_expr = find_decl_by_name(root, code, "total").expect("total decl node");
    let init_expr = extract_initializer_node(&decl_expr).expect("expr init node");
    let mut tracer2 = Tracer::Noop;
    let inf_ctx_expr = InferenceContext::new(
        &index,
        &url,
        Position::new(5, 8),
        code,
        Some(root),
        0,
        &mut tracer2,
    );
    let mut ctx_expr = AstEvalContext::new(inf_ctx_expr, code);
    assert!(ctx_expr.expected_type.is_none());
    let _ = infer_expression_node(&mut ctx_expr, init_expr);
    assert_eq!(
        ctx_expr.expected_type,
        Some(KotlinType::LONG),
        "Expression initializer should inherit expected type from enclosing property declaration"
    );
}

#[test]
fn test_lambda_body_expression_evaluates_under_expected_return_type() {
    let index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BodyEval.kt").unwrap();
    let code = "{ 100 + 200 }";
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();
    let lambda_node = lsp::inference::ast_match::find_root_expression_node(root).unwrap();

    let expected_func = KotlinType::Function {
        receiver: None,
        params: vec![],
        return_type: Box::new(KotlinType::INT),
        is_suspend: false,
    };

    let mut tracer = Tracer::Noop;
    let inf_ctx_body = InferenceContext::new(
        &index,
        &url,
        Position::default(),
        code,
        Some(root),
        0,
        &mut tracer,
    );
    let base_ctx = AstEvalContext::new(inf_ctx_body, code);
    let mut ctx = base_ctx.with_expected(Some(expected_func));

    let inferred = infer_expression_node(&mut ctx, lambda_node);
    assert_eq!(
        inferred,
        Some(KotlinType::Function {
            receiver: None,
            params: vec![],
            return_type: Box::new(KotlinType::INT),
            is_suspend: false,
        })
    );
}

fn find_decl_by_name<'a>(
    node: tree_sitter::Node<'a>,
    source: &'a str,
    name: &str,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "property_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declaration"
                && let Some(id) = lsp::syntax::parser::extract_identifier_text(&child, source)
                && id == name
            {
                return Some(node);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(res) = find_decl_by_name(child, source, name) {
            return Some(res);
        }
    }
    None
}
