use lsp::features::completion_member::find_member_completions;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

fn setup_multi_package_workspace() -> WorkspaceIndex {
    let mut index = WorkspaceIndex::new();

    let pkg_a_url = Url::parse("file:///library/pkg_a/Component.kt").unwrap();
    let pkg_a_code = r#"
package com.example.ui

interface Component {
    fun render(): Component
    companion object : Component
}

fun Component.setHeight(height: Int): Component = this
fun Component.setWidth(width: Int): Component = this
"#;

    let pkg_b_url = Url::parse("file:///library/pkg_b/Component.kt").unwrap();
    let pkg_b_code = r#"
package com.example.reflect

class Component {
    companion object {
        const val PUBLIC_FLAG: Int = 1
        const val PRIVATE_FLAG: Int = 2
        const val ABSTRACT_FLAG: Int = 1024
        fun isPublic(flag: Int): Boolean = (flag and PUBLIC_FLAG) != 0
    }
}
"#;

    index.index_source(pkg_a_url, pkg_a_code);
    index.index_source(pkg_b_url, pkg_b_code);
    index
}

#[test]
fn test_inferred_variable_completion_uses_imported_package_members() {
    let mut index = setup_multi_package_workspace();
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.ui.Component
import com.example.ui.setHeight
import com.example.ui.setWidth

fun test() {
    val comp = Component.setHeight(100)
    comp.
}
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(9, 9); // on `comp.`
    let completions = find_member_completions(&index, &app_url, pos, app_code, None, "comp", "");

    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"setHeight".to_string()),
        "Should suggest ui.Component.setHeight"
    );
    assert!(
        labels.contains(&"setWidth".to_string()),
        "Should suggest ui.Component.setWidth"
    );
    assert!(
        labels.contains(&"render".to_string()),
        "Should suggest ui.Component.render"
    );

    assert!(
        !labels.contains(&"ABSTRACT_FLAG".to_string()),
        "Should NOT suggest reflect.Component.ABSTRACT_FLAG on ui.Component instance"
    );
    assert!(
        !labels.contains(&"PUBLIC_FLAG".to_string()),
        "Should NOT suggest reflect.Component.PUBLIC_FLAG on ui.Component instance"
    );
}

#[test]
fn test_companion_object_completion_uses_imported_package_members() {
    let mut index = setup_multi_package_workspace();
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.ui.Component
import com.example.ui.setHeight

fun test() {
    Component.
}
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(7, 14); // on `Component.`
    let completions =
        find_member_completions(&index, &app_url, pos, app_code, None, "Component", "");

    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"setHeight".to_string()),
        "Should suggest ui.Component extension"
    );
    assert!(
        !labels.contains(&"ABSTRACT_FLAG".to_string()),
        "Should NOT suggest reflect.Component constants on ui.Component companion"
    );
    assert!(
        !labels.contains(&"PUBLIC_FLAG".to_string()),
        "Should NOT suggest reflect.Component constants on ui.Component companion"
    );
}

#[test]
fn test_alternate_package_import_completion_suggests_respective_members() {
    let mut index = setup_multi_package_workspace();
    let app_url = Url::parse("file:///project/ReflectApp.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.reflect.Component

fun test() {
    Component.
}
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(6, 14); // on `Component.`
    let completions =
        find_member_completions(&index, &app_url, pos, app_code, None, "Component", "");

    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"ABSTRACT_FLAG".to_string()),
        "Should suggest reflect.Component.ABSTRACT_FLAG when imported"
    );
    assert!(
        labels.contains(&"PUBLIC_FLAG".to_string()),
        "Should suggest reflect.Component.PUBLIC_FLAG when imported"
    );
    assert!(
        !labels.contains(&"setHeight".to_string()),
        "Should NOT suggest ui.Component.setHeight when reflect.Component is imported"
    );
}

#[test]
fn test_single_expression_function_without_explicit_return_type_infers_correctly() {
    let mut index = setup_multi_package_workspace();
    let custom_url = Url::parse("file:///project/CustomComponent.kt").unwrap();
    let custom_code = r#"
package com.example.app

import com.example.ui.Component

fun Component.customOperation() = this.render()
fun Component.typedOperation(): Component = this.render()

fun test() {
    val untyped = Component.customOperation()
    val typed = Component.typedOperation()
}
"#;
    index.index_source(custom_url.clone(), custom_code);

    let pos_untyped = Position::new(9, 8);
    let inferred_untyped = infer_variable_type(
        &index,
        &custom_url,
        "untyped",
        pos_untyped,
        custom_code,
        None,
    );
    assert_eq!(
        inferred_untyped,
        KotlinType::parse("com.example.ui.Component"),
        "Custom single-expression function without explicit return type should now infer correctly"
    );

    let pos_typed = Position::new(10, 8);
    let inferred_typed =
        infer_variable_type(&index, &custom_url, "typed", pos_typed, custom_code, None);
    assert_eq!(
        inferred_typed,
        KotlinType::parse("com.example.ui.Component"),
        "Typed function call should infer full qualified type"
    );
}
