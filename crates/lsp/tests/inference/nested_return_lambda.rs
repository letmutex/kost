use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::{KotlinType, PrimitiveType};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn nested_lambda_parameter_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedLambda.kt").unwrap();
    let code = r#"
package com.example.test

inline fun <T> provide(action: () -> T): T = action()

fun test() {
    val handler: (String) -> Unit = provide {
        { value ->
            val result = value
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 10);
    let inferred = infer_variable_type(&index, &url, "value", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::Primitive(PrimitiveType::String)));
}
