use tower_lsp::lsp_types::{Location, Position, SymbolKind, Url};
use tree_sitter::Node;

use super::completion_context::extract_receiver_before_node;
pub use super::definition_local::{BindingRole, find_local_binding, find_local_binding_node};
use super::definition_match::{find_global_or_imported_definition, pick_best_decl_with_overload};
pub use super::definition_receiver::{
    ReceiverMemberQuery, find_decl_in_package, find_receiver_member_definition,
};
use super::named_arg::find_named_argument_definition;
use crate::indexer::WorkspaceIndex;
use crate::inference::KotlinType;
use crate::parser::{find_node_at_position, parse_source};
use crate::syntax::ast_util::{
    extract_callable_reference_receiver, is_call_callee, is_callable_reference_target,
};
use crate::syntax::symbol_item::{DeclarationItem, FileSymbols};

#[derive(Debug, Clone, Copy, Default)]
pub struct DefinitionOptions<'a> {
    /// When true, lambdas are treated as `DynamicOrAny` return type during reference search
    /// so that coerced `Unit` returns or imperfect inference do not drop valid references.
    pub skip_lambda_return_type: bool,
    pub target_location: Option<&'a Location>,
}

pub fn find_definition(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
) -> Option<Location> {
    let tree = parse_source(source);
    find_definition_with_root(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        tree.as_ref().map(|t| t.root_node()),
    )
}

pub fn find_definition_with_root(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<Location> {
    find_definition_with_root_opts(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
        DefinitionOptions::default(),
    )
}

fn check_call_status(target_node: Option<Node>, symbol_name: &str, source: &str) -> (bool, bool) {
    let node_matches = target_node
        .and_then(|node| node.utf8_text(source.as_bytes()).ok())
        .is_some_and(|text| text == symbol_name);
    let is_direct_call =
        node_matches && target_node.is_some_and(|node| is_call_callee(&node, source));
    let is_callable_ref =
        node_matches && target_node.is_some_and(|node| is_callable_reference_target(&node));
    (is_direct_call, is_callable_ref)
}

fn resolve_call_site_args(
    target_node: Option<Node>,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    root: Option<Node>,
    is_direct_call: bool,
    skip_lambda_return_type: bool,
) -> (Vec<String>, Vec<Option<KotlinType>>) {
    if is_direct_call {
        crate::inference::overload::extract_call_site_arguments_opts(
            target_node,
            source,
            index,
            current_url,
            pos,
            root,
            skip_lambda_return_type,
        )
    } else {
        (Vec::new(), Vec::new())
    }
}

pub fn find_definition_with_root_opts(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    opts: DefinitionOptions<'_>,
) -> Option<Location> {
    let target_node = root.and_then(|r| find_node_at_position(r, pos));
    let (is_direct_call, is_callable_ref) = check_call_status(target_node, symbol_name, source);
    let is_call = is_direct_call || is_callable_ref;
    let binding_role = if is_callable_ref {
        BindingRole::CallableRef
    } else if is_direct_call {
        BindingRole::DirectCall
    } else {
        BindingRole::Value
    };

    if let Some(node) = target_node
        && crate::syntax::ast_util::extract_receiver_node(&node).is_none()
        && let Some(loc) = find_local_binding(node, symbol_name, source, current_url, binding_role)
    {
        return Some(loc);
    }

    let (raw_args, inferred_types) = resolve_call_site_args(
        target_node,
        source,
        index,
        current_url,
        pos,
        root,
        is_direct_call,
        opts.skip_lambda_return_type,
    );
    let raw_args_refs: Vec<&str> = raw_args.iter().map(|s| s.as_str()).collect();

    let query = DefinitionQuery {
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
        is_call,
        raw_args: &raw_args_refs,
        inferred_types: &inferred_types,
        target_location: opts.target_location,
    };

    let (has_receiver, rec_loc) = find_node_receiver_or_named_def(target_node, &query);
    if has_receiver {
        return rec_loc;
    }

    let current_file = index.files.get(current_url);
    let is_visible = is_declaration_visible_in_scope(index, current_url, root, pos, source);

    if let Some(loc) = find_current_file_definition(current_file, &query, &is_visible) {
        return Some(loc);
    }

    find_global_or_imported_definition(
        index,
        current_file,
        symbol_name,
        is_call,
        &raw_args_refs,
        &inferred_types,
        &is_visible,
        query.target_location,
    )
}

fn is_declaration_visible_in_scope<'a>(
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    root: Option<Node<'a>>,
    pos: Position,
    source: &'a str,
) -> impl Fn(&DeclarationItem) -> bool + 'a {
    let cell: std::cell::OnceCell<Vec<String>> = std::cell::OnceCell::new();
    move |decl: &DeclarationItem| {
        if decl.kind == SymbolKind::CONSTRUCTOR {
            return true;
        }
        let Some(ref container) = decl.container_name else {
            return true;
        };
        let visible_containers = cell.get_or_init(|| {
            crate::inference::scope_function::collect_visible_containers_with_url(
                index,
                current_url,
                root,
                pos,
                source,
            )
            .into_iter()
            .map(|enclosing| {
                enclosing
                    .split('<')
                    .next()
                    .unwrap_or(&enclosing)
                    .trim()
                    .to_string()
            })
            .collect()
        });
        visible_containers.iter().any(|enclosing_class| {
            crate::syntax::hierarchy::containers_match(enclosing_class, container.as_str())
        })
    }
}

fn extract_ast_receiver(node: &Node, source: &str) -> Option<String> {
    if is_callable_reference_target(node) {
        return extract_callable_reference_receiver(node, source);
    }
    if let Some(parent) = node.parent() {
        let pk = parent.kind();
        if pk == "navigation_suffix"
            && let Some(nav_expr) = parent.parent()
            && nav_expr.kind() == "navigation_expression"
            && let Some(lhs) = nav_expr.child(0)
        {
            return source
                .get(lhs.start_byte()..lhs.end_byte())
                .map(|s| s.trim().to_string());
        }
        if pk == "navigation_expression"
            && let Some(lhs) = parent.child(0)
            && lhs.id() != node.id()
        {
            return source
                .get(lhs.start_byte()..lhs.end_byte())
                .map(|s| s.trim().to_string());
        }
        if pk == "infix_expression"
            && parent.child(1).is_some_and(|c| c.id() == node.id())
            && let Some(lhs) = parent.child(0)
        {
            return source
                .get(lhs.start_byte()..lhs.end_byte())
                .map(|s| s.trim().to_string());
        }
    }
    extract_receiver_before_node(node, source)
}

pub struct DefinitionQuery<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub symbol_name: &'a str,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub is_call: bool,
    pub raw_args: &'a [&'a str],
    pub inferred_types: &'a [Option<KotlinType>],
    pub target_location: Option<&'a Location>,
}

fn find_node_receiver_or_named_def(
    target_node: Option<Node>,
    query: &DefinitionQuery<'_>,
) -> (bool, Option<Location>) {
    let Some(node) = target_node else {
        return (false, None);
    };
    let node_matches = node
        .utf8_text(query.source.as_bytes())
        .ok()
        .is_some_and(|t| t == query.symbol_name);
    let rec_node = crate::syntax::ast_util::extract_receiver_node(&node);
    if node_matches && (rec_node.is_some() || extract_ast_receiver(&node, query.source).is_some()) {
        let receiver = extract_ast_receiver(&node, query.source).unwrap_or_default();
        let loc = find_receiver_member_definition(
            query.index,
            ReceiverMemberQuery {
                current_url: query.current_url,
                symbol_name: query.symbol_name,
                pos: query.pos,
                source: query.source,
                root: query.root,
                receiver: &receiver,
                receiver_node: rec_node,
                is_call: query.is_call,
                raw_args: query.raw_args,
                inferred_types: query.inferred_types,
            },
        )
        .or_else(|| find_decl_in_package(query.index, &receiver, query.symbol_name, query.is_call));
        return (true, loc);
    }
    if let Some(arg_loc) = find_named_argument_definition(
        query.index,
        query.current_url,
        query.symbol_name,
        node,
        query.source,
        query.root,
    ) {
        return (true, Some(arg_loc));
    }
    (false, None)
}

fn find_current_file_definition<F>(
    current_file: Option<&FileSymbols>,
    query: &DefinitionQuery<'_>,
    is_visible: &F,
) -> Option<Location>
where
    F: Fn(&DeclarationItem) -> bool,
{
    let file_symbols = current_file?;
    let matching = file_symbols
        .declarations
        .iter()
        .filter(|decl| decl.name == query.symbol_name && is_visible(decl));
    let decl = pick_best_decl_with_overload(
        matching,
        query.is_call,
        query.raw_args,
        query.inferred_types,
        query.index,
        Some(query.current_url),
        query.target_location,
    )?;
    Some(Location::new(
        query.current_url.clone(),
        decl.selection_range,
    ))
}
