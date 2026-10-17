# Linter Architecture Specification

High-level architectural design for the Kotlin linter in `kost`. This document outlines the core concepts, key type contracts, configuration model, strict rule parser specification, caveats, and design invariants.

---

## 1. Design Goals

1. **Single-Pass Execution**: Parse once with tree-sitter and perform a single depth-first AST traversal per file. All syntactic rules and scope-level metric aggregations execute concurrently in this single walk.
2. **Dual-Model Rule Separation**:
   - **AST Rules**: Imperative, node-level pattern matches corresponding to standard Kotlin conventions.
   - **Statistical Rules**: Declarative, metric-based conditions over code scopes (lines, complexity, parameter counts, nesting) configured via YAML.
3. **Configurable Rule Options**: Rules can declare typed options with default values. Options can be overridden per project and dynamically referenced inside conditions (`where`) and diagnostic templates (`message`).
4. **Strict, Fail-Fast Configuration Parsing**: Rule IDs, scope bindings, property names, types, operator operands, and template placeholders are rigorously validated at configuration load time.
5. **No Autofix in Linter**: Linting only detects and reports violations. Formatting and structural transformations belong exclusively to the formatter engine (`crates/format`), avoiding duplicate AST mutation pipelines and synchronization hazards.
6. **Zero-Recompile Extensibility**: Builtin statistical rules are fully overridable, and custom statistical rules can be introduced solely via YAML configuration files without Rust code changes.
7. **High Throughput & Low Allocations**:
   - Filtered dispatch: AST rules declare node kinds of interest; non-matching nodes bypass rule invocation entirely.
   - Lazy property extraction: Scope metrics are computed only when active rules require them.
   - Parallel evaluation across files using work-stealing parallelism (`rayon`).

---

## 2. Core Concepts

### 2.1 The Two Rule Families

The engine purposefully distinguishes between two execution models rather than forcing a single abstraction:

```
                      Single DFS AST Traversal
                                  │
         ┌────────────────────────┴────────────────────────┐
         ▼                                                 ▼
   AST Rules (Imperative)                         Scope & Metric Tracker
   • Target: Specific AST node kinds             • Target: Scopes (File, Class, Fn)
   • Timing: Pre-order / Post-order visit        • Timing: Scope entry -> accumulate
   • Mechanism: Compiled Rust code               • Evaluation: Scope exit (Post-order)
   • Config: Severity toggle only                • Mechanism: Declarative Condition AST
                                                 • Config: Full override (where, severity, msg, options)
```

| Dimension | AST Rules | Statistical Rules |
|:---|:---|:---|
| **Primary Focus** | Syntax style, whitespace, token layout, ordering | Structural complexity, size metrics, conventions |
| **Origin** | Compiled Rust (`AstRule` trait implementations) | YAML definitions (built-in defaults + user rules) |
| **Identifier Format** | `standard:<rule-name>` | `statistics:<name>` or `<filename>:<name>` |
| **Trigger Point** | Immediately when visiting an interested AST node | When closing a scope frame (`after_node` of scope root) |
| **Configurability** | Enable/disable (`severity: none`) | Overridable severity, options, message; conditions overridable if permitted by `WherePolicy` |
| **Statefulness** | Stateless per node | Aggregates metrics throughout scope subtree |

### 2.2 Scope & Property Hierarchy

A **Scope** represents a lexical boundary capable of computing and holding metrics.

- **`File`**: File-wide line counts, import statistics, top-level declaration count, max indentation depth.
- **`Class`**: Class/interface/object line count, method count, property count, constructor parameters, supertypes.
- **`Function`**: Function lines, body lines, parameter counts, cyclomatic complexity, max nesting depth, annotations.
- **`Property`**: Property declaration metrics, custom accessors.
- **`Lambda`**: Closure size, statement count, argument count.
- **`Import`**: Package and imported symbol attributes.

### 2.3 Rule Options & Type Inference

Rules may define an optional `options` mapping containing rule-specific thresholds or parameters:
- **Non-Empty if Defined**: If the `options` block is present on a rule, it must contain at least one entry. Empty `options: {}` mappings are rejected during config validation.
- **Type Inference from Default Values**: Every option key defined by a rule must provide a default value. The option's type (`Int`, `Float`, `Str`, `Bool`, `List`) is inferred directly from this default value. No explicit type annotations are required.
- **Overridable with Strict Type Checking**: When overriding options in configuration, overridden values must match the inferred type of the default value. Type mismatches fail validation.
- **Referenced in Conditions (`where`)**: Condition operators accept an option reference (`options.<name>`) instead of a hardcoded literal (e.g. `function.lines: gt(options.max_lines)`).
- **Referenced in Messages (`message`)**: Message templates can interpolate option values using `${options.<name>}` alongside scope properties (e.g. `limit: ${options.max_lines}`).

### 2.4 Condition Evaluation Model

Statistical rules evaluate boolean condition trees against a scope's computed `PropertyBag` and the rule's resolved `options`:
- **Combinators**: Exactly one top-level combinator: `all` (logical AND), `any` (logical OR), or a single condition.
- **Operators**: Numeric comparisons (`gt`, `lt`, `gte`, `lte`), equality (`eq`), set containment (`has`), string pattern matching (`matches`), and logical inversion (`not(...)`).
- **Operands**: Operators accept either concrete literals or option references (`options.<name>`).
- **Scope Inference**: The target scope is strictly inferred from the property prefix (e.g. `function.lines` binds to `Scope::Function`).

---

## 3. Key Types

### 3.1 Severity & Diagnostics

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    None, // Rule is disabled; skipped during execution
}

pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

pub struct Diagnostic {
    pub rule_id: SmolStr, // e.g. "kost.yaml:composable-function-too-long"
    pub severity: Severity,
    pub range: TextRange,
    pub message: String,
}
```

### 3.2 Property Keys & Values

Properties use a typed enum representing all statically known metrics to eliminate string allocation and string matching on the hot traversal path:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    // Function scope
    FunctionName,
    FunctionLines,
    FunctionBodyLines,
    FunctionParametersCount,
    FunctionAnnotations,
    FunctionVisibility,
    FunctionMaxNesting,
    FunctionCyclomaticComplexity,
    FunctionHasExpressionBody,
    FunctionReturnType,

    // Class scope
    ClassName,
    ClassLines,
    ClassMethodsCount,
    ClassPropertiesCount,
    ClassAnnotations,
    ClassSupertypes,
    ClassVisibility,
    ClassKind,
    ClassConstructorParametersCount,

    // Property scope
    PropertyName,
    PropertyLines,
    PropertyType,
    PropertyHasCustomGetter,
    PropertyHasCustomSetter,

    // Import scope
    ImportPath,
    ImportAlias,
    ImportIsWildcard,

    // File scope
    FileLines,
    FileImportsCount,
    FileTopLevelDeclarationsCount,
    FileMaxIndentSpaces,
    FilePackage,

    // Lambda scope
    LambdaLines,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Int(i64),
    Float(f64),
    Str(SmolStr),
    Bool(bool),
    StrList(SmallVec<[SmolStr; 4]>),
}
```

### 3.3 Rule Options & Operands

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Int(i64),
    Float(f64),
    Str(SmolStr),
    Bool(bool),
    StrList(SmallVec<[SmolStr; 4]>),
}

/// Unified numeric representation supporting mixed integer/float comparisons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numeric {
    Int(i64),
    Float(f64),
}

/// Dynamic or literal operand in condition evaluations.
#[derive(Debug, Clone)]
pub enum Operand<T> {
    Literal(T),
    OptionRef(SmolStr), // references rule.options[key]
}
```

### 3.4 AST Rule Contract & Definition API

```rust
pub trait AstRule: Send + Sync {
    /// Canonical local name, e.g. "no-semi". Scope 'standard:' is auto-attached by registry.
    fn id(&self) -> &'static str;

    /// Specific tree-sitter node kinds this rule inspects.
    /// Traversal invokes check_node ONLY when encountering these node kinds (O(1) dispatch).
    fn interested_kinds(&self) -> &'static [&'static str];

    /// Pre-order hook: invoked before traversing child nodes.
    fn check_node(&self, node: Node, source: &[u8], ctx: &mut LintCtx);

    /// Post-order hook: invoked after all child nodes are traversed.
    fn after_node(&self, _node: Node, _source: &[u8], _ctx: &mut LintCtx) {}
}

/// Minimal rule implementation example
pub struct NoSemi;

impl AstRule for NoSemi {
    fn id(&self) -> &'static str { "no-semi" }
    fn interested_kinds(&self) -> &'static [&'static str] { &[";", "semicolon"] }

    fn check_node(&self, node: Node, source: &[u8], ctx: &mut LintCtx) {
        if is_enum_entry_semicolon(node) {
            return;
        }
        // Ergonomic reporting helper: extracts TextRange automatically from Node
        ctx.report(node, "Unnecessary semicolon");
    }
}
```

#### Traversal Context Reporting Helpers

`LintCtx` tracks the currently active rule's identifier and severity, eliminating boilerplate at call sites:

```rust
impl<'a> LintCtx<'a> {
    #[inline]
    pub fn report(&mut self, node: Node, message: impl Into<String>) {
        self.report_range(TextRange::from_node(node), message);
    }

    #[inline]
    pub fn report_range(&mut self, range: TextRange, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            rule_id: self.active_report_id.clone(),
            severity: self.active_severity,
            range,
            message: message.into(),
        });
    }
}
```

### 3.5 Statistical Rule & Condition AST

Statistical rules use a dual definition model:
- **Primary Pattern (Embedded YAML Manifests)**: Standard statistical rules (`statistics:long-function`, `statistics:deep-indent`, `statistics:too-many-parameters`) are stored in embedded YAML manifests (`include_str!("../rules/stat.yaml")`). They are parsed by the exact same rule parser as user configurations, ensuring 100% feature parity, shared type inference, and zero code drift, verified by compile-time unit tests.
- **Secondary Pattern (Fluent Rust Builder)**: An optional strongly-typed builder API (`StatRule::builder(...)`) enables constructing synthetic statistical rules for unit test fixtures and benchmarks without file I/O.

```rust
pub struct StatRule {
    pub id: SmolStr,
    /// Precomputed diagnostic identifier (e.g. "kost.yaml:long-function")
    /// avoids string formatting and splitting on the evaluation hot path.
    pub report_id: SmolStr,
    pub severity: Severity,
    pub scope: Scope,
    pub options: Option<IndexMap<SmolStr, OptionValue>>,
    pub condition: Condition,
    pub message_template: SmolStr,
    /// Capability contract governing whether configuration files may override `where` conditions.
    pub where_policy: WherePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WherePolicy {
    /// Condition tree can be replaced or refined in configuration overrides.
    AllowOverride,
    /// Condition tree is locked; overrides may only alter options, severity, and message.
    RejectOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    File,
    Class,
    Function,
    Property,
    Lambda,
    Import,
}

#[derive(Debug, Clone)]
pub enum Condition {
    All(Vec<PropertyCondition>),
    Any(Vec<PropertyCondition>),
    Single(PropertyCondition),
}

#[derive(Debug, Clone)]
pub struct PropertyCondition {
    pub property: PropertyKey,
    pub op: CondOp,
}

#[derive(Debug, Clone)]
pub enum CondOp {
    Gt(Operand<Numeric>),
    Lt(Operand<Numeric>),
    Gte(Operand<Numeric>),
    Lte(Operand<Numeric>),
    Eq(Operand<PropertyValue>),
    Has(Operand<SmolStr>),
    Not(Box<CondOp>),
    /// Compiled regular expression pre-built during configuration load;
    /// avoids repeated regex compilation during AST traversal.
    Matches(regex::Regex),
}
```

### 3.6 Registry & Context

```rust
pub struct RuleRegistry {
    pub ast_rules: Vec<Box<dyn AstRule>>,
    /// Node kind string -> indices in `ast_rules` for O(1) dispatch
    pub ast_dispatch: HashMap<&'static str, SmallVec<[usize; 4]>>,
    /// Target scope -> active statistical rules
    pub stat_dispatch: HashMap<Scope, Vec<StatRule>>,
    /// Set of property keys needed by at least one active statistical rule
    pub needed_properties: HashSet<PropertyKey>,
}

pub struct LintCtx<'a> {
    pub source: &'a [u8],
    pub registry: &'a RuleRegistry,
    pub scope_tracker: ScopeTracker,
    pub diagnostics: Vec<Diagnostic>,
    pub active_report_id: SmolStr,
    pub active_severity: Severity,
}

pub struct ScopeTracker {
    pub stack: Vec<ScopeFrame>,
}

pub struct ScopeFrame {
    pub scope: Scope,
    pub range: TextRange,
    pub properties: SmallVec<[(PropertyKey, PropertyValue); 8]>,
    // Transient counters (nesting, branch counts) tracked only if requested
}
```

---

## 4. Configuration Model & Strict Rule Parser

Configurations are written in YAML and loaded hierarchically with strict validation.

### 4.1 Schema Shape

```yaml
lint:
  includes:
    # ── Whole ruleset / whole file includes (Full Import) ────────
    - standard           # Standard compiled AST rules (explicit opt-in)
    - statistics         # Standard statistical rules (explicit opt-in)
    - ./shared_team.yaml # Path to external config file

    # ── Inline qualified rule import (scope/file:id) ─────────────
    - standard:no-semi                         # Import single AST rule from virtual ruleset
    - statistics:long-function                 # Import single statistical rule
    - ./shared_team.yaml:disallow-todo         # Import single rule from relative file path
    - shared_team.yaml:enforce-doc             # Import single rule by scope/filename

    # ── Structured path + rules selection list ───────────────────
    - path: standard
      rules:
        - colon-spacing
        - curly-spacing
    - path: statistics
      rules:
        - long-function
    - path: ./shared_team.yaml
      rules:
        - team-naming-convention
        - disallow-raw-threading

  rules:
    # ── Disabling an included rule (no negative imports; use severity: none) ──
    - id: standard:curly-spacing
      severity: none

    # ── Override an existing AST rule (severity only) ────────────
    - id: standard:no-semi
      severity: none

    # ── Override an existing statistical rule's options and condition
    - id: statistics:long-function
      options:
        max_lines: 80                         # Overrides default (60)
      where:
        all:
          - function.annotations: not(has("@Composable"))
          - function.lines: gt(options.max_lines)
      message: "Function '${function.name}' is ${function.lines} lines (limit: ${options.max_lines})"

    # ── Override options only (keeps default conditions) ─────────
    - id: statistics:too-many-parameters
      options:
        max_params: 5                         # Overrides default threshold (7)
      severity: error

    # ── Define a new custom statistical rule (no scope prefix in id)
    - id: composable-function-too-long
      severity: warning
      options:
        max_lines: 100                        # Default value provides type inference (Int)
        marker: "@Composable"                 # Default value provides type inference (Str)
      where:
        all:
          - function.annotations: has(options.marker)
          - function.lines: gt(options.max_lines)
      message: "Composable '${function.name}' is ${function.lines} lines (limit: ${options.max_lines})"

    # ── File-scoped custom rule with locked condition ────────────
    - id: deep-nested
      severity: error
      allow_where_override: false           # Optional (default: true); locks condition from downstream override
      options:
        max_indent: 36
      where:
        file.max_indent_spaces: gt(options.max_indent)
      message: "Max indent depth ${file.max_indent_spaces} exceeds limit of ${options.max_indent}"
```

### 4.2 Includes Resolution & Override Mechanics

The engine resolves included configuration files into a unified, immutable `RuleRegistry` using a Directed Acyclic Graph (DAG) with strictly deterministic semantics.

#### 1. Partial & Whole Includes Model
The `includes:` section is strictly additive (positive-only) and supports three interoperable forms:
- **Full Includes**: Specifying a virtual identifier (`standard`, `statistics`) or file path (`./shared.yaml`) without rule constraints imports all active rules from that target.
- **Inline Qualified Rule Imports**: Specifying `<target>:<rule-id>` (e.g. `standard:no-semi`, `statistics:long-function`, `./shared_team.yaml:disallow-todo`) imports only the targeted rule.
- **Structured Selection Lists**: Specifying `path: <target>` with a `rules: [<rule-id>, ...]` list selectively imports only the enumerated rules.
- **No Negative Imports**: To eliminate redundant exclusion mechanisms, negative imports (`exclude`, `except`, `!rule`) are prohibited. Disabling any imported rule is achieved uniformly via the existing `rules: [{ id: ..., severity: none }]` configuration override.
- **Fail-Fast Selection Validation**: Every rule requested in an inline import or structured `rules:` list must exist in the target ruleset. Unmatched rule IDs fail immediately with `ConfigError::UnknownIncludedRule`. Empty selection lists (`rules: []`) fail immediately with `ConfigError::EmptyIncludeRules`. If an unqualified rule name matches multiple exported rules across different scopes, resolution fails fast with `ConfigError::AmbiguousIncludedRule` to prevent silent rule shadowing.

#### 2. Topological Post-Order Merging
Includes are resolved depth-first in post-order:
- Children (included files) are completely evaluated and merged before the parent file's `rules:` block executes.
- **Invariant**: When a parent file overrides an included rule (e.g. `standards.yaml:disallow-todo`), that target rule is guaranteed to already exist and be fully validated in the registry.

#### 3. Path Normalization, Memoization & Cycle Detection
- Include entries are strictly partitioned:
  - **Virtual Module Identifiers** (`standard`, `statistics`): Resolved directly in-memory from embedded engine rulesets; bypass filesystem path resolution, canonicalization, and basename collision checks.
  - **Filesystem Paths** (`./shared/standards.yaml`, `../rules/team.yaml`): Resolved relative to the containing configuration file's parent directory. All filesystem paths are canonicalized (`std::fs::canonicalize`). If multiple files include the exact same shared config, it is loaded and parsed **only once** (memoized). Partial selections from a memoized target are projected into the caller without re-reading the filesystem.
- **Strict Opt-In Model for All Rulesets**: All rulesets—including virtual modules (`standard`, `statistics`) and external files—are 100% opt-in. No rules are enabled implicitly. If `includes:` is omitted or empty, the linter starts with an empty registry (only local custom rules defined in `rules:` will run).
- Circular includes (`a.yaml -> b.yaml -> a.yaml`) are detected during graph traversal and fail immediately with a fatal diagnostic.

#### 4. Strict "Filename is Scope" Model
- **No `scope` Configuration Field**: Configuration files contain no explicit `scope:` metadata. The filesystem filename is strictly the single source of truth for custom rule scoping.
- **Unscoped Rule Definitions**: When defining a new rule in a file, the author provides an unscoped ID (`id: my-rule`). The loader automatically binds it to the containing filename: `<filename>:<my-rule>` (e.g. `standards.yaml:disallow-todo`).
- **Scoped Rule Overrides**: When customizing any rule, authors specify the exact `<scope>:<rule-name>` (`standard:no-semi`, `standards.yaml:disallow-todo`).
- **Filename Collision Detection**: If two distinct included files share the exact same basename (e.g. `./client/rules.yaml` and `./backend/rules.yaml`), the loader fails fast with a clear collision diagnostic, instructing the developer to use distinct filenames (e.g. `client_rules.yaml`).

#### 4. Cascading Delta Merge Algorithm
When an overriding rule entry (`<target-scope>:<rule-name>`) is evaluated:
1. **Existence Check**: Target must already exist in the active ruleset.
2. **AST Rule Check**: If target is an AST rule, `where:`, `options:`, or `message:` are strictly forbidden. Only `severity:` can be updated.
3. **Capability Check (`WherePolicy`)**: If `where:` is provided on a statistical rule, `target.where_policy` must be `AllowOverride`. If `RejectOverride`, parsing fails immediately, directing the user to `options`.
4. **Options Delta Validation**: Each key in `options:` must exist in `target.options`, and each value must match the inferred type of the default. Unspecified options retain their previous values.
5. **Attribution Retargeting**: The rule's `report_id` is updated to `<current-file>:<rule-name>`, ensuring diagnostic messages clearly reflect which configuration file governed the active threshold.

### 4.3 Strict Rule Parser Specification

The rule parser rejects ambiguous or malformed configurations at load time using strict validation rules:

#### 1. Scope Separator (`:`) & ID Format
- The scope separator is strictly the colon character (`:`). Slashes (`/`) are forbidden.
- Base rule names must strictly conform to kebab-case alphanumeric regex: `^[a-z0-9]+(-[a-z0-9]+)*$`.
- **Non-overriding Rule IDs (New Custom Rules)**:
  - Must **not** contain any scope prefix (no `:` allowed). Writing `id: my_scope:rule-name` or `id: :rule-name` is a fatal parse error.
  - The parser automatically attaches the containing configuration filename as the scope!
  - *Example*: In `kost.yaml`, defining `id: composable-function-too-long` registers the canonical ID as:
    `kost.yaml:composable-function-too-long`.
  - *Example*: In `./rules/team.yaml`, defining `id: strict-naming` registers as:
    `team.yaml:strict-naming`.
- **Overriding Rule IDs**:
  - Must contain exactly one scope prefix: `<target-scope>:<rule-name>` (e.g. `statistics:long-function`, `standard:no-semi`).
  - **Existence Invariant**: Both `<target-scope>` (e.g. `standard`, `statistics`) and `<rule-name>` (e.g. `long-function`, `no-semi`) **must already exist** in the registered/included rulesets. Overriding an unknown scope or non-existent rule is a fatal parse error.
  - **Report Attribution**: When an overriding rule triggers a diagnostic, the report attributes the rule ID to the configuration file that customized it:
    *Example*: Overriding `statistics:long-function` in `kost.yaml` causes violations to appear in diagnostic output as `kost.yaml:long-function`.

#### 2. Required vs. Optional Fields Matrix

| Field | New Custom Rules (Unscoped ID) | Overriding Statistical Rules | Overriding AST Rules |
|:---|:---|:---|:---|
| `id` | **Required** (kebab-case, no `:`) | **Required** (`<scope>:<id>`, must exist) | **Required** (`standard:<id>`, must exist) |
| `severity` | **Optional** (defaults to `warning`) | **Optional** (defaults to inherited) | **Optional** (defaults to inherited) |
| `where` | **Required** (defines condition tree) | **Optional** (permitted only if `where_policy == AllowOverride`) | **Prohibited** (fatal parse error) |
| `options` | **Optional** (requires defaults if set) | **Optional** (must match existing keys) | **Prohibited** (fatal parse error) |
| `message` | **Required** (diagnostic template) | **Optional** (overrides default message) | **Prohibited** (fatal parse error) |
| `allow_where_override` | **Optional** (boolean, defaults to `true`) | **Prohibited** (capability contract cannot be altered on override) | **Prohibited** (fatal parse error) |

#### 3. Overriding Restrictions & `WherePolicy` Enforcement
- **AST Rules** (`standard:no-semi`, `standard:colon-spacing`, etc.):
  - Implemented in compiled Rust; they do not have condition trees, configurable options, or customizable message templates.
  - Supplying `where:`, `options:`, `message:`, or `allow_where_override:` when overriding an AST rule is a **fatal parse error**. Only `severity:` can be overridden.
- **Statistical Rules** (`statistics:long-function`, custom rules, etc.):
  - Can override `severity`, `options`, and `message`.
  - Condition overriding (`where:`) is strictly governed by the target rule's `WherePolicy`:
    - **`AllowOverride`** (`allow_where_override: true`): Overriding `where:` is permitted and replaces the rule's condition tree. This is the default for custom rules and enabled on filterable built-in rules (e.g. `statistics:long-function`, custom statistical rules).
    - **`RejectOverride`** (`allow_where_override: false`): Specifying `where:` in an override is a **fatal parse error** at configuration load time. This prevents corrupting fixed metric semantics on pure threshold rules (e.g. `statistics:deep-indent`, `statistics:too-many-parameters`) while still allowing threshold customization via `options`.
  - If `options` is overridden, only declared option keys can be overridden, and their types must match the inferred types of the defaults.
  - Overriding rules cannot specify `allow_where_override` (the rule's capability contract is fixed at definition time).

#### 4. Strict Parsing of `where` Conditions
- **Combinator Selection**:
  - The root must be one of: `all: [ ... ]`, `any: [ ... ]`, or a single property mapping `<property>: <op>`.
  - Multiple top-level combinators or mixing `all` and `any` at the root level is rejected.
- **Property Selection (`<scope>.<field>`)**:
  - Valid scope prefix must be one of `file`, `class`, `function`, `property`, `lambda`, `import`.
  - Property name must map to a known variant of `PropertyKey` (e.g. `function.lines` maps to `PropertyKey::FunctionLines`; `function.length` triggers an immediate parse failure with known alternatives).
  - **Single-Scope Invariant**: All properties in a condition block must target the identical scope prefix.
- **Type Checking for Operators & Operands**:
  - `gt`, `lt`, `gte`, `lte`: Target property must be `Int` or `Float`. Operand must resolve to numeric `Int` or `Float`.
  - `eq`: Property and operand types must match exactly.
  - `has`: Target property must be `StrList`. Operand must resolve to `Str`.
  - `matches`: Target property must be `Str`. Operand must resolve to a valid regular expression string (precompiled at configuration parse time).
  - `not`: Inverts an inner valid operator.
- **Operand Resolution**:
  - Literal: Type checked directly from YAML primitive.
  - Option Reference (`options.<key>`): `<key>` must be present in the rule's `options` declaration, and its inferred type must match the operator's expected type.
- **Message Template Validation**:
  - Every `${...}` placeholder in `message` is validated during parsing.
  - Must reference either a valid `<scope>.<property>` for the rule's scope or a declared `options.<key>`. Unrecognized placeholders fail configuration parsing immediately.

---

## 5. Architectural Caveats & Invariants

1. **Non-Empty Options Constraint**:
   - An empty `options: {}` block is forbidden. Defining `options` obligates specifying at least one typed key-value pair.
2. **Mandatory Defaults for Type Inference**:
   - Option types are never declared syntactically; they are inferred from default values. Rules introducing options must supply concrete defaults for every declared key.
3. **Report Attribution Mapping**:
   - Unmodified built-in rules report as `standard:<name>` or `statistics:<name>`.
   - Any rule customized or defined in a config file reports with that file's namespace: `<filename>:<name>`.
4. **No Scope Mixing in Conditions**:
   - All property references within a single statistical rule's `where` block must originate from the **same scope**. Combining `function.*` and `class.*` in the same rule is prohibited.
5. **AST-First Invariant**:
   - All checks must query tree-sitter AST nodes and token types directly. Raw string scanning or regex passes over entire source buffers are forbidden, except within literal tokens or comment trivia.
6. **Lazy Property Gating**:
   - Complex metrics (e.g. cyclomatic complexity, nesting depth) incur traversal overhead. The traversal loop must consult `ctx.registry.needed_properties` before calculating expensive metrics.
7. **Thread Safety & Immutability**:
   - `RuleRegistry` (including precompiled `report_id` strings and regexes) is compiled once and shared across worker threads (`Send + Sync`). All mutable state resides strictly within thread-local `LintCtx`.
8. **No Autofix Coupling**:
   - Rules must not generate replacement text or mutate nodes. Reporting `range` and `message` guarantees deterministic, side-effect-free execution across all rules.
9. **Configuration Authority**:
   - `kost.yaml` is the primary authority for rule enablement, severity overrides, and statistical rule conditions in the `lint` crate. EditorConfig files are consumed for shared formatting defaults (such as `indent_size`) but do not drive rule execution logic.
10. **Condition Override Governance (`WherePolicy`)**:
    - Rules explicitly declare whether their condition logic may be replaced downstream (`WherePolicy::AllowOverride` vs. `WherePolicy::RejectOverride`). Fixed metric rules lock their conditions to protect semantic integrity while exposing tuning parameters via `options`; compound rules permit condition overriding to allow flexible scope filtering.
11. **Strict Filename Scoping**:
    - Configuration files declare no `scope` metadata; the filename is the sole scope identifier. Duplicate basenames across includes are rejected at load time to prevent ambiguous rule namespaces.
12. **Topological Post-Order Resolution**:
    - Includes form a DAG processed depth-first in post-order, ensuring that every overridden rule is already registered and type-checked before applying parent overrides.
13. **Strict Opt-In Rulesets**:
    - All rulesets are 100% opt-in. Built-in rules (`standard`, `statistics`) are never loaded implicitly. If `includes:` omits `standard` or `statistics`, those built-in rules will not execute. This ensures hermetic test environments, deterministic execution, and zero unrequested lint overhead.



