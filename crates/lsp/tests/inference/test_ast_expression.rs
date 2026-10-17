use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_expression_type, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_compound_binary_expression_with_function_calls() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompoundBinary.kt").unwrap();
    let code = r#"
package com.example.calc

fun getA(): Int = 10
fun getB(): Int = 20

fun compute() {
    val total = getA() + getB()
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "total", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_parenthesized_arithmetic_precedence() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ParenArith.kt").unwrap();
    let code = r#"
package com.example.calc

fun compute() {
    val result = (10 + 20) * 3
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_numeric_type_widening_addition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Widening.kt").unwrap();
    let code = r#"
package com.example.calc

val a: Int = 1
val b: Long = 2L
val c: Double = 3.0

fun compute() {
    val longRes = a + b
    val doubleRes = a + c
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(8, 8);
    let inferred_long = infer_variable_type(&index, &url, "longRes", pos1, code, None);
    assert_eq!(inferred_long, KotlinType::parse("Long"));

    let pos2 = Position::new(9, 8);
    let inferred_double = infer_variable_type(&index, &url, "doubleRes", pos2, code, None);
    assert_eq!(inferred_double, KotlinType::parse("Double"));
}

#[test]
fn test_infer_comparison_and_logical_expressions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Logic.kt").unwrap();
    let code = r#"
package com.example.calc

fun compute(x: Int, y: Int) {
    val isLess = x < y
    val isEqual = x == y
    val isLogical = (x > 0) && (y < 10)
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(4, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "isLess", pos1, code, None),
        KotlinType::parse("Boolean")
    );

    let pos2 = Position::new(5, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "isEqual", pos2, code, None),
        KotlinType::parse("Boolean")
    );

    let pos3 = Position::new(6, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "isLogical", pos3, code, None),
        KotlinType::parse("Boolean")
    );
}

#[test]
fn test_infer_elvis_expression_unwraps_nullability() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Elvis.kt").unwrap();
    let code = r#"
package com.example.calc

fun compute(str: String?) {
    val nonNull = str ?: "default"
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "nonNull", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_infer_range_expression_returns_int_range() {
    let index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Range.kt").unwrap();
    let code = "1..10";
    let inferred = infer_expression_type(&index, &url, code, Position::new(0, 0), code, None);
    assert_eq!(inferred, KotlinType::parse("IntRange"));
}

#[test]
fn test_ast_eval_infix_expression_evaluation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/InfixTest.kt").unwrap();
    let code = r#"
class Pair<A, B>(val first: A, val second: B)
infix fun <A, B> A.to(that: B): Pair<A, B> = Pair(this, that)

fun test() {
    val x = 1 to "abc"
}
"#;
    index.index_source(url.clone(), code);
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();
    let pos = Position::new(5, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, Some(root));
    assert_eq!(inferred, KotlinType::parse("Pair<Int, String>"));
}

#[test]
fn test_direct_ast_initializer_binary_arithmetic() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DirectAst.kt").unwrap();
    let code = r#"
package com.example.direct

fun test() {
    val total = 100 + 200
    val flag = total > 50
}
"#;
    index.index_source(url.clone(), code);
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();

    let total_decl = find_decl_by_name(root, code, "total").expect("total decl");
    let init_node =
        lsp::inference::variable::extract_initializer_node(&total_decl).expect("initializer node");
    assert_eq!(init_node.kind(), "additive_expression");

    let pos_total = Position::new(4, 8);
    let total_type = infer_variable_type(&index, &url, "total", pos_total, code, Some(root));
    assert_eq!(total_type, KotlinType::parse("Int"));

    let pos_flag = Position::new(5, 8);
    let flag_type = infer_variable_type(&index, &url, "flag", pos_flag, code, Some(root));
    assert_eq!(flag_type, KotlinType::parse("Boolean"));
}

#[test]
fn test_infer_binary_expression_with_inline_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CommentBinary.kt").unwrap();
    let code = r#"
package com.example.calc

fun compute() {
    val total = 10 /* inline comment */ + /* another comment */ 20
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "total", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_lambda_without_expected_type_as_functional_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BareLambda.kt").unwrap();
    let code = r#"
package com.example.calc

fun compute() {
    val f = { 42 }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "f", pos, code, None);
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

#[test]
fn test_extract_class_supertypes_from_qualified_specifier() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Inheritance.kt").unwrap();
    let code = r#"
package com.example.hier

open class BaseService
class MyService : com.example.hier.BaseService()
"#;
    index.index_source(url.clone(), code);

    let supers = index.get_all_supertypes("MyService");
    assert!(
        supers.contains(&"BaseService".to_string()),
        "Expected supertypes to include BaseService, found: {:?}",
        supers
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

#[test]
fn test_ast_match_with_multibyte_utf8_does_not_panic() {
    // "测" is a 3-byte UTF-8 character [0xE6, 0xB5, 0x8B]
    // Repeating multi-byte characters to ensure offsets land on non-char boundaries
    let mut code = String::from("// 测\n");
    while code.len() < 300 {
        code.push_str("val item = 1\n");
    }
    let tree = lsp::syntax::parser::parse_source(&code).unwrap();
    let root = tree.root_node();
    // Test multiple positions to trigger the offset.saturating_sub(128) boundary
    for line in 0..15 {
        for col in 0..10 {
            let _ = lsp::inference::ast_match::find_matching_ast_node(
                root,
                Position::new(line, col),
                "missing",
                &code,
            );
        }
    }
}

#[test]
fn test_ast_match_does_not_match_unrelated_previous_statement_identifier() {
    let code = "fun test() {\n    val item = 1\n    service.prop.call()\n}";
    let tree = lsp::syntax::parser::parse_source(code).unwrap();
    let root = tree.root_node();
    // Cursor on 'call' (line 2, col 17)
    let pos = Position::new(2, 17);
    let matched = lsp::inference::ast_match::find_matching_ast_node(root, pos, "item", code);
    assert!(
        matched.is_none(),
        "find_matching_ast_node should not match unrelated 'item' from earlier statement via rfind"
    );
}
