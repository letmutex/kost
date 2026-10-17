use tower_lsp::lsp_types::{Location, Position, Url};
use tree_sitter::Node;

use super::definition::{
    ReceiverMemberQuery, find_definition_with_root, find_receiver_member_definition,
};
use crate::indexer::WorkspaceIndex;
use crate::parser::{find_enclosing_declaration_node, find_node_at_position, parse_source};
use crate::position::{node_to_range, point_to_position};
use crate::syntax::scope::find_parameter;
use crate::syntax::symbol_item::find_identifier_node;

pub fn find_named_argument_definition(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    target_node: Node,
    source: &str,
    root: Option<Node>,
) -> Option<Location> {
    let after = source.get(target_node.end_byte()..)?.trim_start();
    if !after.starts_with('=') || after.starts_with("==") {
        return None;
    }

    let mut args_or_suffix = None;
    let mut curr = target_node.parent();
    while let Some(c) = curr {
        if matches!(
            c.kind(),
            "call_suffix" | "value_arguments" | "constructor_invocation"
        ) {
            args_or_suffix = Some(c);
            if matches!(c.kind(), "call_suffix" | "constructor_invocation") {
                break;
            }
        }
        curr = c.parent();
    }
    let anchor = args_or_suffix?;
    let callee_node = anchor.prev_named_sibling().or_else(|| {
        let p = anchor.parent()?;
        (0..p.named_child_count())
            .filter_map(|i| p.named_child(i))
            .rfind(|c| c.end_byte() <= anchor.start_byte() && c.id() != anchor.id())
    })?;
    let callee_text = callee_node.utf8_text(source.as_bytes()).ok()?;
    let callee_name = callee_text
        .split('.')
        .next_back()
        .unwrap_or(callee_text)
        .trim();
    let callee_pos = point_to_position(callee_node.start_position());

    let fn_loc = callee_text
        .rfind('.')
        .and_then(|idx| {
            let receiver = callee_text[..idx].trim();
            find_receiver_member_definition(
                index,
                ReceiverMemberQuery {
                    current_url,
                    symbol_name: callee_name,
                    pos: callee_pos,
                    source,
                    root,
                    receiver,
                    receiver_node: None,
                    is_call: true,
                    raw_args: &[],
                    inferred_types: &[],
                },
            )
        })
        .or_else(|| {
            find_definition_with_root(index, current_url, callee_name, callee_pos, source, root)
        })?;

    if fn_loc.uri == *current_url {
        return find_param_location_in_ast(
            root?,
            source,
            fn_loc.range.start,
            &fn_loc.uri,
            symbol_name,
        );
    }

    find_named_argument_in_file(index, &fn_loc, symbol_name)
}

fn find_param_location_in_ast(
    root: Node,
    source: &str,
    start_pos: Position,
    uri: &Url,
    param_name: &str,
) -> Option<Location> {
    let fn_node = find_node_at_position(root, start_pos)?;
    let decl = find_enclosing_declaration_node(fn_node);
    let param_node = find_parameter(decl, param_name, source)?;
    let id_range = find_identifier_node(&param_node)
        .map(|n| node_to_range(&n))
        .unwrap_or_else(|| node_to_range(&param_node));
    Some(Location {
        uri: uri.clone(),
        range: id_range,
    })
}

fn find_named_argument_in_file(
    index: &WorkspaceIndex,
    fn_loc: &Location,
    symbol_name: &str,
) -> Option<Location> {
    let other_source = index.get_file_source(&fn_loc.uri)?;
    let tree = parse_source(&other_source)?;
    find_param_location_in_ast(
        tree.root_node(),
        &other_source,
        fn_loc.range.start,
        &fn_loc.uri,
        symbol_name,
    )
}
