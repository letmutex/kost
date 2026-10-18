# EditorConfig Formatting

Kost automatically discovers and applies formatting rules from `.editorconfig` files. This allows teams to share consistent formatting rules across editors and CLI tooling without extra configuration.

## Discovery and Hierarchy

Kost resolves `.editorconfig` files using standard hierarchy rules:

1. **Upward Traversal**: For each target file, Kost checks the file's parent directory and climbs upward toward the root directory until reaching a file containing `root = true` or the filesystem root.
2. **Top-Down Merging**: Properties from the top-most (root) configuration are applied first, with child directories overriding parent values.
3. **Caching**: Parsed `.editorconfig` files are cached in memory for fast resolution across thousands of files.
4. **Fallback Config**: An explicit fallback configuration can be provided via `--editorconfig <PATH>`.
5. **Disabling**: To bypass all `.editorconfig` discovery and use only CLI presets, pass `--no-editorconfig`.

## Supported Formatting Properties

| Property | Values | Description |
| --- | --- | --- |
| `indent_style` | `space`, `tab` | Indentation character style (default: `space`). |
| `indent_size` | Integer, `tab` | Block indentation size in spaces. If `tab`, uses `tab_width` or 4. |
| `tab_width` | Integer | Tab stop width in spaces. |
| `ij_kotlin_indent_size` | Integer | Kotlin-specific block indentation size. Takes precedence over `indent_size`. |
| `ij_continuation_indent_size` | Integer | Continuation indentation size for wrapped lines. |
| `ij_kotlin_continuation_indent_size` | Integer | Alias for `ij_continuation_indent_size`. |
| `continuation_indent_size` | Integer | Alias for `ij_continuation_indent_size`. |
| `max_line_length` | Integer, `off` | Maximum column width before wrapping lines. Set to `off` to disable column limits. |
| `insert_final_newline` | `true`, `false` | Whether to ensure files end with a trailing newline. |
| `end_of_line` | `lf`, `crlf`, `cr` | Target line ending format. |
| `ktfmt_style` | `kotlinlang`, `meta`, `google` | Overrides the base formatter style preset for matching files. |
| `ktfmt_trailing_comma_management_strategy` | `none`, `only_add`, `complete` | Trailing comma insertion strategy. |
| `ij_kotlin_allow_trailing_comma` | `true`, `false` | Enables or disables trailing commas on declaration sites. |
| `ij_kotlin_allow_trailing_comma_on_call_site` | `true`, `false` | Enables or disables trailing commas on call sites. Falls back to `ij_kotlin_allow_trailing_comma`. |
| `ij_formatter_enabled` | `true`, `false` | Enables or disables code formatting for matching files (JetBrains standard). |
| `ktfmt` *(Experimental)* | `enabled`, `disabled` | Enables or disables code formatting for matching files. |

> [!NOTE]
> If both `ij_kotlin_allow_trailing_comma` and `ij_kotlin_allow_trailing_comma_on_call_site` are set to `false`, Kost sets the trailing comma strategy to `none`.

> [!WARNING]
> `ktfmt = disabled` is an **experimental** property. Because official `ktfmt` does not yet natively support `.editorconfig`-based file disabling (tracked in [Kotlin/ktfmt#686](https://github.com/Kotlin/ktfmt/issues/686)), this support might be deprecated or removed in the future if official `ktfmt`'s configuration differs. For standard, forward-compatible setups, prefer `ij_formatter_enabled = false`.

## Property Precedence

When properties overlap, Kost resolves them in the following order:

1. **Formatter Enablement**: `ktfmt = disabled|enabled` overrides `ij_formatter_enabled = false|true` (default: enabled).
2. **Style Preset Baseline**: Defaults chosen via CLI (e.g., `--style kotlinlang`).
3. **`ktfmt_style`**: If specified in `.editorconfig`, overrides the base style preset.
4. **Specific Indent Overrides**: `ij_kotlin_indent_size` overrides `indent_size`.
5. **Specific Comma Overrides**: `ktfmt_trailing_comma_management_strategy` takes precedence over IntelliJ-style trailing comma booleans.

## Example `.editorconfig`

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

[*.{kt,kts}]
indent_style = space
indent_size = 4
continuation_indent_size = 4
max_line_length = 120
ij_kotlin_allow_trailing_comma = true
ij_kotlin_allow_trailing_comma_on_call_site = true

# Use Google style preset for test sources
[**/test/**.kt]
ktfmt_style = google
indent_size = 2
continuation_indent_size = 2

# Exclude generated sources from code formatting
[**/generated/**.kt]
ij_formatter_enabled = false
```

## See Also

- [Format Basics](basic.md)
- [Lint EditorConfig Integration](../lint/editor-config.md)
