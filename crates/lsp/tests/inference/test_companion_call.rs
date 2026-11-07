use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::KotlinType;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_companion_factory_method_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompanionFactory.kt").unwrap();
    let code = r#"
package test.companion

class Service {
    companion object {
        fun create(): Service = TODO()
    }
}

fun test() {
    val service = Service.create()
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 8);
    let inferred = infer_variable_type(&index, &url, "service", pos, code, None);
    assert_eq!(
        inferred,
        Some(KotlinType::simple_class("test.companion.Service"))
    );
}

#[test]
fn test_non_companion_nested_object_not_resolved_on_outer_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedObject.kt").unwrap();
    let code = r#"
package test.companion

class Outer {
    object Helpers {
        fun util(): Int = 1
    }
    companion object Factory {
        fun build(): Outer = TODO()
    }
}

fun test() {
    val bad = Outer.util()
    val good = Outer.build()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(15, 4);
    assert_eq!(
        infer_variable_type(&index, &url, "bad", pos, code, None),
        None
    );
    assert_eq!(
        infer_variable_type(&index, &url, "good", pos, code, None),
        Some(KotlinType::simple_class("test.companion.Outer"))
    );
}
