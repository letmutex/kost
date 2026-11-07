use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::KotlinType;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_local_expression_function_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalFunction.kt").unwrap();
    let code = r#"
package test.local

fun <R> runScope(block: () -> R): R = TODO()

fun test() {
    fun localCompute() = runScope { 42 }
    val result = localCompute()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_local_function_references_own_parameter_and_block_body_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalFunctionParam.kt").unwrap();
    let code = r#"
package test.local

fun test() {
    fun localIdentity(input: String) = input
    fun localAction() {
        val temp = 1
    }
    val echoed = localIdentity("hello")
    val done = localAction()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let inferred_echoed = infer_variable_type(&index, &url, "echoed", pos, code, None);
    assert_eq!(inferred_echoed, Some(KotlinType::STRING));

    let inferred_done = infer_variable_type(&index, &url, "done", pos, code, None);
    assert_eq!(inferred_done, Some(KotlinType::UNIT));
}

#[test]
fn test_soft_keyword_expect_and_actual_variable_declarations_indexed() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SoftKeywords.kt").unwrap();
    let code = r#"
package test.local

val actual = "actual_value"
val expect = 99
fun makeRunner(): suspend () -> Int {
    val hidden = 123
    return { hidden }
}

fun test() {
    val fromActual = actual
    val fromExpect = expect
    val runner = makeRunner()
    val x = 1
}
"#;
    let runner_sig = lsp::inference::SignatureInfo::parse(
        "fun makeRunner(): suspend () -> Int { return { 1 } }",
    )
    .expect("should parse function with suspend return type and body");
    assert_eq!(
        runner_sig.return_type,
        Some(KotlinType::function(
            None,
            Vec::new(),
            KotlinType::INT,
            true
        ))
    );
    let actual_sig = lsp::inference::SignatureInfo::parse("val actual: String = \"actual_value\"")
        .expect("should not mask soft keyword identifier actual");
    assert_eq!(actual_sig.return_type, Some(KotlinType::STRING));

    index.index_source(url.clone(), code);

    let pos = Position::new(14, 4);
    assert_eq!(
        infer_variable_type(&index, &url, "fromActual", pos, code, None),
        Some(KotlinType::STRING)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "fromExpect", pos, code, None),
        Some(KotlinType::INT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "runner", pos, code, None),
        Some(KotlinType::function(
            None,
            Vec::new(),
            KotlinType::INT,
            true
        ))
    );
}
