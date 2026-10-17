use tower_lsp::lsp_types::{Hover, HoverContents, MarkedString, Position, Url};
use tree_sitter::Node;

use super::hover_format::{
    extract_package_name_from_ast, format_index_declaration_markdown, format_local_node_markdown,
    format_source_footer,
};
use crate::config::VariableHoverTypeInference;
use crate::indexer::WorkspaceIndex;
use crate::parser::{find_node_at_position, parse_source};
use crate::position::node_to_range;

pub use super::hover_augment::augment_variable_signature_with_type;
pub use super::hover_format::build_qualifying_path;

pub fn find_hover(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
) -> Option<Hover> {
    find_hover_with_config(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        None,
        VariableHoverTypeInference::default(),
    )
}

pub fn find_hover_with_root(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<Hover> {
    find_hover_with_config(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
        VariableHoverTypeInference::default(),
    )
}

#[derive(Clone, Copy)]
pub struct HoverContext<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub type_infer_opt: VariableHoverTypeInference,
}

pub fn find_hover_with_config(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    type_infer_opt: VariableHoverTypeInference,
) -> Option<Hover> {
    let parsed_tree = if root.is_none() {
        parse_source(source)
    } else {
        None
    };
    let root = root.or_else(|| parsed_tree.as_ref().map(|t| t.root_node()));

    let hover_range = root
        .and_then(|r| find_node_at_position(r, pos))
        .map(|n| node_to_range(&n));

    let ctx = HoverContext {
        index,
        current_url,
        pos,
        source,
        root,
        type_infer_opt,
    };

    let markdown = if let Some(loc) = super::definition::find_definition_with_root(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
    ) {
        format_hover_for_location(ctx, &loc, symbol_name)
    } else if symbol_name == "it" {
        super::hover_format::format_implicit_it_hover(
            index,
            current_url,
            pos,
            source,
            root,
            type_infer_opt,
        )
    } else {
        None
    }?;

    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(markdown)),
        range: hover_range,
    })
}

fn format_hover_for_location(
    ctx: HoverContext<'_>,
    loc: &tower_lsp::lsp_types::Location,
    symbol_name: &str,
) -> Option<String> {
    let mut markdown = format_index_declaration_markdown(ctx, loc, symbol_name);

    merge_local_markdown_if_needed(ctx, &mut markdown, loc, symbol_name);

    if markdown.is_empty() {
        return None;
    }

    if let Some(footer) = format_source_footer(loc, ctx.current_url) {
        markdown.push_str(&footer);
    }

    Some(markdown)
}

fn merge_local_markdown_if_needed(
    ctx: HoverContext<'_>,
    markdown: &mut String,
    loc: &tower_lsp::lsp_types::Location,
    symbol_name: &str,
) {
    if markdown.is_empty() {
        if let Some(lmd) = resolve_local_markdown(ctx, loc, symbol_name) {
            *markdown = lmd;
        }
    } else if !markdown.contains("---")
        && let Some(lmd) = resolve_local_markdown(
            HoverContext {
                type_infer_opt: VariableHoverTypeInference::Auto,
                ..ctx
            },
            loc,
            symbol_name,
        )
    {
        attach_local_doc_if_missing(markdown, &lmd);
    }
}

fn attach_local_doc_if_missing(markdown: &mut String, local_md: &str) {
    if let Some((_, doc)) = local_md.split_once("\n---\n\n") {
        if let Some(trace_pos) = markdown.find("\n\n### Type Inference Trace") {
            let mut combined = markdown[..trace_pos].to_string();
            combined.push_str("\n---\n\n");
            combined.push_str(doc.trim_end());
            combined.push_str(&markdown[trace_pos..]);
            *markdown = combined;
        } else {
            markdown.push_str("\n---\n\n");
            markdown.push_str(doc.trim_end());
        }
    }
}

fn resolve_local_markdown(
    ctx: HoverContext<'_>,
    loc: &tower_lsp::lsp_types::Location,
    symbol_name: &str,
) -> Option<String> {
    if loc.uri == *ctx.current_url {
        let pkg = ctx
            .index
            .files
            .get(&loc.uri)
            .and_then(|f| f.package_name.as_deref())
            .or_else(|| {
                ctx.root
                    .as_ref()
                    .and_then(|r| extract_package_name_from_ast(r, ctx.source))
            });
        ctx.root
            .and_then(|r| find_node_at_position(r, loc.range.start))
            .map(|n| format_local_node_markdown(ctx, n, pkg, symbol_name))
    } else {
        let other_source = ctx.index.get_file_source(&loc.uri)?;
        let tree = parse_source(&other_source)?;
        let root_node = tree.root_node();
        let pkg = ctx
            .index
            .files
            .get(&loc.uri)
            .and_then(|f| f.package_name.as_deref())
            .or_else(|| extract_package_name_from_ast(&root_node, &other_source));
        let node_at_loc = find_node_at_position(root_node, loc.range.start)?;
        Some(format_local_node_markdown(
            HoverContext {
                current_url: &loc.uri,
                pos: loc.range.start,
                source: &other_source,
                root: Some(root_node),
                ..ctx
            },
            node_at_loc,
            pkg,
            symbol_name,
        ))
    }
}
