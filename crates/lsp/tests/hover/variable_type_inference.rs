use lsp::config::VariableHoverTypeInference;
use lsp::features::find_hover_with_config;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn extract_hover_text(hover: tower_lsp::lsp_types::Hover) -> String {
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text,
        _ => panic!("Expected scalar string hover contents"),
    }
}

fn create_test_index() -> (WorkspaceIndex, Url, &'static str) {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val name: String)
fun createUser(): User = User("Alice")
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.createUser
import com.example.model.User

fun test() {
    val user = createUser()
    val typedUser: User = createUser()
    val count = 42
    val unk = someUnresolved()
    val u1 = user
    val u2 = typedUser
    val c = count
    val k = unk
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);
    (index, main_url, main_code)
}

#[test]
fn test_hover_variable_type_inference_auto_default() {
    let (index, main_url, main_code) = create_test_index();

    let pos = Position::new(11, 13); // on `user` in `val u1 = user`
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "user",
        pos,
        main_code,
        None,
        VariableHoverTypeInference::Auto,
    )
    .expect("hover should resolve");

    let text = extract_hover_text(hover);
    assert!(
        text.contains("val user = createUser()"),
        "Auto mode should not append type if not present in signature, got:\n{}",
        text
    );
}

#[test]
fn test_hover_variable_type_inference_always_inferred_class() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user = Position::new(11, 13);
    let hover_user = find_hover_with_config(
        &index,
        &main_url,
        "user",
        pos_user,
        main_code,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover on user should resolve");
    let text_user = extract_hover_text(hover_user);
    assert!(
        text_user.contains("val user: User = createUser()"),
        "Always mode should append short type User, got:\n{}",
        text_user
    );
}

#[test]
fn test_hover_variable_type_inference_always_primitive_and_unresolved() {
    let (index, main_url, main_code) = create_test_index();

    let pos_count = Position::new(13, 13);
    let hover_count = find_hover_with_config(
        &index,
        &main_url,
        "count",
        pos_count,
        main_code,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover on count should resolve");
    let text_count = extract_hover_text(hover_count);
    assert!(
        text_count.contains("val count: Int = 42"),
        "Always mode should append short type Int, got:\n{}",
        text_count
    );

    let pos_unk = Position::new(14, 13);
    let hover_unk = find_hover_with_config(
        &index,
        &main_url,
        "unk",
        pos_unk,
        main_code,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover on unk should resolve");
    let text_unk = extract_hover_text(hover_unk);
    assert!(
        text_unk.contains("val unk: ? = someUnresolved()"),
        "Always mode should append ': ?' when type is not inferred, got:\n{}",
        text_unk
    );
}

#[test]
fn test_hover_variable_type_inference_always_full_name_inferred_class() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user = Position::new(11, 13);
    let hover_user = find_hover_with_config(
        &index,
        &main_url,
        "user",
        pos_user,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysFullName,
    )
    .expect("hover on user should resolve");
    let text_user = extract_hover_text(hover_user);
    assert!(
        text_user.contains("val user: com.example.model.User = createUser()"),
        "AlwaysFullName mode should display full qualified type, got:\n{}",
        text_user
    );
}

#[test]
fn test_hover_variable_type_inference_always_full_name_explicit_and_primitive() {
    let (index, main_url, main_code) = create_test_index();

    let pos_typed = Position::new(12, 13);
    let hover_typed = find_hover_with_config(
        &index,
        &main_url,
        "typedUser",
        pos_typed,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysFullName,
    )
    .expect("hover on typedUser should resolve");
    let text_typed = extract_hover_text(hover_typed);
    assert!(
        text_typed.contains("val typedUser: com.example.model.User = createUser()"),
        "AlwaysFullName mode should expand explicit short type to full qualified type, got:\n{}",
        text_typed
    );

    let pos_count = Position::new(13, 13);
    let hover_count = find_hover_with_config(
        &index,
        &main_url,
        "count",
        pos_count,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysFullName,
    )
    .expect("hover on count should resolve");
    let text_count = extract_hover_text(hover_count);
    assert!(
        text_count.contains("val count: Int = 42"),
        "AlwaysFullName mode for primitive should be Int, got:\n{}",
        text_count
    );
}

#[test]
fn test_hover_variable_type_inference_always_full_name_unresolved() {
    let (index, main_url, main_code) = create_test_index();

    let pos_unk = Position::new(14, 13);
    let hover_unk = find_hover_with_config(
        &index,
        &main_url,
        "unk",
        pos_unk,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysFullName,
    )
    .expect("hover on unk should resolve");
    let text_unk = extract_hover_text(hover_unk);
    assert!(
        text_unk.contains("val unk: ? = someUnresolved()"),
        "AlwaysFullName mode should display ': ?' when not inferred, got:\n{}",
        text_unk
    );
}

#[test]
fn test_hover_delegated_var_with_always_inference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///StateHolder.kt").unwrap();
    let source = r#"package com.example

fun render() {
    var state by remember { mutableStateOf(0) }
    val current = state
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(4, 18);
    let hover = find_hover_with_config(
        &index,
        &url,
        "state",
        pos,
        source,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover on delegated var should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("var state: ? by remember { mutableStateOf(0) }"),
        "Delegated var without inferred type should have ': ?' before 'by', got:\n{}",
        text
    );
}

fn create_compose_test_index() -> (WorkspaceIndex, Url, &'static str) {
    let mut index = WorkspaceIndex::new();
    let state_url = Url::parse("file:///SnapshotState.kt").unwrap();
    let state_code = r#"
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
"#;
    let main_url = Url::parse("file:///Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import androidx.compose.runtime.remember
import androidx.compose.runtime.mutableStateOf

fun test() {
    val state = remember { mutableStateOf(1) }
    val s = state
}
"#;
    index.index_source(state_url, state_code);
    index.index_source(main_url.clone(), main_code);
    (index, main_url, main_code)
}

#[test]
fn test_hover_variable_type_inference_always_inferred_generic_type_arguments() {
    let (index, main_url, main_code) = create_compose_test_index();
    let pos = Position::new(7, 8); // on `state`
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "state",
        pos,
        main_code,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover on state should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val state: MutableState<Int> = remember { mutableStateOf(1) }"),
        "Always mode should include resolved generic type arguments, got:\n{}",
        text
    );
}

#[test]
fn test_hover_variable_type_inference_always_with_trace_inferred_generic_type_arguments() {
    let (index, main_url, main_code) = create_compose_test_index();
    let pos = Position::new(7, 8); // on `state`
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "state",
        pos,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on state should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val state: MutableState<Int> = remember { mutableStateOf(1) }"),
        "AlwaysWithTrace mode should include resolved generic type arguments, got:\n{}",
        text
    );
    assert!(
        text.contains("Variable: `state` -> `androidx.compose.runtime.MutableState<Int>`")
            || text.contains("Variable: `state` -> `MutableState<Int>`"),
        "Trace should record inferred generic type, got:\n{}",
        text
    );
}
