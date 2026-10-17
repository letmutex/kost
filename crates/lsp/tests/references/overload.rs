use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_excludes_member_function_overload_in_same_class() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Calculator.kt").unwrap();
    let code = r#"
package com.example

class Calculator {
    fun calculate(num: Int): Int = num * 2
    fun calculate(str: String): String = str + str
}

fun testCalc(calc: Calculator) {
    calc.calculate(10)
    calc.calculate("hello")
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun calculate(num: Int)` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "calculate", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only find call to calculate(10), not calculate(\"hello\")"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_excludes_subclass_method_with_different_signature() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/CustomJob.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

class CustomJob : Job {
    fun cancel(cause: String) {}
}

fun testSubclass(job: CustomJob) {
    job.cancel("error")
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Should not match cancel(\"error\") as reference to parameterless Job.cancel()"
    );
}

#[test]
fn test_references_navigation_receiver_not_pruned_by_callee_arity() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/WorkerTest.kt").unwrap();
    let code = r#"
package com.example

class Worker {
    fun process(first: Int, second: Int) {}
}

fun getWorker(): Worker = Worker()

fun testNavigation() {
    getWorker().process(10, 20)
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(7, 4);
    let refs = find_references(&index, &file_url, "getWorker", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should find getWorker() call site despite subsequent 2-arg chained call"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_arity_filtering_with_default_and_vararg_parameters() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Logger.kt").unwrap();
    let code = r#"
package com.example

class Logger {
    fun log(level: Int, msg: String = "info", vararg tags: String) {}
    fun log() {}
}

fun testLog(logger: Logger) {
    logger.log()
    logger.log(1)
    logger.log(2, "custom")
    logger.log(3, "custom", "tag1", "tag2")
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "log", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        3,
        "Should match 1-arg, 2-arg, and 4-arg calls, excluding 0-arg overload"
    );
    let lines: Vec<_> = refs.iter().map(|r| r.range.start.line).collect();
    assert_eq!(lines, vec![10, 11, 12]);
}

#[test]
fn test_references_arity_unparseable_signature_fallback_not_pruned() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/RawService.kt").unwrap();
    let code = r#"
package com.example

class RawService {
    fun dispatch(cmd: String) {}
}

fun testDispatch(service: RawService) {
    service.dispatch("start")
}
"#;
    index.index_source(file_url.clone(), code);

    if let Some(file_syms) = index.files.get_mut(&file_url) {
        for decl in &mut file_syms.declarations {
            if decl.name == "dispatch" {
                decl.signature = Some(smol_str::SmolStr::new("dispatch"));
            }
        }
    }

    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "dispatch", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should not prune candidate when signature parameter list cannot be parsed"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_arity_filtering_with_trailing_lambda_and_value_arguments() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ConvertTest.kt").unwrap();
    let code = r#"
package com.example

class Converter {
    fun convert(step: Int, transform: (String) -> String): String = transform(step.toString())
    fun convert(step: Int): String = step.toString()
}

fun testConvert(c: Converter) {
    c.convert(1) { it }
    c.convert(2)
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "convert", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should match 2-arg overload (1 value arg + 1 trailing lambda)"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_arity_filtering_zero_params_with_return_type() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ActionTest.kt").unwrap();
    let code = r#"
package com.example

class Action {
    fun execute(): Boolean = true
    fun execute(data: String): Boolean = false
}

fun testAction(action: Action) {
    action.execute()
    action.execute("payload")
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should match only 0-arg overload execute(), excluding 1-arg execute(payload)"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_exclude_shadowed_default_parameter_when_exact_overload_exists() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/JobTest.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel(cause: String? = null)
    fun cancel()
}

fun testJob(job: Job) {
    job.cancel()
    job.cancel("reason")
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel(cause: String? = null)` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only match 1-arg call job.cancel(\"reason\"), excluding 0-arg call which resolves to parameterless cancel()"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_match_member_property_on_typed_receiver() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/PropertyTest.kt").unwrap();
    let code = r#"
package com.example

interface Worker {
    val isActive: Boolean
}

fun testWorker(worker: Worker) {
    if (worker.isActive) {
        println("active")
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `val isActive: Boolean` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "isActive", decl_pos, code, false);

    assert_eq!(refs.len(), 1, "Should find worker.isActive property access");
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_preserves_lambda_call_regardless_of_inferred_return_type() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun execute(action: () -> Unit) {}
fun execute(action: () -> String) {}

fun test() {
    execute {
        "hello"
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun execute(action: () -> Unit)` declaration (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &file_url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to execute(() -> Unit) should not lose references when lambda body returns String"
    );
}

#[test]
fn test_references_matches_exact_lambda_overload_when_targeted() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun execute(action: () -> Unit) {}
fun execute(action: () -> String) {}

fun test() {
    execute {
        "hello"
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun execute(action: () -> String)` declaration (line 4, col 4)
    let decl_pos = Position::new(4, 4);
    let refs = find_references(&index, &file_url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to execute(() -> String) should match the lambda call"
    );
}
