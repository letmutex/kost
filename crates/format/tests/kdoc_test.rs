use format::format_default;

#[test]
fn test_empty_kdoc_comment_formatting_does_not_panic() {
    let source = "val x = /**/ 1\n";
    let formatted = format_default(source);
    assert!(
        formatted.is_ok(),
        "formatting empty kdoc comment /**/ must not panic"
    );
}
