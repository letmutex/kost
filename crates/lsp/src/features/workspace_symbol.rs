use ahash::AHashMap;
use tower_lsp::lsp_types::{Location, OneOf, Url, WorkspaceSymbol};

use crate::indexer::WorkspaceIndex;
use crate::syntax::symbol_item::FileSymbols;

pub fn search_workspace(index: &WorkspaceIndex, query: &str) -> Vec<WorkspaceSymbol> {
    search_files(&index.files, query)
}

pub fn search_files(files: &AHashMap<Url, FileSymbols>, query: &str) -> Vec<WorkspaceSymbol> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (url, file) in files {
        for decl in &file.declarations {
            if query.is_empty() || decl.name.to_lowercase().contains(&query_lower) {
                #[allow(deprecated)]
                results.push(WorkspaceSymbol {
                    name: decl.name.to_string(),
                    kind: decl.kind,
                    tags: None,
                    container_name: decl.container_name.as_ref().map(|c| c.to_string()),
                    location: OneOf::Left(Location {
                        uri: url.clone(),
                        range: decl.range,
                    }),
                    data: None,
                });
            }
        }
    }

    results
}

pub fn search_workspace_symbol_information(
    index: &WorkspaceIndex,
    query: &str,
) -> Vec<tower_lsp::lsp_types::SymbolInformation> {
    let symbols = search_workspace(index, query);
    symbols
        .into_iter()
        .filter_map(|s| {
            if let OneOf::Left(loc) = s.location {
                #[allow(deprecated)]
                Some(tower_lsp::lsp_types::SymbolInformation {
                    name: s.name,
                    kind: s.kind,
                    tags: s.tags,
                    location: loc,
                    container_name: s.container_name,
                    deprecated: None,
                })
            } else {
                None
            }
        })
        .collect()
}
