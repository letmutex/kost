use lsp::syntax::parser::{extract_identifier_text, find_node_at_position, parse_source};
use tower_lsp::lsp_types::Position;

#[test]
fn test_find_node_at_trailing_identifier_boundary_preceding_parameter_list() {
    let code = "fun DeviceInfoSection(modifier: Modifier = Modifier) {}";
    // "fun DeviceInfoSection" ends at character index 21.
    let tree = parse_source(code).expect("source should parse");
    let pos = Position::new(0, 21);
    let node = find_node_at_position(tree.root_node(), pos).expect("node should be found");
    let symbol = extract_identifier_text(&node, code).expect("symbol should be extracted");

    assert_eq!(
        symbol, "DeviceInfoSection",
        "Cursor at trailing boundary of function identifier should resolve to function name"
    );
}

#[test]
fn test_extract_identifier_text_rejects_function_value_parameters_container() {
    let code = "fun DeviceInfoSection(modifier: Modifier = Modifier) {}";
    let tree = parse_source(code).expect("source should parse");
    let mut cursor = tree.root_node().walk();
    let fn_node = tree
        .root_node()
        .children(&mut cursor)
        .find(|n| n.kind() == "function_declaration")
        .expect("function declaration should exist");

    let mut fn_cursor = fn_node.walk();
    let params_node = fn_node
        .children(&mut fn_cursor)
        .find(|n| n.kind() == "function_value_parameters")
        .expect("parameters container should exist");

    let extracted = extract_identifier_text(&params_node, code);
    assert_eq!(
        extracted, None,
        "extract_identifier_text must not extract child identifiers from parameters container"
    );
}

#[test]
fn test_find_node_at_trailing_call_identifier_boundary_preceding_argument_list() {
    let code = "fun test() { DeviceInfoSection(modifier = Modifier) }";
    // "DeviceInfoSection" starts at 13, length 17 -> ends at 30.
    let tree = parse_source(code).expect("source should parse");
    let pos = Position::new(0, 30);
    let node = find_node_at_position(tree.root_node(), pos).expect("node should be found");
    let symbol = extract_identifier_text(&node, code).expect("symbol should be extracted");

    assert_eq!(
        symbol, "DeviceInfoSection",
        "Cursor at trailing boundary of call identifier should resolve to callee name"
    );
}
