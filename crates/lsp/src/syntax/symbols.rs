use smol_str::SmolStr;
use tower_lsp::lsp_types::SymbolKind;
use tree_sitter::Node;

use crate::doc::extract_doc_comment;
use crate::parser::{
    KotlinParser, extract_identifier_text, extract_signature, is_private_declaration,
};
use crate::position::node_to_range;

pub use super::symbol_item::{
    DeclarationItem, DeclarationMetadata, FileSymbols, ImportItem, create_declaration_item,
    find_identifier_node, map_declaration_item_info,
};

fn find_body_node<'a>(node: Node<'a>) -> Node<'a> {
    if matches!(node.kind(), "lambda_literal" | "statements" | "class_body") {
        return node;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "lambda_literal" | "statements" | "class_body" | "annotated_lambda" | "call_suffix"
        ) {
            return find_body_node(child);
        }
    }
    node
}

pub fn check_infix_class_or_interface<'a>(
    node: &Node<'a>,
    source: &'a str,
) -> Option<(&'a str, SymbolKind, Node<'a>, Node<'a>)> {
    if node.kind() != "infix_expression" {
        return None;
    }
    let left_text = node
        .child(0)
        .and_then(|c| source.get(c.start_byte()..c.end_byte()))?
        .trim();
    let mid_text = node
        .child(1)
        .and_then(|c| source.get(c.start_byte()..c.end_byte()))?
        .trim();

    let (kind, name_node, body_expr) = match (left_text, mid_text) {
        (l, _) if l == "interface" || l.ends_with(" interface") => {
            (SymbolKind::INTERFACE, node.child(1)?, node.child(2)?)
        }
        (l, _) if l == "class" || l.ends_with(" class") => {
            (SymbolKind::CLASS, node.child(1)?, node.child(2)?)
        }
        (l, _) if l == "object" || l.ends_with(" object") => {
            (SymbolKind::OBJECT, node.child(1)?, node.child(2)?)
        }
        (_, "interface") => (
            SymbolKind::INTERFACE,
            node.child(2)?.child(0)?,
            node.child(2)?,
        ),
        (_, "class") => (SymbolKind::CLASS, node.child(2)?.child(0)?, node.child(2)?),
        (_, "object") => (SymbolKind::OBJECT, node.child(2)?.child(0)?, node.child(2)?),
        _ => return None,
    };

    let name = extract_identifier_text(&name_node, source)?;
    let body_node = find_body_node(body_expr);
    Some((name, kind, name_node, body_node))
}

pub(crate) fn parse_import_header(node: Node, source: &str) -> Option<ImportItem> {
    let mut cursor = node.walk();
    let mut path = None;
    let mut alias = None;
    let mut is_star = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                path = source
                    .get(child.start_byte()..child.end_byte())
                    .map(|s| s.trim().to_string());
            }
            "wildcard_import" => {
                is_star = true;
            }
            "import_alias" => {
                let mut alias_cursor = child.walk();
                for alias_child in child.children(&mut alias_cursor) {
                    if matches!(alias_child.kind(), "type_identifier" | "simple_identifier") {
                        alias = source
                            .get(alias_child.start_byte()..alias_child.end_byte())
                            .map(|s| s.trim().to_string());
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    let path = path?;
    Some(ImportItem {
        path,
        alias,
        is_star,
    })
}

pub fn parse_file_symbols(parser: &mut KotlinParser, source: &str) -> Option<FileSymbols> {
    let masked = crate::body_masker::mask_function_bodies(source);
    let tree = parser.parse(&masked)?;
    let root = tree.root_node();
    let mut file_symbols = FileSymbols::default();

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "package_header" => {
                let mut package_cursor = child.walk();
                for package_child in child.children(&mut package_cursor) {
                    if package_child.kind() == "identifier" {
                        if let Some(text) =
                            source.get(package_child.start_byte()..package_child.end_byte())
                        {
                            let clean = text.trim();
                            if !clean.is_empty() {
                                file_symbols.package_name = Some(clean.to_string());
                            }
                        }
                        break;
                    }
                }
            }
            "import_list" => {
                let mut c = child.walk();
                for imp in child.children(&mut c) {
                    if imp.kind() == "import_header"
                        && let Some(item) = parse_import_header(imp, source)
                    {
                        file_symbols.imports.push(item);
                    }
                }
            }
            "import_header" => {
                if let Some(item) = parse_import_header(child, source) {
                    file_symbols.imports.push(item);
                }
            }
            _ => {}
        }
    }

    collect_declarations(root, source, None, &mut file_symbols);
    Some(file_symbols)
}

fn handle_infix_declaration(
    child: &Node,
    source: &str,
    container: &Option<String>,
    file_symbols: &mut FileSymbols,
) -> bool {
    let Some((name, kind, id_node, body_node)) = check_infix_class_or_interface(child, source)
    else {
        return false;
    };
    let sig = extract_signature(child, source);
    let is_priv = is_private_declaration(child, source);
    file_symbols.declarations.push(DeclarationItem::new(
        name,
        kind,
        node_to_range(child),
        node_to_range(&id_node),
        DeclarationMetadata::new(
            extract_doc_comment(child, source).map(SmolStr::new),
            sig.map(SmolStr::new),
            container.as_deref().map(SmolStr::new),
            is_priv,
        ),
    ));
    let next_container = container
        .as_ref()
        .map(|p| common::concat_str!(p, ".", name))
        .unwrap_or_else(|| name.to_string());
    collect_declarations(body_node, source, Some(next_container), file_symbols);
    true
}

fn collect_declarations(
    node: Node,
    source: &str,
    container: Option<String>,
    file_symbols: &mut FileSymbols,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if handle_infix_declaration(&child, source, &container, file_symbols) {
            continue;
        }

        let Some((name, kind, decl_node)) =
            map_declaration_item_info(child, source, container.as_deref())
        else {
            let ck = child.kind();
            if child.child_count() > 0
                && !matches!(
                    ck,
                    "comment"
                        | "import_list"
                        | "package_header"
                        | "line_comment"
                        | "multiline_comment"
                        | "modifiers"
                )
            {
                collect_declarations(child, source, container.clone(), file_symbols);
            }
            continue;
        };

        let item = create_declaration_item(
            name,
            kind,
            decl_node,
            source,
            container.clone(),
            &file_symbols.imports,
            file_symbols.package_name.as_deref(),
        );
        file_symbols.declarations.push(item);

        super::hierarchy::handle_declaration_extensions(&decl_node, source, name, file_symbols);

        let dk = decl_node.kind();

        let should_recurse = matches!(
            dk,
            "class_declaration" | "object_declaration" | "companion_object"
        );
        if should_recurse {
            let next_container = container
                .as_ref()
                .map(|p| common::concat_str!(p, ".", name))
                .unwrap_or_else(|| name.to_string());
            collect_declarations(decl_node, source, Some(next_container), file_symbols);
        }
    }
}
