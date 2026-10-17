# KtLint Built-in Rules & Engine Reference

This document serves strictly as the reference catalog and feature extraction of Ktlint's 106 built-in rules, CLI capabilities, configuration schema, and visitor model analyzed directly from the Ktlint codebase.

---

## Table of Contents

- [1. Engine Architecture & Core Principles](#1-engine-architecture--core-principles)
  - [1.1 Rule Lifecycle & Hooks](#11-rule-lifecycle--hooks)
  - [1.2 AST Traversal Model](#12-ast-traversal-model)
  - [1.3 Autocorrect Decision & Convergence Loop](#13-autocorrect-decision--convergence-loop)
  - [1.4 Rule Dependencies & Topological Ordering](#14-rule-dependencies--topological-ordering)
  - [1.5 Rule Opt-in Markers & Classification](#15-rule-opt-in-markers--classification)
- [2. CLI Capabilities & Workflow Integration](#2-cli-capabilities--workflow-integration)
  - [2.1 Execution Modes & File Pattern Matching](#21-execution-modes--file-pattern-matching)
  - [2.2 CLI Options & Flags](#22-cli-options--flags)
  - [2.3 Subcommands](#23-subcommands)
  - [2.4 Reporter Ecosystem](#24-reporter-ecosystem)
  - [2.5 Baseline System](#25-baseline-system)
- [3. Configuration & EditorConfig Specification](#3-configuration--editorconfig-specification)
  - [3.1 Hierarchy & Resolution Precedence](#31-hierarchy--resolution-precedence)
  - [3.2 Code Style Profiles](#32-code-style-profiles)
  - [3.3 Rule & Ruleset Toggling](#33-rule--ruleset-toggling)
  - [3.4 Global & Standard Properties](#34-global--standard-properties)
  - [3.5 Rule-Specific Configuration Settings](#35-rule-specific-configuration-settings)
- [4. Suppression Mechanisms](#4-suppression-mechanisms)
  - [4.1 Code Annotation Suppressions](#41-code-annotation-suppressions)
  - [4.2 Inline Comment Directives](#42-inline-comment-directives)
  - [4.3 Special Suppression Handling](#43-special-suppression-handling)
- [5. Built-in Rules Catalog](#5-built-in-rules-catalog)
  - [5.1 File Structure, Package, Imports & Declarations](#51-file-structure-package-imports--declarations)
  - [5.2 Spacing & Whitespace Rules](#52-spacing--whitespace-rules)
  - [5.3 Wrapping & Line Break Rules](#53-wrapping--line-break-rules)
  - [5.4 Blank Lines & Structural Spacing Rules](#54-blank-lines--structural-spacing-rules)
  - [5.5 Naming Conventions & Identifiers](#55-naming-conventions--identifiers)
  - [5.6 Language Constructs & Idiomatic Kotlin Rules](#56-language-constructs--idiomatic-kotlin-rules)
  - [5.7 Comments & KDoc Rules](#57-comments--kdoc-rules)
  - [5.8 Indentation, Line Length & Trailing Commas](#58-indentation-line-length--trailing-commas)

---

## 1. Engine Architecture & Core Principles

The lint engine (`KtLintRuleEngine`) is designed as an anti-bikeshedding, deterministic code analysis and formatting system. It consumes Kotlin source code or script files (`.kt`, `.kts`), constructs an abstract syntax tree (AST), and executes a configured set of lint rules against the tree.

```
       Source (.kt / .kts)
               │
               ▼
   AST Parser (PSI / Tree-Sitter)
               │
               ▼
 ┌───────────────────────────────┐
 │       KtLintRuleEngine        │
 │  ┌─────────────────────────┐  │
 │  │   EditorConfig Loader   │  │ (Resolves directory hierarchy,
 │  └─────────────┬───────────┘  │  defaults, overrides)
 │                │              │
 │  ┌─────────────▼───────────┐  │
 │  │   RuleProviderSorter    │  │ (Topological sort with RunAsLateAsPossible,
 │  └─────────────┬───────────┘  │  RunAfterRule dependencies)
 │                │              │
 │  ┌─────────────▼───────────┐  │
 │  │   Format/Lint Loop      │◄─┼── Autocorrect Convergence Loop
 │  │   (Up to 4 iterations)  │──┼── (Applies AST mutations safely)
 │  └─────────────┬───────────┘  │
 └────────────────┼──────────────┘
                  │
                  ▼
         LintErrors / Formatted Output
```

### 1.1 Rule Lifecycle & Hooks

Each rule inherits from `RuleV2` (or `StandardRule` for standard built-ins). Rules are instantiated per-file via `RuleV2Provider` to ensure thread safety without shared mutable state across threads.

During the traversal of a single file, the engine invokes up to four lifecycle hooks on each rule:

1. **`beforeFirstNode(editorConfig: EditorConfig)`**:
   - Invoked once before AST traversal begins.
   - Used to extract rule-specific configuration properties declared in `usesEditorConfigProperties`.
2. **`beforeVisitChildNodes(node: ASTNode, emit: EmitLambda)`**:
   - Depth-first preorder traversal hook called before children of `node` are visited.
   - Primary hook for detecting syntax violations and requesting autocorrect modifications.
   - Rules can call `stopTraversalOfAST()` to abort further traversal (e.g. for files that should be ignored entirely).
3. **`afterVisitChildNodes(node: ASTNode, emit: EmitLambda)`**:
   - Depth-first postorder traversal hook called after all descendants of `node` have been visited.
   - Used for structural checks requiring bottom-up context or child aggregate metrics.
4. **`afterLastNode()`**:
   - Invoked once after AST traversal completes.
   - Used for cleanup, aggregate violations, or file-level assertions.

The `emit` callback has signature:
```kotlin
(offset: Int, errorMessage: String, canBeAutoCorrected: Boolean) -> AutocorrectDecision
```
Rules invoke `emit` when a violation is detected. In format mode, rules inspect the returned `AutocorrectDecision` and invoke `.ifAutocorrectAllowed { ... }` to mutate the AST.

### 1.2 AST Traversal Model

- **Nodes and Leaves**: The AST consists of composite nodes (`ASTNode`) and terminal tokens (`LeafPsiElement`, such as identifiers, keywords, whitespace, comments, strings).
- **Whitespace Representation**: Whitespace (spaces, tabs, newlines) is explicitly represented in the tree as leaf nodes (`PsiWhiteSpaceImpl`). Formatting and spacing rules operate directly by inserting, deleting, or altering whitespace leaves.
- **Node Navigation**: Standard navigation primitives include `treeParent`, `treeNext`, `treePrev`, `firstChildNode`, `lastChildNode`, `nextLeaf()`, `prevLeaf()`, `nextCodeLeaf()`, `prevCodeLeaf()`, `nextSibling()`, and `prevSibling()`.

### 1.3 Autocorrect Decision & Convergence Loop

When running in **Format Mode** (`ktlint -F` / `codeFormatter.format`):
- Formatting can introduce new violations or resolve prerequisites for subsequent rules.
- The engine executes a multi-pass format loop (`MAX_FORMAT_RUNS_PER_FILE = 4`).
- In each pass:
  1. A fresh AST is prepared or maintained.
  2. Active rules traverse the tree.
  3. When an autocorrectable error is detected and permitted by the autocorrect handler, the rule modifies the AST in-place.
  4. If the AST was mutated, another pass runs until:
     - No further mutations occur (stable output reached), OR
     - The maximum run limit (4) is reached.
- Any remaining uncorrected errors after the final pass are reported to the caller.

### 1.4 Rule Dependencies & Topological Ordering

Rules cannot run in arbitrary order because certain rules depend on the AST being normalized by earlier rules. `RuleProviderSorter` uses topological sorting guided by:
- **`VisitorModifier.RunAsLateAsPossible`**: Defer rules to the end of the pipeline (e.g. `indent`, `no-trailing-spaces`, `final-newline`, `max-line-length`).
- **`VisitorModifier.RunAfterRule(ruleId, runOnlyWhenOtherRuleIsEnabled)`**: Explicit dependency constraints where rule $A$ must run after rule $B$.

### 1.5 Rule Opt-in Markers & Classification

Rules declare their stability and activation criteria using marker interfaces:
- **Standard Stable**: Default active rules that conform to standard Kotlin style.
- **`RuleV2.Experimental`**: Rules under evaluation or subject to breaking changes. Only executed when `ktlint_experimental = enabled` in `.editorconfig` or when individually enabled.
- **`RuleV2.OfficialCodeStyle`**: Rules enforcing the `ktlint_official` code style. Active when `ktlint_code_style = ktlint_official` (or individually enabled).
- **`RuleV2.OnlyWhenEnabledInEditorconfig`**: Opt-in only rules disabled by default across all styles (e.g., `no-unused-imports`).
- **`@SinceKtlint(version, status)`**: Metadata tracking the release version when introduced and lifecycle status (`EXPERIMENTAL` or `STABLE`).

---

## 2. CLI Capabilities & Workflow Integration

The `ktlint-cli` module provides the command-line interface based on Clikt, supporting linting, automated fixing, git hook integration, baseline management, and multiple reporter targets.

### 2.1 Execution Modes & File Pattern Matching

- **Default Glob Patterns**: When no file arguments are passed, ktlint scans `**/*.kt` and `**/*.kts`, skipping hidden folders.
- **Pattern Negation**: Prepending `!` negates a pattern (using `.gitignore` style glob syntax). Globs are evaluated in order of declaration.
  - Example: `ktlint "src/**/*.kt" "!src/**/*Test.kt"`
  - Example: `ktlint "src/**/*.kt" "!src/**/generated/**"`
- **Standard Input (`--stdin`)**: Allows piping source code directly from stdin.
  - `--stdin-path <path>`: Provides a virtual file path for stdin code so path-dependent rules (like `.editorconfig` resolution) function properly. In format mode with stdin, output is written to stdout.
- **Batch Processing from Stdin (`--patterns-from-stdin`)**:
  - Reads patterns or file lists from stdin separated by newline or NUL byte (`\0`).
  - Enables direct piping from git diffs: `git diff --name-only -z | ktlint --patterns-from-stdin=""`.

### 2.2 CLI Options & Flags

| Flag | Long Option | Description |
|:---|:---|:---|
| `-F` | `--format` | Fix code style deviations automatically where possible |
| | `--baseline=<path>` | Path to XML baseline file containing known violations to ignore |
| | `--reporter=<config>` | Configure reporters (can be specified multiple times) |
| `-R` | `--ruleset=<paths>` | Comma-separated paths to external JAR files providing custom rule sets |
| | `--editorconfig=<path>` | Path to fallback default `.editorconfig` file |
| | `--relative` | Output file paths relative to the current working directory |
| | `--limit=<int>` | Maximum number of errors to report before aborting |
| | `--ignore-autocorrect-failures` | Filter out errors that cannot be automatically corrected |
| | `--color` | Enable ANSI colored console output |
| | `--color-name=<name>` | Set custom highlight color (default: `DARK_GRAY`) |
| `-l` | `--log-level=<level>` | Set minimum log level (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`, `NONE`) |
| `-v` | `--version` | Display version information and exit |

### 2.3 Subcommands

1. **`generateEditorConfig`**:
   - Scans all rule providers and creates a complete `.editorconfig` section matching the active code style and default property settings.
2. **`installGitPreCommitHook`**:
   - Generates and writes an executable `.git/hooks/pre-commit` script that runs ktlint on staged Kotlin files before allowing commits.
3. **`installGitPrePushHook`**:
   - Generates and writes an executable `.git/hooks/pre-push` script that verifies all outgoing commits against ktlint rules.

### 2.4 Reporter Ecosystem

Ktlint supports multiple simultaneous reporters via `--reporter=<name>[,param=value...]`:
- **`plain`** (default): Human-readable terminal output (`path:line:col: message (rule-id)`).
  - Parameter `group_by_file`: Groups violations under file headers.
  - Parameter `color`: Enables ANSI colorization.
- **`plain-summary`**: High-level error count and rule violation breakdown summary.
- **`json`**: Machine-readable JSON array of errors grouped by file.
- **`html`**: Comprehensive standalone HTML report with syntax highlighting and filtering.
- **`checkstyle`**: Standard Checkstyle XML format compatible with CI systems (Jenkins, TeamCity, GitHub Actions).
- **`sarif`**: Static Analysis Results Interchange Format (SARIF v2.1.0) for GitHub Code Scanning integration.
- **`baseline`**: Reporter used to generate or regenerate XML baseline files.
- **Custom Reporters**: Loaded dynamically via `,artifact=/path/to/custom-reporter.jar`.

### 2.5 Baseline System

The baseline system enables teams to introduce ktlint into legacy codebases without failing CI on thousands of existing violations:
- Stored as an XML file:
  ```xml
  <baseline version="1.0">
    <file name="src/main/kotlin/Legacy.kt">
      <error line="12" column="5" rule="standard:no-semi" />
      <error line="25" column="1" rule="standard:max-line-length" />
    </file>
  </baseline>
  ```
- When `--baseline=<path>` is specified, violations matching the baseline entries are suppressed.
- Only newly introduced violations trigger errors.
- Running with `--reporter=baseline,output=<path>` regenerates or updates the baseline file.

---

## 3. Configuration & EditorConfig Specification

Ktlint uses standard `.editorconfig` files for rule configuration and code style customization.

### 3.1 Hierarchy & Resolution Precedence

Settings are resolved in strict hierarchical order:
1. **CLI / Engine Overrides (`EditorConfigOverride`)**: Highest priority; programmatic or CLI flags.
2. **Directory Tree `.editorconfig`**: Resolved upward from the source file directory to the root (`root = true`). Child directory settings override parent settings.
3. **Fallback Defaults (`EditorConfigDefaults` / `--editorconfig`)**: Supplied when no on-disk `.editorconfig` provides the property.
4. **Built-in Defaults**: Hardcoded rule defaults defined inside the ktlint engine.

### 3.2 Code Style Profiles

Configured via the `ktlint_code_style` property under `[*.{kt,kts}]`:

| Code Style | Description | Indent | Trailing Commas | Notes |
|:---|:---|:---|:---|:---|
| **`ktlint_official`** (default) | Recommended style combining Kotlin Coding Conventions & Android Guide with strict consistency rules | 4 spaces | Enabled | Activates rules implementing `RuleV2.OfficialCodeStyle` |
| **`intellij_idea`** | Matches standard JetBrains IntelliJ IDEA Kotlin formatter | 4 spaces | Disabled | Avoids discrepancies with default IDE autoformat |
| **`android_studio`** | Matches Google Android Kotlin style guide | 4 spaces | Disabled | Tailored for Android development workflows |

### 3.3 Rule & Ruleset Toggling

Rules and rulesets can be enabled or disabled selectively in `.editorconfig`:

```ini
[*.{kt,kts}]
# Toggle entire rulesets:
ktlint_standard = enabled
ktlint_experimental = enabled

# Toggle specific individual rules:
ktlint_standard_final-newline = disabled
ktlint_standard_no-unused-imports = enabled
ktlint_standard_chain-method-continuation = disabled
```

*Rule-level settings take precedence over ruleset-level settings.*

### 3.4 Global & Standard Properties

| Property | Allowed Values | Default | Used by Rules |
|:---|:---|:---|:---|
| `indent_size` | Integer (`> 0`) | `4` | `indent` |
| `indent_style` | `space`, `tab` | `space` | `indent` |
| `insert_final_newline` | `true`, `false` | `true` | `final-newline` |
| `end_of_line` | `lf`, `crlf`, `cr` | `lf` | Core Formatter |
| `max_line_length` | Integer (`> 0`) or `off` | `140` (`ktlint_official`) / `120` / `100` | `max-line-length`, wrapping rules |

### 3.5 Rule-Specific Configuration Settings

| Property | Type | Default | Associated Rule |
|:---|:---|:---|:---|
| `ij_kotlin_allow_trailing_comma` | Boolean | `true` (`official`), `false` (others) | `trailing-comma-on-declaration-site` |
| `ij_kotlin_allow_trailing_comma_on_call_site` | Boolean | `true` (`official`), `false` (others) | `trailing-comma-on-call-site` |
| `ij_kotlin_imports_layout` | Comma-separated list | `*,java.**,javax.**,kotlin.**,^` | `import-ordering` |
| `ij_kotlin_packages_to_use_import_on_demand` | Comma-separated list | `""` | `no-wildcard-imports` |
| `ktlint_chain_method_rule_force_multiline_when_chain_operator_count_greater_or_equal_than` | Integer | `4` | `chain-method-continuation` |
| `ktlint_class_signature_rule_force_multiline_when_parameter_count_greater_or_equal_than` | Integer | `unset` | `class-signature` |
| `ktlint_function_signature_rule_force_multiline_when_parameter_count_greater_or_equal_than` | Integer | `unset` | `function-signature` |
| `ktlint_function_signature_body_expression_wrapping` | `default`, `always`, `multiline` | `default` | `function-signature` |
| `ktlint_function_naming_ignore_when_annotated_with` | Comma-separated list | `Composable` | `function-naming` |
| `ktlint_ignore_back_ticked_identifier` | Boolean | `false` | `max-line-length` |

---

## 4. Suppression Mechanisms

Ktlint provides robust mechanisms to suppress lint violations at declaration, expression, line, or block levels.

### 4.1 Code Annotation Suppressions

Violations can be suppressed using standard Kotlin `@Suppress` annotations with the rule identifier:

```kotlin
// Fully-qualified syntax (recommended):
@Suppress("ktlint:standard:no-semi")
val x = 1;

// File-level suppression:
@file:Suppress("ktlint:standard:filename", "ktlint:standard:package-name")

// Multiple rules:
@Suppress("ktlint:standard:function-naming", "ktlint:standard:max-line-length")
fun My_Function() { ... }
```

### 4.2 Inline Comment Directives

Comments can disable and re-enable rules around arbitrary blocks or for single lines:

```kotlin
// Disable for an entire block:
/* ktlint-disable standard:no-semi */
val a = 1;
val b = 2;
/* ktlint-enable standard:no-semi */

// Disable all rules:
/* ktlint-disable */
val untidy = "code" ;
/* ktlint-enable */

// Single line suppression (placed at end of line or on preceding line):
val c = 3; // ktlint-disable standard:no-semi
```

### 4.3 Special Suppression Handling

- **`IgnoreKtlintSuppressions`**: Certain critical rules implement `IgnoreKtlintSuppressions` (such as `no-unused-imports`). These rules bypass standard suppression comments/annotations to avoid false suppression states or AST corruption.
- **Suppression Locator**: The engine builds an interval map (`SuppressionLocator`) of all suppression ranges in the file prior to running rules, ensuring $O(\log n)$ lookup performance during AST traversal.


---

## 5. Built-in Rules Catalog

Ktlint bundles 106 built-in rules within `ktlint-ruleset-standard`. Below, the rules are systematically categorized and specified with their identifiers, behavior, configuration options, AST element targets, and autocorrect capabilities.

---

### 5.1 File Structure, Package, Imports & Declarations

This family of rules enforces top-of-file structural invariants, naming conventions for files and packages, import organization, and boundary spacing between declarations.

#### 1. `filename` (`FilenameRule`)
- **Rule ID**: `standard:filename`
- **Autocorrectable**: No (cannot safely rename files on disk).
- **Markers**: Stable.
- **AST Nodes**: `FILE`, `CLASS`, `OBJECT_DECLARATION`.
- **Specification**:
  - If a Kotlin source file contains a single top-level class, interface, or object, the filename **must match the declaration name** followed by `.kt` (in PascalCase).
  - If a file contains multiple top-level classes or solely top-level functions/properties, the filename must be PascalCase describing the contents (e.g., `StringUtils.kt`, `EventHandlers.kt`).
  - Allows common suffix patterns (e.g., `Test.kt`, `Tests.kt`, `Spec.kt`).
  - Ignores script files (`.kts`) and files containing `@file:JvmName` or `@file:JvmMultifileClass`.

#### 2. `package-name` (`PackageNameRule`)
- **Rule ID**: `standard:package-name`
- **Autocorrectable**: No (refactoring package requires modifying directory structure and import references).
- **Markers**: Stable.
- **AST Nodes**: `PACKAGE_DIRECTIVE`.
- **Specification**:
  - Enforces that package names must be all lowercase without underscores or hyphens (e.g. `package com.example.myproject.feature`).
  - Consecutive segment names separated by dots (`.`).
  - Backticked segments that correspond to Kotlin keywords or reserved identifiers (e.g., ``package com.example.`in` ``) are permitted.

#### 3. `blank-line-before-package` (`BlankLineBeforePackage`)
- **Rule ID**: `standard:blank-line-before-package`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`, `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `PACKAGE_DIRECTIVE`.
- **Specification**:
  - Requires exactly **one blank line** before the `package` statement when it is preceded by a file header, copyright comment, or `@file:` annotations.
  - If `package` is the very first element in the file, no blank line is required before it.

#### 4. `blank-line-before-imports` (`BlankLineBeforeImports`)
- **Rule ID**: `standard:blank-line-before-imports`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`, `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `IMPORT_LIST`.
- **Specification**:
  - Requires exactly **one blank line** before the import list when preceded by a `package` declaration or top-of-file comments.
  - If the file has no package directive and imports begin at the top of the file, no leading blank line is enforced.

#### 5. `blank-line-before-file-annotation` (`BlankLineBeforeFileAnnotation`)
- **Rule ID**: `standard:blank-line-before-file-annotation`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`, `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `FILE_ANNOTATION_LIST`.
- **Specification**:
  - Requires a blank line before `@file:...` annotations when preceded by a license or copyright comment.
  - Disallowed: Top-of-file comment immediately adjoining `@file:OptIn(...)` without an intervening blank line.

#### 6. `import-ordering` (`ImportOrderingRule`)
- **Rule ID**: `standard:import-ordering`
- **Autocorrectable**: Yes (reorders and groups AST import directives).
- **Markers**: Stable.
- **AST Nodes**: `IMPORT_LIST`, `IMPORT_DIRECTIVE`.
- **Configuration**:
  - `ij_kotlin_imports_layout`: Specifies ordering pattern.
    - Default (`ktlint_official` & `intellij_idea`): `*,java.**,javax.**,kotlin.**,^` (all regular imports, followed by Java/Javax, Kotlin, then static/alias imports marked by `^`).
    - Android (`android_studio`): `*` (pure alphabetical order).
- **Specification**:
  - Reorders import statements into canonical groups separated by exactly one blank line.
  - Within each group, imports are sorted lexicographically by canonical FQ name.
  - Comments preceding or inside imports are hoisted to preserve documentation without breaking grouping.
  - Eliminates duplicate import directives.

#### 7. `no-wildcard-imports` (`NoWildcardImportsRule`)
- **Rule ID**: `standard:no-wildcard-imports`
- **Autocorrectable**: No (expanding star imports requires symbol resolution from compiler classpath).
- **Markers**: Stable.
- **AST Nodes**: `IMPORT_DIRECTIVE`.
- **Configuration**:
  - `ij_kotlin_packages_to_use_import_on_demand`: Comma-separated list of allowed wildcard packages (e.g. `java.util.*`).
- **Specification**:
  - Prohibits wildcard / star imports (e.g., `import foo.bar.*`).
  - Standard Kotlin convention requires explicit individual symbol imports to prevent namespace pollution and ambiguous shadowing.

#### 8. `no-unused-imports` (`NoUnusedImportsRule`)
- **Rule ID**: `standard:no-unused-imports`
- **Autocorrectable**: Yes (deletes unused `IMPORT_DIRECTIVE` nodes).
- **Markers**: `RuleV2.OnlyWhenEnabledInEditorconfig`, `IgnoreKtlintSuppressions`.
- **AST Nodes**: `IMPORT_DIRECTIVE`, `REFERENCE_EXPRESSION`, `KDOC_TAG`, `KDOC_LINK`.
- **Specification**:
  - Detects and removes unused import statements.
  - **Operator Overloads**: Preserves imports for Kotlin operator overloads (`plus`, `minus`, `times`, `div`, `rem`, `get`, `set`, `invoke`, `contains`, `iterator`, `getValue`, `setValue`, `provideDelegate`, `compareTo`, `equals`, `inc`, `dec`, `unaryPlus`, `unaryMinus`, `not`).
  - **Destructuring**: Preserves `component1`, `component2`, etc.
  - **KDoc References**: Preserves imports referenced in KDoc link tags (`[ClassName]`, `@see ClassName`, `@throws ExceptionName`).
  - Note: Opt-in only due to potential compilation breakage on unresolved type aliases or synthetic compiler imports.

#### 9. `blank-line-before-declaration` (`BlankLineBeforeDeclarationRule`)
- **Rule ID**: `standard:blank-line-before-declaration`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN`, `CLASS`, `OBJECT_DECLARATION`, `PROPERTY`.
- **Specification**:
  - Enforces a blank line before major top-level and member declarations (classes, functions, secondary constructors).
  - Exceptions:
    - No blank line before the first declaration inside a class or function body.
    - Consecutive property declarations (e.g. `val a = 1; val b = 2`) do not require blank lines between them unless annotated.

#### 10. `spacing-between-declarations-with-annotations` (`SpacingBetweenDeclarationsWithAnnotationsRule`)
- **Rule ID**: `standard:spacing-between-declarations-with-annotations`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `MODIFIER_LIST`, `ANNOTATION_ENTRY`.
- **Specification**:
  - Requires a blank line before any declaration that contains annotations, distinguishing it from unannotated simple properties.
  - Prevents dense clumps of annotated fields/functions where annotations visually bleed into adjacent declarations.

#### 11. `spacing-between-declarations-with-comments` (`SpacingBetweenDeclarationsWithCommentsRule`)
- **Rule ID**: `standard:spacing-between-declarations-with-comments`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `EOL_COMMENT`, `BLOCK_COMMENT`, `KDOC`.
- **Specification**:
  - Requires a blank line before a declaration that is preceded by a comment or KDoc block, ensuring clear visual separation from prior declarations.

#### 12. `no-empty-file` (`NoEmptyFileRule`)
- **Rule ID**: `standard:no-empty-file`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `FILE`.
- **Specification**:
  - Emits a violation if a `.kt` file contains no executable code or declarations (e.g., completely empty, or contains only whitespace, comments, and/or a shebang).


---

### 5.2 Spacing & Whitespace Rules

This suite of rules enforces uniform horizontal whitespace across operators, delimiters, keywords, declarations, and modifiers, eliminating redundant spaces and ensuring consistent visual rhythm.

#### 13. `annotation-spacing` (`AnnotationSpacingRule`)
- **Rule ID**: `standard:annotation-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `MODIFIER_LIST`, `ANNOTATION_ENTRY`.
- **Specification**:
  - Ensures annotations occur immediately prior to the annotated construct without intervening blank lines.
  - When multiple annotations are written on the same line, exactly one single space is required between them.
  - Multiple annotations on separate lines must each reside on their own line preceding the declaration.

#### 14. `colon-spacing` (`SpacingAroundColonRule`)
- **Rule ID**: `standard:colon-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `COLON`.
- **Specification**:
  - **Type Annotations**: No space before, exactly one space after (`val x: Int`, `fun f(): String`).
  - **Supertype / Inheritance Specifiers**: Exactly one space before, exactly one space after (`class Child : Parent()`, `interface Bar : Foo`).
  - **Secondary Constructor Delegation**: Space before and after (`constructor(...) : this(...)`).
  - **Type Constraints / Where Clauses**: Space before and after (`where T : Comparable<T>`).
  - **Annotation Use-site Targets**: No spaces around the colon (`@file:OptIn`, `@get:Rule`, `@param:Name`).
  - Prohibits unexpected newlines immediately preceding a colon.

#### 15. `comma-spacing` (`SpacingAroundCommaRule`)
- **Rule ID**: `standard:comma-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `COMMA`.
- **Specification**:
  - No whitespace permitted before a comma.
  - Exactly one space required after a comma when followed by another token on the same line.
  - If a newline follows the comma, trailing spaces after the comma are stripped.

#### 16. `curly-spacing` (`SpacingAroundCurlyRule`)
- **Rule ID**: `standard:curly-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `LBRACE`, `RBRACE`.
- **Specification**:
  - Opening curly brace (`{`) must be preceded by a single space (e.g. `class Foo {`, `if (cond) {`, `items.map {`).
  - Single-line lambda bodies require a single space after `{` and before `}`: `{ foo() }`.
  - Empty bodies (`{}`) should have no space between braces: `class Empty {}`.
  - Closing curly brace (`}`) on a line with following keywords (`else`, `finally`, `catch`) must have a space before the keyword: `} else {`.

#### 17. `dot-spacing` (`SpacingAroundDotRule`)
- **Rule ID**: `standard:dot-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `DOT`, `SAFE_ACCESS`.
- **Specification**:
  - No whitespace permitted before or after a dereference dot (`.`) or safe-access operator (`?.`) when on a single line.
  - When chained calls break across lines, the dot begins the new line without leading space after the indent.

#### 18. `double-colon-spacing` (`SpacingAroundDoubleColonRule`)
- **Rule ID**: `standard:double-colon-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `COLONCOLON`.
- **Specification**:
  - No whitespace permitted before or after the callable/class reference operator `::` (e.g., `String::length`, `User::class`, `::println`).

#### 19. `keyword-spacing` (`SpacingAroundKeywordRule`)
- **Rule ID**: `standard:keyword-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: Kotlin keyword tokens (`IF_KEYWORD`, `ELSE_KEYWORD`, `WHILE_KEYWORD`, `FOR_KEYWORD`, `WHEN_KEYWORD`, `TRY_KEYWORD`, `CATCH_KEYWORD`, `FINALLY_KEYWORD`, `RETURN_KEYWORD`, `THROW_KEYWORD`, etc.).
- **Specification**:
  - Control flow keywords (`if`, `for`, `while`, `when`, `catch`) must be followed by exactly one space before the opening parenthesis: `if (condition)`.
  - Keywords preceding curly braces (`else`, `try`, `finally`, `do`) must have a trailing space before `{`.
  - `return`, `throw`, and `yield` must be separated from their expression operand by a space.

#### 20. `op-spacing` (`SpacingAroundOperatorsRule`)
- **Rule ID**: `standard:op-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `OPERATION_REFERENCE` (Binary arithmetic `+`, `-`, `*`, `/`, `%`; assignment `=`, `+=`, `-=`, etc.; comparison `==`, `!=`, `<`, `>`, `<=`, `>=`; logical `&&`, `||`; Elvis `?:`).
- **Specification**:
  - Binary operators must be surrounded by exactly one space on both sides (`a + b`, `x ?: y`, `c == d`).
  - Prohibits missing spaces (`a+b`) or superfluous multi-spaces.

#### 21. `paren-spacing` (`SpacingAroundParensRule`)
- **Rule ID**: `standard:paren-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `LPAR`, `RPAR`.
- **Specification**:
  - No whitespace directly inside parentheses: `(arg)` instead of `( arg )`.
  - No space between function name or constructor and the opening parenthesis `(` (e.g., `foo()`, `User()`).
  - Empty argument/parameter list `()` must have no internal space.

#### 22. `range-spacing` (`SpacingAroundRangeOperatorRule`)
- **Rule ID**: `standard:range-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `RANGE` (`..`), `RANGE_UNTIL` (`..<`).
- **Specification**:
  - Range operators must not be surrounded by spaces: `0..10`, `0..<count`.

#### 23. `square-brackets-spacing` (`SpacingAroundSquareBracketsRule`)
- **Rule ID**: `standard:square-brackets-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `LBRACKET`, `RBRACKET`.
- **Specification**:
  - No whitespace immediately after opening bracket `[` or immediately before closing bracket `]`: `array[index]`.
  - No whitespace between expression and opening index bracket: `list[0]`, not `list [0]`.

#### 24. `unary-op-spacing` (`SpacingAroundUnaryOperatorRule`)
- **Rule ID**: `standard:unary-op-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `PREFIX_EXPRESSION`, `POSTFIX_EXPRESSION` (`!`, `+`, `-`, `++`, `--`, `!!`).
- **Specification**:
  - Prefix operators must have no space between operator and operand: `!flag`, `-count`, `++i`.
  - Postfix operators must have no space between operand and operator: `i++`, `nullable!!`.

#### 25. `spacing-around-angle-brackets` (`SpacingAroundAngleBracketsRule`)
- **Rule ID**: `standard:spacing-around-angle-brackets`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_ARGUMENT_LIST`, `TYPE_PARAMETER_LIST` (`<`, `>`).
- **Specification**:
  - No whitespace inside angle brackets immediately after `<` or before `>`: `List<String>`.
  - No space before opening angle bracket `<` when attached to type name or function: `Map<K, V>`.

#### 26. `spacing-between-function-name-and-opening-parenthesis` (`SpacingBetweenFunctionNameAndOpeningParenthesisRule`)
- **Rule ID**: `standard:spacing-between-function-name-and-opening-parenthesis`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN`, `CALL_EXPRESSION`.
- **Specification**:
  - Forbids whitespace between a function identifier and its parameter/argument opening parenthesis: `fun execute()` (not `fun execute ()`).

#### 27. `fun-keyword-spacing` (`FunKeywordSpacingRule`)
- **Rule ID**: `standard:fun-keyword-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN_KEYWORD`.
- **Specification**:
  - Requires exactly one space following the `fun` keyword before the function identifier, type parameter list, or receiver type.

#### 28. `function-return-type-spacing` (`FunctionReturnTypeSpacingRule`)
- **Rule ID**: `standard:function-return-type-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_REFERENCE` (in function declarations).
- **Specification**:
  - Exactly one space between the colon and the function return type: `fun name(): String`.

#### 29. `function-start-of-body-spacing` (`FunctionStartOfBodySpacingRule`)
- **Rule ID**: `standard:function-start-of-body-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN`.
- **Specification**:
  - Requires single space before the body delimiter (either `{` for block body or `=` for expression body).

#### 30. `function-type-modifier-spacing` (`FunctionTypeModifierSpacingRule`)
- **Rule ID**: `standard:function-type-modifier-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUNCTION_TYPE`.
- **Specification**:
  - Ensures correct spacing between modifiers on function types (e.g. `suspend () -> Unit`).

#### 31. `function-type-reference-spacing` (`FunctionTypeReferenceSpacingRule`)
- **Rule ID**: `standard:function-type-reference-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUNCTION_TYPE_RECEIVER`, `ARROW`.
- **Specification**:
  - Verifies spacing in function type references: `Receiver.() -> ReturnType` (space before and after `->`).

#### 32. `modifier-list-spacing` (`ModifierListSpacingRule`)
- **Rule ID**: `standard:modifier-list-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `MODIFIER_LIST`.
- **Specification**:
  - Requires exactly one single space between successive modifiers on the same line (e.g., `private val`, `open override suspend fun`).

#### 33. `modifier-order` (`ModifierOrderRule`)
- **Rule ID**: `standard:modifier-order`
- **Autocorrectable**: Yes (reorders modifier tokens in AST).
- **Markers**: Stable.
- **AST Nodes**: `MODIFIER_LIST`.
- **Specification**:
  - Enforces the official Kotlin canonical ordering of modifiers:
    1. Annotations (`@Annotation`)
    2. Context parameters (`context(...)`)
    3. Visibility (`public`, `protected`, `private`, `internal`)
    4. Expect / Actual (`expect`, `actual`)
    5. Modality (`final`, `open`, `abstract`, `sealed`, `const`)
    6. External (`external`)
    7. Override (`override`)
    8. Lateinit (`lateinit`)
    9. Tailrec (`tailrec`)
    10. Vararg (`vararg`)
    11. Suspend (`suspend`)
    12. Inner (`inner`)
    13. Type modifier (`enum`, `annotation`, `companion`)
    14. Function modifier (`inline`, `infix`, `operator`)
    15. Data (`data`)

#### 34. `nullable-type-spacing` (`NullableTypeSpacingRule`)
- **Rule ID**: `standard:nullable-type-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `QUEST` (nullability indicator in `NULLABLE_TYPE`).
- **Specification**:
  - No space permitted between a type and its `?` nullability specifier: `String?`, not `String ?`.

#### 35. `parameter-list-spacing` (`ParameterListSpacingRule`)
- **Rule ID**: `standard:parameter-list-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_PARAMETER_LIST`.
- **Specification**:
  - Manages horizontal whitespace within single-line parameter lists, ensuring clean separation between parameters.

#### 36. `then-spacing` (`ThenSpacingRule`)
- **Rule ID**: `standard:then-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `THEN` (in single-line `if` statements).
- **Specification**:
  - Requires space between the condition closing parenthesis `)` and the branch expression when written without braces on the same line: `if (cond) foo()`.

#### 37. `try-catch-finally-spacing` (`TryCatchFinallySpacingRule`)
- **Rule ID**: `standard:try-catch-finally-spacing`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `TRY`, `CATCH`, `FINALLY`.
- **Specification**:
  - Enforces canonical spacing around `try`, `catch`, and `finally` blocks: `} catch (e: Exception) {` and `} finally {` must share the line with the closing brace of the previous block with single space separation.

#### 38. `type-argument-list-spacing` (`TypeArgumentListSpacingRule`)
- **Rule ID**: `standard:type-argument-list-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_ARGUMENT_LIST`.
- **Specification**:
  - Ensures no space between call identifier and opening `<`: `foo<Int>()`.

#### 39. `type-parameter-list-spacing` (`TypeParameterListSpacingRule`)
- **Rule ID**: `standard:type-parameter-list-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_PARAMETER_LIST`.
- **Specification**:
  - Ensures single space between type parameter list and preceding `fun`/`class` keyword or following receiver/name: `fun <T> execute()`.

#### 40. `no-multi-spaces` (`NoMultipleSpacesRule`)
- **Rule ID**: `standard:no-multi-spaces`
- **Autocorrectable**: Yes (collapses multiple spaces to one).
- **Markers**: Stable.
- **AST Nodes**: `WHITE_SPACE`.
- **Specification**:
  - Forbids consecutive space characters (`"  +"`) outside of indentation prefixes, string literals, and comments.

#### 41. `no-trailing-spaces` (`NoTrailingSpacesRule`)
- **Rule ID**: `standard:no-trailing-spaces`
- **Autocorrectable**: Yes.
- **Markers**: Stable (runs as late as possible).
- **AST Nodes**: `WHITE_SPACE`.
- **Specification**:
  - Strips any whitespace at the end of every line before the newline character.


---

### 5.3 Wrapping & Line Break Rules

Wrapping rules govern how statements, expressions, signatures, loops, and parameter lists break across lines when exceeding `max_line_length` or when containing multiline constructs.

#### 42. `argument-list-wrapping` (`ArgumentListWrappingRule`)
- **Rule ID**: `standard:argument-list-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_ARGUMENT_LIST`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - If a function call argument list spans multiple lines, **each argument must start on a separate line**.
  - The closing parenthesis `)` must be placed on its own line, matching the indentation of the call site line.
  - Prohibits mixing single-line and multiline arguments in the same invocation.

#### 43. `binary-expression-wrapping` (`BinaryExpressionWrappingRule`)
- **Rule ID**: `standard:binary-expression-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `BINARY_EXPRESSION`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - When a binary expression is broken across lines, the binary operator (`+`, `-`, `*`, `&&`, `||`, etc.) must appear at the **beginning of the continuation line**, not at the end of the preceding line.

#### 44. `call-expression-wrapping` (`CallExpressionWrappingRule`)
- **Rule ID**: `standard:call-expression-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`.
- **AST Nodes**: `CALL_EXPRESSION`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Enforces consistent line breaks in call chains and chained invocations that exceed line length limits.

#### 45. `chain-method-continuation` (`ChainMethodContinuationRule`)
- **Rule ID**: `standard:chain-method-continuation`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `DOT_QUALIFIED_EXPRESSION`, `SAFE_ACCESS_EXPRESSION`.
- **Configuration**:
  - `ktlint_chain_method_rule_force_multiline_when_chain_operator_count_greater_or_equal_than`: Integer threshold (default: `4`).
- **Specification**:
  - When the count of chained call operators (`.` or `?.`) reaches or exceeds the configured threshold, the entire chain is forced to break into multiline format where every call begins on a new line.

#### 46. `chain-wrapping` (`ChainWrappingRule`)
- **Rule ID**: `standard:chain-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `DOT`, `SAFE_ACCESS`, `ELVIS`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - In chained calls that break across lines, the chain operator (`.`, `?.`, or `?:`) must be placed at the **start of the next line**, not trailing at the end of the previous line:
    ```kotlin
    // Allowed:
    val result = items
        .filter { it.active }
        .map { it.name }

    // Disallowed:
    val result = items.
        filter { it.active }.
        map { it.name }
    ```

#### 47. `class-signature` (`ClassSignatureRule`)
- **Rule ID**: `standard:class-signature`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `CLASS`, `OBJECT_DECLARATION`.
- **Configuration**:
  - `ktlint_class_signature_rule_force_multiline_when_parameter_count_greater_or_equal_than`: Optional parameter threshold to force multiline.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Formats class declarations, constructor parameters, and supertype lists.
  - If the class header exceeds `max_line_length`, constructor parameters are wrapped one per line, and the supertype list is wrapped with continuation indent.

#### 48. `context-parameter-list-wrapping` (`ContextParameterListWrappingRule`)
- **Rule ID**: `standard:context-parameter-list-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `CONTEXT_PARAMETER_LIST` (Kotlin 2.2+ feature).
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Formats Kotlin context parameter lists (`context(param: Type)`). When multiline, each parameter resides on a new line with the closing parenthesis on its own line.

#### 49. `context-receiver-wrapping` (`ContextReceiverWrappingRule`)
- **Rule ID**: `standard:context-receiver-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `CONTEXT_RECEIVER_LIST` (Kotlin context receivers).
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Formats context receiver lists (`context(Context1, Context2)`). If multiline, wraps each receiver onto a separate line.

#### 50. `enum-wrapping` (`EnumWrappingRule`)
- **Rule ID**: `standard:enum-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `CLASS_BODY` (in enum classes).
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Enum entries must either:
    - All fit on a single line separated by commas, OR
    - Each entry must reside on its own line.
  - Semicolon terminating enum entries must be placed after the last entry (either on the same line or on a new line if entries are multiline).

#### 51. `expression-operand-wrapping` (`ExpressionOperandWrappingRule`)
- **Rule ID**: `standard:expression-operand-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`.
- **AST Nodes**: `BINARY_EXPRESSION`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Ensures clean line wrapping of complex operands in multi-operator binary expressions.

#### 52. `function-expression-body` (`FunctionExpressionBodyRule`)
- **Rule ID**: `standard:function-expression-body`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN`.
- **Properties**: `indent_size`, `indent_style`, `ktlint_code_style`.
- **Specification**:
  - Standardizes the formatting of expression-bodied functions (`fun foo() = expr`).
  - When the body expression wraps to a new line, it must be indented with a continuation indent.

#### 53. `function-signature` (`FunctionSignatureRule`)
- **Rule ID**: `standard:function-signature`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUN`.
- **Configuration**:
  - `ktlint_function_signature_rule_force_multiline_when_parameter_count_greater_or_equal_than`: Integer threshold to force multiline parameters.
  - `ktlint_function_signature_body_expression_wrapping`: `default`, `always`, or `multiline`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Wraps long function signatures exceeding `max_line_length`.
  - When multiline, each parameter is placed on a separate line with indentation, followed by the closing `)` and return type specification on a new line.

#### 54. `if-else-wrapping` (`IfElseWrappingRule`)
- **Rule ID**: `standard:if-else-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `IF`, `ELSE`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Enforces that `else` or `else if` must follow the closing brace `}` of the `if` body on the **same line**:
    ```kotlin
    // Allowed:
    if (a) {
        ...
    } else {
        ...
    }

    // Disallowed:
    if (a) {
        ...
    }
    else {
        ...
    }
    ```

#### 55. `multiline-expression-wrapping` (`MultilineExpressionWrappingRule`)
- **Rule ID**: `standard:multiline-expression-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `BINARY_EXPRESSION`, `CALL_EXPRESSION`.
- **Properties**: `indent_size`, `indent_style`, `ktlint_function_signature_body_expression_wrapping`.
- **Specification**:
  - When an expression body spans multiple lines, forces it to start on a new line following the assignment operator `=`.

#### 56. `multiline-if-else` (`MultiLineIfElseRule`)
- **Rule ID**: `standard:multiline-if-else`
- **Autocorrectable**: Yes (adds `{ ... }` braces).
- **Markers**: Stable.
- **AST Nodes**: `IF`, `THEN`, `ELSE`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - If either the `then` or `else` branch of an `if-else` statement spans multiple lines, **both branches must be enclosed in curly braces `{}`**.

#### 57. `multiline-loop` (`MultilineLoopRule`)
- **Rule ID**: `standard:multiline-loop`
- **Autocorrectable**: Yes (adds `{ ... }` braces).
- **Markers**: Stable.
- **AST Nodes**: `FOR`, `WHILE`, `DO_WHILE`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - If the body of a loop (`for`, `while`, `do-while`) spans multiple lines, it **must be enclosed in curly braces `{}`**.

#### 58. `no-line-break-after-else` (`NoLineBreakAfterElseRule`)
- **Rule ID**: `standard:no-line-break-after-else`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `ELSE`.
- **Specification**:
  - Forbids a line break immediately after `else` when followed by `if`: `else if (cond)` must not be split into `else` followed by a newline and `if`.

#### 59. `no-line-break-before-assignment` (`NoLineBreakBeforeAssignmentRule`)
- **Rule ID**: `standard:no-line-break-before-assignment`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `EQ` (in property and variable declarations).
- **Specification**:
  - Forbids placing a line break immediately before the assignment operator `=` in property/variable declarations. The `=` must reside on the same line as the variable identifier/type:
    ```kotlin
    // Allowed:
    val message =
        "A long string"

    // Disallowed:
    val message
        = "A long string"
    ```

#### 60. `parameter-list-wrapping` (`ParameterListWrappingRule`)
- **Rule ID**: `standard:parameter-list-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_PARAMETER_LIST`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - When a function or constructor parameter list breaks across lines, **each parameter must be on a separate line**.
  - The closing parenthesis `)` must be on a new line matching the indentation of the declaration header.

#### 61. `parameter-wrapping` (`ParameterWrappingRule`)
- **Rule ID**: `standard:parameter-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_PARAMETER`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Enforces proper line wrapping for individual parameter components (modifiers, annotations, names, default value expressions).

#### 62. `property-wrapping` (`PropertyWrappingRule`)
- **Rule ID**: `standard:property-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `PROPERTY`.
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Wraps long property declarations, initializers, getters, and setters to prevent exceeding `max_line_length`.

#### 63. `statement-wrapping` (`StatementWrappingRule`)
- **Rule ID**: `standard:statement-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `BLOCK`, `CLASS_BODY`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Forbids multiple statements on a single line separated by semicolons:
    ```kotlin
    // Disallowed:
    val a = 1; val b = 2

    // Allowed:
    val a = 1
    val b = 2
    ```

#### 64. `wrapping` (`WrappingRule`)
- **Rule ID**: `standard:wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: General delimiters (`LPAR`, `RPAR`, `LBRACE`, `RBRACE`, `LBRACKET`, `RBRACKET`).
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - General fallback wrapping rule enforcing consistent bracket and delimiter positioning across arbitrary language constructs.


---

### 5.4 Blank Lines & Structural Spacing Rules

Blank line rules prevent excessive vertical whitespace and ensure clean visual boundaries between statements, blocks, chains, and condition branches.

#### 65. `blank-line-between-when-conditions` (`BlankLineBetweenWhenConditions`)
- **Rule ID**: `standard:blank-line-between-when-conditions`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `WHEN_ENTRY`.
- **Specification**:
  - In `when` expressions, if a branch entry contains a multiline body or multiline condition, **a blank line is required between each branch entry**.
  - Simple single-line `when` entries (e.g. `1 -> true`, `2 -> false`) do not require blank lines between them.

#### 66. `no-blank-line-at-start-of-file` (`NoBlankLineAtStartOfFileRule`)
- **Rule ID**: `standard:no-blank-line-at-start-of-file`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`.
- **AST Nodes**: `FILE`.
- **Specification**:
  - Prohibits leading blank lines at the very start of a source file before the first code element, comment, or shebang line.

#### 67. `no-blank-line-before-rbrace` (`NoBlankLineBeforeRbraceRule`)
- **Rule ID**: `standard:no-blank-line-before-rbrace`
- **Autocorrectable**: Yes (removes excess blank lines).
- **Markers**: Stable.
- **AST Nodes**: `RBRACE`.
- **Specification**:
  - Forbids blank lines immediately preceding a closing curly brace `}` in classes, functions, lambda expressions, and control blocks:
    ```kotlin
    // Disallowed:
    fun foo() {
        doSomething()

    }

    // Allowed:
    fun foo() {
        doSomething()
    }
    ```

#### 68. `no-blank-line-in-list` (`NoBlankLineInListRule`)
- **Rule ID**: `standard:no-blank-line-in-list`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `VALUE_PARAMETER_LIST`, `VALUE_ARGUMENT_LIST`, `TYPE_PARAMETER_LIST`, `TYPE_ARGUMENT_LIST`.
- **Specification**:
  - Forbids blank lines between items inside parameter lists, argument lists, and type argument lists.

#### 69. `no-blank-lines-in-chained-method-calls` (`NoBlankLinesInChainedMethodCallsRule`)
- **Rule ID**: `standard:no-blank-lines-in-chained-method-calls`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `DOT_QUALIFIED_EXPRESSION`, `SAFE_ACCESS_EXPRESSION`.
- **Specification**:
  - Forbids blank lines between chained method calls. Every call in the chain must appear on consecutive lines without empty lines in between:
    ```kotlin
    // Disallowed:
    val result = items
        .filter { it.isValid }

        .map { it.name }
    ```

#### 70. `no-consecutive-blank-lines` (`NoConsecutiveBlankLinesRule`)
- **Rule ID**: `standard:no-consecutive-blank-lines`
- **Autocorrectable**: Yes (collapses multiple blank lines to at most one).
- **Markers**: Stable.
- **AST Nodes**: `WHITE_SPACE`.
- **Specification**:
  - Limits consecutive blank lines throughout the entire file to **at most one** (i.e. no sequence of 2 or more blank lines).
  - Outside of string literals, consecutive newline sequences `\n\n\n+` are collapsed to `\n\n`.

#### 71. `no-consecutive-comments` (`NoConsecutiveCommentsRule`)
- **Rule ID**: `standard:no-consecutive-comments`
- **Autocorrectable**: No (cannot safely determine author's structural intent).
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `EOL_COMMENT`, `BLOCK_COMMENT`, `KDOC_START`, `KDOC_END`.
- **Specification**:
  - Disallows consecutive KDoc blocks (`/** ... */ /** ... */`) even if separated by blank lines.
  - Disallows consecutive block comments (`/* ... */ /* ... */`).
  - Disallows mixing KDoc, block comments, and EOL comments without blank line separation.
  - Consecutive line comments (`// line 1 \n // line 2`) are explicitly allowed as they form continuous multi-line commentary.

#### 72. `no-empty-class-body` (`NoEmptyClassBodyRule`)
- **Rule ID**: `standard:no-empty-class-body`
- **Autocorrectable**: Yes (deletes `{}`).
- **Markers**: Stable.
- **AST Nodes**: `CLASS_BODY`.
- **Specification**:
  - Forbids empty curly braces `{}` in class, interface, and object declarations when there are no member declarations or comments inside:
    ```kotlin
    // Disallowed:
    class Foo {}
    interface Bar {}

    // Allowed:
    class Foo
    interface Bar
    ```

#### 73. `no-empty-first-line-in-class-body` (`NoEmptyFirstLineInClassBodyRule`)
- **Rule ID**: `standard:no-empty-first-line-in-class-body`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `CLASS_BODY`, `LBRACE`.
- **Specification**:
  - Forbids an empty first line immediately following the opening brace `{` in a class, interface, or object body:
    ```kotlin
    // Disallowed:
    class MyClass {

        val x = 1
    }

    // Allowed:
    class MyClass {
        val x = 1
    }
    ```

#### 74. `no-empty-first-line-in-method-block` (`NoEmptyFirstLineInMethodBlockRule`)
- **Rule ID**: `standard:no-empty-first-line-in-method-block`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `BLOCK` (in functions, constructors, and getters/setters).
- **Specification**:
  - Forbids an empty first line immediately following the opening brace `{` of a function or method body.


---

### 5.5 Naming Conventions & Identifiers

Naming rules enforce idiomatic Kotlin naming conventions. Because renaming declarations requires global symbol refactoring across the entire compilation unit, these rules are **non-autocorrectable** (`auto: false`).

#### 75. `class-naming` (`ClassNamingRule`)
- **Rule ID**: `standard:class-naming`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `CLASS`, `OBJECT_DECLARATION`.
- **Specification**:
  - Class, interface, and object names must be in **`PascalCase`** (UpperCamelCase), starting with an uppercase letter and containing no underscores.
  - Exceptions: Backticked identifiers in test classes or platform interop are permitted where appropriate.

#### 76. `function-naming` (`FunctionNamingRule`)
- **Rule ID**: `standard:function-naming`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `FUN`.
- **Configuration**:
  - `ktlint_function_naming_ignore_when_annotated_with`: Comma-separated annotation list (default: `Composable`).
- **Specification**:
  - Function names must be in **`camelCase`** (lowerCamelCase), starting with a lowercase letter without underscores.
  - **Factory Functions**: Functions that act as constructors (e.g. `fun MyClass(): MyClass`) are allowed to follow `PascalCase`.
  - **Test Functions**: Enclosed in backticks (e.g. ``fun `given condition when action then result`()``) or prefixed with `test_` are permitted in test code.
  - **Composable Functions**: Functions annotated with `@Composable` are allowed to follow `PascalCase`.

#### 77. `property-naming` (`PropertyNamingRule`)
- **Rule ID**: `standard:property-naming`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `PROPERTY`.
- **Properties**: `ktlint_constant_naming`, `ktlint_serial_version_uid`.
- **Specification**:
  - Regular properties (`val`, `var`) must follow **`camelCase`**.
  - Top-level `const val` and companion object constant properties must follow **`SCREAMING_SNAKE_CASE`** (e.g. `const val MAX_RETRIES = 5`).
  - `serialVersionUID` is permitted as an explicit JVM serialization exemption.

#### 78. `backing-property-naming` (`BackingPropertyNamingRule`)
- **Rule ID**: `standard:backing-property-naming`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `PROPERTY`.
- **Properties**: `ktlint_code_style`, `ktlint_backing_property`.
- **Specification**:
  - Enforces the Kotlin convention for private backing properties. A private property prefixed with an underscore (e.g. `private val _items = MutableList()`) is permitted **only** when paired with a matching public/internal property of the same name without the underscore (`val items: List get() = _items`).
  - Standalone properties with leading underscores without a matching accessor are flagged.

#### 79. `enum-entry-name-case` (`EnumEntryNameCaseRule`)
- **Rule ID**: `standard:enum-entry-name-case`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `ENUM_ENTRY`.
- **Properties**: `ktlint_enum_entry_name_casing`.
- **Specification**:
  - Enforces consistent casing for enum entry constants:
    - Default: **`SCREAMING_SNAKE_CASE`** (e.g., `enum class Status { IN_PROGRESS, COMPLETED }`).
    - UpperCamelCase is allowed if configured or when following existing project-level styleguide preferences.


---

### 5.6 Language Constructs & Idiomatic Kotlin Rules

These rules enforce Kotlin idioms, eliminate redundant syntax (semicolons, explicit Unit returns, unnecessary template curlies, empty invocation parens), and enforce safe condition grouping.

#### 80. `annotation` (`AnnotationRule`)
- **Rule ID**: `standard:annotation`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `MODIFIER_LIST`, `ANNOTATION_ENTRY`.
- **Properties**: `ktlint_code_style`, `indent_size`, `indent_style`, `ktlint_annotations_with_parameters_not_to_be_wrapped`.
- **Specification**:
  - Annotations with parameters (e.g. `@MyAnnotation(param = "value")`) must each appear on their own separate line preceding the annotated construct.
  - Parameterless annotations (e.g. `@JvmStatic`, `@Deprecated`) may appear on the same line as the declaration or be grouped on a single line preceding it.

#### 81. `function-literal` (`FunctionLiteralRule`)
- **Rule ID**: `standard:function-literal`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `FUNCTION_LITERAL` (Lambda expressions).
- **Properties**: `indent_size`, `indent_style`, `max_line_length`.
- **Specification**:
  - Formats lambda expressions. If lambda parameters or body span multiple lines, forces `->` to follow parameters with a line break before the body.
  - Ensures exactly one space before and after the lambda parameter list arrow `->`.

#### 82. `if-else-bracing` (`IfElseBracingRule`)
- **Rule ID**: `standard:if-else-bracing`
- **Autocorrectable**: Yes (adds missing braces).
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `IF`, `ELSE`.
- **Specification**:
  - Enforces symmetrical bracing in `if-else` chains: if any branch of an `if-else` statement has curly braces `{ ... }`, **all other branches in the chain must also have curly braces**.
    ```kotlin
    // Disallowed (asymmetrical):
    if (condition) {
        doSomething()
    } else
        doOther()

    // Allowed:
    if (condition) {
        doSomething()
    } else {
        doOther()
    }
    ```

#### 83. `lambda-return` (`LambdaReturnRule`)
- **Rule ID**: `standard:lambda-return`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`.
- **AST Nodes**: `RETURN` (labeled return in lambda).
- **Specification**:
  - Detects and removes redundant labeled `return@label` statements at the end of a lambda when the trailing expression is already returned implicitly.

#### 84. `mixed-condition-operators` (`MixedConditionOperatorsRule`)
- **Rule ID**: `standard:mixed-condition-operators`
- **Autocorrectable**: No (compiler cannot guess developer intention for precedence grouping).
- **Markers**: Stable.
- **AST Nodes**: `BINARY_EXPRESSION` (`&&`, `||`).
- **Specification**:
  - Prohibits mixing logical AND (`&&`) and logical OR (`||`) operators in the same expression without explicit parentheses to disambiguate precedence:
    ```kotlin
    // Disallowed:
    if (a && b || c) { ... }

    // Allowed:
    if ((a && b) || c) { ... }
    if (a && (b || c)) { ... }
    ```

#### 85. `no-semi` (`NoSemicolonsRule`)
- **Rule ID**: `standard:no-semi`
- **Autocorrectable**: Yes (removes unnecessary semicolons).
- **Markers**: Stable.
- **AST Nodes**: `SEMICOLON`.
- **Specification**:
  - Removes redundant trailing semicolons `;` at the end of statements and declarations.
  - Semicolons are preserved **only** when syntactically required:
    - To separate multiple statements on the same line (though `statement-wrapping` discourages this).
    - To terminate the enum constant list before class member declarations in an `enum class`.

#### 86. `no-unit-return` (`NoUnitReturnRule`)
- **Rule ID**: `standard:no-unit-return`
- **Autocorrectable**: Yes (removes `: Unit`).
- **Markers**: Stable.
- **AST Nodes**: `FUN`, `TYPE_REFERENCE`.
- **Specification**:
  - Removes redundant explicit `: Unit` return type declarations from functions with block bodies:
    ```kotlin
    // Disallowed:
    fun doSomething(): Unit {
        println("Hello")
    }

    // Allowed:
    fun doSomething() {
        println("Hello")
    }
    ```
  - Note: Functions with expression bodies (`fun foo(): Unit = ...`) or interface method declarations without bodies may retain `: Unit` when needed for type inference.

#### 87. `string-template` (`StringTemplateRule`)
- **Rule ID**: `standard:string-template`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `LONG_STRING_TEMPLATE_ENTRY` (`${identifier}`).
- **Specification**:
  - Simplifies `${identifier}` to `$identifier` when curly braces in string templates are redundant:
    ```kotlin
    // Disallowed:
    val msg = "Hello, ${name}!"

    // Allowed:
    val msg = "Hello, $name!"
    ```
  - Retains curly braces when followed by an alphanumeric character or underscore that would otherwise be parsed as part of the identifier (e.g. `"${name}_postfix"`).

#### 88. `string-template-indent` (`StringTemplateIndentRule`)
- **Rule ID**: `standard:string-template-indent`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `STRING_TEMPLATE` (Raw multiline strings `"""..."""`).
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Enforces proper indentation of multiline raw strings that use `.trimIndent()` or `.trimMargin()`.
  - Ensures the closing `"""` delimiter aligns correctly with the enclosing statement.

#### 89. `unnecessary-parentheses-before-trailing-lambda` (`UnnecessaryParenthesesBeforeTrailingLambdaRule`)
- **Rule ID**: `standard:unnecessary-parentheses-before-trailing-lambda`
- **Autocorrectable**: Yes (removes `()`).
- **Markers**: Stable.
- **AST Nodes**: `VALUE_ARGUMENT_LIST`, `LAMBDA_ARGUMENT`.
- **Specification**:
  - Removes empty parentheses `()` when a function call has no arguments other than a trailing lambda:
    ```kotlin
    // Disallowed:
    val doubled = list.map() { it * 2 }

    // Allowed:
    val doubled = list.map { it * 2 }
    ```

#### 90. `when-entry-bracing` (`WhenEntryBracing`)
- **Rule ID**: `standard:when-entry-bracing`
- **Autocorrectable**: Yes (adds braces `{ ... }`).
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `WHEN_ENTRY`.
- **Specification**:
  - Requires braces `{ ... }` around the body of a `when` entry if that entry body spans multiple lines or contains multiple statements.


---

### 5.7 Comments & KDoc Rules

Comment rules ensure that source documentation, inline comments, and KDoc docstrings are consistently formatted, readable, and properly aligned.

#### 91. `block-comment-initial-star-alignment` (`BlockCommentInitialStarAlignmentRule`)
- **Rule ID**: `standard:block-comment-initial-star-alignment`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `BLOCK_COMMENT`.
- **Specification**:
  - In a multiline block comment where lines start with an asterisk `*`, **each asterisk must align vertically** with the initial asterisk of `/*`:
    ```kotlin
    // Allowed:
    /*
     * Line 1
     * Line 2
     */

    // Disallowed:
    /*
      * Line 1
    * Line 2
     */
    ```

#### 92. `comment-spacing` (`CommentSpacingRule`)
- **Rule ID**: `standard:comment-spacing`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `EOL_COMMENT`, `BLOCK_COMMENT`.
- **Specification**:
  - Requires exactly one space following the comment delimiter before the comment text: `// text` instead of `//text`.
  - Exactly one space after `/*` and before `*/` in block comments.
  - Exceptions: Shebang lines (`#!/...`), ktlint suppression directives (`// ktlint-disable ...`), compiler directives, ASCII art, commented-out code, and URLs (`// https://...`).

#### 93. `comment-wrapping` (`CommentWrappingRule`)
- **Rule ID**: `standard:comment-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `EOL_COMMENT`, `BLOCK_COMMENT`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - Disallows block comments on the same line after a code statement when that statement wraps to multiple lines.
  - Ensures comments attached to multiline declarations are placed on preceding lines rather than trailing inline.

#### 94. `kdoc` (`KdocRule`)
- **Rule ID**: `standard:kdoc`
- **Autocorrectable**: No (documentation content requires manual editorial review).
- **Markers**: Stable.
- **AST Nodes**: `KDOC`.
- **Specification**:
  - Validates KDoc structure and tags (`@param`, `@return`, `@throws`, `@see`, `@sample`).
  - Flags duplicate tags, missing tag descriptions, or malformed docstrings.

#### 95. `kdoc-delimiter` (`KdocDelimiterRule`)
- **Rule ID**: `standard:kdoc-delimiter`
- **Autocorrectable**: Yes.
- **Markers**: `RuleV2.Experimental`, `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `KDOC`.
- **Specification**:
  - Standardizes opening `/**` and closing `*/` delimiters. In `ktlint_official` style, multiline KDocs must have `/**` on its own line followed by leading `*` on each subsequent line.

#### 96. `kdoc-wrapping` (`KdocWrappingRule`)
- **Rule ID**: `standard:kdoc-wrapping`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `KDOC`.
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - When KDoc content exceeds line boundaries or contains multiline tags, wraps KDoc lines and maintains vertical asterisk alignment.

#### 97. `no-single-line-block-comment` (`NoSingleLineBlockCommentRule`)
- **Rule ID**: `standard:no-single-line-block-comment`
- **Autocorrectable**: Yes (converts `/* ... */` to `// ...`).
- **Markers**: `RuleV2.OfficialCodeStyle`.
- **AST Nodes**: `BLOCK_COMMENT`.
- **Specification**:
  - Replaces single-line block comments with standard end-of-line comments:
    ```kotlin
    // Disallowed:
    /* This is a comment */

    // Allowed:
    // This is a comment
    ```
  - Exception: Block comments embedded inside expressions (e.g. `foo(/* flag = */ true)`) are preserved.

#### 98. `type-argument-comment` (`TypeArgumentCommentRule`)
- **Rule ID**: `standard:type-argument-comment`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_ARGUMENT_LIST`.
- **Specification**:
  - Disallows misplaced inline comments inside type argument lists (e.g. `Map<String, /* value */ Int>`) that disrupt syntax parsing and visual hierarchy.

#### 99. `type-parameter-comment` (`TypeParameterCommentRule`)
- **Rule ID**: `standard:type-parameter-comment`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `TYPE_PARAMETER_LIST`.
- **Specification**:
  - Disallows disruptive inline comments inside generic type parameter declarations.

#### 100. `value-argument-comment` (`ValueArgumentCommentRule`)
- **Rule ID**: `standard:value-argument-comment`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_ARGUMENT_LIST`.
- **Specification**:
  - Validates placement of comments inside function call argument lists, ensuring comments do not obscure commas or parameter names.

#### 101. `value-parameter-comment` (`ValueParameterCommentRule`)
- **Rule ID**: `standard:value-parameter-comment`
- **Autocorrectable**: No.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_PARAMETER_LIST`.
- **Specification**:
  - Validates placement of comments inside function and constructor parameter lists.


---

### 5.8 Indentation, Line Length & Trailing Commas

These rules control structural indentation depth, line length limits, file termination, and trailing comma consistency.

#### 102. `indent` (`IndentationRule`)
- **Rule ID**: `standard:indent`
- **Autocorrectable**: Yes.
- **Markers**: Stable (`RunAsLateAsPossible`).
- **AST Nodes**: All code elements and indentation whitespace.
- **Properties**: `indent_size` (default: 4), `indent_style` (default: `space`), `ktlint_code_style`.
- **Specification**:
  - Enforces uniform indentation for all blocks, classes, functions, control flows, and continuation expressions.
  - Multi-level continuation indenting applies to binary expressions, method chains, and nested lambda bodies.
  - Aligns closing brackets (`)`, `]`, `}`) with the declaration/invocation start column.

#### 103. `max-line-length` (`MaxLineLengthRule`)
- **Rule ID**: `standard:max-line-length`
- **Autocorrectable**: No (structural line breaking is delegated to individual wrapping rules).
- **Markers**: Stable (`RunAsLateAsPossible`).
- **AST Nodes**: `FILE`.
- **Properties**: `max_line_length` (default: 140 for `ktlint_official`, 120 for others), `ktlint_ignore_back_ticked_identifier`.
- **Specification**:
  - Emits violations for any line exceeding `max_line_length`.
  - **Exemptions**:
    - `package` and `import` statements are never flagged.
    - Long raw URLs inside single-line or KDoc comments.
    - Backticked test identifiers if `ktlint_ignore_back_ticked_identifier = true`.

#### 104. `final-newline` (`FinalNewlineRule`)
- **Rule ID**: `standard:final-newline`
- **Autocorrectable**: Yes (appends or strips final newline).
- **Markers**: Stable (`RunAsLateAsPossible`).
- **AST Nodes**: `FILE`.
- **Properties**: `insert_final_newline` (default: `true`).
- **Specification**:
  - When `insert_final_newline = true`, ensures that the file ends with exactly one newline character (`\n`).
  - When `insert_final_newline = false`, ensures that the file does not end with an empty line.

#### 105. `trailing-comma-on-call-site` (`TrailingCommaOnCallSiteRule`)
- **Rule ID**: `standard:trailing-comma-on-call-site`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_ARGUMENT_LIST`, `COLLECTION_LITERAL_EXPRESSION`.
- **Configuration**:
  - `ij_kotlin_allow_trailing_comma_on_call_site`: Boolean (default: `true` in `ktlint_official`, `false` in `intellij_idea` and `android_studio`).
- **Specification**:
  - When enabled:
    - If an argument list is **multiline**, a trailing comma **must be present** after the last argument before the closing parenthesis `)` or bracket `]`.
    - If an argument list is **single-line**, trailing commas **must be removed**.
  - When disabled:
    - All trailing commas at call sites are stripped.

#### 106. `trailing-comma-on-declaration-site` (`TrailingCommaOnDeclarationSiteRule`)
- **Rule ID**: `standard:trailing-comma-on-declaration-site`
- **Autocorrectable**: Yes.
- **Markers**: Stable.
- **AST Nodes**: `VALUE_PARAMETER_LIST`, `TYPE_PARAMETER_LIST`, `WHEN_ENTRY`, `DESTRUCTURING_DECLARATION`.
- **Configuration**:
  - `ij_kotlin_allow_trailing_comma`: Boolean (default: `true` in `ktlint_official`, `false` in `intellij_idea` and `android_studio`).
- **Properties**: `indent_size`, `indent_style`.
- **Specification**:
  - When enabled:
    - In **multiline** parameter lists, type parameter lists, when entry lists, or destructuring declarations, **a trailing comma is required** after the last item.
    - In **single-line** declarations, trailing commas **must be removed**.
  - When disabled:
    - All trailing commas at declaration sites are stripped.

