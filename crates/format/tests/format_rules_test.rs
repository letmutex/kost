use format::format_default;

#[test]
fn test_named_argument_trailing_comment_closing_reserve() {
    let source = "fun test() {\n    outer(inner(key = 123 /* comment */))\n}\n";
    let expected = "fun test() {\n    outer(\n        inner(\n            key = 123, /* comment */\n        ),\n    )\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, expected);
}

#[test]
fn test_unparenthesized_elvis_chain_root_ast_lowering() {
    let source =
        "fun test() {\n    val result = primary?.action() ?: fallback.action() ?: default\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, source);
}

#[test]
fn test_semicolon_inside_string_and_comment_preservation() {
    let source = "fun test() {\n    val text = \"hello; world;\"\n    // comment with; semicolons;\n    println(text)\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, source);
}

#[test]
fn test_redundant_semicolon_elimination() {
    let source = "fun test() {\n    val x = 1;\n    println(x);\n}\n";
    let expected = "fun test() {\n    val x = 1\n    println(x)\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, expected);
}
