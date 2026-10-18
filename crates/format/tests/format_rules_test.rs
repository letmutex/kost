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

#[test]
fn test_property_type_wrap_multiline_call_idempotence() {
    let source = "class Foo {\n    companion object {\n        public val DEFAULT_INDENT_CONFIG: IndentConfig = IndentConfig(\n            indentStyle = INDENT_STYLE_PROPERTY.defaultValue,\n            tabWidth = INDENT_SIZE_PROPERTY.defaultValue,\n        )\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(
        pass1, source,
        "formatting output must match expected layout"
    );
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(pass1, pass2, "formatting must be idempotent across passes");
}

#[test]
fn test_expression_body_function_header_break_idempotence() {
    let source = "class Repo {\n    override fun observeAllBookmarked(): Flow<List<UserNewsResource>> =\n        userDataRepository.userData.map { it.bookmarkedNewsResources }.distinctUntilChanged()\n            .flatMapLatest { flowOf(emptyList()) }\n}\n";
    let expected = "class Repo {\n    override fun observeAllBookmarked(): Flow<List<UserNewsResource>> = userDataRepository.userData\n        .map { it.bookmarkedNewsResources }\n        .distinctUntilChanged()\n        .flatMapLatest { flowOf(emptyList()) }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(
        pass1, expected,
        "formatting output must match expected layout"
    );
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function formatting must be idempotent across passes"
    );
}

#[test]
fn test_property_initializer_multiline_call_break_idempotence() {
    let source = "fun test() {\n    LaunchedEffect(Unit) {\n        snapshotFlow { offset }.collect {\n            val currentTravel = track.thumbPosition(\n                dimension = orientation.valueOf(draggedOffset),\n            )\n        }\n    }\n}\n";
    let expected = "fun test() {\n    LaunchedEffect(Unit) {\n        snapshotFlow { offset }\n            .collect {\n                val currentTravel = track.thumbPosition(\n                    dimension = orientation.valueOf(draggedOffset),\n                )\n            }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(
        pass1, expected,
        "formatting output must match expected layout"
    );
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property initializer formatting must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_chain_root_call_with_arguments_preserves_opening_line() {
    let source = "class Service {\n    fun getUserStream(id: String): Flow<User> =\n        repo.getUser(id).map { it.toModel() }\n            .filter { it.isValid() }\n}\n";
    let expected = "class Service {\n    fun getUserStream(id: String): Flow<User> = repo\n        .getUser(id)\n        .map { it.toModel() }\n        .filter { it.isValid() }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(pass1, pass2);
}

#[test]
fn test_property_initializer_call_exceeding_max_width_wraps() {
    let source = "val veryLongDescriptiveVariableName: SomeType = calculateSomething(firstParameterWithLongName, secondParameterWithLongName)\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(pass1, pass2);
    assert!(pass1.lines().all(|l| l.len() <= 100));
}
