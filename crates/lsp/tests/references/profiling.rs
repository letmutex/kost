use lsp::features::find_references_profiled;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_profiled_captures_top_slowest_occurrences() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ProfileTest.kt").unwrap();
    let code = r#"
fun targetFunc() {}

fun caller() {
    targetFunc()
    targetFunc()
    targetFunc()
    targetFunc()
    targetFunc()
    targetFunc()
    targetFunc()
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(1, 4);
    let (locations, slowest) = find_references_profiled(
        &index,
        &file_url,
        "targetFunc",
        decl_pos,
        code,
        None,
        false,
        true,
    );

    assert_eq!(locations.len(), 7);
    assert_eq!(
        slowest.len(),
        5,
        "Should cap tracked slowest occurrences at 5"
    );

    for pair in slowest.windows(2) {
        assert!(
            pair[0].elapsed >= pair[1].elapsed,
            "Slowest occurrences must be ordered in descending duration"
        );
    }

    for item in &slowest {
        assert_eq!(item.uri, file_url);
        assert!(item.matched);
    }
}

#[test]
fn test_references_profiled_disabled_when_track_slowest_false() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ProfileTestDisabled.kt").unwrap();
    let code = r#"
fun worker() {}

fun caller() {
    worker()
    worker()
}
"#;
    index.index_source(file_url.clone(), code);

    let decl_pos = Position::new(1, 4);
    let (locations, slowest) = find_references_profiled(
        &index, &file_url, "worker", decl_pos, code, None, false, false,
    );

    assert_eq!(locations.len(), 2);
    assert!(
        slowest.is_empty(),
        "Slowest occurrences should not be tracked when flag is false"
    );
}
