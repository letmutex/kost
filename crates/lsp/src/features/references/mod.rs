mod candidate;
mod conformance;
mod filter;
mod hierarchy;
mod local;
mod overload;
mod receiver_filter;
mod search;
mod target;
mod verify;

use std::time::Duration;

use tower_lsp::lsp_types::{Location, Position, Range, Url};
use tree_sitter::Node;

pub use filter::{contains_identifier_token, find_producer_names, is_file_candidate};
pub use hierarchy::conforms_to_container_or_companion;
pub use local::find_local_references;
pub use search::{deduplicate_locations, find_workspace_references};
pub use target::{
    ReferenceTarget, is_declaration_name_node, is_position_in_range, resolve_reference_target,
};

use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceTiming {
    pub uri: Url,
    pub range: Range,
    pub elapsed: Duration,
    pub matched: bool,
}

pub const MAX_SLOWEST_OCCURRENCES: usize = 5;

pub fn should_record_slowest(slowest: &[OccurrenceTiming], elapsed: Duration) -> bool {
    slowest.len() < MAX_SLOWEST_OCCURRENCES
        || slowest.last().is_some_and(|last| elapsed > last.elapsed)
}

pub fn merge_top_slowest(
    target: &mut Vec<OccurrenceTiming>,
    incoming: impl IntoIterator<Item = OccurrenceTiming>,
    limit: usize,
) {
    target.extend(incoming);
    target.sort_unstable_by_key(|a| std::cmp::Reverse(a.elapsed));
    if target.len() > limit {
        target.truncate(limit);
    }
}

pub struct ReferenceQuery<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub symbol_name: &'a str,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub include_declaration: bool,
    pub track_slowest: bool,
}

pub fn find_references(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    include_declaration: bool,
) -> Vec<Location> {
    find_references_profiled(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        None,
        include_declaration,
        false,
    )
    .0
}

pub fn find_references_with_root(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    include_declaration: bool,
) -> Vec<Location> {
    find_references_profiled(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
        include_declaration,
        false,
    )
    .0
}

pub fn find_references_profiled(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    include_declaration: bool,
    track_slowest: bool,
) -> (Vec<Location>, Vec<OccurrenceTiming>) {
    if let Some(root_node) = root {
        find_references_query_profiled(ReferenceQuery {
            index,
            current_url,
            symbol_name,
            pos,
            source,
            root: Some(root_node),
            include_declaration,
            track_slowest,
        })
    } else {
        let tree = parse_source(source);
        find_references_query_profiled(ReferenceQuery {
            index,
            current_url,
            symbol_name,
            pos,
            source,
            root: tree.as_ref().map(|parsed_tree| parsed_tree.root_node()),
            include_declaration,
            track_slowest,
        })
    }
}

pub fn find_references_query(query: ReferenceQuery<'_>) -> Vec<Location> {
    find_references_query_profiled(query).0
}

pub fn find_references_query_profiled(
    query: ReferenceQuery<'_>,
) -> (Vec<Location>, Vec<OccurrenceTiming>) {
    let Some(target) = resolve_reference_target(&query) else {
        return (Vec::new(), Vec::new());
    };

    let (locations, slowest) = if target.is_local {
        find_local_references(&query, &target)
    } else {
        find_workspace_references(&query, &target)
    };

    (deduplicate_locations(locations), slowest)
}

pub fn log_references_interaction(
    symbol_name: &str,
    pos: Position,
    locations_count: usize,
    elapsed: std::time::Duration,
    slowest_occurrences: &[OccurrenceTiming],
    is_verbose: bool,
) {
    crate::logger::log!(
        "interaction",
        "references: '{}' at {}:{} found {} locations in {:?}",
        symbol_name,
        pos.line,
        pos.character,
        locations_count,
        elapsed
    );

    if is_verbose && !slowest_occurrences.is_empty() {
        crate::logger::log!(
            "interaction",
            "references: top {} slowest occurrences for '{}':",
            slowest_occurrences.len(),
            symbol_name
        );
        for (rank, occ) in slowest_occurrences.iter().enumerate() {
            let file_name = occ
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .or_else(|| {
                    occ.uri
                        .path_segments()
                        .and_then(|mut segs| segs.next_back())
                        .filter(|segment| !segment.is_empty())
                        .map(|segment| segment.to_string())
                })
                .unwrap_or_else(|| occ.uri.as_str().to_string());
            crate::logger::log!(
                "interaction",
                "  #{}. {:>8.2} ms | {}:{}:{} (matched={})",
                rank + 1,
                occ.elapsed.as_secs_f64() * 1000.0,
                file_name,
                occ.range.start.line,
                occ.range.start.character,
                occ.matched
            );
        }
    }
}
