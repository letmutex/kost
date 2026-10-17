## Language Server

Kost is a fast static Kotlin language server. Use `-ls` or `--language-server` option to launch it as a language server.

## Implemented Features

- Document Outline & Symbols
- Workspace Symbols
- Doc & Type Hover
- Go to Definition
- Auto-Completion
- Auto-Import on Completion
- Signature Help
- Find References

## Limitations

- No compiler diagnostics or error reporting
- No code actions, rename
- Static dependency inspection without running Gradle daemon or compiler plugins
- Heuristic type inference rather than a full compiler frontend, and the support is limited

## Editor Setup

Zed:

1. Install [Kotlin Extension](https://zed.dev/docs/languages/kotlin)
2. Overwrite `kotlin-language-server` binary (or `kotlin-lsp`) in `settings.json`

```json
{
  "lsp": {
    "kotlin-language-server": {
      "binary": {
        "path": "path/to/kost",
        "arguments": ["--language-server"]
      }
    }
  }
}
```

3. Restart Zed

## LSP Options

| field | values | default | description |
| --- | --- | --- | --- |
| `indexing_logging` | `none`, `default`, `verbose` | `default` | Logging level for workspace and dependency indexing |
| `interaction_logging` | `none`, `default` | `none` | Logging level for LSP requests and latency |
| `variable_hover_type_inference` | `auto`, `always`, `always_full_name`, `always_with_trace` | `auto` | Type inference detail shown on variable hover |
| `auto_import` | `true`, `false` | `true` | Automatically import symbols on completion |
