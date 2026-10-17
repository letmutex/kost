use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_class_members_visible_inside_class_methods() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberScopeTest.kt").unwrap();
    let code = r#"
package com.example.model

class User(val id: String, val name: String) {
    val isActive: Boolean = true
    fun getDisplayName(): String {
        
    }
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(7, 8), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"id".to_string()),
        "id should be visible inside User member methods"
    );
    assert!(
        labels.contains(&"name".to_string()),
        "name should be visible inside User member methods"
    );
    assert!(
        labels.contains(&"isActive".to_string()),
        "isActive should be visible inside User member methods"
    );
    assert!(
        labels.contains(&"getDisplayName".to_string()),
        "getDisplayName should be visible inside User member methods"
    );
}

#[test]
fn test_nested_and_inner_class_member_scoping() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedTest.kt").unwrap();
    let code = r#"
package com.example.nested

class Outer {
    val outerVal: Int = 1
    fun outerMethod(): Int = outerVal

    class Inner {
        val innerVal: Int = 2
        fun innerMethod() {
            
        }
    }
}

class Unrelated {
    val unrelatedVal: Int = 99
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(10, 12), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"innerVal".to_string()),
        "innerVal should be in scope inside Inner"
    );
    assert!(
        labels.contains(&"innerMethod".to_string()),
        "innerMethod should be in scope inside Inner"
    );
    assert!(
        labels.contains(&"outerVal".to_string()),
        "outerVal from enclosing Outer class should be in scope"
    );
    assert!(
        labels.contains(&"outerMethod".to_string()),
        "outerMethod from enclosing Outer class should be in scope"
    );
    assert!(
        !labels.contains(&"unrelatedVal".to_string()),
        "unrelatedVal from Unrelated class must NOT leak into Inner"
    );
}

#[test]
fn test_companion_object_member_scoping() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompanionTest.kt").unwrap();
    let code = r#"
package com.example.companion

class Config {
    companion object {
        val DEFAULT_TIMEOUT: Int = 5000
        fun createDefault(): Config = Config()
    }

    fun load() {
        
    }
}

fun outside() {
    
}
"#;
    index.index_source(url.clone(), code);

    let inside_completions = find_completions(&index, &url, Position::new(10, 8), code);
    let inside_labels: Vec<String> = inside_completions.into_iter().map(|c| c.label).collect();

    assert!(inside_labels.contains(&"DEFAULT_TIMEOUT".to_string()));
    assert!(inside_labels.contains(&"createDefault".to_string()));

    let outside_completions = find_completions(&index, &url, Position::new(15, 4), code);
    let outside_labels: Vec<String> = outside_completions.into_iter().map(|c| c.label).collect();

    assert!(outside_labels.contains(&"Config".to_string()));
    assert!(!outside_labels.contains(&"DEFAULT_TIMEOUT".to_string()));
    assert!(!outside_labels.contains(&"createDefault".to_string()));
}

#[test]
fn test_member_access_still_works_with_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ReceiverTest.kt").unwrap();
    let code = r#"
package com.example.test

class Item(val itemId: String, val title: String) {
    fun formatItem(): String = "$itemId: $title"
}

fun run() {
    val item = Item("1", "Hello")
    item.
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(9, 9), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"itemId".to_string()));
    assert!(labels.contains(&"title".to_string()));
    assert!(labels.contains(&"formatItem".to_string()));
}

#[test]
fn test_cross_class_nested_object_members_do_not_leak() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedObjects.kt").unwrap();
    let code = r#"
package com.example.config

class OuterA {
    object Config {
        val hostA: String = "10.0.0.1"
        fun getPortA(): Int = 8080
    }
}

class OuterB {
    object Config {
        val hostB: String = "192.168.1.1"
        fun getPortB(): Int = 9090
    }
}

fun test() {
    OuterA.Config.
}
"#;
    index.index_source(url.clone(), code);

    let pos_outer_a = Position::new(18, 18);
    let completions = find_completions(&index, &url, pos_outer_a, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"hostA".to_string()),
        "OuterA.Config should contain hostA"
    );
    assert!(
        labels.contains(&"getPortA".to_string()),
        "OuterA.Config should contain getPortA"
    );
    assert!(
        !labels.contains(&"hostB".to_string()),
        "OuterA.Config must NOT contain hostB from OuterB.Config"
    );
    assert!(
        !labels.contains(&"getPortB".to_string()),
        "OuterA.Config must NOT contain getPortB from OuterB.Config"
    );
}
