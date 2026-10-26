use std::collections::HashSet;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position, Url};
use tree_sitter::Node;

use crate::indexer::WorkspaceIndex;

pub fn find_enclosing_call_name(pos: Position, source: &str) -> Option<(Option<String>, String)> {
    let offset = crate::position::position_to_byte_offset(pos, source);
    let prefix = &source[..offset];
    let chars: Vec<char> = prefix.chars().collect();
    super::signature_parser::find_enclosing_call_header(&chars)
        .map(|header| (header.receiver, header.callee))
}

pub struct CallArgumentCompletionQuery<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub receiver: Option<&'a str>,
    pub callee: &'a str,
    pub partial_lower: &'a str,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
}

pub fn collect_call_argument_completions(
    query: CallArgumentCompletionQuery<'_>,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let Some(loc) = query
        .receiver
        .and_then(|r| {
            super::definition::find_receiver_member_definition(
                query.index,
                super::definition::ReceiverMemberQuery {
                    current_url: query.current_url,
                    symbol_name: query.callee,
                    pos: query.pos,
                    source: query.source,
                    root: query.root,
                    receiver: r,
                    receiver_node: None,
                    is_call: true,
                    raw_args: &[],
                    inferred_types: &[],
                },
            )
        })
        .or_else(|| {
            super::definition::find_definition_with_root(
                query.index,
                query.current_url,
                query.callee,
                query.pos,
                query.source,
                query.root,
            )
        })
    else {
        return;
    };
    let other_src = if loc.uri == *query.current_url {
        Some(query.source.to_string())
    } else {
        query.index.get_file_source(&loc.uri)
    };
    let Some(src) = other_src else { return };
    let Some(tree) = crate::parser::parse_source(&src) else {
        return;
    };
    let Some(fn_node) = crate::parser::find_node_at_position(tree.root_node(), loc.range.start)
    else {
        return;
    };
    let decl = crate::parser::find_enclosing_declaration_node(fn_node);
    collect_parameters_from_decl(decl, &src, query.partial_lower, seen_labels, completions);
}

fn collect_parameters_from_decl(
    decl: Node,
    source: &str,
    partial_lower: &str,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let mut cursor = decl.walk();
    for child in decl.children(&mut cursor) {
        let k = child.kind();
        if matches!(k, "parameter" | "class_parameter")
            && let Some(name) = crate::parser::extract_identifier_text(&child, source)
            && (partial_lower.is_empty() || name.to_lowercase().contains(partial_lower))
            && seen_labels.insert(name.to_string())
        {
            let sig = crate::parser::extract_signature(&child, source);
            completions.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: sig,
                insert_text: Some(common::concat_str!(name, " = $0")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
        if matches!(
            k,
            "function_value_parameters"
                | "class_parameters"
                | "primary_constructor"
                | "secondary_constructor"
                | "class_body"
        ) {
            collect_parameters_from_decl(child, source, partial_lower, seen_labels, completions);
        }
    }
}
