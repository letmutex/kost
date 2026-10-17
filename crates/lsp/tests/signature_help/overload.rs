use lsp::features::find_signature_help;
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, WorkspaceIndex};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{ParameterLabel, Position, Range, SymbolKind, Url};

#[test]
fn test_overloaded_top_level_functions_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/FormatOverload.kt").unwrap();
    let source = r#"
fun format(value: Int): String = "Int: $value"
fun format(value: Double): String = "Double: $value"
fun format(value: String, uppercase: Boolean): String = value

fun test() {
    format(10)
    format("hello", true)
}
"#;

    index.index_source(file_url.clone(), source);

    // 1. Position inside `format(|10)` at line 6, col 11
    let pos_first_call = Position::new(6, 11);
    let help_first = find_signature_help(&index, &file_url, pos_first_call, source)
        .expect("Signature help should be found for format(10)");

    // Should return all 3 overloads
    assert_eq!(help_first.signatures.len(), 3);
    assert_eq!(help_first.active_parameter, Some(0));

    // Verify all 3 overloads are present in signatures
    assert!(
        help_first
            .signatures
            .iter()
            .any(|s| s.label.contains("format(value: Int)"))
    );
    assert!(
        help_first
            .signatures
            .iter()
            .any(|s| s.label.contains("format(value: Double)"))
    );
    assert!(help_first.signatures.iter().any(|s| {
        s.label
            .contains("format(value: String, uppercase: Boolean)")
    }));

    // 2. Position after comma `format("hello", |true)` at line 7, col 20
    let pos_second_call = Position::new(7, 20);
    let help_second = find_signature_help(&index, &file_url, pos_second_call, source)
        .expect("Signature help should be found for format(\"hello\", true)");

    assert_eq!(help_second.signatures.len(), 3);
    assert_eq!(help_second.active_parameter, Some(1));
    let active_sig_idx = help_second.active_signature.unwrap_or(0) as usize;
    assert!(
        help_second.signatures[active_sig_idx]
            .label
            .contains("uppercase: Boolean"),
        "Active signature for 2nd parameter should be the 2-param overload"
    );
}

#[test]
fn test_overloaded_member_functions_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/MemberOverload.kt").unwrap();
    let source = r#"
class Dp

class Layout {
    fun padding(all: Dp) {}
    fun padding(horizontal: Dp, vertical: Dp) {}
}

fun test(l: Layout) {
    l.padding(Dp(), Dp())
}
"#;

    index.index_source(file_url.clone(), source);

    // Position at second parameter in `l.padding(Dp(), |Dp())` at line 9, col 20
    let pos_param2 = Position::new(9, 20);
    let help = find_signature_help(&index, &file_url, pos_param2, source)
        .expect("Signature help should be found for l.padding");

    assert_eq!(
        help.signatures.len(),
        2,
        "Should return both padding member overloads"
    );
    assert_eq!(help.active_parameter, Some(1));

    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx]
            .label
            .contains("horizontal: Dp, vertical: Dp"),
        "Active signature should match the 2-parameter overload"
    );
}

#[test]
fn test_overloaded_extension_functions_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ExtOverload.kt").unwrap();
    let source = r#"
fun String.wrap(tag: String): String = "<$tag>$this</$tag>"
fun String.wrap(openTag: String, closeTag: String): String = "$openTag$this$closeTag"

fun test() {
    "hello".wrap("[", "]")
}
"#;

    index.index_source(file_url.clone(), source);

    let pos = Position::new(5, 22);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for extension wrap");

    assert_eq!(
        help.signatures.len(),
        2,
        "Should return both extension wrap overloads"
    );
    assert_eq!(help.active_parameter, Some(1));

    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    let sig = &help.signatures[active_sig_idx];
    let params = sig.parameters.as_ref().unwrap();
    assert_eq!(params.len(), 2);
    assert_eq!(
        params[0].label,
        ParameterLabel::Simple("openTag: String".to_string())
    );
    assert_eq!(
        params[1].label,
        ParameterLabel::Simple("closeTag: String".to_string())
    );
}

#[test]
fn test_overloaded_partial_named_argument_signature_help() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AreaSigHelp.kt").unwrap();
    let source = r#"
fun area(size: Int): Double = size.toDouble() * size.toDouble()
fun area(width: Int, height: Int): Int = width * height

fun test() {
    area(width = 5)
}
"#;

    index.index_source(file_url.clone(), source);

    let pos = Position::new(5, 14);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for area");

    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.active_parameter, Some(0));
}

#[test]
fn test_signature_help_function_returning_function_type() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/FuncReturn.kt").unwrap();
    let source = r#"
fun getHandler(): () -> Unit = {}
fun getHandler(id: String): () -> Unit = {}

fun test() {
    getHandler()
}
"#;
    index.index_source(file_url.clone(), source);

    let pos = Position::new(5, 15);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for getHandler");

    assert_eq!(help.signatures.len(), 2);
    let no_arg_sig = help
        .signatures
        .iter()
        .find(|s| s.label == "fun getHandler(): () -> Unit")
        .expect("Should contain zero-param overload");
    assert_eq!(
        no_arg_sig.parameters.as_ref().map(|p| p.len()).unwrap_or(0),
        0,
        "Function returning function type should have 0 parameters"
    );
}

#[test]
fn test_signature_help_bytecode_two_overloads_with_one_source_overload() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Format.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Format.kt").unwrap();

    let class_decl1 = DeclarationItem::new(
        "format",
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature("fun format(value: Int): String"),
    );
    let class_decl2 = DeclarationItem::new(
        "format",
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature("fun format(value: Double): String"),
    );

    let src_decl1 = DeclarationItem::new(
        "format",
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::new(
            Some(SmolStr::new("Format Int with source doc")),
            Some(SmolStr::new("fun format(value: Int): String")),
            None,
            false,
        ),
    );

    index.insert_file_symbols(
        src_url,
        FileSymbols {
            package_name: Some("com.example".to_string()),
            declarations: vec![src_decl1],
            ..Default::default()
        },
    );
    index.insert_file_symbols(
        class_url,
        FileSymbols {
            package_name: Some("com.example".to_string()),
            declarations: vec![class_decl1, class_decl2],
            ..Default::default()
        },
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    format(10)\n}\n";
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(2, 11);
    let help = find_signature_help(&index, &app_url, pos, app_code)
        .expect("Signature help should find format");

    // Both overloads (Int and Double) should be available
    assert_eq!(
        help.signatures.len(),
        2,
        "Should retain both Int and Double overloads even when source only defines Int"
    );
}

#[test]
fn test_signature_help_selects_vararg_overload_when_argument_count_exceeds_fixed_arity() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/VarargOverload.kt").unwrap();
    let source = r#"
fun log(tag: String, msg: String): Unit = Unit
fun log(vararg items: Any): Unit = Unit

fun test() {
    log("app", "init", "extra")
}
"#;
    index.index_source(file_url.clone(), source);

    let pos = Position::new(5, 23);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should find log");

    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx].label.contains("vararg"),
        "When active parameter is 2, vararg overload should be selected, but got: {:?}",
        help.signatures[active_sig_idx].label
    );
}

#[test]
fn test_signature_help_selects_matching_named_argument_overload_with_defaults() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AreaDefaultNamedSigHelp.kt").unwrap();
    let source = r#"
fun area(size: Int = 0) {}
fun area(width: Int = 0, height: Int = 0) {}

fun test() {
    area(width = 0)
}
"#;

    index.index_source(file_url.clone(), source);

    let pos = Position::new(5, 14);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for area");

    assert_eq!(help.signatures.len(), 2);
    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx]
            .label
            .contains("width: Int = 0, height: Int = 0"),
        "Active signature should match overload with 'width' parameter, but got: {:?}",
        help.signatures[active_sig_idx].label
    );
}
