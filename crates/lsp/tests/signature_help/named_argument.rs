use lsp::features::find_signature_help;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_signature_help_non_zero_named_argument_sets_active_parameter() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AreaHeightSigHelp.kt").unwrap();
    let source = r#"
fun area(size: Int = 0) {}
fun area(width: Int = 0, height: Int = 0) {}

fun test() {
    area(height = 0)
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
        "Active signature should match overload with 'height' parameter, but got: {:?}",
        help.signatures[active_sig_idx].label
    );
    assert_eq!(
        help.active_parameter,
        Some(1),
        "Active parameter for 'height' must be index 1, not 0"
    );
}

#[test]
fn test_signature_help_trailing_comma_selects_higher_arity_overload() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/FormatTrailingComma.kt").unwrap();
    let source = r#"
fun format(value: Int): String = "Int: $value"
fun format(value: Int, prefix: String): String = "$prefix$value"

fun test() {
    format(10, )
}
"#;

    index.index_source(file_url.clone(), source);

    // Position after comma: `format(10, |)` at line 5, col 15
    let pos = Position::new(5, 15);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should be found for format(10, )");

    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.active_parameter, Some(1));
    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx]
            .label
            .contains("prefix: String"),
        "When cursor is after comma, active signature should be the 2-param overload, got: {:?}",
        help.signatures[active_sig_idx].label
    );
}

#[test]
fn test_signature_help_call_with_lambda_argument() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/LambdaArgSigHelp.kt").unwrap();
    let source = r#"
fun compute(action: () -> Unit, timeout: Long): Unit {}
fun compute(timeout: Long): Unit {}

fun test() {
    compute({ println("run") }, 1000)
}
"#;

    index.index_source(file_url.clone(), source);

    // Position on second argument `1000`: line 5, col 33
    let pos = Position::new(5, 33);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should find compute with lambda argument");

    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.active_parameter, Some(1));
    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx]
            .label
            .contains("action: () -> Unit, timeout: Long"),
        "Active signature should match 2-param overload with lambda action, got: {:?}",
        help.signatures[active_sig_idx].label
    );
}

#[test]
fn test_signature_help_call_with_lambda_cursor_inside_lambda() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/LambdaArgInsideSigHelp.kt").unwrap();
    let source = r#"
fun compute(action: () -> Unit, timeout: Long): Unit {}
fun compute(action: () -> Unit): Unit {}

fun test() {
    compute({ println("run") }, 1000)
}
"#;

    index.index_source(file_url.clone(), source);

    // Position inside lambda: line 5, col 15
    let pos = Position::new(5, 15);
    let help = find_signature_help(&index, &file_url, pos, source)
        .expect("Signature help should find compute with lambda argument");

    assert_eq!(help.signatures.len(), 2);
    assert_eq!(help.active_parameter, Some(0));
    let active_sig_idx = help.active_signature.unwrap_or(0) as usize;
    assert!(
        help.signatures[active_sig_idx]
            .label
            .contains("action: () -> Unit, timeout: Long"),
        "When cursor is inside lambda, full call arguments including timeout should be parsed to select 2-param overload, got: {:?}",
        help.signatures[active_sig_idx].label
    );
}
