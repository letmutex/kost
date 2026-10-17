use lsp::indexer::WorkspaceIndex;
use lsp::inference::types::KotlinType;
use lsp::inference::{
    TraceCollector, Tracer, parse_call_expression, solve_call_generic_return_type_traced,
};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_trace_collector_records_kotlin_type_binding() {
    let mut collector = TraceCollector::new();
    let mut tracer = Tracer::Active(&mut collector);

    tracer.enter_scope("TypeSolver", "boxValue(42)");
    tracer.record_binding_type("T", &KotlinType::INT);
    tracer.exit_scope_type(Some(&KotlinType::class(
        "Box",
        vec![lsp::inference::types::TypeArg::invariant(KotlinType::INT)],
    )));

    let md = collector.render_markdown();
    assert!(
        md.contains("Bound `T` = `Int`"),
        "Trace markdown must contain structured KotlinType binding, got:\n{}",
        md
    );
    assert!(
        md.contains("Box<Int>"),
        "Trace markdown must contain structured return type in outcome, got:\n{}",
        md
    );
}

#[test]
fn test_solve_call_generic_return_type_traced_records_type_bindings() {
    let index = WorkspaceIndex::new();
    let dummy_url = Url::parse("file:///Test.kt").unwrap();
    let sig = "fun <T> identity(item: T): T";
    let call_info = parse_call_expression("identity(\"hello\")");

    let mut collector = TraceCollector::new();
    let mut tracer = Tracer::Active(&mut collector);

    let ctx = lsp::inference::InferenceContext::new(
        &index,
        &dummy_url,
        Position::new(0, 0),
        "",
        None,
        0,
        &mut tracer,
    );

    let result = solve_call_generic_return_type_traced(ctx, sig, "T", None, &call_info, None);

    assert_eq!(result, KotlinType::STRING);

    let md = collector.render_markdown();
    assert!(
        md.contains("Bound `T` = `String`"),
        "Trace must record resolved type parameter binding for T, got:\n{}",
        md
    );
}
