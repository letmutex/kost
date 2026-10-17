use lsp::features::find_references_with_root;
use lsp::indexer::WorkspaceIndex;
use lsp::syntax::parser::{extract_identifier_text, find_node_at_position, parse_source};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_at_trailing_function_identifier_boundary_with_default_parameter() {
    let mut index = WorkspaceIndex::new();
    let def_url = Url::parse("file:///project/Component.kt").unwrap();
    let def_code = "fun DeviceInfoSection(modifier: Modifier = Modifier) {}\n";
    let usage_url = Url::parse("file:///project/Usage.kt").unwrap();
    let usage_code = "fun run() { DeviceInfoSection() }\n";

    index.index_source(def_url.clone(), def_code);
    index.index_source(usage_url.clone(), usage_code);

    let tree = parse_source(def_code).expect("source should parse");
    let root = tree.root_node();

    // Cursor at character 21, the trailing edge of "DeviceInfoSection"
    let pos = Position::new(0, 21);
    let node = find_node_at_position(root, pos).expect("node should be found");
    let symbol_name = extract_identifier_text(&node, def_code).expect("symbol should be extracted");

    assert_eq!(
        symbol_name, "DeviceInfoSection",
        "Should extract DeviceInfoSection, not Modifier, at trailing boundary"
    );

    let refs = find_references_with_root(
        &index,
        &def_url,
        symbol_name,
        pos,
        def_code,
        Some(root),
        false,
    );

    assert_eq!(
        refs.len(),
        1,
        "Should find 1 usage of DeviceInfoSection in workspace"
    );
    assert_eq!(refs[0].uri, usage_url);
}
