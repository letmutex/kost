use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_property_with_annotation_and_kdoc() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Properties.kt").unwrap();
    let source = r#"
package com.example

class StateHolder {
    /**
     * Current cached status of the connection.
     */
    @Deprecated("Use newConnectionStatus instead")
    var status: String = "idle"
        get() = field
        set(value) { field = value }

    fun readStatus() {
        val current = status
    }
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 13,
        character: 24,
    };
    let hover = find_hover(&index, &url, "status", pos, source).expect("hover should resolve");

    let range = hover.range.expect("hover range should be present");
    assert_eq!(range.start.line, 13);
    assert_eq!(range.end.line, 13);

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("@Deprecated(\"Use newConnectionStatus instead\")"));
        assert!(text.contains("var status: String = \"idle\""));
        assert!(text.contains("Current cached status of the connection."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_top_level_property_with_single_line_comment() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///TopLevel.kt").unwrap();
    let source = r#"
package com.example

// Maximum retry attempts for network requests
const val MAX_RETRIES: Int = 3

fun test() {
    val r = MAX_RETRIES
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 7,
        character: 14,
    };
    let hover = find_hover(&index, &url, "MAX_RETRIES", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("const val MAX_RETRIES: Int = 3"));
        assert!(text.contains("Maximum retry attempts for network requests"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_local_variable_with_type_and_initializer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///LocalVar.kt").unwrap();
    let source = r#"
package com.example

class TaskRepository(val dao: String)

fun run() {
    val taskRepository = TaskRepository("dao")
    val count: Int = 10
    val result = taskRepository
    val c = count
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 7,
        character: 18,
    };
    let hover = find_hover(&index, &url, "taskRepository", pos, source)
        .expect("hover on local variable should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val taskRepository = TaskRepository(\"dao\")"));
    } else {
        panic!("unexpected hover contents type");
    }

    let pos_count = Position {
        line: 8,
        character: 13,
    };
    let hover_count = find_hover(&index, &url, "count", pos_count, source)
        .expect("hover on typed local variable should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_count.contents {
        assert!(text.contains("val count: Int = 10"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_local_variable_in_documented_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///DocFuncLocal.kt").unwrap();
    let source = r#"
package com.example

/**
 * Does something critical.
 * @param id The entity ID.
 */
fun processEntity(id: String) {
    val localCount = 42
    val res = localCount
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 9,
        character: 16,
    };
    let hover = find_hover(&index, &url, "localCount", pos, source)
        .expect("hover on local variable should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val localCount = 42"));
        assert!(!text.contains("Does something critical."));
        assert!(!text.contains("@param id"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_destructuring_declaration_variables_scoping_and_hover() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Destructuring.kt").unwrap();
    let code = r#"
package com.example.destruct

data class Coordinate(val latitude: Double, val longitude: Double)

fun process() {
    val (lat, lon) = Coordinate(35.6762, 139.6503)
    val currentLat = lat
    val currentLon = lon
    
}
"#;
    index.index_source(url.clone(), code);

    let pos_lat = Position::new(7, 22);
    let def_lat = find_definition(&index, &url, "lat", pos_lat, code);
    assert!(
        def_lat.is_some(),
        "Destructured variable 'lat' should resolve to definition"
    );

    let pos_lon = Position::new(8, 22);
    let def_lon = find_definition(&index, &url, "lon", pos_lon, code);
    assert!(
        def_lon.is_some(),
        "Destructured variable 'lon' should resolve to definition"
    );

    let hover_lat = find_hover(&index, &url, "lat", pos_lat, code);
    assert!(
        hover_lat.is_some(),
        "Destructured variable 'lat' should show hover"
    );

    let completions = find_completions(&index, &url, Position::new(9, 4), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"lat".to_string()),
        "Completions should contain destructured variable 'lat'"
    );
    assert!(
        labels.contains(&"lon".to_string()),
        "Completions should contain destructured variable 'lon'"
    );
}
