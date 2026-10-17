# EditorConfig Linting

Kost natively supports `.editorconfig` properties compatible with [ktlint](https://github.com/ktlint/ktlint).

## Rule Toggles

Rules are evaluated in order of specificity:
1. `ktlint_standard_<rule_id>` (specific rule toggle)
2. `ktlint_standard` (ruleset toggle)
3. Default (enabled for `standard` rules)

```ini
[*.{kt,kts}]
# Ruleset toggles (enabled/disabled, true/false, 1/0)
ktlint_standard = enabled
ktlint_experimental = disabled

# Rule toggles (supports kebab-case and snake_case)
ktlint_standard_no-semi = disabled
ktlint_standard_no_wildcard_imports = disabled

# Scoped overrides
[**/test/**.kt]
ktlint_standard_function-naming = disabled
max_line_length = off
```

## Code Style Presets

```ini
[*.{kt,kts}]
ktlint_code_style = ktlint_official # official, intellij_idea, android_studio
```

## Configurable Properties

| Property | Values | Target Rule | Description |
| --- | --- | --- | --- |
| `max_line_length` | Integer, `off` | `standard:max-line-length` | Max column width (`off` disables limit) |
| `indent_size` | Integer, `tab` | `standard:indent` | Block indentation in spaces |
| `ij_kotlin_indent_size` | Integer | `standard:indent` | Kotlin block indent (overrides `indent_size`) |
| `ij_continuation_indent_size` | Integer | `standard:indent` | Continuation indent size |
| `insert_final_newline` | `true`, `false` | `standard:final-newline` | Require final trailing newline |
| `end_of_line` | `lf`, `crlf`, `cr` | `standard:final-newline` | Expected line ending |
| `ij_kotlin_allow_trailing_comma` | `true`, `false` | `standard:trailing-comma-on-declaration-site` | Declaration trailing comma |
| `ij_kotlin_allow_trailing_comma_on_call_site` | `true`, `false` | `standard:trailing-comma-on-call-site` | Call site trailing comma |

## CLI Options

```bash
kost lint --editorconfig path/to/.editorconfig # Explicit/fallback config
kost lint --no-editorconfig                   # Disable .editorconfig discovery
```

## See Also

- [Lint Basics](basic.md)
- [Built-in Rulesets](builtin-rulesets.md)
- [Disabling Rules and Suppressions](disable-rules.md)
- [Format EditorConfig](../format/editor-config.md)
