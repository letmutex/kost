use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_private_detection_ignores_top_level_variable_sharing_container_name() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Outer.kt").unwrap();
    let file_a_code = r#"
package com.example

private val Config = "internal"

class Config {
    fun dump() = println("dump")
}
"#;

    let file_b_url = Url::parse("file:///project/Usage.kt").unwrap();
    let file_b_code = r#"
package com.example

fun run(cfg: Config) {
    cfg.dump()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on Config.dump declaration (line 6, col 8)
    let decl_pos = Position::new(6, 8);
    let refs = find_references(&index, &file_a_url, "dump", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Public class Config method should have workspace visibility despite private val Config"
    );
    assert_eq!(refs[0].uri, file_b_url);
}

#[test]
fn test_nested_class_member_visibility_ignores_top_level_private_name_collision() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Outer.kt").unwrap();
    let file_a_code = r#"
package com.example

private class Inner

class Outer {
    class Inner {
        fun doWork() {}
    }
}
"#;

    let file_b_url = Url::parse("file:///project/Usage.kt").unwrap();
    let file_b_code = r#"
package com.example

fun run(inner: Outer.Inner) {
    inner.doWork()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on Outer.Inner.doWork declaration (line 6, col 12)
    let decl_pos = Position::new(6, 12);
    let refs = find_references(&index, &file_a_url, "doWork", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Public Outer.Inner method must be visible in other files despite unrelated private top-level Inner"
    );
    assert_eq!(refs[0].uri, file_b_url);
}

#[test]
fn test_references_excludes_sibling_file_same_name_toplevel_declaration() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example

fun helper(): Unit {}

fun testA() {
    helper()
}
"#;

    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.example

fun helper(): Unit {}

fun testB() {
    helper()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on helper in A.kt (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &file_a_url, "helper", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "References to helper() in A.kt should only include testA's call in A.kt, not testB in B.kt"
    );
    assert_eq!(refs[0].uri, file_a_url);
    assert_eq!(refs[0].range.start.line, 6);
}

#[test]
fn test_references_excludes_import_with_partial_package_name_prefix() {
    let mut index = WorkspaceIndex::new();
    let decl_url = Url::parse("file:///project/Decl.kt").unwrap();
    let decl_code = r#"
package app

class apple {
    class Fruit {
        fun eat(): Unit {}
    }
}
"#;
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_code = r#"
package other

import apple.Fruit.eat

fun test() {
}
"#;
    index.index_source(decl_url.clone(), decl_code);
    index.index_source(consumer_url.clone(), consumer_code);

    // Cursor on eat() declaration: line 5, col 12
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &decl_url, "eat", decl_pos, decl_code, false);

    assert_eq!(
        refs.len(),
        0,
        "Import apple.Fruit.eat should not match app.apple.Fruit.eat"
    );
}

#[test]
fn test_references_excludes_call_with_partial_package_name_prefix() {
    let mut index = WorkspaceIndex::new();
    let decl_url = Url::parse("file:///project/Decl.kt").unwrap();
    let decl_code = r#"
package app

class apple {
    class Fruit {
        fun eat(): Unit {}
    }
}
"#;
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_code = r#"
package other

class apple {
    class Fruit {
        fun eat(): Unit {}
    }
}

fun test(f: apple.Fruit) {
    f.eat()
}
"#;
    index.index_source(decl_url.clone(), decl_code);
    index.index_source(consumer_url.clone(), consumer_code);

    // Cursor on eat() declaration in Decl.kt: line 5, col 12
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &decl_url, "eat", decl_pos, decl_code, false);

    assert_eq!(
        refs.len(),
        0,
        "Call f.eat() where f: other.apple.Fruit should not match app.apple.Fruit.eat"
    );
}

#[test]
fn test_references_excludes_top_level_overload_with_different_custom_class_argument() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TopLevelOverload.kt").unwrap();
    let source = r#"
package com.example

class Order
class User

fun handle(o: Order) {}
fun handle(u: User) {}

fun test(order: Order, user: User) {
    handle(order)
    handle(user)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun handle(u: User)` declaration: line 7, col 4
    let decl_pos = Position::new(7, 4);
    let refs = find_references(&index, &url, "handle", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "References to handle(u: User) should only match handle(user), not handle(order)"
    );
    assert_eq!(refs[0].range.start.line, 11);
}

#[test]
fn test_references_finds_top_level_symbol_in_default_root_package_across_files() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
fun helper(): Unit {}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
fun test() {
    helper()
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on `fun helper()` in A.kt: line 1, col 4
    let decl_pos = Position::new(1, 4);
    let refs = find_references(&index, &file_a_url, "helper", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "helper() in default package should be visible to B.kt in default package"
    );
    assert_eq!(refs[0].uri, file_b_url);
    assert_eq!(refs[0].range.start.line, 2);
}

#[test]
fn test_references_overload_resolution_considers_overload_sharing_range_in_different_file() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = "package com.example\n\nclass Service {\n    fun compute(x: Int = 1, y: Int = 2) = x + y\n}\n";
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = "package com.example\n\nclass Service {\n    fun compute(x: Int) = x\n}\n\nfun runner(s: Service) {\n    s.compute(5)\n}\n";
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on `fun compute(x: Int = 1, y: Int = 2)` in A.kt: line 3, col 8
    let decl_pos = Position::new(3, 8);
    let refs = find_references(&index, &file_a_url, "compute", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        0,
        "s.compute(5) should match exact-arity compute(x: Int) in B.kt, not defaulted compute(x, y) in A.kt"
    );
}

#[test]
fn test_references_overload_resolution_ignores_unrelated_class_same_name_in_different_package() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.a

class Service {
    fun execute(code: Int, retries: Int = 3) {}
}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.b

class Service {
    fun execute(code: Int) {}
}
"#;
    let file_c_url = Url::parse("file:///project/C.kt").unwrap();
    let file_c_code = r#"
package com.a

fun runner(s: Service) {
    s.execute(42)
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_c_url.clone(), file_c_code);

    // Cursor on `fun execute(code: Int, retries: Int = 3)` in com.a.Service (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_a_url, "execute", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "s.execute(42) must match com.a.Service.execute and not be disqualified by unrelated com.b.Service"
    );
    assert_eq!(refs[0].uri, file_c_url);
    assert_eq!(refs[0].range.start.line, 4);
}
