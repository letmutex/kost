use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_if_expression_least_upper_bound_inference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfExpr.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val x = if (cond) 10 else 20
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_if_expression_with_throw_branch_returns_non_throw_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfThrow.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val x = if (cond) "success" else throw RuntimeException()
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_when_expression_multiple_branches_least_upper_bound() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenExpr.kt").unwrap();
    let code = r#"
package com.example.control

fun test(x: Int) {
    val result = when (x) {
        1 -> "one"
        2 -> "two"
        else -> "other"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_try_catch_expression_least_upper_bound() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TryCatch.kt").unwrap();
    let code = r#"
package com.example.control

fun test() {
    val res = try {
        42
    } catch (e: Exception) {
        0
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_hex_and_binary_and_unsigned_literals_inference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Literals.kt").unwrap();
    let code = r#"
package com.example.lit

val hexVal = 0x1A
val binVal = 0b1010
val unsignedVal = 42u
"#;
    index.index_source(url.clone(), code);

    let inferred_hex = infer_variable_type(&index, &url, "hexVal", Position::new(3, 4), code, None);
    assert_eq!(inferred_hex, Some(KotlinType::INT));

    let inferred_bin = infer_variable_type(&index, &url, "binVal", Position::new(4, 4), code, None);
    assert_eq!(inferred_bin, Some(KotlinType::INT));

    let inferred_u =
        infer_variable_type(&index, &url, "unsignedVal", Position::new(5, 4), code, None);
    assert_eq!(inferred_u, Some(KotlinType::simple_class("UInt")));
}

#[test]
fn test_downward_scope_stack_resolves_this_keyword() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ScopeThis.kt").unwrap();
    let code = r#"
package com.example.scope

class Context {
    fun doWork(): String = "done"
}

fun runWith(ctx: Context, block: Context.() -> Unit) {}

fun test(c: Context) {
    runWith(c) {
        val self = this
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(11, 12);
    let inferred = infer_variable_type(&index, &url, "self", pos, code, None);
    assert_eq!(
        inferred,
        Some(KotlinType::simple_class("com.example.scope.Context"))
    );
}

#[test]
fn test_loop_break_and_continue_jump_expressions_unify_to_other_branch() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LoopBreak.kt").unwrap();
    let code = r#"
package com.example.control

fun test(list: List<Int>) {
    for (x in list) {
        val resContinue = if (x > 0) "positive" else continue
        val resBreak = if (x > 0) "positive" else break
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos_continue = Position::new(5, 12);
    let inferred_continue =
        infer_variable_type(&index, &url, "resContinue", pos_continue, code, None);
    assert_eq!(inferred_continue, Some(KotlinType::STRING));

    let pos_break = Position::new(6, 12);
    let inferred_break = infer_variable_type(&index, &url, "resBreak", pos_break, code, None);
    assert_eq!(inferred_break, Some(KotlinType::STRING));
}

#[test]
fn test_if_expression_with_return_branch_unifies_to_other_branch() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfReturn.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val res1 = if (cond) "success" else return
    val res2 = if (cond) "success" else return 42
}
"#;
    index.index_source(url.clone(), code);
    let pos1 = Position::new(4, 8);
    let inferred1 = infer_variable_type(&index, &url, "res1", pos1, code, None);
    assert_eq!(inferred1, Some(KotlinType::STRING));

    let pos2 = Position::new(5, 8);
    let inferred2 = infer_variable_type(&index, &url, "res2", pos2, code, None);
    assert_eq!(inferred2, Some(KotlinType::STRING));
}

#[test]
fn test_if_else_lub_primitive_numeric_unifies_to_number() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfNumeric.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val x = if (cond) 1 else 2.0
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::NUMBER));
}

#[test]
fn test_if_without_else_evaluates_to_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfNoElse.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val x = if (cond) 42
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::UNIT));
}

#[test]
fn test_if_lub_nothing_nullable_propagation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfNull.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val x = if (cond) "foo" else null
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING.make_nullable()));
}

#[test]
fn test_if_consequence_smart_cast_val() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfSmartCast.kt").unwrap();
    let code = r#"
package com.example.control

fun test(x: Any) {
    val res = if (x is String) x.length else 0
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_if_condition_null_check_unwrap() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfNullCheck.kt").unwrap();
    let code = r#"
package com.example.control

fun test(x: String?) {
    val res = if (x != null) x.length else 0
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_when_subject_val_scoped_to_entries() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenSubjectVal.kt").unwrap();
    let code = r#"
package com.example.control

fun test(str: String) {
    val res = when (val len = str.length) {
        0 -> "empty"
        else -> len.toString()
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_when_entry_smart_cast_is_test() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenSmartCast.kt").unwrap();
    let code = r#"
package com.example.control

fun test(x: Any) {
    val res = when (x) {
        is String -> x.length
        else -> 0
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_branch_block_intermediate_var_usable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BranchVar.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val res = if (cond) {
        var count = 42
        count
    } else {
        0
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_single_expression_function_if_branch_inference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SingleExprIf.kt").unwrap();
    let code = r#"
package com.example.control

fun choose(cond: Boolean) = if (cond) "apple" else "banana"
"#;
    index.index_source(url.clone(), code);
    let inferred = lsp::inference::infer_function_return_type(
        &index,
        &url,
        "choose",
        Position::new(3, 4),
        code,
        None,
    );
    assert_eq!(inferred, Some(KotlinType::STRING));
}
