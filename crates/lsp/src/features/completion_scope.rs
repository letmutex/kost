use std::collections::HashSet;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};
use tree_sitter::Node;

use crate::syntax::scope::collect_local_scope_symbols;

pub fn collect_local_scope_completions(
    node: Node,
    source: &str,
    partial_lower: &str,
    before_byte: usize,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let mut symbols = Vec::new();
    collect_local_scope_symbols(
        node,
        source,
        partial_lower,
        before_byte,
        seen_labels,
        &mut symbols,
    );
    for sym in symbols {
        let kind = if sym.is_param {
            CompletionItemKind::VARIABLE
        } else if sym.is_function {
            CompletionItemKind::FUNCTION
        } else {
            CompletionItemKind::PROPERTY
        };
        completions.push(CompletionItem {
            label: sym.name,
            kind: Some(kind),
            detail: sym.signature,
            ..Default::default()
        });
    }
}
