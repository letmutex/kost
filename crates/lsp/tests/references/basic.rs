use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_local_variable_usages_include_and_exclude_declaration() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/LocalTest.kt").unwrap();
    let code = r#"
fun calculate(): Int {
    val total = 100
    val doubled = total * 2
    println(total)
    return total
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at "val total = 100" (line 2, char 8)
    let decl_pos = Position::new(2, 8);

    let refs_without_decl = find_references(&index, &file_url, "total", decl_pos, code, false);
    assert_eq!(
        refs_without_decl.len(),
        3,
        "Should find 3 usages of 'total' excluding declaration"
    );

    let refs_with_decl = find_references(&index, &file_url, "total", decl_pos, code, true);
    assert_eq!(
        refs_with_decl.len(),
        4,
        "Should find 4 locations of 'total' including declaration"
    );
}

#[test]
fn test_references_local_variable_with_shadowing_excludes_inner_scope() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ShadowTest.kt").unwrap();
    let code = r#"
fun run() {
    val count = 1
    if (count > 0) {
        val count = 99
        println(count)
    }
    println(count)
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on outer "val count = 1" (line 2, char 8)
    let decl_pos = Position::new(2, 8);
    let refs = find_references(&index, &file_url, "count", decl_pos, code, false);

    // Usages should be: `count > 0` (line 3) and `println(count)` (line 7)
    assert_eq!(
        refs.len(),
        2,
        "Should only find outer count usages, not inner shadowed count"
    );
    assert_eq!(refs[0].range.start.line, 3);
    assert_eq!(refs[1].range.start.line, 7);
}

#[test]
fn test_references_function_parameter_usages() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ParamTest.kt").unwrap();
    let code = r#"
fun greet(message: String) {
    println(message)
    val copy = message
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on parameter "message" (line 1, char 10)
    let param_pos = Position::new(1, 10);
    let refs = find_references(&index, &file_url, "message", param_pos, code, false);

    assert_eq!(refs.len(), 2, "Should find 2 usages of parameter 'message'");
    assert_eq!(refs[0].range.start.line, 2);
    assert_eq!(refs[1].range.start.line, 3);
}

#[test]
fn test_references_top_level_function_across_files() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/OrderService.kt").unwrap();
    let file_a_code = r#"
package com.example.service

fun processOrder(): Int {
    return 42
}
"#;

    let file_b_url = Url::parse("file:///project/OrderController.kt").unwrap();
    let file_b_code = r#"
package com.example.controller

import com.example.service.processOrder

fun handleRequest() {
    val result = processOrder()
}
"#;

    let file_c_url = Url::parse("file:///project/OrderBatch.kt").unwrap();
    let file_c_code = r#"
package com.example.service

fun runBatch() {
    val batchResult = processOrder()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_c_url.clone(), file_c_code);

    // Cursor on declaration of "processOrder" in File A (line 3, char 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(
        &index,
        &file_a_url,
        "processOrder",
        decl_pos,
        file_a_code,
        false,
    );

    // Should find import in File B, call in File B, and call in File C
    assert_eq!(
        refs.len(),
        3,
        "Should find 3 usages across files (import in File B, call in File B, call in File C)"
    );
    assert!(refs.iter().any(|loc| loc.uri == file_b_url));
    assert!(refs.iter().any(|loc| loc.uri == file_c_url));
}

#[test]
fn test_references_class_name_usages() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_a_code = r#"
package com.example.model

class UserProfile(val id: String)
"#;

    let file_b_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let file_b_code = r#"
package com.example.model

fun inspect(profile: UserProfile) {
    val instance = UserProfile("123")
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on "UserProfile" class declaration in File A (line 3, char 6)
    let class_pos = Position::new(3, 6);
    let refs = find_references(
        &index,
        &file_a_url,
        "UserProfile",
        class_pos,
        file_a_code,
        false,
    );

    assert_eq!(
        refs.len(),
        2,
        "Should find type parameter usage and constructor call in File B"
    );
    assert!(refs.iter().all(|loc| loc.uri == file_b_url));
}

#[test]
fn test_references_class_member_distinguishes_different_classes() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Types.kt").unwrap();
    let file_a_code = r#"
package com.example

class Alpha {
    val value: Int = 1
}

class Beta {
    val value: Int = 2
}
"#;

    let file_b_url = Url::parse("file:///project/Usage.kt").unwrap();
    let file_b_code = r#"
package com.example

fun run(a: Alpha, b: Beta) {
    println(a.value)
    println(b.value)
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on Alpha.value (line 4, char 8)
    let alpha_val_pos = Position::new(4, 8);
    let refs = find_references(
        &index,
        &file_a_url,
        "value",
        alpha_val_pos,
        file_a_code,
        false,
    );

    assert_eq!(
        refs.len(),
        1,
        "Should only find Alpha.value usage, not Beta.value"
    );
    assert_eq!(refs[0].uri, file_b_url);
    assert_eq!(refs[0].range.start.line, 4); // `a.value` is on line 4
}

#[test]
fn test_references_invoked_from_call_site() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Math.kt").unwrap();
    let file_a_code = r#"
package com.example

fun compute(): Int = 10
"#;

    let file_b_url = Url::parse("file:///project/CallSite.kt").unwrap();
    let file_b_code = r#"
package com.example

fun execute() {
    val v = compute()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on compute() at call site in File B (line 4, char 12)
    let call_pos = Position::new(4, 12);
    let refs = find_references(&index, &file_b_url, "compute", call_pos, file_b_code, true);

    assert_eq!(
        refs.len(),
        2,
        "Should find call site in File B and declaration in File A"
    );
    assert!(refs.iter().any(|loc| loc.uri == file_a_url));
    assert!(refs.iter().any(|loc| loc.uri == file_b_url));
}

#[test]
fn test_references_private_function_restricted_to_declaring_file() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/FileA.kt").unwrap();
    let file_a_code = r#"
package com.example

private fun helper(): Int = 1

fun runA() {
    helper()
}
"#;

    let file_b_url = Url::parse("file:///project/FileB.kt").unwrap();
    let file_b_code = r#"
package com.example

private fun helper(): Int = 2

fun runB() {
    helper()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on File A's private helper (line 3, char 12)
    let pos_a = Position::new(3, 12);
    let refs = find_references(&index, &file_a_url, "helper", pos_a, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Private helper should only match within FileA"
    );
    assert_eq!(refs[0].uri, file_a_url);
    assert_eq!(refs[0].range.start.line, 6);
}
