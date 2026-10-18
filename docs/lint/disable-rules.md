# Disabling Rules and Suppressions

Kost provides four primary ways to disable rules or suppress violations:

| Approach | Scope | Example |
| --- | --- | --- |
| **In-Code Annotations** | File, declaration, or line | `@Suppress("ktlint:standard:no-semi")` |
| **EditorConfig** | Project or path pattern | `ktlint_standard_no-semi = disabled` |
| **Config File (`kost.yml`)** | Project-wide | `severity: none` or selective `includes` |
| **CLI Flags** | Command execution | `kost lint --rulesets statistics` |

## 1. In-Code Annotations (`@Suppress`)

Use standard `@Suppress` or `@file:Suppress` annotations compatible with ktlint:

```kotlin
// File-wide suppression (before package declaration)
@file:Suppress("ktlint")                               // Suppress all rules
@file:Suppress("ktlint:standard:no-wildcard-imports")  // Suppress specific rule

package com.example

// Declaration or statement suppression
@Suppress("ktlint")
class LegacyClass {
    @Suppress("ktlint:standard:no-semi")
    val value = 1;
}
```

Accepted IDs: `"ktlint"`, `"ktlint:suppress-all-rules"`, `"ktlint:standard:<rule>"`, or shorthand `"ktlint:<rule>"`.

## 2. EditorConfig (`.editorconfig`)

Disable all rules globally, entire rulesets, or specific rules across files or directory glob patterns:

```ini
[*.{kt,kts}]
ktlint = disabled                      # Disable all ktlint rules globally
ktlint_standard = disabled             # Disable entire ruleset
ktlint_standard_no-semi = disabled     # Disable specific rule (kebab-case)
ktlint_standard_no_wildcard_imports = disabled # Snake-case is also supported

# Scope rule relaxations to test sources
[**/test/**.kt]
ktlint_standard_function-naming = disabled
```

## 3. Configuration File (`kost.yml`)

### Set Severity to `none`

```yaml
lint:
  includes:
    - standard
  rules:
    - id: standard:no-semi
      severity: none
```

### Selective Rule Inclusion

Include only desired rules rather than importing the entire ruleset:

```yaml
lint:
  includes:
    - standard:no-wildcard-imports
    - standard:package-name
    - statistics:long-function
```

## 4. CLI Flags

Run specific rulesets or bypass `.editorconfig` overrides:

```bash
kost lint --rulesets statistics # Run only statistics, omitting standard rules
kost lint --no-editorconfig     # Ignore .editorconfig disable settings
```

## See Also

- [Lint Basics](basic.md)
- [Built-in Rulesets](builtin-rulesets.md)
- [Custom Rules](custom-rules.md)
- [EditorConfig Integration](editor-config.md)
