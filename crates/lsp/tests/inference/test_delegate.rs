use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_delegated_property_member_getvalue() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///DelegateTest.kt").unwrap();
    let code = r#"
package com.example

class StateDelegate<T>(val initial: T) {
    operator fun getValue(thisRef: Any?, property: Any?): T = initial
}

fun test() {
    val stateInt by StateDelegate(42)
    val stateStr by StateDelegate("hello")
    val res1 = stateInt
    val res2 = stateStr
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(8, 8);
    let inferred1 = infer_variable_type(&index, &url, "stateInt", pos1, code, None);
    assert_eq!(inferred1, KotlinType::parse("Int"));

    let pos2 = Position::new(9, 8);
    let inferred2 = infer_variable_type(&index, &url, "stateStr", pos2, code, None);
    assert_eq!(inferred2, KotlinType::parse("String"));
}

#[test]
fn test_infer_delegated_property_extension_getvalue_same_file() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ExtDelegate.kt").unwrap();
    let code = r#"
package com.example

class Box<T>(val item: T)

operator fun <T> Box<T>.getValue(thisRef: Any?, property: Any?): T = item

fun test() {
    val box = Box("kotlin")
    val content by box
    val c = content
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "content", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_infer_delegated_property_extension_cross_file_explicit_import() {
    let mut index = WorkspaceIndex::new();
    let ext_url = Url::parse("file:///Extensions.kt").unwrap();
    let ext_code = r#"
package com.example.ext

class CustomHolder<T>(val data: T)

operator fun <T> CustomHolder<T>.getValue(thisRef: Any?, property: Any?): T = data
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.app

import com.example.ext.CustomHolder
import com.example.ext.getValue

fun test() {
    val holder = CustomHolder(100)
    val num by holder
    val x = num
}
"#;
    index.index_source(ext_url, ext_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &main_url, "num", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_delegated_property_extension_cross_file_star_import() {
    let mut index = WorkspaceIndex::new();
    let ext_url = Url::parse("file:///Extensions.kt").unwrap();
    let ext_code = r#"
package com.example.ext

class CustomHolder<T>(val data: T)

operator fun <T> CustomHolder<T>.getValue(thisRef: Any?, property: Any?): T = data
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.app

import com.example.ext.*

fun test() {
    val holder = CustomHolder(true)
    val flag by holder
    val f = flag
}
"#;
    index.index_source(ext_url, ext_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &main_url, "flag", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("Boolean"));
}

#[test]
fn test_infer_delegated_property_unimported_extension_negative() {
    let mut index = WorkspaceIndex::new();
    let ext_url = Url::parse("file:///Extensions.kt").unwrap();
    let ext_code = r#"
package com.example.ext

class CustomHolder<T>(val data: T)

operator fun <T> CustomHolder<T>.getValue(thisRef: Any?, property: Any?): T = data
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.app

import com.example.ext.CustomHolder

fun test() {
    val holder = CustomHolder(100)
    val num by holder
    val x = num
}
"#;
    index.index_source(ext_url, ext_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &main_url, "num", pos, main_code, None);
    assert_eq!(
        inferred, None,
        "Without importing getValue, delegated property type should not resolve"
    );
}

#[test]
fn test_infer_delegated_property_no_getvalue_negative() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///NoDelegate.kt").unwrap();
    let code = r#"
package com.example

class NoDelegate(val num: Int)

fun test() {
    val d = NoDelegate(1)
    val bad by d
    val b = bad
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "bad", pos, code, None);
    assert_eq!(
        inferred, None,
        "Class without getValue should not resolve delegate type"
    );
}
