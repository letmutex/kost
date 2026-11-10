use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn infer_class_collision_with_import_alias() {
    let mut index = WorkspaceIndex::new();
    let a_url = Url::parse("file:///project/a/MyClass.kt").unwrap();
    let a_code = r#"
package a

class MyClass
"#;
    index.index_source(a_url, a_code);

    let b_url = Url::parse("file:///project/b/MyClass.kt").unwrap();
    let b_code = r#"
package b

class MyClass
"#;
    index.index_source(b_url, b_code);

    let main_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let main_code = r#"
package app

import a.MyClass as MyClassA
import b.MyClass

fun main() {
    val myClass = MyClass()
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &main_url, "myClass", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("b.MyClass"));
}

#[test]
fn infer_static_function_collision_with_import_alias() {
    let mut index = WorkspaceIndex::new();
    let a_url = Url::parse("file:///project/a/Function.kt").unwrap();
    let a_code = r#"
package a

fun compute(): String = "from a"
"#;
    index.index_source(a_url, a_code);

    let b_url = Url::parse("file:///project/b/Function.kt").unwrap();
    let b_code = r#"
package b

fun compute(): Int = 42
"#;
    index.index_source(b_url, b_code);

    let main_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let main_code = r#"
package app

import a.compute as computeA
import b.compute

fun main() {
    val result = compute()
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &main_url, "result", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn infer_aliased_class_when_colliding_class_also_imported() {
    let mut index = WorkspaceIndex::new();
    let a_url = Url::parse("file:///project/a/MyClass.kt").unwrap();
    index.index_source(
        a_url,
        "package a

class MyClass
",
    );

    let b_url = Url::parse("file:///project/b/MyClass.kt").unwrap();
    index.index_source(
        b_url,
        "package b

class MyClass
",
    );

    let main_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let main_code = "package app

import a.MyClass as MyClassA
import b.MyClass

fun main() {
    val myClassA = MyClassA()
}
";
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &main_url, "myClassA", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("a.MyClass"));
}

#[test]
fn infer_aliased_function_when_colliding_function_also_imported() {
    let mut index = WorkspaceIndex::new();
    let a_url = Url::parse("file:///project/a/Function.kt").unwrap();
    index.index_source(
        a_url,
        r#"package a
fun compute(): String = "from a""#,
    );

    let b_url = Url::parse("file:///project/b/Function.kt").unwrap();
    index.index_source(
        b_url,
        "package b

fun compute(): Int = 42
",
    );

    let main_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let main_code = "package app

import a.compute as computeA
import b.compute

fun main() {
    val resultA = computeA()
}
";
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &main_url, "resultA", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}
