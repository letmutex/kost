use lsp::indexer::WorkspaceIndex;
use lsp::inference::qualification::is_declaration_visible;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, SymbolKind, Url};

#[test]
fn infer_generic_class_constructor_ignores_unimported_same_name_class() {
    let mut index = WorkspaceIndex::new();
    let other_url = Url::parse("file:///project/other/Box.kt").unwrap();
    let other_code = r#"
package com.other

class Box<T>(val item: T)
"#;
    index.index_source(other_url, other_code);

    let app_box_url = Url::parse("file:///project/app/Box.kt").unwrap();
    let app_box_code = r#"
package com.app

class Box<K, V>(val key: K, val value: V)
"#;
    index.index_source(app_box_url, app_box_code);

    let main_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let main_code = r#"
package com.app

val b = Box(1, "hello")
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(3, 4);
    let inferred = infer_variable_type(&index, &main_url, "b", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("com.app.Box<Int, String>"));
}

#[test]
fn infer_nested_class_constructor_with_container_name() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Outer {
    class Inner<T>(val item: T)
}
"#;
    index.index_source(model_url, model_code);

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.model.Outer.Inner

val x = Inner(42)
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &app_url, "x", pos, app_code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.model.Outer.Inner<Int>")
    );
}

#[test]
fn infer_qualified_nested_class_constructor_with_generics() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedGeneric.kt").unwrap();
    let code = r#"
package com.example.nested

class Outer {
    class Inner<T>(val item: T)
}

val x = Outer.Inner("hello")
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "x", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.nested.Outer.Inner<String>")
    );
}

#[test]
fn infer_generic_constructor_resolves_argument_type_from_call_site_import() {
    let mut index = WorkspaceIndex::new();
    let data_url = Url::parse("file:///project/model/Data.kt").unwrap();
    let data_code = r#"
package com.model

class Data
"#;
    index.index_source(data_url, data_code);

    let holder_url = Url::parse("file:///project/util/Holder.kt").unwrap();
    let holder_code = r#"
package com.util

class Holder<T>(val value: T)
"#;
    index.index_source(holder_url, holder_code);

    let app_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let app_code = r#"
package com.app

import com.model.Data
import com.util.Holder

val h = Holder(Data())
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(6, 4);
    let inferred = infer_variable_type(&index, &app_url, "h", pos, app_code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.util.Holder<com.model.Data>")
    );
}

#[test]
fn infer_nested_class_constructor_rejects_private_nested_class() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Outer {
    private class Secret(val id: Int)
}
"#;
    index.index_source(model_url, model_code);

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.model.Outer

val s = Outer.Secret(1)
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &app_url, "s", pos, app_code, None);
    assert_eq!(inferred, None);
}

#[test]
fn infer_constructor_return_type_for_aliased_import() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Account.kt").unwrap();
    let model_code = r#"
package com.example.model

class Account<T>(val id: T)
"#;
    index.index_source(model_url, model_code);

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.model.Account as User

val u = User("123")
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &app_url, "u", pos, app_code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.model.Account<String>")
    );
}

#[test]
fn import_alias_does_not_leak_unrelated_same_name_class() {
    let mut index = WorkspaceIndex::new();
    let other_url = Url::parse("file:///project/other/Other.kt").unwrap();
    let other_code = r#"
package com.other

class CustomWidget
"#;
    index.index_source(other_url.clone(), other_code);

    let util_url = Url::parse("file:///project/util/Util.kt").unwrap();
    let util_code = r#"
package com.util

class InternalTool
"#;
    index.index_source(util_url, util_code);

    let app_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let app_code = r#"
package com.app

import com.util.InternalTool as CustomWidget

val w = CustomWidget()
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &app_url, "w", pos, app_code, None);
    assert_eq!(inferred, KotlinType::parse("com.util.InternalTool"));

    // Also assert that com.other.CustomWidget is NOT visible in app_url
    let other_decl = index
        .files
        .get(&other_url)
        .unwrap()
        .declarations
        .iter()
        .find(|d| d.name == "CustomWidget" && d.kind == SymbolKind::CLASS)
        .unwrap();
    assert!(
        !is_declaration_visible(&index, &app_url, &other_url, other_decl),
        "unrelated class with name matching alias must not be visible"
    );
}

#[test]
fn infer_imported_deeply_nested_class_constructor() {
    let mut index = WorkspaceIndex::new();
    let a_url = Url::parse("file:///project/a/Outer.kt").unwrap();
    let a_code = r#"
package a

sealed class Outer {
    sealed class Inner {
        class Text : Inner()
        class Image : Inner()
    }
}
"#;
    index.index_source(a_url, a_code);

    let b_url = Url::parse("file:///project/b/Main.kt").unwrap();
    let b_code = r#"
package b

import a.Outer

fun main() {
    val part = Outer.Inner.Text()
}
"#;
    index.index_source(b_url.clone(), b_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &b_url, "part", pos, b_code, None);
    assert_eq!(inferred, KotlinType::parse("a.Outer.Inner.Text"));
}
