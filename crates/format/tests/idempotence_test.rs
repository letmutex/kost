use format::format_default;

#[test]
fn test_jump_expression_trailing_line_comment_wrap_idempotence() {
    let source = "class A {\n  class B {\n    class C {\n      class D {\n        fun test() {\n          if (c) {\n            return // Never add a trailing comma after there is already a terminating semicolon\n          }\n        }\n      }\n    }\n  }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "return statement with trailing line comment must be idempotent across passes"
    );
}

#[test]
fn test_property_initializer_wrapped_arguments_call_idempotence() {
    let source = "class A {\n  class B {\n    fun test() {\n      val table =\n          Table(columns, widths, rows, align, lines.subList(start, i).map { lineContent(it) })\n    }\n  }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property initializer with wrapped call arguments must be idempotent across passes"
    );
}

#[test]
fn test_assignment_wrapped_arguments_call_idempotence() {
    let source = "fun test() {\n    outputFile =\n        layout.buildDirectory.file(\"generated/main/kotlin/org/jetbrains/kotlinx/ktfmt/util/Ktfmt.kt\")\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "assignment with wrapped call arguments must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_multiline_receiver_chain_break_idempotence() {
    let source = "class C {\n    private fun String.copyResourceToFileIn(tempDir: Path) =\n        Paths\n            .get(\"$TEST_RESOURCE_PATH/$this\")\n            .copyTo(tempDir, overwrite = true)\n            .pathString\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function with multiline receiver chain must be idempotent across passes"
    );
}

#[test]
fn test_property_delegate_wrapped_arguments_chain_break_idempotence() {
    let source = "class C {\n    private val ignoreAutocorrectFailures: Boolean by\n        option(\"--ignore-autocorrect-failures\", help = \"Ignore all violations for which no autocorrect is available\")\n            .flag(default = false)\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property delegate with wrapped call arguments chain must be idempotent across passes"
    );
}

#[test]
fn test_elvis_expression_trailing_line_comment_wrap_idempotence() {
    let source = "class A {\n  fun test() {\n    val inspectNode =\n      node\n        .children\n        .lastOrNull { it.elementType == ARROW } // lambda w/o an arrow -> no arguments -> no commas\n        ?: return\n  }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "elvis expression with trailing line comment must be idempotent across passes"
    );
}

#[test]
fn test_as_cast_statement_followed_by_parenthesized_expression_separation() {
    let source = "fun test() {\n    val insertionPoint = prevLeaf.prevCodeLeaf as LeafPsiElement\n    (node as LeafPsiElement).remove()\n}\n";
    let formatted = format_default(source).expect("formatting should succeed");
    assert!(
        !formatted.contains("LeafPsiElement("),
        "as cast statement followed by parenthesized expression must remain separate statements, got:\n{}",
        formatted
    );
    let pass2 = format_default(&formatted).expect("formatting pass 2 should succeed");
    assert_eq!(
        formatted, pass2,
        "as cast statement followed by parenthesized expression must be idempotent across passes"
    );
}

#[test]
fn test_chained_call_multiline_arguments_indentation_idempotence() {
    let source = "class DokkaVerifier {\n  fun verify() {\n    val path =\n        listOf(analysis, base, compiler, intellij, coroutines, html, freemarker).joinToString(\n            pathSeparator,\n        ) {\n          it.path\n        }\n  }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "argument indentation inside multiline chained call must be idempotent across passes"
    );
}

#[test]
fn test_class_header_contiguous_line_comment_wrap_idempotence() {
    let source = "public class KtlintSuppressionRule(\n    private val allowedRuleIds: List<RuleId>,\n) : InternalRule(\"ktlint-suppression\"),\n    // The SuppressionLocatorBuilder no longer support the old ktlint suppression directives using comments. This rule may not be disabled\n    // in any way as it would fail to process suppressions.\n    IgnoreKtlintSuppressions {\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "wrapped multi-line comments in class header must not insert blank lines across passes"
    );
}

#[test]
fn test_property_initializer_comment_continuation_indent_idempotence() {
    let source = "class C {\n    companion object {\n        private val WAIT_INTERVAL_MAX_OCCURRENCES =\n            // The local machine is most often faster than the GitHub CICD when running the CLI tests. A system environment property has\n            // been defined in the gradle-pr-build workflow to increase the timeout.\n            System\n                .getenv(\"CLI_TEST_MAX_DURATION_IN_SECONDS\")\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "continuation line of wrapped comment in property initializer must not shift indentation across passes"
    );
}

#[test]
fn test_elvis_operator_trailing_comment_wrap_idempotence() {
    let source = "class C {\n    private fun visitFunctionLiteral() {\n        val inspectNode =\n            node\n                .children\n                .lastOrNull { it.elementType == ARROW }\n                ?: // lambda w/o an arrow -> no arguments -> no commas\n                return\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "trailing comment on elvis operator must wrap and be idempotent across passes"
    );
}

#[test]
fn test_trailing_lambda_multiline_call_arguments_indentation_idempotence() {
    let source = "val archive =\n    tasks.register(\"nativeImageArchive\", if (currentOs == Os.WINDOWS) Zip::class else Tar::class) {\n        description = \"Packs the native image distribution...\"\n    }\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "trailing lambda body indentation on multiline call must be idempotent across passes"
    );
}

#[test]
fn test_expression_body_multiline_call_assignment_break_idempotence() {
    let source = "class C {\n    private fun ktLintRuleExecutionEditorConfigProperty(\n        ruleId: RuleId,\n        ruleExecution: RuleExecution,\n    ) = ktLintRuleExecutionEditorConfigProperty(ruleId.createRuleExecutionEditorConfigProperty().name, ruleExecution)\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function header assignment break must be idempotent across passes"
    );
}

#[test]
fn test_destructuring_declaration_chained_call_break_idempotence() {
    let source = "class C {\n    fun test() {\n        if (condition) {\n            if (anotherCondition) {\n                val (actualIndentString, actualContentString) = it.text.splitIndentAt(it.text.length)\n            }\n        }\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "destructuring declaration chained call break must be idempotent across passes"
    );
}

#[test]
fn test_property_initializer_chained_multiline_call_break_idempotence() {
    let source = "class C {\n    fun test() {\n        val actual =\n            ImportOrderingRule.IJ_KOTLIN_IMPORTS_LAYOUT_PROPERTY.propertyWriter(\n                listOf(PatternEntry.ALL_OTHER_IMPORTS_ENTRY, PatternEntry(\"java\", true, false)),\n            )\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "property initializer chained multiline call break must be idempotent across passes"
    );
}

#[test]
fn test_catch_parameter_wrapped_comment_continuation_indent_idempotence() {
    let source = "fun test() {\n    try {\n    } catch (\n        // e itself (the internal-only RuleExecutionException wrapper) is intentionally not chained; e.cause below is\n        // the real original exception, and chaining e instead would only add a meaningless\n        e: Exception\n    ) {\n    }\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "catch parameter wrapped comments must not shift indentation across passes"
    );
}

#[test]
fn test_expression_body_multiline_opening_chain_break_idempotence() {
    let source = "class C {\n    private fun ASTNode.getFunctionBody(splitNode: ASTNode?): List<ASTNode> =\n        this\n            .collectLeavesRecursively()\n            .childrenBetween(\n                startASTNodePredicate = { it == splitNode },\n                endASTNodePredicate = {\n                    false\n                },\n            )\n}\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "expression body function with multiline opening chain must be idempotent across passes"
    );
}

#[test]
fn test_class_annotation_trailing_line_comment_idempotence() {
    let source = "@OkHttpInternalApi\n@OptIn(ExperimentalTime::class) // We know Clock and Instant will be stable in Kotlin 2.3.\nclass RealDnsCache\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "class annotation with trailing line comment must be idempotent across passes"
    );
    assert!(
        pass1.contains("@OptIn(ExperimentalTime::class) // We know Clock and Instant will be stable in Kotlin 2.3.\nclass RealDnsCache"),
        "trailing line comment must remain attached to annotation on same line: actual:\n{pass1}"
    );
}

#[test]
fn test_simple_receiver_call_multiline_arguments_idempotence() {
    let source = "public val CSRF =\n    createRouteScopedPlugin(\"CSRF\", ::CSRFConfig) {\n        if (!checkHost && allowedOrigins.isEmpty() && headerChecks.isEmpty()) {\n            application.log.info(\"CSRF configuration is empty; defaulting to allow only local origins\")\n        }\n    }\n";
    let pass1 = format_default(source).expect("formatting pass 1 should succeed");
    let pass2 = format_default(&pass1).expect("formatting pass 2 should succeed");
    assert_eq!(
        pass1, pass2,
        "simple receiver call with multiline arguments must not oscillate between passes"
    );
    assert!(
        pass1.contains("            application.log.info(\n                \"CSRF configuration is empty; defaulting to allow only local origins\",\n            )"),
        "call on simple receiver must stay on single line with properly indented arguments: actual:\n{pass1}"
    );
}

#[test]
fn test_destructuring_declaration_lambda_call_idempotence() {
    let source = "class C {\n    private suspend fun ByteReadChannel.scanHeaders() = Headers.build {\n        do {\n            val line = readLineStrict()\n            if (line.isNullOrBlank()) break\n\n            val (header, value) = line.chomp(\":\") { throw IOException(\"Illegal header line $line\") }\n            append(header.trimEnd(), value.trimStart())\n        } while (true)\n    }\n}\n";
    let pass1 = format_default(source).expect("pass 1");
    let pass2 = format_default(&pass1).expect("pass 2");
    assert_eq!(
        pass1, pass2,
        "destructuring declaration with lambda call must not oscillate"
    );
}
