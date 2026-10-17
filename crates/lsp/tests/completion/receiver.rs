use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_chained_safe_call_member_completion_and_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SafeCallTest.kt").unwrap();
    let code = r#"
package com.example.safecall

class Address(val street: String, val city: String)
class User {
    fun getAddress(): Address? = Address("Main St", "Springfield")
}

fun test(user: User?) {
    user?.getAddress()?.
    val c = user?.getAddress()?.city
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion after chained safe call `user?.getAddress()?.` at line 9, col 24:
    let pos_dot = Position::new(9, 24);
    let completions = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"street".to_string()),
        "user?.getAddress()?. should offer 'street'"
    );
    assert!(
        labels.contains(&"city".to_string()),
        "user?.getAddress()?. should offer 'city'"
    );

    // 2. Definition of `city` in chained safe call `user?.getAddress()?.city` at line 10, col 32:
    let pos_city = Position::new(10, 32);
    let def = find_definition(&index, &url, "city", pos_city, code);
    assert!(
        def.is_some(),
        "user?.getAddress()?.city should resolve definition of 'city'"
    );

    // 3. Hover on `city` in chained safe call:
    let hover = find_hover(&index, &url, "city", pos_city, code);
    assert!(
        hover.is_some(),
        "user?.getAddress()?.city should resolve hover on 'city'"
    );
}

#[test]
fn test_enum_entry_member_completion_and_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/EnumMemberTest.kt").unwrap();
    let code = r#"
package com.example.planets

enum class Planet(val mass: Double) {
    EARTH(5.97e24);

    fun surfaceGravity(): Double = mass * 9.8
}

fun test() {
    Planet.EARTH.
    val m = Planet.EARTH.mass
    val g = Planet.EARTH.surfaceGravity()
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion on enum entry `Planet.EARTH.` at line 10, col 17:
    let pos_dot = Position::new(10, 17);
    let completions = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"mass".to_string()),
        "Planet.EARTH. should offer property 'mass'"
    );
    assert!(
        labels.contains(&"surfaceGravity".to_string()),
        "Planet.EARTH. should offer method 'surfaceGravity'"
    );

    // 2. Definition on `mass` in `Planet.EARTH.mass` at line 11, col 26:
    let pos_mass = Position::new(11, 26);
    let def_mass = find_definition(&index, &url, "mass", pos_mass, code);
    assert!(
        def_mass.is_some(),
        "Planet.EARTH.mass should resolve definition of 'mass'"
    );

    // 3. Hover on `surfaceGravity` in `Planet.EARTH.surfaceGravity()` at line 12, col 26:
    let pos_gravity = Position::new(12, 26);
    let hover_gravity = find_hover(&index, &url, "surfaceGravity", pos_gravity, code);
    assert!(
        hover_gravity.is_some(),
        "Planet.EARTH.surfaceGravity() should resolve hover on 'surfaceGravity'"
    );
}

#[test]
fn test_labeled_this_completion_inside_inner_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LabeledThisTest.kt").unwrap();
    let code = r#"
package com.example.labeled

class Window {
    val windowTitle: String = "Main"

    inner class Panel {
        val panelName: String = "Sidebar"

        fun render() {
            this@Window.
            this@Panel.
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion after `this@Window.` at line 10, col 24:
    let pos_window = Position::new(10, 24);
    let comps_window = find_completions(&index, &url, pos_window, code);
    let labels_window: Vec<String> = comps_window.into_iter().map(|c| c.label).collect();

    assert!(
        labels_window.contains(&"windowTitle".to_string()),
        "this@Window. should offer 'windowTitle' from enclosing Window"
    );

    // 2. Completion after `this@Panel.` at line 11, col 23:
    let pos_panel = Position::new(11, 23);
    let comps_panel = find_completions(&index, &url, pos_panel, code);
    let labels_panel: Vec<String> = comps_panel.into_iter().map(|c| c.label).collect();

    assert!(
        labels_panel.contains(&"panelName".to_string()),
        "this@Panel. should offer 'panelName' from inner Panel"
    );
}

#[test]
fn test_multiline_chained_call_member_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultilineChainTest.kt").unwrap();
    let code = r#"
package com.example.chain

class QueryBuilder {
    fun select(fields: String): QueryBuilder = this
    fun whereClause(cond: String): QueryBuilder = this
    fun execute(): List<String> = emptyList()
}

fun test(builder: QueryBuilder) {
    builder
        .select("id")
        .
}
"#;
    index.index_source(url.clone(), code);

    // Line 11 is `        .` (cursor at col 9 after dot)
    let pos_dot = Position::new(11, 9);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"whereClause".to_string()),
        "builder.select(). on newline should offer 'whereClause'"
    );
    assert!(
        labels.contains(&"execute".to_string()),
        "builder.select(). on newline should offer 'execute'"
    );
}

#[test]
fn test_extension_function_this_receiver_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionThisTest.kt").unwrap();
    let code = r#"
package com.example.extthis

class Session(val sessionId: String, val userId: String) {
    fun terminate(): Boolean = true
}

fun Session.validate() {
    this.
}
"#;
    index.index_source(url.clone(), code);

    // Line 8: `    this.` (cursor at col 9 after dot)
    let pos_dot = Position::new(8, 9);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"sessionId".to_string()),
        "this. inside Session.validate() extension should offer 'sessionId'"
    );
    assert!(
        labels.contains(&"userId".to_string()),
        "this. inside Session.validate() extension should offer 'userId'"
    );
    assert!(
        labels.contains(&"terminate".to_string()),
        "this. inside Session.validate() extension should offer 'terminate'"
    );
}

#[test]
fn test_subclass_super_receiver_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SuperReceiverTest.kt").unwrap();
    let code = r#"
package com.example.superrec

open class BaseWorker {
    val workerId: String = "base"
    fun performTask(): String = "done"
}

class SpecializedWorker : BaseWorker() {
    fun execute() {
        super.
    }
}
"#;
    index.index_source(url.clone(), code);

    // Line 10: `        super.` (cursor at col 14 after dot)
    let pos_dot = Position::new(10, 14);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"workerId".to_string()),
        "super. inside SpecializedWorker should offer 'workerId' from BaseWorker"
    );
    assert!(
        labels.contains(&"performTask".to_string()),
        "super. inside SpecializedWorker should offer 'performTask' from BaseWorker"
    );
}
