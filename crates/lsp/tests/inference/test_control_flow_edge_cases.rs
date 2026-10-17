use lsp::indexer::WorkspaceIndex;
use lsp::inference::ast_context::find_enclosing_initializer_expected_type;
use lsp::inference::ast_eval::AstEvalContext;
use lsp::inference::types::KotlinType;
use lsp::inference::unify::is_class_subtype;
use lsp::inference::{infer_variable_type, unify_types};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_mutable_destructuring_variable_disables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DestructuringVar.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)
data class Pair(val first: Any, val second: Any)

fun test(p: Pair) {
    var (a, b) = p
    val res = if (a is Box) {
        a.size
    } else {
        "fallback"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    // Because 'a' was declared in 'var (a, b) = p', smart casting is disabled; falls back to "fallback"
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_immutable_destructuring_variable_enables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DestructuringVal.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)
data class Pair(val first: Any, val second: Any)

fun test(p: Pair) {
    val (a, b) = p
    val res = if (a is Box) {
        a.size
    } else {
        "fallback"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    // Immutable 'val (a, b) = p' allows smart cast; LUB(Int, String) = Any
    assert_eq!(inferred, Some(KotlinType::ANY));
}

#[test]
fn test_custom_getter_property_disables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CustomGetter.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)

class Holder {
    val prop: Any
        get() = Box(1)

    fun test() {
        val res = if (prop is Box) {
            prop.size
        } else {
            "fallback"
        }
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(10, 12);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    // Custom getter disallows smart casting; falls back to String
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_user_defined_number_does_not_subsume_primitive_int() {
    let index = WorkspaceIndex::new();
    let custom_number = KotlinType::simple_class("com.example.Number");
    let unified = unify_types(&index, &[KotlinType::INT, custom_number.clone()]);
    assert_ne!(unified, Some(custom_number));

    let std_number = KotlinType::NUMBER;
    let unified_std = unify_types(&index, &[KotlinType::INT, std_number.clone()]);
    assert_eq!(unified_std, Some(std_number));
}

#[test]
fn test_user_defined_any_does_not_subsume_arbitrary_classes() {
    let index = WorkspaceIndex::new();
    assert!(!is_class_subtype(
        &index,
        "com.example.Foo",
        "com.example.Any"
    ));
    assert!(is_class_subtype(&index, "com.example.Foo", "kotlin.Any"));
    assert!(is_class_subtype(&index, "com.example.Foo", "Any"));
}

#[test]
fn test_user_defined_number_class_not_shadowed_by_kotlin_number() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CustomNumber.kt").unwrap();
    let code = r#"
package com.example

class Number(val value: String)

fun test(n: Number) {
    val res = n
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(
        inferred,
        Some(KotlinType::simple_class("com.example.Number"))
    );
}

#[test]
fn test_branch_tail_assignment_evaluates_to_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BranchAssignment.kt").unwrap();
    let code = r#"
package com.example

fun test(c: Boolean) {
    var count = 0
    val res = if (c) {
        count = 1
    } else {
        count = 2
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(5, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::UNIT));
}

#[test]
fn test_branch_tail_destructuring_declaration_evaluates_to_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BranchDestructure.kt").unwrap();
    let code = r#"
package com.example

data class Pair(val first: Int, val second: Int)

fun test(c: Boolean, p: Pair) {
    val res = if (c) {
        val (x, y) = p
    } else {
        val (x, y) = p
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::UNIT));
}

#[test]
fn test_jump_expression_branch_unifies_with_alternative_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/JumpBranch.kt").unwrap();
    let code = r#"
package com.example

fun outer(): Int {
    fun inner(): String {
        val res = if (true) return@outer 42 else "fallback"
        return "ok"
    }
    return 0
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(5, 12);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_labeled_return_expected_type_discovery_across_inner_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedLabeledReturn.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)

fun outer(): Box {
    fun inner(): Int {
        return@outer Box(42)
    }
    return Box(0)
}
"#;
    index.index_source(url.clone(), code);
    let mut parser = lsp::syntax::parser::KotlinParser::new();
    let tree = parser.parse(code).unwrap();
    let root = tree.root_node();
    let mut target = None;
    fn find_box_call<'a>(
        node: tree_sitter::Node<'a>,
        source: &str,
        target: &mut Option<tree_sitter::Node<'a>>,
    ) {
        if node.kind() == "call_expression"
            && source.get(node.start_byte()..node.end_byte()) == Some("Box(42)")
        {
            *target = Some(node);
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            find_box_call(child, source, target);
        }
    }
    find_box_call(root, code, &mut target);
    let call_node = target.expect("Box(42) node found");
    let mut tracer = lsp::inference::Tracer::Noop;
    let inf_ctx = lsp::inference::InferenceContext::new(
        &index,
        &url,
        Position::default(),
        code,
        Some(root),
        0,
        &mut tracer,
    );
    let ctx = AstEvalContext::new(inf_ctx, code);
    let expected = find_enclosing_initializer_expected_type(&ctx, call_node);
    assert_eq!(expected, Some(KotlinType::simple_class("com.example.Box")));
}

#[test]
fn test_single_expression_function_propagates_expected_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SingleExprExpected.kt").unwrap();
    let code = r#"
package com.example

fun choose(c: Boolean): Number = if (c) 1 else 2
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(3, 4);
    let inferred =
        lsp::inference::infer_function_return_type(&index, &url, "choose", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::NUMBER));
}

#[test]
fn test_mutable_class_parameter_disables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ClassParamVar.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)

class User(var data: Any) {
    fun test() {
        val res = if (data is Box) {
            data.size
        } else {
            "fallback"
        }
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(8, 12);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_immutable_class_parameter_enables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ClassParamVal.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)

class User(val data: Any) {
    fun test() {
        val res = if (data is Box) {
            data.size
        } else {
            "fallback"
        }
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(8, 12);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::ANY));
}

#[test]
fn test_primitive_int_does_not_unify_with_incompatible_comparable() {
    let index = WorkspaceIndex::new();
    let comp_string = KotlinType::class(
        "kotlin.Comparable",
        vec![lsp::inference::types::TypeArg::out(KotlinType::STRING)],
    );
    let lub_string = unify_types(&index, &[KotlinType::INT, comp_string]);
    assert_eq!(lub_string, Some(KotlinType::ANY));

    let comp_int = KotlinType::class(
        "kotlin.Comparable",
        vec![lsp::inference::types::TypeArg::out(KotlinType::INT)],
    );
    let lub_int = unify_types(&index, &[KotlinType::INT, comp_int.clone()]);
    assert_eq!(lub_int, Some(comp_int));
}
