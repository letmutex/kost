use lsp::features::find_signature_help;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Documentation, ParameterLabel, Position, Url};

#[test]
fn test_simple_function_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Main.kt").unwrap();
    let source = r#"
fun greet(name: String, age: Int): String {
    return "Hello, $name"
}

fun main() {
    greet("Alice", 30)
}
"#;

    index.index_source(file_url.clone(), source);

    // Position inside `greet(|"Alice", 30)` at line 6, col 10
    let pos_first_param = Position::new(6, 10);
    let help_first = find_signature_help(&index, &file_url, pos_first_param, source)
        .expect("Signature help should be found for greet");

    assert_eq!(help_first.signatures.len(), 1);
    let sig = &help_first.signatures[0];
    assert!(sig.label.contains("greet(name: String, age: Int)"));
    assert_eq!(help_first.active_parameter, Some(0));

    let params = sig.parameters.as_ref().expect("Parameters expected");
    assert_eq!(params.len(), 2);
    assert_eq!(
        params[0].label,
        ParameterLabel::Simple("name: String".to_string())
    );
    assert_eq!(
        params[1].label,
        ParameterLabel::Simple("age: Int".to_string())
    );

    // Position after comma `greet("Alice", |30)` at line 6, col 19
    let pos_second_param = Position::new(6, 19);
    let help_second = find_signature_help(&index, &file_url, pos_second_param, source)
        .expect("Signature help should be found for greet second param");

    assert_eq!(help_second.active_parameter, Some(1));
}

#[test]
fn test_extension_function_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Modifier.kt").unwrap();
    let source = r#"package example

interface Modifier {
    companion object : Modifier {}
}

fun Modifier.padding(start: Int = 0, top: Int = 0, end: Int = 0, bottom: Int = 0): Modifier = this

fun test() {
    Modifier.padding(10, 20, 30)
}
"#;

    index.index_source(file_url.clone(), source);

    // Position after second comma `Modifier.padding(10, 20, |30)` at line 9, col 29
    let pos_third_param = Position::new(9, 29);
    let help = find_signature_help(&index, &file_url, pos_third_param, source)
        .expect("Signature help should be found for extension function");

    assert_eq!(help.signatures.len(), 1);
    assert_eq!(help.active_parameter, Some(2));

    let sig = &help.signatures[0];
    assert!(sig.label.contains("padding"));
    let params = sig.parameters.as_ref().expect("Parameters expected");
    assert_eq!(params.len(), 4);
    assert_eq!(
        params[0].label,
        ParameterLabel::Simple("start: Int = 0".to_string())
    );
    assert_eq!(
        params[1].label,
        ParameterLabel::Simple("top: Int = 0".to_string())
    );
    assert_eq!(
        params[2].label,
        ParameterLabel::Simple("end: Int = 0".to_string())
    );
    assert_eq!(
        params[3].label,
        ParameterLabel::Simple("bottom: Int = 0".to_string())
    );
}

#[test]
fn test_nested_function_call_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Nested.kt").unwrap();
    let source = r#"
fun inner(a: Int, b: String): Int = a
fun outer(x: Int, y: Boolean): Unit {}

fun test() {
    outer(inner(1, "test"), true)
}
"#;

    index.index_source(file_url.clone(), source);

    // Position inside `inner(1, |"test")` at line 5, col 19
    let pos_inner = Position::new(5, 19);
    let help_inner = find_signature_help(&index, &file_url, pos_inner, source)
        .expect("Signature help should be for inner function");

    assert!(help_inner.signatures[0].label.contains("inner"));
    assert_eq!(help_inner.active_parameter, Some(1));

    // Position after `inner(...)` call `outer(inner(...), |true)` at line 5, col 28
    let pos_outer = Position::new(5, 28);
    let help_outer = find_signature_help(&index, &file_url, pos_outer, source)
        .expect("Signature help should be for outer function");

    assert!(help_outer.signatures[0].label.contains("outer"));
    assert_eq!(help_outer.active_parameter, Some(1));
}

#[test]
fn test_kdoc_param_documentation_in_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Doc.kt").unwrap();
    let source = r#"
/**
 * Configures the surface.
 * @param modifier The modifier to apply.
 * @param isOpaque Whether the surface is opaque.
 */
fun configureSurface(modifier: String, isOpaque: Boolean) {}

fun test() {
    configureSurface("mod", true)
}
"#;

    index.index_source(file_url.clone(), source);

    let pos = Position::new(9, 21);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found");

    let sig = &help.signatures[0];
    let params = sig.parameters.as_ref().expect("Parameters expected");
    assert_eq!(params.len(), 2);

    if let Some(Documentation::MarkupContent(ref doc)) = params[0].documentation {
        assert!(doc.value.contains("The modifier to apply"));
    } else {
        panic!("Expected parameter doc for modifier");
    }

    if let Some(Documentation::MarkupContent(ref doc)) = params[1].documentation {
        assert!(doc.value.contains("Whether the surface is opaque"));
    } else {
        panic!("Expected parameter doc for isOpaque");
    }
}

#[test]
fn test_class_constructor_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Constructor.kt").unwrap();
    let source = r#"
class Person(val name: String, val age: Int)

fun test() {
    val p = Person("Bob", 25)
}
"#;

    index.index_source(file_url.clone(), source);

    let pos = Position::new(4, 26);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for class constructor");

    let sig = &help.signatures[0];
    assert!(sig.label.contains("Person"));
    assert_eq!(help.active_parameter, Some(1));
    let params = sig.parameters.as_ref().expect("Parameters expected");
    assert_eq!(params.len(), 2);
}

#[test]
fn test_commas_in_strings_do_not_skew_active_parameter() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/StringCommas.kt").unwrap();
    let source = r#"
fun format(template: String, arg1: Int, arg2: Int): String = template

fun test() {
    format("a, b, c, d", 10, 20)
}
"#;

    index.index_source(file_url.clone(), source);

    let pos_first_arg = Position::new(4, 26);
    let help = find_signature_help(&index, &file_url, pos_first_arg, source)
        .expect("Signature help should be found");

    assert_eq!(help.active_parameter, Some(1));
}

#[test]
fn test_source_function_prioritized_over_bytecode_class() {
    let mut index = WorkspaceIndex::new();
    let class_bytes = include_bytes!("../gradle/resources/AndroidExternalSurfaceKt.class");
    let class_symbols = lsp::gradle::class_file::parse_class_file_symbols(class_bytes)
        .expect("Should parse valid class file");

    let class_url = Url::parse(
        "jar:file:///cache/classes.jar!/androidx/compose/foundation/AndroidExternalSurfaceKt.class",
    )
    .unwrap();
    index.insert_file_symbols(class_url, class_symbols);

    let source_url = Url::parse("file:///project/Surface.kt").unwrap();
    let source_code = r#"package androidx.compose.foundation

fun AndroidExternalSurface(
    customModifier: String = "default",
    isOpaque: Boolean = true
) {
}

fun usage() {
    AndroidExternalSurface("custom", false)
}
"#;

    index.index_source(source_url.clone(), source_code);

    let pos = Position::new(9, 27);
    let help = find_signature_help(&index, &source_url, pos, source_code)
        .expect("Signature help should be found");

    assert_eq!(help.signatures.len(), 1);
    let sig = &help.signatures[0];
    assert!(sig.label.contains("customModifier: String = \"default\""));
    let params = sig.parameters.as_ref().unwrap();
    assert_eq!(
        params[0].label,
        ParameterLabel::Simple("customModifier: String = \"default\"".to_string())
    );
}

#[test]
fn test_signature_help_safe_call_and_instantiation() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Operation.kt").unwrap();
    let source = r#"class Operation(val type: String = "add") {
    fun execute(x: Int, y: Int): Int = TODO()
}

fun main() {
    val opt = Operation()
    opt?.execute()
    Operation().execute()
    Operation()?.execute()
}
"#;
    index.index_source(file_url.clone(), source);

    // 1. opt?.execute() - line 6, col 17
    let pos1 = Position::new(6, 17);
    let help1 = find_signature_help(&index, &file_url, pos1, source);
    assert!(
        help1.is_some(),
        "Signature help should be found for opt?.execute()"
    );
    let help1 = help1.unwrap();
    assert!(
        help1.signatures.iter().any(|s| s.label.contains("execute")),
        "Signature should contain execute"
    );

    // 2. Operation().execute() - line 7, col 24
    let pos2 = Position::new(7, 24);
    let help2 = find_signature_help(&index, &file_url, pos2, source);
    assert!(
        help2.is_some(),
        "Signature help should be found for Operation().execute()"
    );
    let help2 = help2.unwrap();
    assert!(
        help2.signatures.iter().any(|s| s.label.contains("execute")),
        "Signature should contain execute"
    );

    // 3. Operation()?.execute() - line 8, col 25
    let pos3 = Position::new(8, 25);
    let help3 = find_signature_help(&index, &file_url, pos3, source);
    assert!(
        help3.is_some(),
        "Signature help should be found for Operation()?.execute()"
    );
    let help3 = help3.unwrap();
    assert!(
        help3.signatures.iter().any(|s| s.label.contains("execute")),
        "Signature should contain execute"
    );
}
