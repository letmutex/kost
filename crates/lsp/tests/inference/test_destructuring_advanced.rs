use lsp::config::VariableHoverTypeInference;
use lsp::features::completion::find_completions;
use lsp::features::hover::find_hover_with_config;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_expression_type, infer_variable_type};
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_destructured_variable_hover_and_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DestructHover.kt").unwrap();
    let code = r#"
package com.example.hover

data class Coordinate(val latitude: Double, val longitude: Double)

fun process() {
    val (lat, lon) = Coordinate(35.6762, 139.6503)
    val cur = lat
}
"#;
    index.index_source(url.clone(), code);

    let pos_lat = Position::new(6, 9);
    let hover = find_hover_with_config(
        &index,
        &url,
        "lat",
        pos_lat,
        code,
        None,
        VariableHoverTypeInference::Always,
    );
    assert!(hover.is_some());
    if let HoverContents::Scalar(MarkedString::String(content)) = hover.unwrap().contents {
        assert!(
            content.contains("val lat: Double"),
            "Expected hover to contain 'val lat: Double', got: {}",
            content
        );
    } else {
        panic!("Unexpected hover content format");
    }

    let completions = find_completions(&index, &url, Position::new(7, 4), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(labels.contains(&"lat".to_string()));
    assert!(labels.contains(&"lon".to_string()));
}

#[test]
fn test_unannotated_parameter_does_not_shift_subsequent_component_index() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/IncompleteDataClass.kt").unwrap();
    let code = r#"
package com.example.incomplete

data class Triplet(val a: Int, val b: (Int), val c: String)

fun process(t: Triplet) {
    val (first, second, third) = t
    val currentThird = third
}
"#;
    index.index_source(url.clone(), code);

    let pos_third = Position::new(7, 23);
    let inferred_third = infer_variable_type(&index, &url, "third", pos_third, code, None);
    assert_eq!(inferred_third, Some(KotlinType::STRING));
}

#[test]
fn test_destructuring_arity_exceeded_returns_none() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArityExceeded.kt").unwrap();
    let code = r#"
package com.example.arity

data class Pair(val first: String, val second: Int)

fun process(p: Pair) {
    val (a, b, c) = p
    val currentExtra = c
}
"#;
    index.index_source(url.clone(), code);

    let pos_c = Position::new(7, 23);
    let inferred_c = infer_variable_type(&index, &url, "c", pos_c, code, None);
    assert_eq!(inferred_c, None);
}

#[test]
fn test_underscore_placeholder_destructuring_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnderscoreDestruct.kt").unwrap();
    let code = r#"
package com.example.underscore

data class Pair(val first: String, val second: Int)

fun process(p: Pair) {
    val (_, second) = p
    val currentSecond = second
}
"#;
    index.index_source(url.clone(), code);

    let pos_second = Position::new(7, 24);
    let inferred_second = infer_variable_type(&index, &url, "second", pos_second, code, None);
    assert_eq!(inferred_second, Some(KotlinType::INT));
}

#[test]
fn test_chained_map_lambda_destructuring_param_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainedMapDestruct.kt").unwrap();
    let code = r#"
package com.example.destruct

data class Pair2<A, B>(val first: A, val second: B)

public infix fun <A, B> A.pairWith(that: B): Pair2<A, B> = Pair2(this, that)

class Flow<T>(val value: T)

public fun <T1, T2, R> combine(
    flow: Flow<T1>,
    flow2: Flow<T2>,
    transform: suspend (a: T1, b: T2) -> R
): Flow<R> = TODO()

fun <T, R> Flow<T>.map(block: (T) -> R): R = TODO()

val combined = combine(Flow(1), Flow("")) { value1, value2 ->
        value1 pairWith value2
    }
    .map { paired -> Flow(paired) }
    .map { (value1, value2) -> 1 }
"#;
    index.index_source(url.clone(), code);

    let pos_combine_val1 = Position::new(17, 45);
    let inferred_combine_val1 =
        infer_variable_type(&index, &url, "value1", pos_combine_val1, code, None);
    assert_eq!(inferred_combine_val1, Some(KotlinType::INT));

    let pos_combine_val2 = Position::new(17, 53);
    let inferred_combine_val2 =
        infer_variable_type(&index, &url, "value2", pos_combine_val2, code, None);
    assert_eq!(inferred_combine_val2, Some(KotlinType::STRING));

    let pos_paired = Position::new(20, 11);
    let inferred_paired = infer_variable_type(&index, &url, "paired", pos_paired, code, None);
    assert_eq!(
        inferred_paired,
        KotlinType::parse("com.example.destruct.Pair2<Int, String>")
    );

    let pos_val1 = Position::new(21, 12);
    let inferred_val1 = infer_variable_type(&index, &url, "value1", pos_val1, code, None);
    assert_eq!(inferred_val1, Some(KotlinType::INT));

    let pos_val2 = Position::new(21, 20);
    let inferred_val2 = infer_variable_type(&index, &url, "value2", pos_val2, code, None);
    assert_eq!(inferred_val2, Some(KotlinType::STRING));
}

#[test]
fn test_chained_higher_order_call_distinct_lambdas_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainedHigherOrder.kt").unwrap();
    let code = r#"
package com.example.chained

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
fun <T> List<T>.first(predicate: (T) -> Boolean): T = TODO()
val String.length: Int get() = 0

fun test() {
    val result = listOf("hello").map { it.length }.first { it > 0 }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
    assert_eq!(
        infer_expression_type(
            &index,
            &url,
            "listOf(\"hello\").map { it.length }.first { it > 0 }",
            pos,
            code,
            None
        ),
        Some(KotlinType::INT)
    );
}

#[test]
fn test_resolve_higher_order_call_compound_expression_call_selection() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompoundHigherOrder.kt").unwrap();
    let code = r#"
package com.example.compound

fun calculate(): Int = 1
fun map(transform: () -> String): String = transform()

val x = 1
"#;
    index.index_source(url.clone(), code);

    let mut tracer = lsp::inference::Tracer::Noop;
    let ctx = lsp::inference::InferenceContext::new(
        &index,
        &url,
        Position::new(6, 4),
        code,
        None,
        0,
        &mut tracer,
    );
    let inferred =
        lsp::inference::resolve_higher_order_call(ctx, None, "calculate() + map { \"hello\" }");
    assert_eq!(inferred, Some(KotlinType::STRING));
}
