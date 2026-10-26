# Kost

Fast JVM-free Kotlin linter, formatter, and language server.

## Highlights

- No JVM and Gradle overheads
- Fast and memory efficient
- [ktlint](https://github.com/ktlint/ktlint) compatible linter
- [ktfmt](https://github.com/Kotlin/ktfmt) compatible formatter
- Language server indexes project sources, kotlin stdlib, gradle dependencies, and android jar automatically

## Installations

- Terminal

Linux / macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/letmutex/kost/main/install.sh | sh
```

Windows:

```powershell
irm https://raw.githubusercontent.com/letmutex/kost/main/install.ps1 | iex
```

- Download binaries from [GitHub Releases](https://github.com/letmutex/kost/releases).

- Install using Cargo: `cargo install kost`

- Build from source: 

```bash
git clone https://github.com/letmutex/kost.git
cd kost
cargo build --release
```

## Quickstart

**Lint:**

```bash
kost lint
kost lint -D warnings # Deny warnings in CI
kost lint androidApp/ # Specify folders or files
kost lint --rulesets statistics # Specify lint rulesets
```

**Format:**

```bash
kost fmt # Format workspace source files
kost fmt --check # Preview format issues
kost fmt androidApp/ # Specify folders or files
```

**Language Server in Zed:**

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

## Documentations

[Language Server](docs/language-server.md)

[Lint](docs/lint/basic.md)

[Format](docs/format/basic.md)

## Acknowledgements

[tree-sitter-kotlin](https://github.com/fwcd/tree-sitter-kotlin)

[ktlint](https://github.com/ktlint/ktlint)

[ktfmt](https://github.com/Kotlin/ktfmt)

## License

[Apache License 2.0](LICENSE)
