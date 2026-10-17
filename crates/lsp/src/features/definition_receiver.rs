use tower_lsp::lsp_types::{Location, Position, Url};
use tree_sitter::Node;

use super::definition_match::{pick_best_match, pick_best_match_with_overload};
use super::receiver_type::{collect_type_containers, resolve_receiver_type};
use crate::indexer::WorkspaceIndex;
use crate::inference::KotlinType;

/// Query parameters for resolving receiver members.
///
/// Note: `target_location` tie-breaking is intentionally omitted here because receiver
/// members are scoped strictly by container type and package locality. Allowing a cross-file
/// target location tie-breaker would override package sorting and falsely hijack local class
/// members when unrelated classes in different packages share simple container names.
pub struct ReceiverMemberQuery<'a> {
    pub current_url: &'a Url,
    pub symbol_name: &'a str,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub receiver: &'a str,
    pub receiver_node: Option<Node<'a>>,
    pub is_call: bool,
    pub raw_args: &'a [&'a str],
    pub inferred_types: &'a [Option<KotlinType>],
}

pub fn find_receiver_member_definition(
    index: &WorkspaceIndex,
    query: ReceiverMemberQuery<'_>,
) -> Option<Location> {
    let container_type = if let Some(rec_node) = query.receiver_node {
        crate::features::receiver_type::resolve_receiver_node_type(
            index,
            query.current_url,
            rec_node,
            query.source,
            query.root,
        )
    } else {
        None
    }
    .or_else(|| {
        resolve_receiver_type(
            index,
            query.current_url,
            query.receiver,
            query.pos,
            query.source,
            query.root,
        )
    })?;
    let containers = collect_type_containers(index, &container_type);
    let current_file = index.files.get(query.current_url);
    let current_pkg = current_file.and_then(|file_symbols| file_symbols.package_name.as_deref());

    let mut all_matching: Vec<&crate::indexer::DeclarationRef> = Vec::new();
    for container in &containers {
        if let Some(decls) = index.container_to_decls.get(container.as_str()) {
            let mut matching: Vec<_> = decls
                .iter()
                .filter(|(_, decl)| decl.name == query.symbol_name)
                .collect();

            if matching.is_empty() {
                continue;
            }

            matching.sort_by_key(|(url, _)| {
                if url.as_ref() == query.current_url {
                    0
                } else if current_pkg.is_some_and(|pkg| {
                    index
                        .files
                        .get(url.as_ref())
                        .and_then(|file_symbols| file_symbols.package_name.as_deref())
                        == Some(pkg)
                }) {
                    1
                } else {
                    2
                }
            });

            all_matching.extend(matching);
        }
    }

    if !all_matching.is_empty() {
        return pick_best_match_with_overload(
            all_matching.into_iter(),
            query.is_call,
            query.raw_args,
            query.inferred_types,
            index,
            None,
        );
    }
    None
}

pub fn find_decl_in_package(
    index: &WorkspaceIndex,
    receiver: &str,
    symbol_name: &str,
    is_call: bool,
) -> Option<Location> {
    if let Some(decls) = index.name_to_decls.get(symbol_name)
        && let Some(loc) = pick_best_match(
            decls.iter().filter(|(url, decl)| {
                !decl.is_private
                    && index
                        .files
                        .get(url.as_ref())
                        .and_then(|file_syms| file_syms.package_name.as_deref())
                        == Some(receiver)
            }),
            is_call,
        )
    {
        return Some(loc);
    }
    if let Some((pkg, class_name)) = receiver.rsplit_once('.')
        && let Some(decls) = index.name_to_decls.get(symbol_name)
        && let Some(loc) = pick_best_match(
            decls.iter().filter(|(url, decl)| {
                !decl.is_private
                    && decl.container_name.as_deref() == Some(class_name)
                    && index
                        .files
                        .get(url.as_ref())
                        .and_then(|file_syms| file_syms.package_name.as_deref())
                        == Some(pkg)
            }),
            is_call,
        )
    {
        return Some(loc);
    }
    None
}

pub fn find_explicit_import_definition(
    index: &WorkspaceIndex,
    current_file: Option<&crate::indexer::FileSymbols>,
    symbol_name: &str,
    is_call: bool,
    raw_args: &[&str],
    inferred_types: &[Option<KotlinType>],
    target_location: Option<&Location>,
) -> Option<Location> {
    let file_symbols = current_file?;
    for import in &file_symbols.imports {
        let is_alias = import.alias.as_deref() == Some(symbol_name);
        let is_explicit = !import.is_star
            && (import.path.ends_with(symbol_name)
                && (import.path.len() == symbol_name.len()
                    || import.path.as_bytes()[import.path.len() - symbol_name.len() - 1] == b'.'));
        if is_alias || is_explicit {
            let target_name = if is_alias {
                import.path.rsplit('.').next().unwrap_or(symbol_name)
            } else {
                symbol_name
            };
            if let Some(target_decls) = index.name_to_decls.get(target_name) {
                let import_pkg = import.path.rsplit_once('.').map(|(pkg, _)| pkg);
                if let Some(loc) = pick_best_match_with_overload(
                    target_decls.iter().filter(|(url, decl)| {
                        !decl.is_private
                            && import_pkg.is_none_or(|pkg| {
                                index
                                    .files
                                    .get(url.as_ref())
                                    .and_then(|file_syms| file_syms.package_name.as_deref())
                                    == Some(pkg)
                            })
                    }),
                    is_call,
                    raw_args,
                    inferred_types,
                    index,
                    target_location,
                ) {
                    return Some(loc);
                }
            }
        }
    }
    None
}
