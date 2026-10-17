use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_sibling_subclass_method_call_excludes_unrelated_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/HierarchySibling.kt").unwrap();
    let code = r#"
package com.example

interface Base {
    fun inspect(): String = "base"
}

class FirstSubclass : Base {
    override fun inspect(): String = "first"
}

class SecondSubclass : Base {
    fun run() {
        super.inspect()
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `override fun inspect()` in FirstSubclass (line 8, col 17)
    let decl_pos = Position::new(8, 17);
    let refs = find_references(&index, &file_url, "inspect", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Super call in sibling class should not match unrelated override in FirstSubclass"
    );
}

#[test]
fn test_null_argument_matches_nullable_parameter_overload() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NullArg.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel(cause: Exception? = null)
    fun cancel(): Unit
}

fun testCancel(job: Job) {
    job.cancel(null)
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel(cause: Exception? = null)` declaration in Job (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Passing null to nullable parameter should match the overload taking nullable Exception"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_named_argument_matches_parameter_name_overload() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NamedArgOverload.kt").unwrap();
    let code = r#"
package com.example

class Context
class Options

interface Launcher {
    fun launch(context: Context = Context(), options: Options = Options(), block: () -> Unit)
    fun launch(options: Options)
}

fun testNamed(launcher: Launcher) {
    launcher.launch(options = Options()) {
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun launch` declaration in Launcher (line 7, col 8)
    let decl_pos = Position::new(7, 8);
    let refs = find_references(&index, &file_url, "launch", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Named argument options should match parameter named options instead of positional parameter context"
    );
    assert_eq!(refs[0].range.start.line, 12);
}

#[test]
fn test_overload_call_with_indirect_external_supertype_argument() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ExternalHierarchy.kt").unwrap();
    let code = r#"
package com.example

class CustomElement : MidInterface

interface Processor {
    fun process(context: RootInterface = object : RootInterface {}, action: () -> Unit)
    fun process(count: Int)
}

fun testProcess(processor: Processor, element: CustomElement) {
    processor.process(element) {
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun process` declaration taking RootInterface (line 6, col 8)
    let decl_pos = Position::new(6, 8);
    let refs = find_references(&index, &file_url, "process", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Call with CustomElement should match process(RootInterface) overload even when intermediate hierarchy is external"
    );
    assert_eq!(refs[0].range.start.line, 11);
}

#[test]
fn test_trailing_lambda_receiver_from_supertype_method_resolves_implicit_call() {
    let mut index = WorkspaceIndex::new();
    let base_url = Url::parse("file:///project/TestBase.kt").unwrap();
    let base_code = r#"
package com.example.testing

interface Scope

fun Scope.launch() {}

open class TestBase {
    fun runTest(
        expected: ((Throwable) -> Boolean)? = null,
        unhandled: List<String> = emptyList(),
        block: Scope.() -> Unit
    ) {}
}
"#;
    index.index_source(base_url.clone(), base_code);

    let test_url = Url::parse("file:///project/ConsumerTest.kt").unwrap();
    let test_code = r#"
package com.example

import com.example.testing.*

class ConsumerTest : TestBase() {
    fun testRun() = runTest {
        launch()
    }
}
"#;
    index.index_source(test_url.clone(), test_code);

    // Cursor at `fun Scope.launch()` in TestBase.kt (line 5, col 10)
    let decl_pos = Position::new(5, 10);
    let refs = find_references(&index, &base_url, "launch", decl_pos, base_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Implicit call to launch inside runTest trailing lambda should match Scope.launch"
    );
    assert_eq!(refs[0].uri, test_url);
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_trailing_lambda_receiver_binds_to_last_functional_parameter_with_preceding_functional_param()
 {
    let mut index = WorkspaceIndex::new();
    let base_url = Url::parse("file:///project/TestBaseActual.kt").unwrap();
    let base_code = r#"
package com.example.testing

interface Scope

fun Scope.launch() {}

open class TestBase {
    fun runTest(
        expected: ((Throwable) -> Boolean)?,
        unhandled: List<String>,
        block: Scope.() -> Unit
    ) {}
}
"#;
    index.index_source(base_url.clone(), base_code);

    let test_url = Url::parse("file:///project/ConsumerTest.kt").unwrap();
    let test_code = r#"
package com.example

import com.example.testing.*

class ConsumerTest : TestBase() {
    fun testRun() = runTest {
        launch()
    }
}
"#;
    index.index_source(test_url.clone(), test_code);

    // Cursor at `fun Scope.launch()` in TestBaseActual.kt (line 5, col 10)
    let decl_pos = Position::new(5, 10);
    let refs = find_references(&index, &base_url, "launch", decl_pos, base_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Trailing lambda should bind to block with Scope receiver, not preceding expected parameter"
    );
    assert_eq!(refs[0].uri, test_url);
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_implicit_call_in_lambda_with_star_imported_supertype_receiver_matches_reference() {
    let mut index = WorkspaceIndex::new();
    let scope_url = Url::parse("file:///project/Scope.kt").unwrap();
    let scope_code = r#"
package com.example.scope

interface CoroutineScope

fun CoroutineScope.launch() {}
"#;
    index.index_source(scope_url.clone(), scope_code);

    let test_scope_url = Url::parse("file:///project/TestScope.kt").unwrap();
    let test_scope_code = r#"
package com.example.test

import com.example.scope.*

interface TestScope : CoroutineScope

open class TestBase {
    fun runTest(block: TestScope.() -> Unit) {}
}
"#;
    index.index_source(test_scope_url.clone(), test_scope_code);

    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_code = r#"
package com.example

import com.example.scope.*
import com.example.test.*

class Consumer : TestBase() {
    fun testRun() = runTest {
        launch()
    }
}
"#;
    index.index_source(consumer_url.clone(), consumer_code);

    // Cursor at `fun CoroutineScope.launch()` in Scope.kt (line 5, col 20)
    let decl_pos = Position::new(5, 20);
    let refs = find_references(&index, &scope_url, "launch", decl_pos, scope_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Implicit launch() inside runTest should match CoroutineScope.launch via TestScope supertype from star import"
    );
    assert_eq!(refs[0].uri, consumer_url);
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_infix_call_matches_overloaded_signature() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/InfixOverload.kt").unwrap();
    let source = r#"
package com.example

class MathBox {
    infix fun add(x: Int): Int = x
    infix fun add(s: String): String = s
}

fun runner(box: MathBox) {
    val a = box add 42
    val b = box add "text"
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `infix fun add(x: Int)` declaration (line 4, col 14)
    let decl_pos = Position::new(4, 14);
    let refs = find_references(&index, &url, "add", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Infix call `box add 42` must match `add(x: Int)`"
    );
    assert_eq!(refs[0].range.start.line, 9);
}
