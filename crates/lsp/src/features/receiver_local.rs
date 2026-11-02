use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use crate::indexer::WorkspaceIndex;

pub fn resolve_local_receiver(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<String> {
    crate::inference::variable::resolve_local_receiver(
        index,
        current_url,
        clean,
        pos,
        source,
        root,
        depth,
    )
    .map(|t| t.to_string())
}
