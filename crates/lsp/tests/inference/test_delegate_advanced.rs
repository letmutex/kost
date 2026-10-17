use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_class_member_delegated_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ClassDelegate.kt").unwrap();
    let code = r#"
package com.example

class LazyHolder<T>(val item: T) {
    operator fun getValue(thisRef: Any?, property: Any?): T = item
}

class Container {
    val message by LazyHolder("delegated member")
}

fun test() {
    val c = Container()
    val msg = c.message
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(13, 8);
    let inferred = infer_variable_type(&index, &url, "msg", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_infer_delegated_property_supertype_getvalue() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///SuperDelegate.kt").unwrap();
    let code = r#"
package com.example

interface ReadOnlyProperty<in T, out V> {
    operator fun getValue(thisRef: T, property: Any?): V
}

class CustomProp<T>(val v: T) : ReadOnlyProperty<Any?, T> {
    override fun getValue(thisRef: Any?, property: Any?): T = v
}

fun test() {
    val prop by CustomProp(999)
    val p = prop
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(13, 8);
    let inferred = infer_variable_type(&index, &url, "prop", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_delegated_property_generic_class_without_getvalue_negative() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///GenericNoDelegate.kt").unwrap();
    let code = r#"
class StateDelegate<T>(val initial: T)

fun test() {
    val stateInt by StateDelegate(42)
    val stateStr by StateDelegate("hello")
    val res1 = stateInt
    val res2 = stateStr
}
"#;
    let stdlib_url = Url::parse("file:///kotlin/properties/ReadOnlyProperty.class").unwrap();
    let stdlib_code = r#"
package kotlin.properties

interface ReadOnlyProperty<in T, out V> {
    fun getValue(thisRef: Object, property: Object): Object
}
"#;
    index.index_source(stdlib_url, stdlib_code);
    index.index_source(url.clone(), code);

    let pos1 = Position::new(4, 8);
    let inferred_state_int = infer_variable_type(&index, &url, "stateInt", pos1, code, None);
    assert_eq!(
        inferred_state_int, None,
        "stateInt should not be inferred when StateDelegate lacks getValue"
    );

    let pos2 = Position::new(5, 8);
    let inferred_state_str = infer_variable_type(&index, &url, "stateStr", pos2, code, None);
    assert_eq!(
        inferred_state_str, None,
        "stateStr should not be inferred when StateDelegate lacks getValue"
    );

    let pos3 = Position::new(6, 8);
    let inferred_res1 = infer_variable_type(&index, &url, "res1", pos3, code, None);
    assert_eq!(
        inferred_res1, None,
        "res1 should not be inferred when stateInt is unresolved"
    );

    let pos4 = Position::new(7, 8);
    let inferred_res2 = infer_variable_type(&index, &url, "res2", pos4, code, None);
    assert_eq!(
        inferred_res2, None,
        "res2 should not be inferred when stateStr is unresolved"
    );
}

#[test]
fn test_infer_delegated_property_bytecode_erased_recovery() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("file:///SnapshotStateKt.class").unwrap();
    let class_code = r#"
package androidx.compose.runtime

interface State<out T>
interface MutableState<T> : State<T>

fun <T> mutableStateOf(value: T): MutableState<T> = object : MutableState<T> {}
@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun State.getValue(arg1: Object, arg2: Object): Object
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.app

import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue

fun test() {
    val direct = remember { mutableStateOf(10) }
    val count by remember { mutableStateOf(10) }
    val c = count
}
"#;
    index.index_source(class_url, class_code);
    index.index_source(main_url.clone(), main_code);

    let pos_direct = Position::new(8, 8);
    let inferred_direct =
        infer_variable_type(&index, &main_url, "direct", pos_direct, main_code, None);
    assert_eq!(
        inferred_direct,
        KotlinType::parse("androidx.compose.runtime.MutableState<Int>")
    );

    let pos_count = Position::new(9, 8);
    let inferred_count =
        infer_variable_type(&index, &main_url, "count", pos_count, main_code, None);
    assert_eq!(
        inferred_count,
        KotlinType::parse("Int"),
        "count should infer Int via container type parameter propagation on erased bytecode"
    );

    let pos_c = Position::new(10, 8);
    let inferred_c = infer_variable_type(&index, &main_url, "c", pos_c, main_code, None);
    assert_eq!(inferred_c, KotlinType::parse("Int"));

    let unimp_code = r#"
package com.example.app

import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf

fun test() {
    val count by remember { mutableStateOf(10) }
}
"#;
    let unimp_url = Url::parse("file:///Unimported.kt").unwrap();
    index.index_source(unimp_url.clone(), unimp_code);
    let pos_unimp = Position::new(7, 8);
    let inferred_unimp =
        infer_variable_type(&index, &unimp_url, "count", pos_unimp, unimp_code, None);
    assert_eq!(
        inferred_unimp, None,
        "count should not be inferred when getValue is unimported"
    );
}
