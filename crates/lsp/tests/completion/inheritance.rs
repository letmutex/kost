use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_inheritance_interface_and_base_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/InheritanceTest.kt").unwrap();
    let code = r#"
package com.example.inheritance

interface Repository {
    fun findById(id: String): String
    val count: Int
}

open class BaseService : Repository {
    override fun findById(id: String): String = id
    override val count: Int = 0
    fun baseAction(): Unit {}
}

class UserService : BaseService() {
    fun customAction(): String = "custom"
}

fun test(service: UserService) {
    service.
    val item = service.findById("42")
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion on service. should offer inherited members from BaseService and Repository:
    let pos_dot = Position::new(19, 12);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"customAction".to_string()),
        "service. should contain own method 'customAction'"
    );
    assert!(
        labels.contains(&"baseAction".to_string()),
        "service. should contain inherited method 'baseAction' from BaseService"
    );
    assert!(
        labels.contains(&"findById".to_string()),
        "service. should contain interface method 'findById' from Repository"
    );
    assert!(
        labels.contains(&"count".to_string()),
        "service. should contain interface property 'count' from Repository"
    );

    // 2. Definition of inherited member call service.findById:
    let pos_find = Position::new(20, 24);
    let def = find_definition(&index, &url, "findById", pos_find, code);
    assert!(
        def.is_some(),
        "service.findById should resolve definition of inherited interface method"
    );
}

#[test]
fn test_member_completion_abstract_class_inherited_members() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AbstractInheritanceTest.kt").unwrap();
    let code = r#"
package com.example.abstracts

abstract class AbstractWorker {
    fun executeTask(): Unit {}
    val isRunning: Boolean = false
}

class WorkerImpl : AbstractWorker()

fun test(worker: WorkerImpl) {
    worker.
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(11, 11);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"executeTask".to_string()),
        "worker. should offer 'executeTask' inherited from AbstractWorker"
    );
    assert!(
        labels.contains(&"isRunning".to_string()),
        "worker. should offer 'isRunning' inherited from AbstractWorker"
    );
}
