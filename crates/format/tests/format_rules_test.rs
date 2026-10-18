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

#[test]
fn test_bracket_annotation_in_type_modifiers_inline() {
    let source = "fun test() {\n    val x: @[A B] Int = 1\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, source);
}

#[test]
fn test_multiline_call_arguments_not_split_into_statements() {
    let source = "fun test() {\n    val x = compute\n        (arg1, arg2)\n}\n";
    let expected = "fun test() {\n    val x = compute(arg1, arg2)\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, expected);
}

#[test]
fn test_nested_call_with_wrapped_arguments_preserves_outer_callee() {
    let source = "fun test() {\n    val x = outer(\n        inner\n            (arg),\n    )\n}\n";
    let expected = "fun test() {\n    val x = outer(\n        inner(arg),\n    )\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, expected);
}

#[test]
fn test_assignment_same_line_block_comment_preserves_inline_space() {
    let source = "fun test() {\n    x = /* comment */ y\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, source);
}

#[test]
fn test_local_variable_field_assignment_with_comment_continuation_indent() {
    let source = "fun test() {\n    field =\n        // comment\n        target == null\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert_eq!(formatted, source);
}
