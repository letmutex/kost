# Unsupported and Errored Tree-Sitter Parsing in Kotlin Formatter

This document catalogs all 38 test fixtures from `crates/format/tests/ktfmt/cases/` that fail during formatting due to upstream Tree-sitter parse errors (`FormatError::ParseError` / syntax error near `ERROR` or `MISSING`).

Tree-sitter uses `tree-sitter-kotlin` grammar to parse Kotlin source files into concrete syntax trees. When the parser encounters syntax that is missing from its grammar specification or violates strict parser rules, it injects `ERROR` or `MISSING` nodes. Because our formatter adheres strictly to AST-first fidelity, inputs producing parse errors are rejected.

---

## Executive Summary

| Category | Count | Primary Root Cause in `tree-sitter-kotlin` |
| :--- | :--- | :--- |
| **1. Annotations on Expressions & Statements** | 5 | Grammar only allows annotations on declarations and types; expression annotations produce syntax errors. |
| **2. Annotations on Complex Types & Supertypes** | 5 | Missing grammar support for annotations preceding parenthesized function types, supertype delegations, and variance. |
| **3. Kotlin 2.2 Context Parameters & Receivers** | 2 | Grammar only supports Kotlin 1.6 unnamed context receivers (`context(A)`); fails on named parameters (`context(a: A)`) and secondary constructors. |
| **4. Kotlin 2.0 Explicit Backing Fields** | 3 | Grammar lacks rules for the Kotlin 2.0 explicit `field = ...` and `field: Type = ...` property backing field construct. |
| **5. Kotlin 2.1 When Guard Expressions** | 1 | Grammar lacks rules for `if <guard>` clauses within `when` condition branches. |
| **6. Definitely Non-Nullable / Intersection Types (`&`)** | 1 | Grammar lacks intersection type operator `&` introduced in Kotlin 1.7 (`T & Any`). |
| **7. Nullable Callable References (`Type?::member`)** | 1 | Grammar rule for callable references requires non-nullable `user_type` before `::`. |
| **8. Backtick Quoted Identifiers in Labels** | 1 | Grammar lexical regex for labels only permits standard unquoted identifiers before `@`. |
| **9. Experimental Bracket / Name-Based Destructuring** | 1 | Non-standard experimental KEEP proposal (`val [a, b] = d`) not part of standard Kotlin. |
| **10. Trailing Commas in Catch Clauses** | 1 | Catch block parameter rule requires single parameter without optional trailing comma. |
| **11. Constructors on `object` Declarations** | 1 | Invalid Kotlin: singletons cannot have primary constructors (`object Foo(n: Int)`). |
| **12. `val`/`var` in Secondary Constructors** | 4 | Invalid Kotlin: `val`/`var` modifiers are restricted to primary constructors. |
| **13. Multiple Semicolons in Empty Enums** | 3 | Enum class body only permits a single delimiter semicolon after entries. |
| **14. Lambda Parameters with Newlines before `->`** | 2 | Lexer treats newline before arrow as statement boundary. |
| **15. Trailing Commas / Comments in Setter Parameters** | 1 | Setter parameter list grammar rejects trailing commas or intermixed comments. |
| **16. `val`/`var` on Regular Function Parameters** | 1 | Invalid Kotlin: function value parameters cannot declare property modifiers. |
| **17. Assignment Statements Chained with `&&`** | 3 | Invalid Kotlin: assignments are statements in Kotlin and cannot be chained as boolean expressions. |
| **18. Uninitialized File-Level Abstract Properties** | 1 | Invalid Kotlin: top-level properties must have initializers or accessors. |
| **19. Standalone Semicolons in Statements (Idempotency)** | 1 | Formatted output leaves standalone `;` on its own line which `tree-sitter-kotlin` rejects. |
| **Total Errored Fixtures** | **38** | |

---

## Detailed Issue Breakdown

### 1. Annotations on Expressions and Statements (5 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/annotation/expressions.input`
  - `crates/format/tests/ktfmt/cases/format/annotation/expressions2.input`
  - `crates/format/tests/ktfmt/cases/format/annotation/annotationsInDesctucturingDeclaration.input`
  - `crates/format/tests/ktfmt/cases/new_codestyle/annotation/AnnotationOnExpression.input`
  - `crates/format/tests/ktfmt/cases/new_codestyle/annotation/AnnotationOnExpressionFullWidth.input`

- **Reproducing Snippet:**
  ```kotlin
  fun test() {
      // Expression annotations
      add(@Suppress("MagicNumber") 10)
      @Suppress("MagicNumber") add(10) && add(20)

      // Assignment annotations
      @Suppress("UNCHECKED_CAST") b = f(1) as Int

      // Lambda / destructuring variable annotations
      val x = { (@Anno x, @Anno y) -> x }
      val (@Anno a, @Anno b) = pair
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 4, column 38: Syntax error near 'ERROR'`
  - `Parse error at line 3, column 33: Syntax error near 'ERROR'`
  - `Parse error at line 1, column 11: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  In Kotlin specification, annotations (`annotation`) may prefix expressions, statements, assignments, and destructuring entries. `tree-sitter-kotlin` only permits `modifiers` before declaration nodes and certain type nodes. When an annotation precedes an expression or statement, the parser attempts to parse a declaration, fails, and produces an `ERROR` node.

- **Upstream Resolution:**
  Add `annotated_expression: $ => prec(PREC.annotated, seq(repeat1($.annotation), $.expression))` and allow annotations in `destructuring_declaration` parameter entries in `grammar.js`.

---

### 2. Annotations on Complex Types and Supertypes (5 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/annotation/functionTypes.input`
  - `crates/format/tests/ktfmt/cases/format/misc/twoModifierLists.input`
  - `crates/format/tests/ktfmt/cases/format/type/nullableTypes.input`
  - `crates/format/tests/ktfmt/cases/new_codestyle/annotation/AnnotationsOnTypes.input`
  - `crates/format/tests/ktfmt/cases/new_codestyle/annotation/DeclarationAnnotations.input`

- **Reproducing Snippet:**
  ```kotlin
  // Parenthesized function types
  fun foo(error: @Composable ((x) -> Unit)) {}
  fun foo(a: @A (suspend () -> String)) {}

  // Parenthesized nullable types
  var x: @Anno (Int?) = null

  // Supertypes and delegation specifiers
  class SupertypeFits : @Ann Base()
  class Variance<@Anno out @Anno T>
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 5, column 1: Syntax error near 'ERROR'`
  - `Parse error at line 1, column 1: Syntax error near 'ERROR'`
  - `Parse error at line 12, column 18: Syntax error near 'ERROR'`
  - `Parse error at line 27, column 21: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  `tree-sitter-kotlin`'s `parenthesized_type` does not accept leading annotations or type modifiers. Furthermore, `delegation_specifier` within `super_type_list` only expects a `constructor_invocation` or `user_type` without an optional `modifiers` prefix.

- **Upstream Resolution:**
  Update type grammar to support `@Annotation (Type)` and permit annotations on `delegation_specifier` nodes.

---

### 3. Kotlin 2.2 Context Parameters & Receivers (2 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/misc/contextParameters.input`
  - `crates/format/tests/ktfmt/cases/format/misc/contextReceivers.input`

- **Reproducing Snippet:**
  ```kotlin
  // Kotlin 2.2 context parameters (with names and colons)
  context(something: Something)
  class A {
      context(logger: Logger, raise: Raise<Error>, _: Ignored)
      fun doNothing() {}

      fun <T> testSuspend(block: suspend context(someContext: SomeContext) T.() -> Unit)
  }

  // Context receivers on secondary constructors
  class B(val x: Int) {
      context(Something)
      constructor() : this(0)
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 1, column 18: Syntax error near 'ERROR'`
  - `Parse error at line 4, column 3: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  `tree-sitter-kotlin` implemented the experimental Kotlin 1.6.20 context receiver syntax `context(Type, Type2)`. Kotlin 2.2 introduces official Context Parameters with named bindings `context(name: Type)`. The grammar rejects the `:` separator inside context lists. Furthermore, `secondary_constructor` does not include `context_receiver` in its declaration head.

- **Upstream Resolution:**
  Extend `context_receiver` parameters to accept optional parameter identifiers `seq(optional(seq($.simple_identifier, ':')), $.type)`. Add `context_receiver` to `secondary_constructor`.

---

### 4. Kotlin 2.0 Explicit Backing Fields (3 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/misc/explicitBackingField.input`
  - `crates/format/tests/ktfmt/cases/format/misc/explicitBackingFieldWithoutType.input`
  - `crates/format/tests/ktfmt/cases/format/misc/explicitBackingWithPrivateSet.input`

- **Reproducing Snippet:**
  ```kotlin
  class Foo {
      val city: StateFlow<String>
          field = MutableStateFlow("")

      var numbers: List<Int>
          field: MutableList<Int> = mutableListOf()
          private set
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 2, column 30: Syntax error near 'val'`
  - `Parse error at line 2, column 17: Syntax error near 'val'`

- **Root Cause in Grammar:**
  Kotlin 2.0 added explicit backing fields (KEEP-278). The `property_declaration` body allows a `field` declaration: `field [: Type] = expression`. `tree-sitter-kotlin` has no rule recognizing the `field` keyword as an accessor or backing field declaration within class properties.

- **Upstream Resolution:**
  Add an `explicit_backing_field` rule to `property_delegate` / accessor alternatives in `property_declaration`.

---

### 5. Kotlin 2.1 When Guard Expressions (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/when/guards.input`

- **Reproducing Snippet:**
  ```kotlin
  fun feedAnimal(animal: Animal) {
      when (animal) {
          is Animal.Cat if !animal.mouseHunter -> animal.feedCat()
          else if animal.eatsPlants -> animal.giveLettuce()
          else -> println("Unknown animal")
      }
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 9, column 17: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Kotlin 2.1 introduced pattern matching guard expressions (`if <expression>`) in `when_entry`. `tree-sitter-kotlin` expects the `->` token immediately following `when_condition`, failing when encountering `if`.

- **Upstream Resolution:**
  Update `when_entry` to allow an optional guard clause: `seq($.when_condition, optional(seq('if', $.expression)), '->', ...)`.

---

### 6. Definitely Non-Nullable / Intersection Types (`&`) (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/type/intersections.input`

- **Reproducing Snippet:**
  ```kotlin
  fun f() {
      val l: Decl<A & B & C>
      val p = Ctor<A & B & C, T & Y & Z>
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 2, column 21: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Kotlin 1.7 introduced definitely non-nullable types and intersection types (`T & Any`). The grammar for `type` does not include binary intersection type expressions chained by `&`.

- **Upstream Resolution:**
  Add `intersection_type: $ => prec.left(PREC.intersection, seq($.type, '&', $.type))` to `type` alternatives.

---

### 7. Nullable Callable References (`Type?::member`) (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/call/functionReference.input`

- **Reproducing Snippet:**
  ```kotlin
  fun f(a: List<Int>) {
      a.map(String?::isNullOrEmpty)
      a.map(SuperLongClassName?::functionName)
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 7, column 17: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Kotlin permits nullable receiver types on callable references (e.g. `Any?::toString`). `tree-sitter-kotlin`'s `callable_reference` rule requires `seq(optional($.user_type), '::', $.simple_identifier)`, which forbids the `?` nullable specifier before `::`.

- **Upstream Resolution:**
  Update `callable_reference` to accept `optional(seq($.type, optional('?')))` or general `$.type` before `::`.

---

### 8. Backtick Quoted Identifiers in Labels (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/misc/labels.input`

- **Reproducing Snippet:**
  ```kotlin
  fun f(a: List<Int>) {
      a.map `inner map`@{
          return@`inner map`
      }
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 9, column 26: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  The `label` token in `tree-sitter-kotlin` is implemented with a strict regex that only matches standard unescaped alpha-numeric identifiers followed by `@`. Kotlin specification permits any valid identifier, including backtick-escaped strings (`` `label with spaces`@ ``).

- **Upstream Resolution:**
  Update the `label` rule to support backtick-quoted identifier tokens before `@`.

---

### 9. Experimental Bracket / Name-Based Destructuring (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/misc/nameBasedDestructuringDeclaration.input`

- **Reproducing Snippet:**
  ```kotlin
  fun f(d: D) {
      val [a, b] = d
      val (a, x = b) = d
      [var a, var b] = d
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 5, column 21: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Bracketed and name-based destructuring is an experimental Kotlin Evolution and Enhancement Process (KEEP) proposal, not part of standard Kotlin. `tree-sitter-kotlin` only recognizes parenthesized destructuring declarations `(a, b)`.

- **Upstream Resolution:**
  Not part of stable Kotlin language specification. Would require custom grammar extension if experimental syntax support is desired.

---

### 10. Trailing Commas in Catch Clauses (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/misc/trailingCommas.input`

- **Reproducing Snippet:**
  ```kotlin
  try {
      //
  } catch (e: Error,) {
      //
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 59, column 20: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Kotlin 1.4 introduced trailing commas in parameter lists. `tree-sitter-kotlin` added trailing commas to function parameters and value arguments, but omitted the trailing comma from the single-parameter `catch_block` rule `seq('catch', '(', $.value_parameter, ')')`.

- **Upstream Resolution:**
  Change `catch_block` parameter rule to `seq('catch', '(', $.value_parameter, optional(','), ')')`.

---

### 11. Constructors on `object` Declarations (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/class/object.input`

- **Reproducing Snippet:**
  ```kotlin
  object Foo(n: Int) {}
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 1, column 13: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  This is invalid Kotlin syntax. Singleton objects cannot have constructors. `ktfmt` includes this fixture to test formatting resilience on broken code, but `tree-sitter-kotlin` strictly conforms to the grammar specification by rejecting primary constructors on `object_declaration`.

---

### 12. `val`/`var` in Secondary Constructor Parameter Lists (4 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/class/secondaryConstructor.input`
  - `crates/format/tests/ktfmt/cases/format/class/secondaryConstructor3.input`
  - `crates/format/tests/ktfmt/cases/format/class/secondaryConstructorDelegate.input`
  - `crates/format/tests/ktfmt/cases/google/secondaryConstructor.input`

- **Reproducing Snippet:**
  ```kotlin
  data class Foo {
      constructor(
          val number: Int = 0,
          val name: String,
      ) {}
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 6, column 11: Syntax error near 'ERROR'`
  - `Parse error at line 5, column 9: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  In Kotlin, parameter property modifiers (`val`/`var`) are strictly limited to primary constructor declarations. Secondary constructors do not allow property declarations. `tree-sitter-kotlin` correctly rejects `val`/`var` in secondary constructors.

---

### 13. Multiple Consecutive Semicolons in Empty Enums (3 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/format/class/emptyEnumWithSemicolon2.input`
  - `crates/format/tests/ktfmt/cases/format/class/emptyEnumWithSemicolon3.input`
  - `crates/format/tests/ktfmt/cases/format/class/emptyEnumWithSemicolon6.input`

- **Reproducing Snippet:**
  ```kotlin
  enum class Empty {
      ;
      ;
      ;
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 3, column 3: Syntax error near 'ERROR'`
  - `Parse error at line 5, column 3: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  In `enum_class_body`, tree-sitter allows a single `;` token after the optional `enum_entries` list. Subsequent semicolons are rejected because the body expects class member declarations rather than empty statements.

---

### 14. Lambda Parameters with Newlines before `->` (2 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/google/commaSeparatedLambdaParams.input`
  - `crates/format/tests/ktfmt/cases/google/trailingCommasSingleElementLists.input`

- **Reproducing Snippet:**
  ```kotlin
  someExpr.let { paramFits
      ->
      butNotArrow
  }

  foo {
      a //
       ->
      0
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 8, column 5: Syntax error near 'ERROR'`
  - `Parse error at line 12, column 6: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  When a newline appears immediately before the arrow token `->` in a lambda parameter list, tree-sitter's scanner interprets the newline as a statement terminator for an expression inside the lambda body rather than a parameter list boundary, causing the subsequent `->` to produce a syntax error.

---

### 15. Trailing Commas / Comments in Setter Parameters (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/google/comments.input`

- **Reproducing Snippet:**
  ```kotlin
  var x: Int = 0
      set(
          // Comment
          someItem: Int,
          // Comment
      ) = Unit
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 41, column 20: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  The `setter` rule in `tree-sitter-kotlin` defines parameter syntax as `seq('set', '(', optional($.parameter), ')')` without permitting a trailing comma `,` or comments before the closing parenthesis.

---

### 16. `val`/`var` on Regular Function Parameters (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/google/functionParams.input`

- **Reproducing Snippet:**
  ```kotlin
  fun foo12(
      a: Int,
      var b: Double,
      val c: String,
  ) {
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 5, column 7: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  Invalid Kotlin: regular functions cannot have `val` or `var` parameter declarations. `tree-sitter-kotlin` correctly disallows modifiers in `function_value_parameters`.

---

### 17. Assignment Statements Chained with `&&` in Control Conditions (3 Fixtures)

- **Affected Fixtures:**
  - `crates/format/tests/ktfmt/cases/google/ifWithMaxWidthCondition.input`
  - `crates/format/tests/ktfmt/cases/google/whenWithMaxWidthCondition.input`
  - `crates/format/tests/ktfmt/cases/google/whileMaxWidthCondition.input`

- **Reproducing Snippet:**
  ```kotlin
  fun foo() {
      if (
          e1 && e2 && e3 = e4
      ) {
          bar()
      }
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 3, column 11: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  In Kotlin, assignment (`=`) is a statement, not an expression (unlike C or Java). An assignment cannot appear inside a boolean expression or be chained with `&&`. `tree-sitter-kotlin` strictly parses assignments as statements, producing an `ERROR` node when encountered within expression conditions.

---

### 18. Uninitialized File-Level Abstract Properties (1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/new_codestyle/annotation/UseSiteTargets.input`

- **Reproducing Snippet:**
  ```kotlin
  @set:Magic(name = "Jane", tag = "long") var propertyTargetOverflows: String
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 5, column 11: Syntax error near 'ERROR'`

- **Root Cause in Grammar:**
  In Kotlin, properties at file scope must have an initializer or explicit getter/setter implementation. An abstract property declaration `var x: String` without a body or initializer is only valid inside interfaces or abstract classes.

---

### 19. Standalone Semicolons in Statements (Idempotency Parse Error - 1 Fixture)

- **Affected Fixture:**
  - `crates/format/tests/ktfmt/cases/format/misc/semicolonsBetweenCalls.input`

- **Reproducing Snippet:**
  ```kotlin
  fun f() {
    foo(0)
    /** a */
    ;
    /** b */
    { dead -> lambda }
  }
  ```

- **Tree-Sitter Error Location:**
  - `Parse error at line 10, column 3: Syntax error near 'ERROR'` (fails on second formatting pass / idempotency check)

- **Root Cause in Grammar:**
  When ktfmt formats a statement where comments surround a statement-disambiguating semicolon, it places `;` on its own line (`\n  ;\n`). In `tree-sitter-kotlin`, an isolated semicolon inside a function body is not parsed as an empty statement, causing a syntax error near `ERROR` when formatting the formatted output.

---

## Conclusion & Actionability

Out of the 38 failing fixtures:

1. **16 fixtures represent valid, modern Kotlin features** that should be patched upstream in `tree-sitter-kotlin`:
   - Annotations on expressions, statements, assignments, and destructuring (5 fixtures)
   - Annotations on parenthesized types, modifier types, and supertypes (5 fixtures)
   - Kotlin 2.2 context parameters (1 fixture)
   - Kotlin 2.0 explicit backing fields (3 fixtures)
   - Kotlin 2.1 `when` guards (1 fixture)
   - Intersection types `&` (1 fixture)

2. **12 fixtures represent parser edge cases / minor grammar gaps**:
   - Nullable callable references `Type?::member` (1 fixture)
   - Backtick labels (1 fixture)
   - Trailing commas in catch clauses and setter parameters (2 fixtures)
   - Context receiver on constructor / comments (1 fixture)
   - Semicolons in empty enums (3 fixtures)
   - Lambda parameters with newline before `->` (2 fixtures)
   - Use site target on top-level uninitialized property (1 fixture)
   - Standalone semicolon between comments (1 fixture)

3. **10 fixtures represent invalid Kotlin syntax** where `tree-sitter-kotlin`'s strict adherence is technically correct according to the Kotlin language specification:
   - `object Foo(...)` constructors (1 fixture)
   - `constructor(val x: Int)` in secondary constructors (4 fixtures)
   - `fun foo(val x: Int, var y: Double)` in regular functions (1 fixture)
   - `if (e1 && e2 = e3)` assignment chained with `&&` (3 fixtures)
   - Bracket destructuring proposal `val [a, b] = d` (1 fixture)
