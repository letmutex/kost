use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_user_reported_unresolved_receiver_leakage() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExactLeakTest1.kt").unwrap();
    let code = r#"
class TestClass(
    val text: String = "a text",
)

fun main() {
    val t0 = text
    val text = randomVar.text
    val another = "a".text
}
"#;
    index.index_source(url.clone(), code);

    // 1. `val t0 = text` at line 6, col 13 (before `text` is declared locally):
    let pos_t0 = Position::new(6, 13);
    assert!(find_definition(&index, &url, "text", pos_t0, code).is_none());
    assert!(find_hover(&index, &url, "text", pos_t0, code).is_none());

    // 2. `val text = randomVar.text` at line 7, col 25 (member on unresolved receiver):
    let pos_rand = Position::new(7, 25);
    assert!(find_definition(&index, &url, "text", pos_rand, code).is_none());
    assert!(find_hover(&index, &url, "text", pos_rand, code).is_none());

    // 3. `val another = "a".text` at line 8, col 23 (member on String receiver which has no `text`):
    let pos_str = Position::new(8, 23);
    assert!(find_definition(&index, &url, "text", pos_str, code).is_none());
    assert!(find_hover(&index, &url, "text", pos_str, code).is_none());
}

#[test]
fn test_user_reported_local_variable_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExactLeakTest2.kt").unwrap();
    let code = r#"
class TestClass(
    val text: String = "a text",
)

fun main() {
    val text = randomVar.text
    text
}
"#;
    index.index_source(url.clone(), code);

    let pos_local = Position::new(7, 4);
    let def_local = find_definition(&index, &url, "text", pos_local, code);
    assert!(
        def_local.is_some(),
        "`text` should resolve to local `val text`"
    );
    assert_eq!(def_local.unwrap().range.start.line, 6);

    let hover_local = find_hover(&index, &url, "text", pos_local, code);
    assert!(
        hover_local.is_some(),
        "`text` should show hover for local variable"
    );
    if let HoverContents::Scalar(MarkedString::String(text_doc)) = hover_local.unwrap().contents {
        assert!(!text_doc.contains("a text"));
        assert!(text_doc.contains("val text"));
    } else {
        panic!("unexpected hover contents");
    }
}

#[test]
fn test_annotated_interface_members_do_not_leak_to_global_scope() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AnnotatedInterfaceTest.kt").unwrap();
    let code = r#"
@Random
@Annotation
interface TestInterface {
    val shouldNotLeak: String
}

fun main() {
    
}
"#;
    index.index_source(url.clone(), code);

    let pos_main = Position::new(8, 4);
    let completions = find_completions(&index, &url, pos_main, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(!labels.contains(&"shouldNotLeak".to_string()));
    assert!(labels.contains(&"TestInterface".to_string()));

    let def = find_definition(&index, &url, "shouldNotLeak", pos_main, code);
    assert!(def.is_none());

    let hover = find_hover(&index, &url, "shouldNotLeak", pos_main, code);
    assert!(hover.is_none());

    let pos_inside = Position::new(4, 8);
    let def_inside = find_definition(&index, &url, "shouldNotLeak", pos_inside, code);
    assert!(def_inside.is_some());
}

#[test]
fn test_annotated_sealed_interface_members_do_not_leak() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SealedInterfaceTest.kt").unwrap();
    let code = r#"
@Random
@Annotation
sealed interface TestInterface {
    val shouldNotLeak: String
}

fun main() {
    
}
"#;
    index.index_source(url.clone(), code);

    let pos_main = Position::new(8, 4);
    let completions = find_completions(&index, &url, pos_main, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(!labels.contains(&"shouldNotLeak".to_string()));
    assert!(labels.contains(&"TestInterface".to_string()));

    let def = find_definition(&index, &url, "shouldNotLeak", pos_main, code);
    assert!(def.is_none());

    let hover = find_hover(&index, &url, "shouldNotLeak", pos_main, code);
    assert!(hover.is_none());

    let pos_inside = Position::new(4, 8);
    let def_inside = find_definition(&index, &url, "shouldNotLeak", pos_inside, code);
    assert!(def_inside.is_some());
}

#[test]
fn test_annotated_modified_classes_do_not_leak() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ModifiedClassesTest.kt").unwrap();
    let code = r#"
@Random
@Annotation
data class DataClass {
    val dataProp: String = ""
}

@Random
@Annotation
abstract class AbstractClass {
    val abstractProp: String = ""
}

@Random
@Annotation
sealed class SealedClass {
    val sealedProp: String = ""
}

fun main() {
    
}
"#;
    index.index_source(url.clone(), code);

    let pos_main = Position::new(18, 4);
    let completions = find_completions(&index, &url, pos_main, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(!labels.contains(&"dataProp".to_string()));
    assert!(!labels.contains(&"abstractProp".to_string()));
    assert!(!labels.contains(&"sealedProp".to_string()));

    assert!(labels.contains(&"DataClass".to_string()));
    assert!(labels.contains(&"AbstractClass".to_string()));
    assert!(labels.contains(&"SealedClass".to_string()));
}

#[test]
fn test_extension_on_nullable_type_scoping_and_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NullableExt.kt").unwrap();
    let code = r#"
package com.example.extensions

fun String?.isNullOrEmptyCustom(): Boolean = this == null || this.isEmpty()
val String?.isNotNullOrBlankCustom: Boolean get() = this != null && this.isNotBlank()

fun main() {
    isNullOrEmptyCustom
    isNotNullOrBlankCustom
}

fun test(s: String?) {
    s.
}
"#;
    index.index_source(url.clone(), code);

    let pos_main = Position::new(7, 4);
    let main_completions = find_completions(&index, &url, pos_main, code);
    let main_labels: Vec<String> = main_completions.into_iter().map(|c| c.label).collect();
    assert!(
        !main_labels.contains(&"isNullOrEmptyCustom".to_string()),
        "isNullOrEmptyCustom must not be offered in bare scope completion without receiver"
    );
    assert!(
        !main_labels.contains(&"isNotNullOrBlankCustom".to_string()),
        "isNotNullOrBlankCustom must not be offered in bare scope completion without receiver"
    );

    let pos_member = Position::new(12, 6);
    let member_completions = find_completions(&index, &url, pos_member, code);
    let member_labels: Vec<String> = member_completions.into_iter().map(|c| c.label).collect();
    assert!(
        member_labels.contains(&"isNullOrEmptyCustom".to_string()),
        "s. on String? should offer isNullOrEmptyCustom"
    );
    assert!(
        member_labels.contains(&"isNotNullOrBlankCustom".to_string()),
        "s. on String? should offer isNotNullOrBlankCustom"
    );
}

#[test]
fn test_annotated_fun_interface_members_do_not_leak() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FunInterfaceLeakTest.kt").unwrap();
    let code = r#"
package com.example.sam

@Random
@Annotation
fun interface ActionHandler {
    fun handleAction(action: String)
}

fun main() {
    
}
"#;
    index.index_source(url.clone(), code);

    let pos_main = Position::new(10, 4);
    let completions = find_completions(&index, &url, pos_main, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        !labels.contains(&"handleAction".to_string()),
        "handleAction must NOT leak to bare scope completions in main()"
    );
    assert!(
        find_definition(&index, &url, "handleAction", pos_main, code).is_none(),
        "handleAction definition must NOT resolve in global scope without receiver"
    );
    assert!(
        find_hover(&index, &url, "handleAction", pos_main, code).is_none(),
        "handleAction hover must NOT resolve in global scope without receiver"
    );
}

#[test]
fn test_unresolved_receiver_cat_run_does_not_leak() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CatLeakTest.kt").unwrap();
    let code = r#"
class Cat {
    fun run()
}

class House {
    protected lateinit var cat: Cat
}

fun test() {
    cat.run()
}
"#;
    index.index_source(url.clone(), code);

    // In `fun test()`, `cat` is not in scope (it is a protected member of House).
    // Verify `cat.run`: neither `cat` nor `run` should leak/resolve.
    let pos_cat = Position::new(10, 4);
    assert!(
        find_definition(&index, &url, "cat", pos_cat, code).is_none(),
        "`cat` must not resolve outside House"
    );
    assert!(
        find_hover(&index, &url, "cat", pos_cat, code).is_none(),
        "`cat` hover must not resolve outside House"
    );

    let pos_run = Position::new(10, 8);
    assert!(
        find_definition(&index, &url, "run", pos_run, code).is_none(),
        "`cat.run` definition must not resolve when receiver `cat` is unresolvable"
    );
    assert!(
        find_hover(&index, &url, "run", pos_run, code).is_none(),
        "`cat.run` hover must not resolve when receiver `cat` is unresolvable"
    );

    let member_completions = find_completions(&index, &url, Position::new(10, 8), code);
    let member_labels: Vec<String> = member_completions.into_iter().map(|c| c.label).collect();
    assert!(
        !member_labels.contains(&"run".to_string()),
        "`cat.` completion must not offer `run` when receiver is unresolvable"
    );

    let scope_completions = find_completions(&index, &url, Position::new(10, 4), code);
    let scope_labels: Vec<String> = scope_completions.into_iter().map(|c| c.label).collect();
    assert!(
        !scope_labels.contains(&"cat".to_string()),
        "Scope completion inside test() must not leak `cat` from House"
    );
    assert!(
        !scope_labels.contains(&"run".to_string()),
        "Scope completion inside test() must not leak `run` from Cat"
    );
}
