use lsp::config::VariableHoverTypeInference;
use lsp::features::find_hover_with_config;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::TraceCollector;
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
    val nullableUser: User? = null
    val nu = nullableUser
    val userName = user.name
    val userDesc = user.describe("info")
}

fun User.describe(tag: String): String = tag + ": " + name
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);
    (index, main_url, main_code)
}

#[test]
fn test_hover_variable_type_inference_always_with_trace_renders_markdown_trace() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user = Position::new(11, 13);
    let hover_user = find_hover_with_config(
        &index,
        &main_url,
        "user",
        pos_user,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on user with trace should resolve");
    let text_user = extract_hover_text(hover_user);

    assert!(
        text_user.contains("val user: User = createUser()"),
        "AlwaysWithTrace should augment signature with short type, got:\n{}",
        text_user
    );
    assert!(
        text_user.contains("### Type Inference Trace ("),
        "AlwaysWithTrace should render raw markdown heading, got:\n{}",
        text_user
    );
    assert!(
        !text_user.contains("<details") && !text_user.contains("<summary"),
        "AlwaysWithTrace must not contain HTML details/summary tags, got:\n{}",
        text_user
    );
    assert!(
        text_user.contains("Variable: `user` -> `com.example.model.User`"),
        "AlwaysWithTrace should include variable scope, got:\n{}",
        text_user
    );
    assert!(
        text_user.contains("Evaluating initializer `createUser()`"),
        "AlwaysWithTrace should trace initializer step, got:\n{}",
        text_user
    );
}

#[test]
fn test_hover_variable_type_inference_always_with_trace_on_unresolved_variable() {
    let (index, main_url, main_code) = create_test_index();

    let pos_unk = Position::new(14, 13);
    let hover_unk = find_hover_with_config(
        &index,
        &main_url,
        "unk",
        pos_unk,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on unk with trace should resolve");
    let text_unk = extract_hover_text(hover_unk);

    assert!(
        text_unk.contains("val unk: ? = someUnresolved()"),
        "AlwaysWithTrace should retain ': ?' for unresolved var, got:\n{}",
        text_unk
    );
    assert!(
        text_unk.contains("### Type Inference Trace ("),
        "AlwaysWithTrace should render raw markdown heading for unresolved var, got:\n{}",
        text_unk
    );
    assert!(
        !text_unk.contains("<details"),
        "AlwaysWithTrace must not contain HTML details, got:\n{}",
        text_unk
    );
    assert!(
        text_unk.contains("Variable: `unk` -> `?`"),
        "Trace should record unresolved result '?', got:\n{}",
        text_unk
    );
}

#[test]
fn test_hover_variable_type_inference_always_with_trace_contains_no_emojis() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user = Position::new(11, 13);
    let hover_user = find_hover_with_config(
        &index,
        &main_url,
        "user",
        pos_user,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on user should resolve");
    let text_user = extract_hover_text(hover_user);

    // Ensure no emojis or decorative unicode symbols exist in the hover popup
    let has_emoji = text_user.chars().any(|c| {
        ('\u{1F300}'..='\u{1F9FF}').contains(&c)
            || ('\u{2600}'..='\u{26FF}').contains(&c)
            || ('\u{2700}'..='\u{27BF}').contains(&c)
    });
    assert!(
        !has_emoji,
        "Trace output must not contain emojis, got:\n{}",
        text_user
    );
}

#[test]
fn test_trace_collector_scope_stack_not_corrupted_when_event_limit_exceeded() {
    let mut collector = TraceCollector::new();
    for i in 0..63 {
        collector.record_step(format!("step {}", i));
    }
    collector.enter_scope("ParentPhase", "parent_target");
    collector.enter_scope("ChildPhase", "child_target");
    collector.exit_scope(Some("ChildType"));
    collector.exit_scope(Some("ParentType"));

    let md = collector.render_markdown();
    assert!(
        md.contains("ParentPhase: `parent_target` -> `ParentType`"),
        "Parent span must preserve ParentType when child scope is dropped, got:\n{}",
        md
    );
}

#[test]
fn test_hover_variable_type_inference_trace_preserves_nullable_type_marker() {
    let (index, main_url, main_code) = create_test_index();

    let pos_nu = Position::new(16, 8);
    let hover_nu = find_hover_with_config(
        &index,
        &main_url,
        "nu",
        pos_nu,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on nu with trace should resolve");
    let text_nu = extract_hover_text(hover_nu);

    assert!(
        text_nu.contains("val nu: User? = nullableUser"),
        "Signature should contain nullable type User?, got:\n{}",
        text_nu
    );
    assert!(
        text_nu.contains("Variable: `nu` -> `com.example.model.User?`")
            || text_nu.contains("Variable: `nu` -> `User?`"),
        "Trace must preserve nullable '?' suffix in inferred type, got:\n{}",
        text_nu
    );
}
