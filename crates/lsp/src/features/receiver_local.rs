use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use crate::indexer::WorkspaceIndex;
pub use crate::inference::scope_function::{
    collect_visible_containers, resolve_scope_keyword_receiver,
};
pub use crate::inference::variable::extract_initializer_expression;

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
