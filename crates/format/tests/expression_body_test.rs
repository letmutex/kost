use format::format_default;

#[test]
fn test_expression_body_multiline_call_arguments_preserves_inline_header() {
    let source =
        "fun transform(data: String): Result = processData(\n    param1,\n    param2,\n)\n";
    let formatted = format_default(source).expect("formatting should succeed");
    let expected =
        "fun transform(data: String): Result = processData(\n    param1,\n    param2,\n)\n";
    assert_eq!(
        formatted, expected,
        "single-line function header with multiline call arguments must preserve call on same line as equals"
    );
    let pass2 = format_default(&formatted).expect("pass 2 should succeed");
    assert_eq!(
        formatted, pass2,
        "expression body with multiline call arguments must be idempotent"
    );
}

#[test]
fn test_destructuring_declaration_chained_call_breaks_after_equals() {
    let source = "fun test() {\n    val (firstEntry, secondEntry) = list.filter { it.hasSomeLongCondition() }.map { it.toSomeOtherTransformedEntry() }\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    let expected = "fun test() {\n    val (firstEntry, secondEntry) =\n        list\n            .filter { it.hasSomeLongCondition() }\n            .map { it.toSomeOtherTransformedEntry() }\n}\n";
    assert_eq!(
        formatted, expected,
        "destructuring declaration chained call exceeding max width must break after equals with continuation indent"
    );
    let pass2 = format_default(&formatted).expect("pass 2 should succeed");
    assert_eq!(
        formatted, pass2,
        "destructuring declaration chained call must be idempotent"
    );
}

#[test]
fn test_chained_call_opening_call_with_receiver_multiline_args_preserves_inline_header() {
    let source = "class C {\n    val myProperty = receiver.fetch(\n        param1,\n        param2,\n    ).process()\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    let expected = "class C {\n    val myProperty = receiver.fetch(\n        param1,\n        param2,\n    )\n        .process()\n}\n";
    assert_eq!(
        formatted, expected,
        "property initializer with receiver-qualified opening call having multiline arguments must keep opening call on same line"
    );
    let pass2 = format_default(&formatted).expect("pass 2 should succeed");
    assert_eq!(
        formatted, pass2,
        "receiver-qualified opening call formatting must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_in_init_block_target_indent() {
    let source = "class C {\n    init {\n        fun local(): VeryLongReturnTypeNameThatForcesNewline =\n            someExpressionThatIsAlsoVeryLongAndForcesContinuationIndent()\n    }\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    let expected = "class C {\n    init {\n        fun local(): VeryLongReturnTypeNameThatForcesNewline =\n            someExpressionThatIsAlsoVeryLongAndForcesContinuationIndent()\n    }\n}\n";
    assert_eq!(
        formatted, expected,
        "expression body in init block should have 8 spaces for function and 12 spaces for continuation body"
    );
    let pass2 = format_default(&formatted).expect("pass 2 should succeed");
    assert_eq!(
        formatted, pass2,
        "expression body in init block must be idempotent"
    );
}

#[test]
fn test_chained_call_interspersed_comment_recognizes_broken_chain() {
    let source = "fun test() {\n    receiver // comment\n        .action()\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    let expected = "fun test() {\n    receiver\n        // comment\n        .action()\n}\n";
    assert_eq!(formatted, expected);
    let pass2 = format_default(&formatted).expect("pass 2 should succeed");
    assert_eq!(formatted, pass2);
}

#[test]
fn test_broken_chain_with_multiline_args_and_trailing_lambda_single_continuation_indent() {
    let options = format::FormattingOptions::google();
    let source = "fun test() {\n  receiver\n    .fetch(\n      param1,\n      param2,\n    ) { result ->\n      process(result)\n    }\n}\n";
    let formatted = format::format(source, &options).expect("formatting should succeed");
    let expected = "fun test() {\n  receiver\n    .fetch(\n      param1,\n      param2,\n    ) { result ->\n    process(result)\n  }\n}\n";
    assert_eq!(formatted, expected);
}
