use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_completion_unbound_reference_omits_call_parens() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnboundCompletion.kt").unwrap();
    let source = r#"
package com.example

fun performAction(data: String): Boolean = true

fun main() {
    val f = ::per
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `val f = ::per` (line 6, col 17)
    let pos = Position::new(6, 17);
    let completions = find_completions(&index, &url, pos, source);

    let perform_item = completions
        .iter()
        .find(|item| item.label == "performAction")
        .expect("performAction should be suggested");

    if let Some(ref insert_text) = perform_item.insert_text {
        assert_eq!(
            insert_text, "performAction",
            "Insert text for callable reference must not contain call parentheses"
        );
    }
}

#[test]
fn test_completion_unbound_reference_excludes_keywords() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NoKeywordsCompletion.kt").unwrap();
    let source = r#"
package com.example

fun doSomething() {}

fun main() {
    val f = ::
}
"#;
    index.index_source(url.clone(), source);

    // Cursor right after `::` (line 6, col 14)
    let pos = Position::new(6, 14);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();

    assert!(labels.contains(&"doSomething"));
    assert!(
        !labels.contains(&"fun"),
        "Keywords should not be suggested after ::"
    );
    assert!(
        !labels.contains(&"val"),
        "Keywords should not be suggested after ::"
    );
    assert!(
        !labels.contains(&"package"),
        "Keywords should not be suggested after ::"
    );
    assert!(
        !labels.contains(&"import"),
        "Keywords should not be suggested after ::"
    );
}

#[test]
fn test_completion_unbound_reference_suggests_constructors() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ConstructorCompletion.kt").unwrap();
    let source = r#"
package com.example

class TaskRunner(val id: Int)

fun main() {
    val f = ::Task
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `::Task` (line 6, col 18)
    let pos = Position::new(6, 18);
    let completions = find_completions(&index, &url, pos, source);

    let task_item = completions
        .iter()
        .find(|item| item.label == "TaskRunner")
        .expect("TaskRunner constructor should be suggested");

    if let Some(ref insert_text) = task_item.insert_text {
        assert_eq!(
            insert_text, "TaskRunner",
            "Constructor reference must omit parentheses"
        );
    }
}
#[test]
fn test_completion_callable_reference_inside_arguments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArgumentCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun computeLength(s: String): Int = s.length

fun applyTransform(transform: (String) -> Int) {}

fun main() {
    applyTransform(::comp)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `::comp` (line 8, col 25)
    let pos = Position::new(8, 25);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();

    assert!(labels.contains(&"computeLength"));
    assert!(
        !labels.contains(&"transform = "),
        "Named arguments should not be suggested after ::"
    );
}

#[test]
fn test_unbound_callable_reference_completion_after_return_keyword() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ReturnCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun helperAction() {}

fun getAction(): () -> Unit {
    return ::help
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `return ::help` (line 6, col 17)
    let pos = Position::new(6, 17);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"helperAction"),
        "Unbound top-level function should be completed following return ::"
    );
}

#[test]
fn test_unbound_callable_reference_completion_after_else_keyword() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ElseCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun defaultAction() {}

fun pickAction(cond: Boolean): () -> Unit = if (cond) { () -> Unit } else ::def
"#;
    index.index_source(url.clone(), source);

    // Cursor at `else ::def` (line 5, col 79)
    let pos = Position::new(5, 79);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"defaultAction"),
        "Unbound top-level function should be completed following else ::"
    );
}

#[test]
fn test_completion_unbound_callable_not_suppressed_by_local_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalVarPoisoning.kt").unwrap();
    let source = r#"
package com.example

val globalValue: Int = 100

fun main() {
    val globalValue = 42
    val ref = ::global
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `global` in `val ref = ::global` (line 7, col 22)
    let pos = Position::new(7, 22);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"globalValue"),
        "Top-level property `globalValue` should be completed even when local variable `globalValue` is in scope"
    );
}

#[test]
fn test_unbound_callable_reference_completion_after_previous_statement() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/StatementPrecededCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun helperAction() {}

fun main() {
    val count = 1
    ::help
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `::help` (line 7, col 10)
    let pos = Position::new(7, 10);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"helperAction"),
        "Unbound top-level function should be completed when :: is on a new line after a statement"
    );
}

#[test]
fn test_unbound_callable_reference_excludes_local_variables() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalVarCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun main() {
    val myLocalData = 10
    fun myLocalFunc() {}
    ::myLocal
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `::myLocal` (line 6, col 13)
    let pos = Position::new(6, 13);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"myLocalFunc"),
        "Local function should be suggested in callable references"
    );
    assert!(
        !labels.contains(&"myLocalData"),
        "Local variables must not be suggested in callable references"
    );
}

#[test]
fn test_completion_callable_reference_after_labeled_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LabeledReturnCallable.kt").unwrap();
    let source = r#"
package com.example

fun performAction() {}

fun test() {
    listOf(1).forEach {
        return@forEach ::performAct
    }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `::performAct` (line 7, col 35)
    let pos = Position::new(7, 35);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"performAction"),
        "Callable reference after labeled return should suggest performAction"
    );
}

#[test]
fn test_completion_callable_reference_after_if_condition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IfConditionCallable.kt").unwrap();
    let source = r#"
package com.example

fun performAction() {}

fun test(flag: Boolean) {
    val fnRef = if (flag) ::performAct else ::performAct
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at first `::performAct` (line 6, col 38)
    let pos = Position::new(6, 38);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"performAction"),
        "Callable reference after `if (condition)` should suggest unbound performAction"
    );
}
