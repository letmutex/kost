use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::KotlinType;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn destructuring_generic_lambda_param_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DestructureGenericParam.kt").unwrap();
    let code = r#"
package test.destructuring

data class Entry(val key: String, val value: Int)
data class Payload(val success: Boolean)

fun <P, R> process(name: String, action: (P) -> R) {}

fun test() {
    process<Entry, Payload>("handle") { args ->
        val (key) = args
        val x = 1
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 8);
    let inferred = infer_variable_type(&index, &url, "key", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}
