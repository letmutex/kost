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
fn test_hover_variable_type_inference_trace_on_chained_member_expression() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user_name = Position::new(17, 8);
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "userName",
        pos_user_name,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on userName with trace should resolve");
    let text = extract_hover_text(hover);

    assert!(
        text.contains("val userName: String = user.name"),
        "Signature should contain inferred type String, got:\n{}",
        text
    );
    assert!(
        text.contains("MemberCall: `name on com.example.model.User` -> `String`")
            || text.contains("MemberCall: `name on User` -> `String`"),
        "Trace should record MemberCall for user.name, got:\n{}",
        text
    );
}

#[test]
fn test_hover_variable_type_inference_trace_on_extension_member_call_with_arguments() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user_desc = Position::new(18, 8);
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "userDesc",
        pos_user_desc,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on userDesc with trace should resolve");
    let text = extract_hover_text(hover);

    assert!(
        text.contains("val userDesc: String = user.describe(\"info\")"),
        "Signature should contain inferred type String, got:\n{}",
        text
    );
    assert!(
        text.contains("MemberCall: `describe(\"info\") on com.example.model.User` -> `String`")
            || text.contains("MemberCall: `describe(\"info\") on User` -> `String`"),
        "Trace should record MemberCall for user.describe(\"info\"), got:\n{}",
        text
    );
    assert!(
        text.contains("Evaluated argument"),
        "Trace should record evaluated arguments, got:\n{}",
        text
    );
    assert!(
        text.contains("candidate(s) for `describe`"),
        "Trace should record candidate search, got:\n{}",
        text
    );
    assert!(
        text.contains("Selected container member `describe`")
            || text.contains("Selected candidate `describe`"),
        "Trace should record selected candidate, got:\n{}",
        text
    );
}

#[test]
fn test_hover_variable_type_inference_trace_shows_candidate_signature_score_and_origin() {
    let (index, main_url, main_code) = create_test_index();

    let pos_user_desc = Position::new(18, 8);
    let hover = find_hover_with_config(
        &index,
        &main_url,
        "userDesc",
        pos_user_desc,
        main_code,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on userDesc with trace should resolve");
    let text = extract_hover_text(hover);

    assert!(
        text.contains("[from `Main.kt`]") || text.contains("[from `Model.kt`]"),
        "Trace should display candidate origin file, got:\n{}",
        text
    );
    assert!(
        text.contains("score"),
        "Trace should display candidate match score, got:\n{}",
        text
    );
    assert!(
        text.contains("fun User.describe(tag: String): String"),
        "Trace should display full candidate signature, got:\n{}",
        text
    );
}

#[test]
fn test_trace_collector_empty_spans_renders_empty_string() {
    let collector = TraceCollector::new();
    assert_eq!(collector.render_markdown(), "");
}

#[test]
fn test_trace_collector_records_bindings_and_reports_activity() {
    let mut collector = TraceCollector::new();
    let mut tracer = lsp::inference::Tracer::Active(&mut collector);
    assert!(tracer.is_active());
    let noop = lsp::inference::Tracer::Noop;
    assert!(!noop.is_active());

    tracer.enter_scope("Call", "map(transform)");
    tracer.record_binding("T", "String");
    tracer.exit_scope(Some("List<String>"));

    let md = collector.render_markdown();
    assert!(
        md.contains("Bound `T` = `String`"),
        "Markdown trace must contain recorded bindings, got:\n{}",
        md
    );
}
