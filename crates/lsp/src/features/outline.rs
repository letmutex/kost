use tower_lsp::lsp_types::DocumentSymbol;
use tree_sitter::Node;

use crate::parser::extract_signature;
use crate::position::node_to_range;
use crate::syntax::symbol_item::{find_identifier_node, map_declaration_item_info};
use crate::syntax::symbols::check_infix_class_or_interface;

/// Recursively extract document symbols for LSP outline
pub fn extract_document_symbols(node: Node, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if let Some((name, kind, id_node, body_node)) =
            check_infix_class_or_interface(&child, source)
        {
            let children = extract_document_symbols(body_node, source);
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: name.to_string(),
                detail: extract_signature(&child, source),
                kind,
                tags: None,
                deprecated: None,
                range: node_to_range(&child),
                selection_range: node_to_range(&id_node),
                children: (!children.is_empty()).then_some(children),
            });
            continue;
        }

        let id_range = find_identifier_node(&child)
            .map(|n| node_to_range(&n))
            .unwrap_or_else(|| node_to_range(&child));

        let Some((name, kind, _)) = map_declaration_item_info(child, source, None) else {
            let nested = extract_document_symbols(child, source);
            symbols.extend(nested);
            continue;
        };

        let children = extract_document_symbols(child, source);
        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: name.to_string(),
            detail: extract_signature(&child, source),
            kind,
            tags: None,
            deprecated: None,
            range: node_to_range(&child),
            selection_range: id_range,
            children: (!children.is_empty()).then_some(children),
        });
    }

    symbols
}
