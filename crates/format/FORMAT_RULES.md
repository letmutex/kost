# Kotlin Formatter Specification & Formatting Rules

This document specifies the exact formatting rules, layout semantics, whitespace invariants, and syntactic disambiguation behaviors implemented by `ktfmt` across its three primary styles: **Meta (default)**, **Google**, and **Kotlinlang**.

---

## 1. Core Principles and Style Presets

The formatting engine targets a fixed maximum line length (default: 100 columns) and applies the Wadler-Leijen style algebraic document layout.

### 1.1 Style Presets Matrix

| Parameter / Behavior | Meta (Default) | Google (`--google-style`) | Kotlinlang (`--kotlinlang-style`) |
| :--- | :--- | :--- | :--- |
| **Max Width** | 100 columns | 100 columns | 100 columns |
| **Block Indent** | 2 spaces | 2 spaces | 4 spaces |
| **Continuation Indent** | 4 spaces | 2 spaces | 4 spaces |
| **Trailing Comma Strategy** | `ONLY_ADD` | `COMPLETE` | `COMPLETE` |
| **Trailing Lambda Breaks** | Preserved (`true`) | Preserved (`true`) | Preserved (`true`) |
| **Unused Imports Removal** | Enabled (`true`) | Enabled (`true`) | Enabled (`true`) |
| **Expression Body Indent** | Block (2 spaces) | Block (2 spaces) | Continuation (4 spaces) |
| **Parameter List Wrapping** | Standard | Standard | Constructor / Call nuances |
| **Supertype List Indent** | Continuation (4 spaces) | Continuation (2 spaces) | Continuation (4 spaces) |

### 1.2 Trailing Comma Strategies

1. **`NONE`**: Do not add or remove trailing commas; format only tokens present in the source.
2. **`ONLY_ADD` (Meta Default)**:
   - When a comma-separated list breaks across multiple lines, insert a trailing comma before the closing delimiter (`)`, `]`, `}`).
   - If a list fits on a single line but already has a user-provided trailing comma, **retain** the trailing comma and keep the list multiline (the user comma acts as an explicit break hint).
   - Redundant trailing commas on single lines are not forcefully stripped if the element was split.
3. **`COMPLETE` (Google & Kotlinlang)**:
   - Fully manage trailing commas: insert when multiline, **strip** when the list collapses to a single line.
   - User-inserted trailing commas cannot be used as permanent break hints if the list fits within `maxWidth`.

---

## 2. File Structure, Packages, and Imports

### 2.1 File-Level Layout & Blank Lines
- **Top-of-File Elements**:
  - Shebang line (e.g. `#!/usr/bin/env kscript`) is preserved at the very top.
  - File-level annotations (e.g. `@file:OptIn(...)`, `@file:JvmName(...)`) precede `package` with forced breaks.
- **Package Directive**:
  - Format: `package <name>`. Multi-segment names joined by dots without spaces (`foo.bar.baz`).
  - Escaped identifiers with backticks (e.g. ``package foo.`in`.bar``) preserve backticks.
  - Redundant semicolons after `package` are removed unless followed by another statement on the same line.
  - Followed by exactly **1 blank line** before the import list or declarations.
- **Spacing Invariants Between Top-Level Elements**:
  - Context receivers (`context(...)`) preceding top-level functions MUST NOT have any blank line between the context receiver and the function (`BlankLineWanted.NO`).
  - Between two top-level properties: preserve user blank line (0 or 1 blank line).
  - Before comments: preserve blank lines (0 or 1).
  - Between all other top-level elements (classes, functions, interfaces, objects, scripts): force exactly **1 blank line**.
  - No trailing blank lines at end of file (terminated with a single `\n`).

### 2.2 Import Formatting & Canonical Sorting
- **Contiguity Requirement**: All import directives must form a single contiguous block. If a non-comment non-whitespace element appears inside the import list, a parse error is raised.
- **Interspersed Comments**:
  - Comments appearing before or inside the import list (`/* ... */`, `// ...`) are hoisted in their original relative order to the top of the import block.
  - Trailing line comments on individual imports stay attached to the corresponding import.
- **Canonical Sorting Key**:
  - Imports are sorted lexicographically by canonical text:
    `canonicalText = importedFqName + " " + (alias ?: "") + " " + (if (isAllUnder) "*" else "")`
    where backticks in aliases are stripped for sorting.
  - Duplicate imports with identical canonical keys are deduplicated to a single entry.
- **Wildcard / Star Imports**:
  - Format: `import <fqname>.*`.
  - Star imports are never removed by unused import detection because they may import arbitrary symbols.
- **Aliases**:
  - Format: `import <fqname> as <alias>`.
  - Single space around `as`.
- **Semicolons**:
  - Trailing semicolons on imports are removed, followed by a forced break.

### 2.3 Redundant and Unused Import Pruning
When `removeUnusedImports = true`, candidate imports are pruned according to strict semantic rules:
- **Never Pruned (Exempt from Cleanup)**:
  - Wildcard / star imports (`import foo.bar.*`).
  - Kotlin operator overload identifiers:
    - Prefix: `unaryPlus`, `unaryMinus`, `not`
    - Inc/dec: `inc`, `dec`
    - Arithmetic: `plus`, `minus`, `times`, `div`, `rem`, `mod`
    - Range: `rangeTo`
    - Containment: `contains`
    - Indexing: `get`, `set`
    - Invocation: `invoke`
    - Augmented assignment: `plusAssign`, `minusAssign`, `timesAssign`, `divAssign`, `remAssign`, `modAssign`
    - Equality / comparison: `equals`, `compareTo`
    - Iteration: `iterator`, `next`, `hasNext`
    - Bitwise: `and`, `or`
    - Property delegation: `getValue`, `setValue`, `provideDelegate`
    - Gradle assignment: `assign`
  - Destructuring component operators matching regex: `component\d+` (e.g., `component1`, `component10`).
- **KDoc Reference Resolution**:
  - Bracketed links in KDoc docstrings (e.g. `[Foo]`, `[Bar.baz]`) count as used references for `Foo` and `Bar`.
  - Tag links (`@throws AnException`, `@see Bar`, `@sample Example`) count as used references.
  - **KDoc Tag Exemption**: For `@param <name>` and `@property <name>`, the first identifier immediately following the tag is the parameter/property name, NOT a type reference, and MUST NOT count as a used import symbol. Subsequent bracketed references (e.g. `@param unused [Param]`) count towards `Param`.
- **Same-Package Import Pruning**:
  - An import matching the current file's package directive is pruned as redundant UNLESS:
    - It has an alias (`import com.example.Foo as Bar` is kept).
    - It is a member / nested class import (e.g. `import com.example.Sample.CONSTANT` when package is `com.example` is kept).
    - It is overloaded: multiple candidate imports share the same short name (e.g., `com.example.a` and `com.notexample.a` both import `a`; neither is pruned).
- **Backtick Escaping**:
  - Identifiers escaped with backticks in the source match backticked references in the AST after stripping surrounding backticks.
  - Entire backticked paths (e.g. ``import `com.example.Foo.bar` ``) extract the last dot segment (`bar`) for usage resolution.

---

## 3. Semicolon Retention and Redundancy Rules

In idiomatic Kotlin, semicolons are almost always optional. However, blindly removing all semicolons leads to compiler errors or silent semantic mutations. Semicolons are categorized as either **redundant** (stripped) or **required** (strictly preserved).

### 3.1 Redundant Semicolons (Always Removed)
- Semicolons at the end of statements on distinct lines (e.g., `val x = 1;` followed by newline).
- Semicolons after package directives and import statements.
- Semicolons inside class bodies between member declarations when members are placed on separate lines.
- Semicolons in empty enum bodies: `enum class Foo { ; }` collapses to `enum class Foo {}`.
- Semicolons after enum entries if the enum class contains no following members (e.g. `enum class Foo { A, B; }` -> `enum class Foo {\n  A,\n  B,\n}`).
- Semicolons after companion objects if the companion object is the last child in the class body (`class Foo { companion object; }` -> `class Foo {\n  companion object\n}`).
- Duplicate or consecutive semicolons (`;;;`) are collapsed/removed.

### 3.2 Required Semicolons (Strictly Preserved)
The formatter MUST preserve semicolons in the following critical cases:

1. **Trailing Lambda Disambiguation Guard**:
   - Any semicolon that is immediately followed by a lambda expression `{ ... }` or a dot-qualified call starting with a lambda `{ ... }.chain()`:
     ```kotlin
     foo(0);
     { dead -> lambda }
     
     val x = foo();
     { dead -> lambda }
     
     funcCall();
     { dead -> lambda }.withChained(call)
     ```
   - **Rationale**: If the semicolon were removed, the Kotlin compiler would parse the subsequent `{ ... }` as a trailing lambda argument to the preceding call or property access (e.g. `foo(0) { dead -> lambda }`), completely altering program semantics or causing type mismatch errors.
   - When split across lines, the semicolon stays at the end of the preceding call line, and the lambda starts on the next line.
   - Comments around the semicolon are preserved in their relative position.

2. **Nameless Companion Object Guard**:
   - A nameless `companion object;` followed by another class body element (such as `init { ... }`, a property, or a function):
     ```kotlin
     class Foo {
       val a: String

       companion object;

       init {
         a = "Hello"
       }
     }
     ```
   - **Rationale**: Without the semicolon, `companion object init { ... }` parses `init` as the identifier name of the companion object rather than the class's `init` block!

3. **Enum Entry Terminating Semicolon**:
   - In an enum class, if the enum entries are followed by class members (properties, functions, companion objects, or an `init` block), the semicolon separating the entries from the members MUST be preserved:
     ```kotlin
     enum class Foo {
       A,
       B;

       fun member() {}
     }
     ```
   - If the enum entries list has no entries but members exist, the lonely semicolon is kept:
     ```kotlin
     enum class Empty {
       ;

       fun member() {}
     }
     ```

4. **Empty Control Structure Body Guard**:
   - An empty body of an `if` or `while` loop terminated by a semicolon:
     ```kotlin
     while (condition());
     doWork()
     ```
   - **Rationale**: If the semicolon were dropped, `doWork()` would become the body of the `while` loop.

5. **String Literals and Templates**:
   - Semicolons within string literals (`"foo;bar"`) or string template expressions are part of the string content and never touched.

---

## 4. Declarations: Classes, Enums, Functions, and Properties

### 4.1 Class, Interface, and Object Layout
- **Class Header**:
  - Modifiers (e.g. `public final`, annotations) formatted with single spaces.
  - Keyword (`class`, `interface`, `object`, `enum class`, `data class`, `sealed class`, `fun interface`).
  - Name and Type Parameters: `class Foo<T : Any>` (no space before `<`).
  - Primary Constructor:
    - Without keyword: `class Foo(val x: Int)`.
    - Explicit `constructor` keyword: `class Foo @Inject constructor(...)` (space before `constructor`).
  - Supertypes & Delegation:
    - Colon followed by single space or unified break: `class Derived : Super1, Super2`.
    - When supertypes wrap, indented by continuation indent (`expressionBreakIndent`: 4 spaces in Meta/Kotlinlang, 2 spaces in Google).
    - Delegation entry: `<Type> by <delegateExpression>` with single space around `by`.
  - Type Constraints (`where` clauses):
    - `where T : Comparable<T>, U : Any`.
    - If preceded by delegated supertype (`by`), forces a break before `where`.
- **Class Body & Member Spacing Invariants**:
  - Braces: opening `{` on the same line after header, closing `}` on its own line.
  - Blank line invariants between class body members:
    - Between two properties without accessors: preserve user blank line (0 or 1 empty line).
    - If either property has an accessor (`get`, `set`) or backing field (`field`): **force 1 blank line**.
    - Between a property and any non-property member (function, class, object, init): **force 1 blank line**.
    - Between two non-property members (e.g. between two functions): **force 1 blank line**.
    - Before the first member and after the last member: NO extra blank line inside `{` and `}` (`BlankLineWanted.NO`).

### 4.2 Enum Classes and Enum Entries
- **Enum Entry Layout**:
  - Each enum entry is formatted on its own line or grouped on a single line if short and simple:
    ```kotlin
    enum class Direction {
      NORTH,
      SOUTH,
      WEST,
      EAST,
    }
    ```
  - Entries with arguments: `A(1), B(2)`.
  - Entries with class bodies:
    ```kotlin
    enum class Op {
      ADD {
        override fun apply(x: Int) = x + 1
      };

      abstract fun apply(x: Int): Int
    }
    ```
- **Terminating Semicolon Invariant**:
  - If an enum has class members (functions, companion objects, properties, init blocks), the terminating semicolon after the last enum entry is **mandatory** and preserved.
  - If an enum has NO subsequent members, any terminating semicolon is **redundant** and stripped.
  - Empty enums with only a semicolon (`enum class Foo { ; }`) collapse to `enum class Foo {}`.

### 4.3 Function Declarations
- **Function Header**:
  - Context receivers: `context(A, B)` placed above the function without intervening blank lines.
  - Modifiers: visibility, `inline`, `suspend`, `operator`, `tailrec`, annotations.
  - Keyword: `fun`.
  - Type Parameters: `<T, R>` placed between `fun` and receiver / function name.
  - Receiver Type: `<Receiver>.functionName(...)`. If receiver breaks, break before `.` with continuation indent.
- **Parameters List**:
  - Empty parameters: `()`.
  - Parameter format: `name: Type = defaultValue` or `val/var name: Type`.
  - When parameters wrap, each parameter is placed on its own line indented by `continuationIndent` (4 in Meta, 2 in Google, 4 in Kotlinlang).
  - Trailing comma inserted before closing `)` when multiline (under `ONLY_ADD` or `COMPLETE` strategy).
- **Return Type & Single-Expression Body**:
  - Return type: `: ReturnType` appended to parameter list.
  - Block body: `{ ... }` placed on the same line after a single space.
  - Single-expression body (`= expr`):
    - In Meta/Google style: if expression fits, stays on same line; if wraps, breaks after `=` with continuation/block indent.
    - In Kotlinlang style: block-like calls (e.g. `foo(...)` that spans multiple lines) stay on the same line without forced break after `=`.

### 4.4 Properties and Accessors
- **Property Header**:
  - Modifiers, `val` or `var`, type parameters, optional receiver type, name, `: Type`.
  - Delegation: `val foo by lazy { ... }` or `val foo by delegate`.
    - Lambda or scoping function delegates (`by lazy { ... }`) stay on the same line.
    - Chained scoping delegates (`by lazy { ... }.filter { ... }`) break before chained calls.
    - Other complex expressions break after `by` with continuation indent.
- **Accessors & Backing Fields**:
  - Explicit accessors (`get() = ...`, `set(value) { ... }`) and backing fields (`field = ...`) are indented by `blockIndent` (2 spaces in Meta/Google, 4 spaces in Kotlinlang) on new lines below the property header:
    ```kotlin
    val foo: Int
      get() = 42

    var bar: String
      get() = field
      private set
    ```
  - Accessors force a blank line before the next class member.
- **Destructuring Declarations**:
  - `val (a, b) = pair` or `val (x: Int, y: String) = tuple`.
  - When wrapping, initializer expression breaks after `=` with continuation indent.
---

## 5. Control Flow Expressions

### 5.1 `if` and `else` Expressions
- **Structure**:
  - `if (<condition>) <thenBody> else <elseBody>`.
  - Condition enclosed in parentheses with a space after `if`: `if (x > 0)`.
- **Block vs Single Expression**:
  - Block bodies: `if (condition) { ... } else { ... }` with braces attached on the same line.
  - Chained conditions: `} else if (another) { ... } else { ... }`.
  - Single-expression branches (without braces):
    - When broken across lines, the branch expression breaks with continuation indent (`expressionBreakIndent`):
      ```kotlin
      if (a + b < 20)
          a + b
      else
          a
      ```
    - Inside return or assignment statements:
      ```kotlin
      val c =
          if (a + b < 20) a + b
          else a
      ```
- **Multiline Conditions**:
  - Condition expressions that wrap are indented by `expressionBreakIndent` inside parentheses.

### 5.2 `when` Expressions
- **Subject**:
  - With subject: `when (subject) { ... }`.
  - Without subject: `when { ... }`.
  - Braces: opening `{` on the same line after header, closing `}` on its own line.
- **Entry Conditions & Guards**:
  - Multiple conditions: comma-separated on a single line or broken with forced breaks:
    ```kotlin
    when (x) {
      1, 2, 3 -> doA()
      is String -> doB()
      !in 1..10 -> doC()
    }
    ```
  - Pattern conditions: `is <Type>`, `!is <Type>`.
  - Range conditions: `in <Range>`, `!in <Range>`.
  - Guard conditions (Kotlin 2.1+): `if <guardExpression>` without parentheses following the condition:
    ```kotlin
    when (animal) {
      is Animal.Cat if !animal.mouseHunter -> animal.feedCat()
      else if animal.eatsPlants -> animal.giveLettuce()
    }
    ```
  - Else branch: `else -> <expression>`.
- **Branch Arrow `->` and Body Layout**:
  - Block body or lambda body: `is Foo -> { ... }` or `is Bar -> { it -> ... }` on the same line.
  - Single expression: if it fits, single space after `->`; if it wraps, breaks with `expressionBreakIndent`:
    ```kotlin
    when (x) {
      1 ->
          veryLongExpressionCalculation(a, b)
    }
    ```
  - Trailing comma: when trailing commas are managed, a comma can be placed after the condition list before `->`, forcing a break before `->`.
  - Blank line preservation: user-authored blank lines between `when` branches are preserved.

### 5.3 Loops (`for`, `while`, `do-while`)
- **`for` Loop**:
  - Format: `for (<param> in <range>) <body>`.
  - If range breaks, indented with continuation indent inside parentheses.
  - Block body `{ ... }` attached on the same line.
- **`while` Loop**:
  - Format: `while (<condition>) <body>`.
  - Empty body guard: `while (condition());` retains semicolon.
- **`do-while` Loop**:
  - Format: `do { ... } while (<condition>)`.

### 5.4 Exception Handling (`try`, `catch`, `finally`)
- Format: `try { ... } catch (e: Exception) { ... } finally { ... }`.
- `catch` clauses and `finally` blocks attach to the closing brace on the same line.
- Multiple catches formatted sequentially without intervening blank lines.

### 5.5 Jump Statements (`return`, `throw`, `break`, `continue`)
- Labels: attached directly to keyword: `return@label expr`, `break@outer`, `continue@loop`.
- Semicolons: redundant semicolons after jump statements are stripped unless protected by trailing lambda guard.

---

## 6. Calls, Lambdas, Navigation Chains, and Arguments

### 6.1 Function Calls and Argument Lists
- **Basic Invocation**: `foo(arg1, arg2)`.
  - Type arguments: `foo<T, R>(arg1, arg2)` (no space between callee and `<`).
  - Empty argument list: `foo()`.
- **Wrapping and Multi-Line Arguments**:
  - When arguments exceed `maxWidth`, each argument is wrapped onto its own line indented by `continuationIndent` (4 spaces in Meta/Kotlinlang, 2 spaces in Google):
    ```kotlin
    myMethodCall(
        firstArgument,
        secondArgument,
        thirdArgument,
    )
    ```
  - Trailing comma inserted before closing `)` when multiline under `ONLY_ADD` or `COMPLETE` strategy.
- **Named Arguments**:
  - Lambda argument: `name = { ... }` stays on the same line after `=` without break.
  - Non-lambda argument: `name = value`. If it wraps, breaks immediately after `=` with continuation indent:
    ```kotlin
    myMethod(
        config =
            ConfigurationBuilder()
                .enableFeature()
                .build(),
        retries = 3,
    )
    ```
  - Spread operator: `*args` attached directly to the expression.

### 6.2 Lambda Expressions and Preserved Breaks
- **Single-Line vs Multi-Line Lambdas**:
  - Single-line: `{ x -> x + 1 }` or `{ Text("Hello") }`.
  - Multi-line:
    ```kotlin
    items.map { item ->
      val intermediate = compute(item)
      transform(intermediate)
    }
    ```
- **Trailing Lambda Syntax**:
  - If the last argument to a function call is a lambda, it is placed outside the parentheses: `foo(1, 2) { ... }`.
  - If the only argument is a lambda, parentheses are omitted: `foo { ... }`.
- **Preserve Lambda Breaks Rule (`preserveLambdaBreaks = true`)**:
  - If a lambda spans multiple lines in the source input, ktfmt **retains** its multi-line structure regardless of whether it could mathematically fit on a single line.
  - This preserves UI DSL hierarchies (e.g., Jetpack Compose / DSL builders):
    ```kotlin
    App {
      SelectableCard {
        Button { Text("Hello") }
      }
    }
    ```
  - If `preserveLambdaBreaks = false`, lambdas that fit within `maxWidth` are collapsed into a single line.
- **Scoping Functions and Conditional Indentation (`brokeBeforeBrace`)**:
  - In single-expression functions returning a scoping call:
    ```kotlin
    fun foo() =
        coroutineScope { x ->
          bar()
          car()
        }
    ```
  - If a line break occurs before the scoping lambda's opening brace (e.g. after `=`), the interior statements are dynamically indented relative to the function declaration level so that the body indentation aligns intuitively with a standard block.

### 6.3 Dot-Qualified and Safe-Navigation Chains
- **Break Placement**:
  - When a call chain wraps across multiple lines, line breaks occur **before** the dot (`.`) or safe-call operator (`?.`), NEVER after:
    ```kotlin
    val result =
        repository
            .findUserById(id)
            ?.orders
            ?.filter { it.isActive }
            ?.map { it.total }
    ```
  - Wrapped selectors are indented by `expressionBreakIndent` (continuation indent = 4 spaces in Meta/Kotlinlang, 2 spaces in Google).
- **Chained Scoping Functions (`.apply`, `.let`, `.run`, `.also`, `.use`)**:
  - When the root receiver is a multiline scoping function or lambda, a forced break is emitted before subsequent chained selectors:
    ```kotlin
    val foo =
        run {
          bar()
          baz()
        }
            .let { result -> process(result) }
    ```
- **Postfix Operators**:
  - `!!`, `++`, `--` attach directly to the operand (`foo!!.bar()`).

### 6.4 Array Access Expressions
- Format: `arr[index]`, `matrix[row, col]`.
- Multiline indices:
  ```kotlin
  array[
      firstIndexExpression,
      secondIndexExpression,
  ]
  ```

---

## 7. Binary Expressions, Types, and Annotations

### 7.1 Binary Operations and Precedence Wrapping
- **Left-to-Right Associativity Formatting**:
  - Chains of binary expressions (e.g. `a + b + c + d`) unwrap the left-most expression and format left-to-right.
  - Standard binary operators (`+`, `-`, `*`, `/`, `%`, `&&`, `||`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `is`, `!is`, `as`, `as?`, `in`, `!in`, infix functions):
    - Spaces around operators: `a + b`.
    - Line break occurs **after** the operator, with unified fill mode and continuation indent:
      ```kotlin
      val condition =
          firstCheck() &&
              secondCheck() &&
              thirdCheck()
      ```
    - If a line-breaking comment precedes the operator, the break mode switches to independent.
- **Range Operators (`..`, `..<`)**:
  - NO spaces around range operators: `0..10`, `0..<count`.
- **Elvis Operator (`?:`)**:
  - Distinct break placement: line break occurs **before** the `?:` operator, followed by a space:
    ```kotlin
    val name =
        user?.profile?.displayName
            ?: defaultGuestName
    ```
  - In chains of elvis operators:
    ```kotlin
    return option1()
        ?: option2()
        ?: option3()
    ```
- **Unary and Postfix Operators**:
  - Postfix (`++`, `--`, `!!`): attached to the operand without space: `x++`, `y!!`.
  - Prefix (`+`, `-`, `!`, `++`, `--`): attached to operand without space: `-x`, `!flag`.
  - Adjacent colliding operators insert a separating space (`+ +a`).
- **Callable and Class References**:
  - Callable references: `Receiver::member`. If receiver is nullable: `String?::isNullOrEmpty`.
  - Class references: `MyClass::class`.
  - Break before `::` with continuation indent if wrapping.

### 7.2 Types and Type Projections
- **Basic and User Types**:
  - Qualified types: `com.example.MyType`.
  - Nullable types: `T?` (no space before `?`).
  - Dynamic type: `dynamic`.
  - Intersection types (Kotlin 2.0+): `A & B & C` with single spaces around `&`.
- **Generics and Type Arguments**:
  - `Map<String, List<Int>>`.
  - Variance projections: `in T`, `out T`, `*` (star projection).
- **Function Types**:
  - `(A, B) -> R`, `() -> Unit`.
  - Extension function types: `Receiver.(A, B) -> R`.
  - Suspend function types: `suspend (A) -> R`.
  - Nullable function types: `((A) -> R)?`.
  - Trailing commas in function type parameter lists: preserved/managed when multi-line.
- **Type Aliases**:
  - `typealias Handler = (Event) -> Unit`.

### 7.3 Annotations and Use-Site Targets
- **Single Annotations**:
  - `@Target`, `@Retention(AnnotationRetention.SOURCE)`.
  - Single space separating annotations on the same line if compact, or forced break when preceding declarations.
- **Grouped Annotations Array Syntax (`@[...]`)**:
  - Format: `@[Anno1 Anno2 Anno3]`.
  - Use-site targets: `@field:[JvmStatic Volatile]`, `@get:JvmName(...)`, `@file:OptIn(...)`.
  - Space separates entries within the brackets.
  - Forced break after the closing bracket `]` when preceding a declaration.
- **Annotated Expressions in Blocks (Semantic Disambiguation Guard)**:
  - If an annotated expression is a binary expression inside a code block:
    ```kotlin
    @Suppress("UNCHECKED_CAST")
    b = f(1) as Int
    ```
    The formatter **forces a line break** between the annotation and the binary expression.
  - **Rationale**: In Kotlin grammar, `@Anno b = c` parses `@Anno` as attached only to `b`, whereas `@Anno \n b = c` attaches `@Anno` to the entire assignment expression statement `(b = c)`. Placing them on the same line would change program semantics.
  - Annotated `return` expressions also force a line break:
    ```kotlin
    @Anno
    return x
    ```
  - Annotated lambda expressions attach on the same line with space: `@Anno { ... }`.

---

## 8. Lists, Parameters, Trailing Commas, and Style Invariants

### 8.1 Comma-Separated List Layout
All comma-separated lists (arguments, parameters, type parameters, type arguments, collection literals, when conditions, enum entries) share fundamental layout rules:
- **Single-Line Form**:
  - `(a, b, c)` or `<T, R>`. Elements separated by `, ` (comma followed by space).
  - No spaces inside enclosing delimiters: `(arg)` not `( arg )`.
- **Multi-Line Form**:
  - When the list cannot fit on a single line within `maxWidth`:
    - Forced or unified line break immediately after the opening delimiter (`(`, `<`, `[`).
    - Each element placed on its own line, indented by continuation indent (`expressionBreakIndent`: 4 in Meta/Kotlinlang, 2 in Google).
    - Closing delimiter (`)`, `>`, `]`) placed on a new line aligned with the outer indentation level.
- **Single Unnamed Lambda Argument Special Case**:
  - If a function call has a single unnamed lambda inside parentheses: `foo({ ... })`.
  - The lambda body wraps without forcing a leading break after `(`, and closing `)` attaches immediately to `}` without a negative closing break:
    ```kotlin
    foo({
      doWork()
    })
    ```

### 8.2 Trailing Comma Lifecycle and Strategies
- **Managed Lists**:
  - Value argument lists: `foo(a, b,)`
  - Parameter lists: `fun bar(a: Int, b: String,)`
  - Type argument lists: `Map<String, Int,>`
  - Type parameter lists: `class Box<T, R,>`
  - Collection literals: `[1, 2, 3,]`
  - When conditions: `1, 2, -> ...`
  - Enum entries: `enum class Color { RED, GREEN, }`
- **Exceptions to Trailing Comma Insertion**:
  - Lambda parameters: `{ a, b -> ... }` NEVER have trailing commas automatically added.
  - Enum entries with terminating semicolon: `enum class Op { A, B; fun f() {} }` NEVER have a trailing comma added after `B` if a terminating semicolon is already present.
  - Empty lists: `()` or `<>` never receive commas.
  - Single-line lists in Google / Kotlinlang styles: trailing commas are actively removed.
- **Multi-Pass Fixed-Point Formatting (`prettyPrintAndManageTrailingCommas`)**:
  - Adding a trailing comma to a list that was near the 100-character line limit can push the line length to 101 characters.
  - Therefore, trailing comma insertion runs before pretty-printing and then iteratively checks if inserting a comma causes a line overflow, re-running layout until code reaches a stable fixed point.

### 8.3 Style Presets Detailed Comparison

| Behavior / Construct | Meta Style (Default) | Google Style (`--google-style`) | Kotlinlang Style (`--kotlinlang-style`) |
| :--- | :--- | :--- | :--- |
| **Block Indent** | 2 spaces | 2 spaces | 4 spaces |
| **Continuation Indent** | 4 spaces | 2 spaces | 4 spaces |
| **Trailing Comma Insertion** | Insert when multiline | Insert when multiline | Insert when multiline |
| **Trailing Comma Removal** | Preserved if user authored | Stripped if single-line | Stripped if single-line |
| **User Comma Break Hint** | Respected (forces multiline) | Ignored (collapses if fits) | Ignored (collapses if fits) |
| **Block-like Call in Property** | Breaks after `=` | Breaks after `=` | Stays on same line after `=` |
| **Expression Body Function** | Indents body +2 / +4 | Indents body +2 | Indents body +4 |
| **Supertype List Indent** | 4 spaces | 2 spaces | 4 spaces |

---

## 9. Comments, KDoc, and Multiline Raw Strings

### 9.1 Line and Block Comments
- **Placement and Spacing**:
  - Line comments (`// ...`) on their own line preserve their relative indentation.
  - Trailing line comments attached to statements or parameters stay on the same line after a single space.
  - Trailing spaces in comments are stripped.
  - Blank lines before comments are preserved (0 or 1 empty line).
- **Comment Fencing (`fenceComments`)**:
  - When line comments appear inside broken expression trees (such as parameter lists, binary chains, or when entries), comments are fenced so that subsequent tokens are never inadvertently emitted on the same line as the line comment (which would comment out executable code).

### 9.2 KDoc Documentation Comments (`/** ... */`)
- **Single-Line Collapse**:
  - If a KDoc comment consists of a single short paragraph that fits on a single line within `maxWidth` (including `/** ` and ` */`), it is collapsed into a single line:
    ```kotlin
    /** Simple single-line doc. */
    fun foo() {}
    ```
- **Multiline KDoc Structure**:
  - When KDoc exceeds line width or has multiple paragraphs/tags:
    ```kotlin
    /**
     * First paragraph describing the declaration.
     * Reflowed greedily to maxLineWidth (100 columns).
     *
     * Second paragraph separated by an empty line with asterisk.
     *
     * @param x the input coordinate
     * @param y the target coordinate
     * @return true if coordinate is inside bounds
     */
    ```
- **Preformatted Blocks & Markdown**:
  - Triple-backtick fenced code blocks (` ```kotlin ... ``` `) are preformatted and their inner whitespace and line breaks are strictly preserved.
  - Indented code blocks (indented by 4 spaces) are also preformatted.
  - Markdown lists (`-`, `*`, `1.`): hanging indent applied to wrapped continuation lines.
  - Markdown tables: column markers `|` aligned.
  - Bracket links (`[Name]` or `[Class.member]`): whitespace around links preserved.
- **Tag Ordering**:
  - Block tags (`@param`, `@property`, `@return`, `@throws`, `@see`, `@sample`) are grouped and ordered towards the end of the docstring.

### 9.3 Multiline Raw Strings (`"""..."""`)
- **Qualifying Strings for Formatter Normalization**:
  - Multiline raw string literal with 2 or more lines followed by `.trimIndent()` or `.trimMargin()`.
  - **Exclusion Rule**: If the multiline raw string contains string template expressions (e.g. `${...}`), it is **NOT** reformatted, because variable interpolations could alter the runtime indentation or margin calculation.
  - **Exclusion Rule**: Multiline strings nested inside other template expressions are left untouched.
- **Formatting Transformation**:
  - Opening delimiter `"""` (or `$$"""` for dollar strings) placed on its own line after the assignment / call:
    ```kotlin
    val message =
        """
        first line
        second line
        """
            .trimIndent()
    ```
  - Inner lines are normalized: common leading whitespace is adjusted to match the declaration's base indentation.
  - Closing `"""` placed on its own line aligned with the declaration.
  - The `.trimIndent()` or `.trimMargin()` method call is placed on the following line, indented by `continuationIndent` (4 spaces in Meta/Kotlinlang, 2 spaces in Google).
  - Any comments between the closing `"""` and `.trimIndent()` are preserved in order.

---

## 10. Range Formatting, EditorConfig, and File Invariants

### 10.1 Range and Partial Formatting
- **Range Specifications**:
  - Line ranges (`--lines=<start>:<end>`): 1-indexed, inclusive bounds translated to 0-indexed character offsets.
  - Character ranges (`--offset=<offset> --length=<length>`): 0-indexed character offsets. Length `0` expands to the full line containing the offset.
- **Cleanup-After-Selection Lifecycle**:
  - Pretty-print replacements are strictly restricted to the specified character ranges (tokens intersecting the range).
  - Whole-file cleanup passes (import sorting, redundant import removal, redundant semicolon detection, multiline raw string formatting) run whole-file on the resulting document.
  - Preserves AST syntactical correctness and idempotency across partial edits.

### 10.2 EditorConfig Integration
When `--editorconfig` is enabled, `.editorconfig` files from the target file directory ascending to the filesystem root are resolved:
- `max_line_length`: Overrides `maxWidth`.
- `indent_size` or `ij_kotlin_indent_size`: Overrides `blockIndent`.
- `ij_continuation_indent_size` or `ij_kotlin_continuation_indent_size`: Overrides `continuationIndent`.
- `ktfmt_trailing_comma_management_strategy`: Overrides trailing comma strategy (`none`, `only_add`, `complete`).

### 10.3 Encoding and Line Ending Preservation
- **Newline Invariants**:
  - Formatter detects the dominant line ending in the input file:
    - LF (`\n`): standard Unix line ending.
    - CRLF (`\r\n`): standard Windows line ending.
    - CR (`\r`): legacy classic Mac line ending.
  - All emitted lines, empty lines, and multiline strings match the detected line ending.
- **BOM (Byte Order Mark) & Shebang**:
  - UTF-8 BOM (`\uFEFF`) is preserved at offset 0 if present.
  - Shebang line (e.g. `#!/usr/bin/env kscript`) is preserved as the very first line before package or directives.

---
