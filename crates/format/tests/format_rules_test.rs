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
fn test_nested_call_with_wrapped_arguments_breaks_after_equals() {
    let source = "fun test() {\n    val x = outer(\n        inner\n            (arg),\n    )\n}\n";
    let expected =
        "fun test() {\n    val x =\n        outer(\n            inner(arg),\n        )\n}\n";
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
    let expected = "class Foo {\n    companion object {\n        public val DEFAULT_INDENT_CONFIG: IndentConfig =\n            IndentConfig(\n                indentStyle = INDENT_STYLE_PROPERTY.defaultValue,\n                tabWidth = INDENT_SIZE_PROPERTY.defaultValue,\n            )\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(
        pass1, expected,
        "formatting output must match expected layout"
    );
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(pass1, pass2, "formatting must be idempotent across passes");
}

#[test]
fn test_expression_body_function_header_break_idempotence() {
    let source = "class Repo {\n    override fun observeAllBookmarked(): Flow<List<UserNewsResource>> =\n        userDataRepository.userData.map { it.bookmarkedNewsResources }.distinctUntilChanged()\n            .flatMapLatest { flowOf(emptyList()) }\n}\n";
    let expected = "class Repo {\n    override fun observeAllBookmarked(): Flow<List<UserNewsResource>> =\n        userDataRepository.userData\n            .map { it.bookmarkedNewsResources }\n            .distinctUntilChanged()\n            .flatMapLatest { flowOf(emptyList()) }\n}\n";
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
    let expected = "fun test() {\n    LaunchedEffect(Unit) {\n        snapshotFlow { offset }\n            .collect {\n                val currentTravel =\n                    track.thumbPosition(\n                        dimension = orientation.valueOf(draggedOffset),\n                    )\n            }\n    }\n}\n";
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
    let expected = "class Service {\n    fun getUserStream(id: String): Flow<User> =\n        repo\n            .getUser(id)\n            .map { it.toModel() }\n            .filter { it.isValid() }\n}\n";
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

#[test]
fn test_property_delegate_multiline_call_break_idempotence() {
    let source = "class Nav {\n    private val settings by\n        composeTestRule.stringResource(SettingsR.string.feature_settings_impl_top_app_bar_action_icon_description)\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property delegate formatting must be idempotent across passes"
    );
}

#[test]
fn test_property_initializer_multiline_call_wrap_idempotence() {
    let source = "class Formatter {\n    fun format(): String {\n        val javaOutput =\n            JavaOutput(lineSeparator, kotlinInput, KDocCommentsHelper(lineSeparator, options.maxWidth))\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property initializer formatting must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_wrapped_parameter_list_break_idempotence() {
    let source = "class ReporterAggregator {\n    private fun List<String>.configurationOption(reporterConfigurationElement: ReporterConfigurationElement) =\n        lastOrNull { it.startsWith(\"${reporterConfigurationElement.name.lowercase()}=\") }\n            ?.substringAfter(\"=\")\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function with wrapped parameters must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_chained_call_return_break_idempotence() {
    let source = "class IdNamingPolicy {\n    fun check(policy: Policy): Result =\n        require(condition) { \"Error message\" }\n            .let { Result.Success }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function with chained call must be idempotent across passes"
    );
}

#[test]
fn test_nested_property_initializer_chained_call_break_idempotence() {
    let source = "class Repo {\n    fun test() {\n        run {\n            run {\n                if (cond) {\n                    val addedNewsResources = newsResourceDao\n                        .getNewsResources(\n                            useFilterTopicIds = true,\n                            filterTopicIds = followedTopicIds,\n                            useFilterNewsIds = true,\n                            filterNewsIds = changedIds.toSet() - existingNewsResourceIdsThatHaveChanged,\n                        )\n                        .first()\n                        .map(PopulatedNewsResource::asExternalModel)\n                }\n            }\n        }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "nested property initializer chained call must be idempotent across passes"
    );
}

#[test]
fn test_raw_string_trim_indent_attached_to_closing_quotes() {
    let source = "fun test() {\n    val x = \"\"\"\n        hello\n    \"\"\".trimIndent()\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert!(
        formatted.contains("\"\"\".trimIndent()"),
        "trimIndent should stay attached to triple-quotes, got:\n{}",
        formatted
    );
}

#[test]
fn test_trailing_line_comment_wrap_idempotence() {
    let source = "class Outer {\n    class Inner {\n        fun test() {\n            if (cond) {\n                if (semi) {\n                    return // Never add a trailing comma after there is already a terminating semicolon\n                }\n            }\n        }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "wrapped trailing line comment must be idempotent across passes"
    );
}

#[test]
fn test_enclosing_block_line_comment_wrap_idempotence() {
    let source = "class Formatter {\nfun sortedImports() {\n    // Find non-import elements; comments are moved, in order, to the top of the import list. Other\n}\n}\n";
    let expected = "class Formatter {\n    fun sortedImports() {\n        // Find non-import elements; comments are moved, in order, to the top of the import list.\n        // Other\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "line comment wrapping inside re-indented block must be idempotent across passes"
    );
}

#[test]
fn test_mismatched_unformatted_line_comment_continuation_indent_idempotence() {
    let source = "fun test() {\n    if (true) {\n        if (true) {\n                // writeText effective replaces the entire content if the file already exists. If appendText is used, the file keeps on growing\n                writeText()\n        }\n    }\n}\n";
    let expected = "fun test() {\n    if (true) {\n        if (true) {\n            // writeText effective replaces the entire content if the file already exists. If\n            // appendText is used, the file keeps on growing\n            writeText()\n        }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "wrapped line comment continuation indent from unformatted offset must be idempotent"
    );
}

#[test]
fn test_binary_expression_wrapped_line_comment_blank_line_idempotence() {
    let source = "fun test() {\n    val length = actualClassSignatureLength +\n        // Calculate the white space correction in case the signature would be rewritten to a single line\n        fixWhiteSpaces()\n}\n";
    let expected = "fun test() {\n    val length =\n        actualClassSignatureLength +\n            // Calculate the white space correction in case the signature would be rewritten to a\n            // single line\n            fixWhiteSpaces()\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "wrapped comment in binary expression must not introduce blank lines across passes"
    );
}

#[test]
fn test_property_initializer_multiline_call_chain_break_idempotence() {
    let source = "class C {\n    fun f() {\n        if (a) {\n            if (b) {\n                if (c) {\n                    val addedNewsResources = newsResourceDao.getNewsResources(\n                        useFilterTopicIds = true,\n                        filterTopicIds = followedTopicIds,\n                        useFilterNewsIds = true,\n                        filterNewsIds = changedIds.toSet() - existingNewsResourceIdsThatHaveChanged,\n                    )\n                        .first()\n                        .map(PopulatedNewsResource::asExternalModel)\n                }\n            }\n        }\n    }\n}\n";
    let expected = "class C {\n    fun f() {\n        if (a) {\n            if (b) {\n                if (c) {\n                    val addedNewsResources =\n                        newsResourceDao.getNewsResources(\n                            useFilterTopicIds = true,\n                            filterTopicIds = followedTopicIds,\n                            useFilterNewsIds = true,\n                            filterNewsIds =\n                                changedIds.toSet() - existingNewsResourceIdsThatHaveChanged,\n                        )\n                            .first()\n                            .map(PopulatedNewsResource::asExternalModel)\n                }\n            }\n        }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property initializer with multiline call chain must be idempotent across passes"
    );
}

#[test]
fn test_trailing_line_comment_multiline_statement_wrap_idempotence() {
    let source = "fun test() {\n    val pathSeparator =\n        \";\" // instead of File.pathSeparator as would have been reasonable (e.g. : on Unix)\n}\n";
    let expected = "fun test() {\n    val pathSeparator = \";\" // instead of File.pathSeparator as would have been reasonable (e.g. :\n    // on Unix)\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "trailing line comment on multiline statement must be idempotent across passes"
    );
}

#[test]
fn test_binary_expression_operator_trailing_comment_continuation_indent() {
    let source = "fun test() {\n    val x = a + // comment\n        b\n}\n";
    let expected = "fun test() {\n    val x =\n        a + // comment\n            b\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "trailing line comment on operator must maintain continuation indent across passes"
    );
}

#[test]
fn test_modified_property_trailing_comment_wrap_idempotence() {
    let word50 = "a".repeat(50);
    let words = format!("{} 12345678901", word50); // 50 + 1 + 11 = 62 chars
    let source = format!(
        "class C {{\n    private val pathSeparator =\n        \";\" // {}\n}}\n",
        words
    );
    let expected = format!(
        "class C {{\n    private val pathSeparator = \";\" // {}\n    // 12345678901\n}}\n",
        word50
    );
    let pass1 = format_default(&source).expect("formatting pass 1 should succeed");
    assert_eq!(pass1, expected);
    for line in pass1.lines() {
        assert!(
            line.len() <= 100,
            "line length {} exceeded max_width 100: '{}'",
            line.len(),
            line
        );
    }
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "modified property with trailing line comment must be idempotent across passes"
    );
}
