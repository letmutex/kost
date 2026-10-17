# Code Formatting

Kost provides a fast, JVM-free Kotlin code formatter compatible with [ktfmt](https://github.com/Kotlin/ktfmt). It formats Kotlin source files (`.kt`) and scripts (`.kts`) using AST-based pretty printing.

## Usage

Format files in place:

```bash
# Format all Kotlin files in the current workspace
kost fmt

# Format specific directories or files
kost fmt src/
kost fmt src/Main.kt

# Format using glob patterns
kost fmt "src/**/*.kt"
```

Verify formatting without modifying files (e.g., in CI):

```bash
kost fmt --check
kost fmt -c src/
```

Format from standard input:

```bash
cat File.kt | kost fmt -
cat File.kt | kost fmt --stdin-path src/File.kt -
```

`kost format` is supported as an alias for `kost fmt`.

## Style Presets

Kost supports three formatting style presets:

| Preset | Block Indent | Continuation Indent | Trailing Comma Strategy | Max Width | CLI Flag |
| --- | --- | --- | --- | --- | --- |
| `kotlinlang` *(default)* | 4 spaces | 4 spaces | `complete` | 100 | `--kotlinlang-style` or `--style kotlinlang` |
| `meta` | 2 spaces | 4 spaces | `only_add` | 100 | `--meta-style` or `--style meta` |
| `google` | 2 spaces | 2 spaces | `complete` | 100 | `--google-style` or `--style google` |

### Selecting a Style

```bash
# Use official Kotlin style guide (default)
kost fmt --kotlinlang-style

# Use Meta style preset
kost fmt --meta-style

# Use Google style preset
kost fmt --google-style

# Or via --style option
kost fmt --style google src/
```

## Features

- **AST-Based Formatting**: Built directly on the Kotlin grammar to ensure correctness and syntax preservation.
- **Trailing Comma Management**:
  - `only_add`: Inserts trailing commas when lists or parameters wrap across lines; leaves single-line commas intact.
  - `complete`: Automatically adds trailing commas when multiline and strips them when elements fit on a single line.
  - `none`: Preserves user-written trailing commas without adding or removing them.
- **Unused Import Removal**: Automatically strips unused import directives during formatting.
- **Script Support**: Seamlessly formats Kotlin scripts (`.kts`) and preserves shebang lines (`#!`) on line 1.
- **Newline Normalization**: Respects target file line endings (`LF` or `CRLF`) and ensures correct final newline placement.
- **EditorConfig Integration**: Automatically inherits project settings from `.editorconfig` files unless disabled. See [EditorConfig Integration](editor-config.md).

## CLI Options

| Option | Value | Default | Description |
| --- | --- | --- | --- |
| `-c, --check` | Flag | `false` | Check formatting and print unified diffs without modifying files. Exits with non-zero code if unformatted files exist. |
| `--print-mode` | `auto`, `full`, `truncated`, `collapsed` | `auto` | Presentation mode in check mode: auto, full, truncated, or collapsed. |
| `--style` | `kotlinlang`, `meta`, `google` | `kotlinlang` | Active style preset. |
| `--kotlinlang-style` | Flag | Active | Shortcut for `--style kotlinlang`. |
| `--meta-style` | Flag | - | Shortcut for `--style meta`. |
| `--google-style` | Flag | - | Shortcut for `--style google`. |
| `--editorconfig` | `<PATH>` | None | Explicit or fallback `.editorconfig` file path. |
| `--no-editorconfig` | Flag | `false` | Disable `.editorconfig` discovery and property inheritance. |
| `--stdin-path` | `<PATH>` | None | Virtual file path for stdin formatting (used for `.editorconfig` matching and file type detection). |
| `--reject-parse-errors` | Flag | `false` | Treat syntax/parse errors as check failures in check mode. |
| `-h, --help` | Flag | - | Print command help information. |

## Target Resolution

- When no targets are specified, Kost walks the current working directory recursively, discovering all `.kt` and `.kts` files while honoring `.gitignore` rules.
- Glob patterns containing `*`, `?`, or `[` are evaluated against the repository tree.
- Use `--` to separate options from paths if a target path begins with a dash (e.g. `kost fmt --check -- -special-name.kt`).

## See Also

- [EditorConfig Integration](editor-config.md)
- [Lint Documentation](../lint/basic.md)
