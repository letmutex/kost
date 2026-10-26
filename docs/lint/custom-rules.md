# Custom Rules

Kost allows you to define declarative custom lint rules and override existing rules using YAML configuration files (`kost.yml` or custom ruleset files).

## Configuration File Structure

Create a `kost.yml` in the root of your project:

```yaml
lint:
  # Included rulesets or files
  includes:
    - standard
    - statistics
    - ./rules/team-conventions.yaml
    - standard:no-wildcard-imports

  # Rule overrides or custom definitions
  rules:
    - id: standard:no-semi
      severity: error

    - id: statistics:long-function
      options:
        max_lines: 80

    - id: max-class-methods
      severity: warning
      options:
        max_methods: 15
      where:
        class.methods_count: gt(options.max_methods)
      message: "Class '${class.name}' has ${class.methods_count} methods (limit: ${options.max_methods})"
```

## Including Rules and Rulesets

The `includes` section supports several include styles:

### Full Ruleset or File

```yaml
lint:
  includes:
    - standard                  # Built-in standard ruleset
    - statistics                # Built-in statistics ruleset
    - ./rules/architecture.yaml # Local YAML file (relative to current file)
```

### Qualified Single Rule Import

Import a single rule from another ruleset or file:

```yaml
lint:
  includes:
    - standard:no-semi
    - statistics:long-function
    - ./rules/shared.yaml:enforce-immutability
```

### Structured Multi-Rule Import

```yaml
lint:
  includes:
    - path: ./rules/shared.yaml
      rules:
        - disallow-raw-types
        - ban-blocking-calls
```

## Overriding Existing Rules

To modify a built-in rule or an included rule, specify its fully-qualified ID containing a colon (`<scope>:<name>`):

### Overriding AST Rules

For compiled standard AST rules (e.g., `standard:no-semi`), you can adjust the diagnostic severity:

```yaml
lint:
  includes:
    - standard
  rules:
    # Promote a warning to an error
    - id: standard:no-wildcard-imports
      severity: error

    # Disable a rule
    - id: standard:no-semi
      severity: none
```

Supported severity levels are `error`, `warning`, and `none` (disables the rule).

### Overriding Statistic Rules

For metric rules (such as `statistics:long-function`), you can override severities, options, where clauses (if allowed), and messages:

```yaml
lint:
  includes:
    - statistics
  rules:
    - id: statistics:long-function
      severity: error
      options:
        max_lines: 100
      message: "Function '${function.name}' exceeds maximum permitted size of ${options.max_lines} lines"
```

## Defining Custom Rules

Custom rules are declared with an unqualified kebab-case ID (without colons).

### Rule Schema

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | String | Yes | Unique rule identifier in kebab-case (e.g. `no-large-class`). |
| `severity` | `warning`, `error` | No | Diagnostic severity (default: `warning`). |
| `allow_where_override` | Boolean | No | Whether consumers can override the `where` condition (default: `true`). |
| `options` | Mapping | No | Declared configuration options (`Int`, `Float`, `Str`, `Bool`, `StrList`). |
| `where` | Mapping | Yes | Condition expression evaluated against AST properties. |
| `message` | String | Yes | Diagnostic message template supporting variable placeholders. |

## Scopes and Properties

Every condition in `where` inspects a property scoped to an AST element:

### `file` Scope

| Property | Type | Description |
| --- | --- | --- |
| `file.lines` | `Int` | Total number of lines in the file. |
| `file.functions_count` | `Int` | Total number of functions declared in the file. |
| `file.imports_count` | `Int` | Number of import directives. |
| `file.top_level_declarations_count` | `Int` | Number of top-level classes, functions, and properties. |
| `file.max_indent_spaces` | `Int` | Deepest indentation level in spaces anywhere in the file. |
| `file.package` | `Str` | Package declaration path string (e.g. `com.example.app`). |

### `class` Scope

| Property | Type | Description |
| --- | --- | --- |
| `class.name` | `Str` | Class or interface name. |
| `class.name.length` | `Int` | Character length of the class name (excluding backtick delimiters). |
| `class.lines` | `Int` | Line count of the class declaration. |
| `class.methods_count` | `Int` | Number of member methods in the class. |
| `class.properties_count` | `Int` | Number of member properties in the class. |
| `class.annotations` | `StrList` | Annotations declared on the class (e.g. `@Entity`). |
| `class.supertypes` | `StrList` | List of direct superclasses and implemented interfaces. |
| `class.visibility` | `Str` | `public`, `internal`, `protected`, or `private`. |
| `class.kind` | `Str` | Declaration kind (`class`, `interface`, `object`, `enum`). |
| `class.constructor_parameters_count` | `Int` | Number of parameters in the primary constructor. |

### `function` Scope

| Property | Type | Description |
| --- | --- | --- |
| `function.name` | `Str` | Function name. |
| `function.name.length` | `Int` | Character length of the function name (excluding backtick delimiters). |
| `function.lines` | `Int` | Total line span from `fun` to the end of the body. |
| `function.body_lines` | `Int` | Number of lines inside the function body block. |
| `function.parameters_count` | `Int` | Number of parameters. |
| `function.annotations` | `StrList` | Annotations declared on the function (e.g. `@Composable`). |
| `function.visibility` | `Str` | `public`, `internal`, `protected`, or `private`. |
| `function.max_nesting` | `Int` | Deepest block nesting level inside the function. |
| `function.cyclomatic_complexity` | `Int` | Branching complexity (`if`, `when`, loops, catch, `&&`, `||`). |
| `function.has_expression_body` | `Bool` | `true` if defined with `= expr` instead of `{ ... }`. |
| `function.return_type` | `Str` | Declared return type string. |

### `variable` Scope

| Property | Type | Description |
| --- | --- | --- |
| `variable.name` | `Str` | Variable or property identifier. |
| `variable.name.length` | `Int` | Character length of the variable name (excluding backtick delimiters). |
| `variable.lines` | `Int` | Line count of the variable or property declaration. |
| `variable.type` | `Str` | Declared variable type. |
| `variable.has_custom_getter` | `Bool` | Whether a custom `get()` accessor is defined. |
| `variable.has_custom_setter` | `Bool` | Whether a custom `set()` accessor is defined. |

### `import` Scope

| Property | Type | Description |
| --- | --- | --- |
| `import.path` | `Str` | Target import path string (e.g. `java.util.Date`). |
| `import.alias` | `Str` | Alias identifier if imported as `import foo as bar`. |
| `import.is_wildcard` | `Bool` | `true` for wildcard imports (`import foo.*`). |

### `lambda` Scope

| Property | Type | Description |
| --- | --- | --- |
| `lambda.lines` | `Int` | Total line span of the lambda expression. |

## Condition Operators

Conditions compare properties using operator expressions:

| Operator | Applicable Types | Description |
| --- | --- | --- |
| `gt(val)` | `Int`, `Float` | Greater than. Supports numbers or `options.<key>`. |
| `lt(val)` | `Int`, `Float` | Less than. Supports numbers or `options.<key>`. |
| `gte(val)` | `Int`, `Float` | Greater than or equal to. |
| `lte(val)` | `Int`, `Float` | Less than or equal to. |
| `eq(val)` | `Int`, `Float`, `Str`, `Bool` | Strict equality. String literals must match type. |
| `has(val)` | `StrList`, `Str` | Checks if item is in list, or substring is contained in string. |
| `startswith(val)` | `Str` | Checks if string starts with prefix. Supports strings or `options.<key>`. |
| `endswith(val)` | `Str` | Checks if string ends with suffix. Supports strings or `options.<key>`. |
| `matches(pattern)` | `Str` | Matches against a regular expression pattern. |
| `not(operator)` | Any | Inverts the enclosed operator (e.g. `not(has('@Composable'))`). |

### Condition Combinators

Combine multiple conditions within the same scope using `all` (AND) or `any` (OR):

```yaml
where:
  all:
    - function.cyclomatic_complexity: gt(10)
    - function.max_nesting: gt(4)
```

## Message Templates

Message strings interpolate variable placeholders:

- `${<scope>.<property>}`: Access any property within the rule's active scope (e.g. `${function.name}`).
- `${options.<key>}`: Access declared option values (e.g. `${options.max_lines}`).

## Examples

### 1. Restricting Forbidden Imports

Ban usages of legacy Java time classes in favor of `kotlinx.datetime`:

```yaml
lint:
  rules:
    - id: ban-legacy-date
      severity: error
      where:
        import.path: matches("^java\\.util\\.(Date|Calendar)$")
      message: "Direct import of '${import.path}' is forbidden. Use 'kotlinx.datetime.Instant' instead"
```

### 2. Limiting Constructor Parameter Count

```yaml
lint:
  rules:
    - id: too-many-constructor-params
      severity: warning
      options:
        max_params: 6
      where:
        class.constructor_parameters_count: gt(options.max_params)
      message: "Class '${class.name}' has ${class.constructor_parameters_count} constructor parameters (limit: ${options.max_params})"
```

## See Also

- [Lint Basics](basic.md)
- [Built-in Rulesets](builtin-rulesets.md)
- [Disabling Rules and Suppressions](disable-rules.md)
- [EditorConfig Integration](editor-config.md)
