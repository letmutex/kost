use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use lsp::syntax::symbol_item::{DeclarationItem, DeclarationMetadata, FileSymbols};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Range, SymbolKind, Url};

#[test]
fn test_references_finds_subclass_inherited_method_call() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/JobHierarchy.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

interface CompletableJob : Job

fun testInherited(job: CompletableJob) {
    job.cancel()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should find call on subclass CompletableJob as reference to Job.cancel"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_finds_subclass_overridden_method_call() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/JobOverride.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

class JobSupport : Job {
    override fun cancel() {}
}

fun testOverride(worker: JobSupport) {
    worker.cancel()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should find call to overridden cancel on JobSupport as reference to Job.cancel"
    );
    assert_eq!(refs[0].range.start.line, 12);
}

#[test]
fn test_references_finds_scope_function_implicit_receiver_call() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/JobScope.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

fun testWith(job: Job) {
    with(job) {
        cancel()
    }
}

fun testApply(job: Job) {
    job.apply {
        cancel()
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        2,
        "Should find implicit receiver calls inside with(job) and job.apply"
    );
}

#[test]
fn test_references_excludes_unrelated_class_same_method_name() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Unrelated.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

class Channel {
    fun cancel() {}
}

fun testMixed(job: Job, channel: Channel) {
    job.cancel()
    channel.cancel()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only find job.cancel(), not channel.cancel()"
    );
    assert_eq!(refs[0].range.start.line, 12);
}

#[test]
fn test_references_excludes_non_callable_nodes_sharing_function_name() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NonCallable.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    fun cancel()
}

fun testParamsAndProps(cancel: Boolean, job: Job) {
    val cancelValue = cancel
    if (cancel) {
        job.cancel()
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun cancel()` declaration in `Job` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "cancel", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only find call job.cancel(), excluding parameter and local variable reads"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_excludes_cross_package_same_simple_name() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/pkgA/Worker.kt").unwrap();
    let code_a = r#"
package com.pkgA

class Worker {
    fun doWork() {}
}

fun callA(worker: Worker) {
    worker.doWork()
}
"#;
    let file_b_url = Url::parse("file:///project/pkgB/Worker.kt").unwrap();
    let code_b = r#"
package com.pkgB

class Worker {
    fun doWork() {}
}

fun callB(worker: Worker) {
    worker.doWork()
}
"#;
    index.index_source(file_a_url.clone(), code_a);
    index.index_source(file_b_url.clone(), code_b);

    // Cursor at `fun doWork()` in com.pkgA.Worker (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_a_url, "doWork", decl_pos, code_a, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only find references within com.pkgA, not unrelated com.pkgB.Worker"
    );
    assert_eq!(refs[0].uri, file_a_url);
}

#[test]
fn test_references_excludes_cross_package_nested_classes_with_same_name() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/pkgA/Outer.kt").unwrap();
    let code_a = r#"
package com.pkgA

class Outer {
    class Inner {
        fun doWork() {}
    }
}

fun callA(inner: Outer.Inner) {
    inner.doWork()
}
"#;
    let file_b_url = Url::parse("file:///project/pkgB/Outer.kt").unwrap();
    let code_b = r#"
package com.pkgB

class Outer {
    class Inner {
        fun doWork() {}
    }
}

fun callB(inner: Outer.Inner) {
    inner.doWork()
}
"#;
    index.index_source(file_a_url.clone(), code_a);
    index.index_source(file_b_url.clone(), code_b);

    // Cursor at `fun doWork()` in com.pkgA.Outer.Inner (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_a_url, "doWork", decl_pos, code_a, false);

    assert_eq!(
        refs.len(),
        1,
        "Should only find references within com.pkgA.Outer.Inner, not com.pkgB.Outer.Inner"
    );
    assert_eq!(refs[0].uri, file_a_url);
}

#[test]
fn test_references_finds_bytecode_declaration_with_default_range() {
    let mut index = WorkspaceIndex::new();
    let jar_url = Url::parse("jar:file:///libs/library.jar!/com/example/LibService.class").unwrap();
    let mut file_symbols = FileSymbols {
        package_name: Some("com.example".to_string()),
        ..Default::default()
    };

    let default_range = Range::default();
    file_symbols.declarations.push(DeclarationItem::new(
        "LibService",
        SymbolKind::CLASS,
        default_range,
        default_range,
        DeclarationMetadata::new(None, None, None, false),
    ));
    file_symbols.declarations.push(DeclarationItem::new(
        "execute",
        SymbolKind::METHOD,
        default_range,
        default_range,
        DeclarationMetadata::new(None, None, Some(SmolStr::new("LibService")), false),
    ));
    index.insert_file_symbols(jar_url.clone(), file_symbols);

    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let code = r#"
package com.example

fun run(service: LibService) {
    service.execute()
}
"#;
    index.index_source(consumer_url.clone(), code);

    // Call site at line 4, col 14
    let call_pos = Position::new(4, 14);
    let refs = find_references(&index, &consumer_url, "execute", call_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Should find references to bytecode method with default range"
    );
    assert_eq!(refs[0].uri, consumer_url);
}
