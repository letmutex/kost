# Linting

Kost includes a high-performance, JVM-free Kotlin static linter. It executes compiled AST rules compatible with [ktlint](https://github.com/ktlint/ktlint) and configurable metric/statistic rules defined in YAML.

## Usage

Lint files in the workspace:

```bash
# Lint all Kotlin files in the current repository
kost lint

# Lint specific directories or files
kost lint src/
kost lint src/Main.kt

# Lint using glob patterns
kost lint "src/**/*.kt"

# Deny warnings in CI
kost lint -D warnings
kost lint --deny warnings src/
```

Select specific rulesets:

```bash
# Run both the standard AST ruleset and code statistics ruleset
kost lint --rulesets standard,statistics

# Run only statistics metrics
kost lint --rulesets statistics src/
```

Lint from standard input:

```bash
cat File.kt | kost lint -
cat File.kt | kost lint --stdin-path src/File.kt -
```

## Ruleset Discovery

When the `kost lint` command runs, it selects rules according to the following order:

1. **Explicit `--rulesets` flag**: If specified, Kost parses the comma-separated list of virtual ruleset names (`standard`, `statistics`) or YAML file paths.
2. **`kost.yml`**: If present in the current working directory, Kost automatically loads rules and configuration from `kost.yml`.
3. **`kost.yaml`**: If present, Kost loads configuration from `kost.yaml`.
4. **Default**: If neither configuration file exists and no ruleset flag is provided, Kost runs the built-in `standard` ruleset.

## Diagnostic Output

Violations are printed to standard output in standard compiler/linter format:

```text
src/Main.kt:12:25: [standard:no-semi] Unnecessary semicolon (warning)
src/Service.kt:45:1: [statistics:long-function] Function 'processOrder' is 82 lines (limit: 60) (warning)
```

## Disabling Rules and Suppressions

Rules can be disabled project-wide via `.editorconfig` or `kost.yml`, or locally suppressed in code using `@Suppress` or `@file:Suppress`:

```kotlin
@file:Suppress("ktlint:standard:no-wildcard-imports")

@Suppress("ktlint:standard:no-semi")
fun computeTotal() {
    val total = 100;
}
```

For comprehensive guidance on all approaches (annotations, `.editorconfig`, `kost.yml`, and CLI flags), see [Disabling Rules and Suppressions](disable-rules.md).

## CLI Options

| Option | Value | Default | Description |
| --- | --- | --- | --- |
| `--rulesets` | `<LIST>` | Auto | Comma-separated list of rulesets or config files (e.g. `standard,statistics` or `team.yaml`). |
| `-D, --deny` | `warnings` | None | Treat specified diagnostics as errors (e.g. `-D warnings`, `-Dwarnings`, or `--deny=warnings`). |
| `--print-mode` | `auto`, `full`, `truncated`, `collapsed` | `auto` | Presentation mode: auto, full, truncated, or collapsed. |
| `--editorconfig` | `<PATH>` | None | Explicit or fallback `.editorconfig` file path. |
| `--no-editorconfig` | Flag | `false` | Disable `.editorconfig` discovery and rule toggle inheritance. |
| `--stdin-path` | `<PATH>` | None | Virtual file path for stdin linting (used for `.editorconfig` path matching). |
| `-h, --help` | Flag | - | Print command help information. |

## Target Resolution

- When no targets are provided, Kost automatically inspects all `.kt` and `.kts` files in the repository tree, honoring `.gitignore` exclusion rules.
- Glob patterns containing `*`, `?`, or `[` are evaluated against workspace paths.
- Use `--` to separate arguments if target filenames begin with dashes.

## See Also

- [Built-in Rulesets](builtin-rulesets.md)
- [Custom Rules](custom-rules.md)
- [Disabling Rules and Suppressions](disable-rules.md)
- [EditorConfig Integration](editor-config.md)
- [Code Formatting](../format/basic.md)
