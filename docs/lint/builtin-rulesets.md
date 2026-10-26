# Built-in Rulesets

Kost includes two built-in virtual rulesets that can be enabled via the `--rulesets` flag or included in `kost.yml`:

- `standard`: 89 compiled AST rules compatible with [ktlint](https://github.com/ktlint/ktlint) conventions.
- `statistics`: Configurable code complexity and metric threshold rules.

## The `standard` Ruleset

The `standard` ruleset is enabled by default. It covers syntax formatting, idiomatic Kotlin practices, spacing, naming, and clean code conventions.

### Naming Conventions

| Rule ID | Description |
| --- | --- |
| `standard:filename` | Top-level file name should match the primary class or interface declared within it. |
| `standard:package-name` | Package names should be lowercase alphanumeric without underscores or uppercase characters. |
| `standard:class-naming` | Classes, objects, and interfaces must use PascalCase. |
| `standard:function-naming` | Function names must use camelCase, unless enclosed in backticks for test methods. |
| `standard:property-naming` | Properties and local variables must use camelCase; compile-time constants use SCREAMING_SNAKE_CASE. |
| `standard:backing-property-naming` | Backing properties may use a leading underscore matching their public property. |
| `standard:enum-entry-name-case` | Enum entries must use either SCREAMING_SNAKE_CASE or PascalCase consistently. |

### Spacing and Operators

| Rule ID | Description |
| --- | --- |
| `standard:spacing-around-angle-brackets` | Enforces proper spacing around type parameter angle brackets `<` and `>`. |
| `standard:colon-spacing` | No space before colon in type annotations; single space after; space before and after in inheritance. |
| `standard:comma-spacing` | No space before comma; single space after. |
| `standard:curly-spacing` | Requires a single space before opening curly braces `{`. |
| `standard:dot-spacing` | No spaces allowed around member access dots `.`. |
| `standard:double-colon-spacing` | No spaces around reflection double colons `::`. |
| `standard:keyword-spacing` | Ensures proper spacing around control flow keywords (`if`, `when`, `for`, `while`, `catch`). |
| `standard:op-spacing` | Requires consistent spacing around binary and assignment operators (`+`, `-`, `=`, `&&`). |
| `standard:paren-spacing` | No whitespace immediately following `(` or preceding `)`. |
| `standard:range-spacing` | No whitespace around range operators (`..` and `..<`). |
| `standard:square-brackets-spacing` | No unnecessary spaces inside square brackets `[` and `]`. |
| `standard:unary-op-spacing` | No space between unary operators (`!`, `++`, `--`) and their operands. |
| `standard:spacing-between-function-name-and-opening-parenthesis` | No space between function name and argument list `(`. |
| `standard:fun-keyword-spacing` | Requires a single space immediately after the `fun` keyword. |
| `standard:function-return-type-spacing` | Proper spacing around the return type colon. |
| `standard:function-start-of-body-spacing` | Single space between function declaration and body `{` or `=`. |
| `standard:function-type-modifier-spacing` | Spacing around function type modifiers (such as `suspend`). |
| `standard:function-type-reference-spacing` | Spacing within function type references `(A) -> B`. |
| `standard:modifier-list-spacing` | Single space separating modifiers on declarations. |
| `standard:nullable-type-spacing` | No space before the `?` nullable type marker. |
| `standard:parameter-list-spacing` | Spacing inside function and constructor parameter lists. |
| `standard:then-spacing` | Single space before `then` in `if` expressions. |
| `standard:try-catch-finally-spacing` | Spacing between `try`, `catch`, and `finally` blocks. |
| `standard:type-argument-list-spacing` | Consistent spacing within type argument lists `<T, R>`. |
| `standard:type-parameter-list-spacing` | Consistent spacing within type parameter declarations `<T : Any>`. |
| `standard:no-multi-spaces` | Disallows multiple consecutive space characters. |
| `standard:no-trailing-spaces` | Disallows trailing whitespace at the end of lines. |
| `standard:annotation-spacing` | Spacing around annotations on declarations. |
| `standard:spacing-between-declarations-with-annotations` | Blank lines and spacing between declarations bearing annotations. |
| `standard:spacing-between-declarations-with-comments` | Spacing between declarations preceded by comments. |

### Blank Lines and Layout

| Rule ID | Description |
| --- | --- |
| `standard:blank-line-before-package` | Enforces required blank line layout before the `package` header. |
| `standard:blank-line-before-imports` | Requires a blank line preceding the import list. |
| `standard:blank-line-before-file-annotation` | Layout rules for file-level `@file:...` annotations. |
| `standard:blank-line-before-declaration` | Blank line separation between top-level declarations. |
| `standard:blank-line-between-when-conditions` | Blank lines separating complex multi-branch `when` cases. |
| `standard:no-blank-line-at-start-of-file` | Disallows empty blank lines at the beginning of source files. |
| `standard:no-blank-line-before-rbrace` | Disallows blank lines immediately before closing curly braces `}`. |
| `standard:no-blank-line-in-list` | Disallows blank lines inside argument and parameter lists. |
| `standard:no-blank-lines-in-chained-method-calls` | Disallows blank lines within chained method call pipelines. |
| `standard:no-consecutive-blank-lines` | Disallows two or more consecutive blank lines anywhere in the file. |
| `standard:no-empty-first-line-in-class-body` | Disallows blank lines as the first line of a class body. |
| `standard:no-empty-first-line-in-method-block` | Disallows blank lines as the first line of a function body. |

### Line Wrapping and Code Structure

| Rule ID | Description |
| --- | --- |
| `standard:argument-list-wrapping` | Enforces multiline wrapping rules for call argument lists. |
| `standard:binary-expression-wrapping` | Line break rules for wrapped binary expressions. |
| `standard:call-expression-wrapping` | Wrapping of chained function and method calls. |
| `standard:chain-method-continuation` | Indentation and wrapping continuation for call chains. |
| `standard:chain-wrapping` | Proper line break placement for chained operators. |
| `standard:class-signature` | Multiline wrapping rules for class declarations and primary constructors. |
| `standard:context-parameter-list-wrapping` | Wrapping of context parameters. |
| `standard:context-receiver-wrapping` | Wrapping of context receivers across lines. |
| `standard:enum-wrapping` | Formatting and wrapping of enum constant lists. |
| `standard:expression-operand-wrapping` | Line wrapping for multiline expression operands. |
| `standard:function-expression-body` | Line break rules for single-expression functions using `=`. |
| `standard:function-signature` | Wrapping of function headers, parameters, and return types. |
| `standard:if-else-wrapping` | Proper wrapping around `else` keywords and multiline branches. |
| `standard:multiline-expression-wrapping` | Wrapping consistency for complex multiline expressions. |
| `standard:multiline-if-else` | Bracing and wrapping requirements for multiline if-else chains. |
| `standard:multiline-loop` | Formatting and bracing for multiline loops. |
| `standard:no-line-break-after-else` | Disallows line breaks immediately following the `else` keyword. |
| `standard:no-line-break-before-assignment` | Disallows line breaks immediately preceding `=`. |
| `standard:parameter-list-wrapping` | Parameter list line-wrapping rules. |
| `standard:parameter-wrapping` | Wrapping individual parameters onto distinct lines when multiline. |
| `standard:property-wrapping` | Wrapping for multiline property declarations and initializers. |
| `standard:statement-wrapping` | Disallows multiple statements on a single line. |
| `standard:wrapping` | General multiline wrapping consistency. |
| `standard:indent` | Verifies indentation sizes and continuation indents. |
| `standard:max-line-length` | Verifies maximum column width (defaults to 100 or `.editorconfig`). |
| `standard:final-newline` | Ensures files terminate with a final trailing newline. |

### Code Style and Cleanliness

| Rule ID | Description |
| --- | --- |
| `standard:no-semi` | Disallows unnecessary semicolons `;`. |
| `standard:no-wildcard-imports` | Disallows wildcard imports (`import foo.*`). |
| `standard:no-unused-imports` | Flags and reports unused `import` statements. |
| `standard:import-ordering` | Enforces alphabetical import ordering and standard grouping. |
| `standard:no-empty-file` | Flags files that contain no code or declarations. |
| `standard:no-fully-qualified-type-names` | Disallows fully qualified type names in type positions (use imports instead). |
| `standard:no-empty-class-body` | Empty `{}` bodies on classes should be omitted. |
| `standard:no-unit-return` | Disallows explicit `: Unit` return types on functions. |
| `standard:unnecessary-parentheses-before-trailing-lambda` | Omit empty parentheses `()` when passing a trailing lambda. |
| `standard:trailing-comma-on-call-site` | Checks trailing comma usage on call argument lists. |
| `standard:trailing-comma-on-declaration-site` | Checks trailing comma usage on parameter declarations. |
| `standard:modifier-order` | Enforces canonical Kotlin modifier order (visibility, modality, etc.). |
| `standard:if-else-bracing` | Requires braces for multiline if/else expressions. |
| `standard:when-entry-bracing` | Bracing conventions for when branch bodies. |
| `standard:function-literal` | Formatting conventions for lambda literals. |
| `standard:lambda-return` | Disallows unnecessary explicit returns inside lambdas. |
| `standard:mixed-condition-operators` | Requires parentheses when mixing `&&` and `||` in expressions. |
| `standard:string-template` | Enforces clean string template expressions (e.g. `$x` instead of `${x}`). |
| `standard:string-template-indent` | Proper indentation and `.trimIndent()` usage in multiline raw strings. |
| `standard:annotation` | Annotation placement and usage rules. |

### Comments and KDoc

| Rule ID | Description |
| --- | --- |
| `standard:block-comment-initial-star-alignment` | Aligns leading `*` characters in multiline block comments. |
| `standard:comment-spacing` | Requires a single space after comment markers `//`. |
| `standard:comment-wrapping` | Enforces line wrapping limits on comments. |
| `standard:kdoc` | Validates KDoc structure and tag formatting. |
| `standard:kdoc-delimiter` | Formatting for `/**` and `*/` delimiters. |
| `standard:kdoc-wrapping` | Line wrapping within KDoc comment blocks. |
| `standard:no-consecutive-comments` | Checks consecutive comment block formatting. |
| `standard:no-single-line-block-comment` | Disallows single-line block comments `/* ... */` where `//` should be used. |
| `standard:type-argument-comment` | Validates comments inside type argument lists. |
| `standard:type-parameter-comment` | Validates comments inside type parameter lists. |
| `standard:value-argument-comment` | Validates comments inside value argument lists. |
| `standard:value-parameter-comment` | Validates comments inside value parameter declarations. |

---

## The `statistics` Ruleset

The `statistics` ruleset provides configurable code complexity and code size checks:

| Rule ID | Default Threshold | Condition | Message |
| --- | --- | --- | --- |
| `statistics:long-function` | `max_lines: 60` | `function.lines > max_lines` | `Function '<name>' is <lines> lines (limit: <limit>)` |
| `statistics:deep-indent` | `max_indent: 32` | `file.max_indent_spaces > max_indent` | `Max indent depth is <spaces> spaces (limit: <limit>)` |
| `statistics:too-many-parameters` | `max_params: 7` | `function.parameters_count > max_params` | `Function '<name>' has <count> parameters (limit: <limit>)` |
| `statistics:class-name-length` | `max_length: 40` | `class.name.length > max_length` (exempts names containing spaces) | `Class '<name>' name length is <length> (limit: <limit>)` |
| `statistics:function-name-length` | `max_length: 30` | `function.name.length > max_length` (exempts names containing spaces or `@Test` annotation) | `Function '<name>' name length is <length> (limit: <limit>)` |
| `statistics:variable-name-length` | `max_length: 30` | `variable.name.length > max_length` (exempts names containing spaces) | `Variable '<name>' name length is <length> (limit: <limit>)` |
| `statistics:file-length` | `max_lines: 600` | `file.lines > max_lines` | `File is <lines> lines (limit: <limit>)` |
| `statistics:too-many-functions` | `max_functions: 15` | `file.functions_count > max_functions` | `File has <count> functions (limit: <limit>)` |

### Enabling `statistics`

Via CLI:

```bash
kost lint --rulesets standard,statistics
```

In `kost.yml`:

```yaml
lint:
  includes:
    - standard
    - statistics
```

For customizing thresholds and options on these rules, see [Custom Rules](custom-rules.md).

## See Also

- [Lint Basics](basic.md)
- [Custom Rules](custom-rules.md)
- [Disabling Rules and Suppressions](disable-rules.md)
- [EditorConfig Integration](editor-config.md)
