use lsp::config::VariableHoverTypeInference;
use lsp::features::hover::find_hover_with_config;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_data_class_destructuring_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Destructuring.kt").unwrap();
    let code = r#"
package com.example.destruct

data class Coordinate(val latitude: Double, val longitude: Double)

fun process() {
    val (lat, lon) = Coordinate(35.6762, 139.6503)
    val curLat = lat
    val curLon = lon
}
"#;
    index.index_source(url.clone(), code);

    let pos_lat = Position::new(7, 17);
    let inferred_lat = infer_variable_type(&index, &url, "lat", pos_lat, code, None);
    assert_eq!(inferred_lat, Some(KotlinType::DOUBLE));

    let pos_lon = Position::new(8, 17);
    let inferred_lon = infer_variable_type(&index, &url, "lon", pos_lon, code, None);
    assert_eq!(inferred_lon, Some(KotlinType::DOUBLE));
}

#[test]
fn test_generic_data_class_destructuring_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericBox.kt").unwrap();
    let code = r#"
package com.example.generic

data class Box<T>(val item: T, val count: Int)

fun process() {
    val (item, count) = Box("Hello", 42)
    val myItem = item
    val myCount = count
}
"#;
    index.index_source(url.clone(), code);

    let pos_item = Position::new(7, 17);
    let inferred_item = infer_variable_type(&index, &url, "item", pos_item, code, None);
    assert_eq!(inferred_item, Some(KotlinType::STRING));

    let pos_count = Position::new(8, 18);
    let inferred_count = infer_variable_type(&index, &url, "count", pos_count, code, None);
    assert_eq!(inferred_count, Some(KotlinType::INT));
}

#[test]
fn test_custom_component_operator_destructuring_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CustomPair.kt").unwrap();
    let code = r#"
package com.example.custom

class CustomPoint(val x: Double, val y: Double) {
    operator fun component1(): Double = x
    operator fun component2(): Double = y
}

fun process() {
    val (posX, posY) = CustomPoint(10.0, 20.0)
    val myX = posX
}
"#;
    index.index_source(url.clone(), code);

    let pos_x = Position::new(10, 14);
    let inferred_x = infer_variable_type(&index, &url, "posX", pos_x, code, None);
    assert_eq!(inferred_x, Some(KotlinType::DOUBLE));
}

#[test]
fn test_for_loop_operator_iterator_destructuring_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LoopDestruct.kt").unwrap();
    let code = r#"
package com.example.loopdestruct

data class Entry(val key: String, val value: Int)

class EntryContainer {
    operator fun iterator(): EntryIterator = EntryIterator()
}

class EntryIterator {
    operator fun hasNext(): Boolean = true
    operator fun next(): Entry = Entry("a", 1)
}

fun process(container: EntryContainer) {
    for ((k, v) in container) {
        val currentKey = k
        val currentValue = v
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos_k = Position::new(16, 25);
    let inferred_k = infer_variable_type(&index, &url, "k", pos_k, code, None);
    assert_eq!(inferred_k, Some(KotlinType::STRING));

    let pos_v = Position::new(17, 27);
    let inferred_v = infer_variable_type(&index, &url, "v", pos_v, code, None);
    assert_eq!(inferred_v, Some(KotlinType::INT));
}

#[test]
fn test_lambda_destructuring_parameter_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LambdaDestruct.kt").unwrap();
    let code = r#"
package com.example.lambdadestruct

data class Pair(val first: String, val second: Int)

fun execute(block: (Pair) -> Unit) {
    block(Pair("test", 1))
}

fun test() {
    execute { (name, age) ->
        val currentName = name
        val currentAge = age
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos_name = Position::new(11, 26);
    let inferred_name = infer_variable_type(&index, &url, "name", pos_name, code, None);
    assert_eq!(inferred_name, Some(KotlinType::STRING));

    let pos_age = Position::new(12, 25);
    let inferred_age = infer_variable_type(&index, &url, "age", pos_age, code, None);
    assert_eq!(inferred_age, Some(KotlinType::INT));
}

#[test]
fn test_explicit_type_annotation_precedence_destructuring() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExplicitDestruct.kt").unwrap();
    let code = r#"
package com.example.explicit

data class Wrapper(val a: Any, val b: Any)

fun process() {
    val (text: String, count: Int) = Wrapper("hello", 123)
    val curText = text
    val curCount = count
}
"#;
    index.index_source(url.clone(), code);

    let pos_text = Position::new(7, 18);
    let inferred_text = infer_variable_type(&index, &url, "text", pos_text, code, None);
    assert_eq!(inferred_text, Some(KotlinType::STRING));

    let pos_count = Position::new(8, 19);
    let inferred_count = infer_variable_type(&index, &url, "count", pos_count, code, None);
    assert_eq!(inferred_count, Some(KotlinType::INT));
}

#[test]
fn test_mutable_destructuring_hover_format() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MutableDestruct.kt").unwrap();
    let code = r#"
package com.example.mutable

data class Point(val x: Double, val y: Double)

fun process() {
    var (posX, posY) = Point(1.0, 2.0)
    val currentX = posX
}
"#;
    index.index_source(url.clone(), code);

    let pos_pos_x = Position::new(6, 9);
    let hover = find_hover_with_config(
        &index,
        &url,
        "posX",
        pos_pos_x,
        code,
        None,
        VariableHoverTypeInference::Always,
    );
    assert!(hover.is_some());
    if let HoverContents::Scalar(MarkedString::String(content)) = hover.unwrap().contents {
        assert!(
            content.contains("var posX: Double"),
            "Expected hover to contain 'var posX: Double', got: {}",
            content
        );
    } else {
        panic!("Unexpected hover content format");
    }
}
