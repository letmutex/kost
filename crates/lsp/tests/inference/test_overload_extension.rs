use lsp::indexer::WorkspaceIndex;
use lsp::inference::types::KotlinType;
use lsp::inference::variable::infer_variable_type;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_extension_overload_bounded_receiver_specificity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionFormat.kt").unwrap();
    let code = r#"
fun <T> T.format(action: () -> Unit): Int = 0
fun <T : CharSequence> T.format(action: () -> Unit): String = ""

val formatted = "hello".format { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "formatted", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_infer_higher_order_call_rejects_missing_preceding_required_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionMissingParam.kt").unwrap();
    let code = r#"
class Text
fun <T> T.process(requiredHeader: String, action: () -> Unit): Int = 0

val result = Text().process { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, None);
}

#[test]
fn test_infer_higher_order_call_unit_coercion_for_functional_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionUnitCoercion.kt").unwrap();
    let code = r#"
class Text
class Block
fun <T> T.consume(action: () -> Unit): String = ""

val result = Text().consume { Block() }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_infer_higher_order_call_default_unit_when_return_type_omitted() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionOmittedReturn.kt").unwrap();
    let code = r#"
class Text
fun <T> T.action(block: () -> Unit) { }

val res = Text().action { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::UNIT));
}
