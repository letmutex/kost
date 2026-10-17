use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_includes_unbound_callable_reference_usages() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnboundRefs.kt").unwrap();
    let source = r#"
package com.example

fun compute(x: Int): Int = x * 2

fun runner() {
    compute(5)
    val ref1 = ::compute
    val list = listOf(1, 2)
    list.map(::compute)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `fun compute` declaration (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "compute", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        3,
        "Should find call site compute(5), val ref1 = ::compute, and list.map(::compute)"
    );
}

#[test]
fn test_references_includes_member_callable_reference_usages() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberRefs.kt").unwrap();
    let source = r#"
package com.example

class Service {
    fun execute(): Boolean = true
}

fun runner(svc: Service) {
    svc.execute()
    val unboundRef = Service::execute
    val boundRef = svc::execute
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `fun execute` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "execute", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        3,
        "Should find direct call, unbound Service::execute, and bound svc::execute"
    );
}

#[test]
fn test_references_invoked_from_unbound_callable_reference_site() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/InvokeUnboundRef.kt").unwrap();
    let source = r#"
package com.example

fun helper() {}

fun main() {
    helper()
    val ref = ::helper
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `helper` in `val ref = ::helper` (line 7, col 17)
    let ref_pos = Position::new(7, 17);
    let refs = find_references(&index, &url, "helper", ref_pos, source, true);

    assert_eq!(
        refs.len(),
        3,
        "Should find declaration, call helper(), and reference ::helper"
    );
}

#[test]
fn test_references_invoked_from_member_callable_reference_site() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/InvokeMemberRef.kt").unwrap();
    let source = r#"
package com.example

class Worker {
    fun work() {}
}

fun main(w: Worker) {
    w.work()
    val ref = Worker::work
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `work` in `Worker::work` (line 9, col 23)
    let ref_pos = Position::new(9, 23);
    let refs = find_references(&index, &url, "work", ref_pos, source, true);

    assert_eq!(
        refs.len(),
        3,
        "Should find Worker.work declaration, w.work(), and Worker::work"
    );
}

#[test]
fn test_references_constructor_reference_usages() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ConstructorUsages.kt").unwrap();
    let source = r#"
package com.example

class Factory(val name: String)

fun main() {
    val f1 = Factory("test")
    val f2 = ::Factory
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `class Factory` declaration (line 3, col 6)
    let decl_pos = Position::new(3, 6);
    let refs = find_references(&index, &url, "Factory", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        2,
        "Should find constructor invocation Factory(\"test\") and constructor reference ::Factory"
    );
}

#[test]
fn test_references_callable_reference_not_shadowed_by_local_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ShadowedRefUsages.kt").unwrap();
    let source = r#"
package com.example

fun performAction() {}

fun main() {
    val performAction = 1
    val ref = ::performAction
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun performAction` declaration (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "performAction", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Should find ::performAction even when a local variable performAction exists"
    );
}

#[test]
fn test_references_local_variable_excludes_callable_reference_to_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalVarRefExclusion.kt").unwrap();
    let source = r#"
package com.example

fun performAction() {}

fun main() {
    val performAction = 1
    val ref = ::performAction
    val result = performAction + 2
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `val performAction = 1` declaration (line 6, col 8)
    let decl_pos = Position::new(6, 8);
    let refs = find_references(&index, &url, "performAction", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Local variable references should only find `performAction + 2`, never `::performAction`"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_from_callable_reference_resolves_function_not_shadowing_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/RefFromCallableRef.kt").unwrap();
    let source = r#"
package com.example

fun performAction() {}

fun main() {
    val performAction = 1
    val ref = ::performAction
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `performAction` inside `val ref = ::performAction` (line 7, col 16)
    let call_site_pos = Position::new(7, 16);
    let refs = find_references(&index, &url, "performAction", call_site_pos, source, true);

    // Should include declaration of `fun performAction` (line 3) and callable ref usage (line 7)
    let lines: Vec<u32> = refs.iter().map(|loc| loc.range.start.line).collect();
    assert!(
        lines.contains(&3),
        "Should resolve to top-level function declaration on line 3, not local variable on line 6"
    );
    assert!(lines.contains(&7), "Should find usage on line 7");
    assert!(
        !lines.contains(&6),
        "Must not include local variable declaration on line 6"
    );
}

#[test]
fn test_references_includes_callable_reference_to_overloaded_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/OverloadedCallableRef.kt").unwrap();
    let source = r#"
package com.example

class Service {
    fun compute(x: Int): Int = x * 2
    fun compute(s: String): String = s
}

fun runner() {
    val ref1 = Service::compute
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun compute(x: Int)` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "compute", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Callable reference Service::compute must be found even when compute is overloaded"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_includes_callable_reference_to_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PropertyCallableRef.kt").unwrap();
    let source = r#"
package com.example

class User {
    val name: String = "Alice"
}

fun runner(u: User) {
    val ref1 = User::name
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `val name: String` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "name", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Callable reference User::name must be found for property declaration"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_includes_unbound_callable_reference_to_top_level_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TopLevelPropertyCallableRef.kt").unwrap();
    let source = r#"
package com.example

val globalConfig: String = "production"

fun runner() {
    val ref1 = ::globalConfig
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `val globalConfig: String` declaration (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "globalConfig", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Unbound callable reference ::globalConfig must be found for top-level property declaration"
    );
    assert_eq!(refs[0].range.start.line, 6);
}
