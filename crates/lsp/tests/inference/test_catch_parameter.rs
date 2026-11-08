use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::KotlinType;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_catch_parameter_member_property_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CatchParamProperty.kt").unwrap();
    let code = r#"
package test.general

open class BaseError(open val message: String? = null)
class CustomError(override val message: String? = null) : BaseError(message)

fun consume(text: String?) {}

fun test(action: () -> Unit) {
    try {
        action()
    } catch (e: CustomError) {
        consume(e.message)
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(12, 18);
    let inferred = infer_variable_type(&index, &url, "message", pos, code, None);
    let expected = KotlinType::STRING.make_nullable();
    assert_eq!(inferred, Some(expected));
}
