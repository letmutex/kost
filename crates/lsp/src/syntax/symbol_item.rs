use ahash::AHashMap;
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Range, SymbolKind};
use tree_sitter::Node;

use crate::doc::{extract_doc_comment, merge_extra_comments};
use crate::parser::{
    extract_extension_receiver, extract_identifier_text, extract_signature_and_comments,
    has_override_modifier, is_class_parameter_property, is_private_declaration,
};
use crate::position::node_to_range;
use crate::syntax::hierarchy::resolve_extension_container;
use crate::syntax::modifiers::{ClassModifiers, parse_class_modifiers};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ImportItem {
    pub path: String,
    pub alias: Option<String>,
    pub is_star: bool,
}

#[derive(Debug, Clone)]
pub struct DeclarationItem {
    pub name: SmolStr,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub doc_comment: Option<SmolStr>,
    pub signature: Option<SmolStr>,
    pub container_name: Option<SmolStr>,
    pub is_private: bool,
    pub is_override: bool,
    pub modifiers: ClassModifiers,
}

#[derive(Debug, Clone, Default)]
pub struct DeclarationMetadata {
    pub doc_comment: Option<SmolStr>,
    pub signature: Option<SmolStr>,
    pub container_name: Option<SmolStr>,
    pub is_private: bool,
    pub is_override: bool,
    pub modifiers: ClassModifiers,
}

impl DeclarationMetadata {
    pub fn new(
        doc_comment: Option<SmolStr>,
        signature: Option<SmolStr>,
        container_name: Option<SmolStr>,
        is_private: bool,
    ) -> Self {
        Self {
            doc_comment,
            signature,
            container_name,
            is_private,
            is_override: false,
            modifiers: ClassModifiers::default(),
        }
    }

    pub fn with_modifiers(
        doc_comment: Option<SmolStr>,
        signature: Option<SmolStr>,
        container_name: Option<SmolStr>,
        is_private: bool,
        is_override: bool,
        modifiers: ClassModifiers,
    ) -> Self {
        Self {
            doc_comment,
            signature,
            container_name,
            is_private,
            is_override,
            modifiers,
        }
    }

    pub fn signature(sig: impl Into<SmolStr>) -> Self {
        Self {
            signature: Some(sig.into()),
            ..Default::default()
        }
    }
}

impl DeclarationItem {
    pub fn new(
        name: impl Into<SmolStr>,
        kind: SymbolKind,
        range: Range,
        selection_range: Range,
        metadata: DeclarationMetadata,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            range,
            selection_range,
            doc_comment: metadata.doc_comment,
            signature: metadata.signature,
            container_name: metadata.container_name,
            is_private: metadata.is_private,
            is_override: metadata.is_override,
            modifiers: metadata.modifiers,
        }
    }

    pub fn simple(
        name: impl Into<SmolStr>,
        kind: SymbolKind,
        range: Range,
        selection_range: Range,
    ) -> Self {
        Self::new(
            name,
            kind,
            range,
            selection_range,
            DeclarationMetadata::default(),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileSymbols {
    pub package_name: Option<String>,
    pub imports: Vec<ImportItem>,
    pub declarations: Vec<DeclarationItem>,
    pub class_supertypes: AHashMap<String, Vec<String>>,
    pub type_aliases: AHashMap<String, String>,
}

/// Find the identifier node for a declaration
pub fn find_identifier_node<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        if k == "simple_identifier" || k == "type_identifier" || k == "identifier" {
            return Some(child);
        }
        if (k == "variable_declaration" || k == "ERROR")
            && let Some(id_node) = find_identifier_node(&child)
        {
            return Some(id_node);
        }
    }
    None
}

fn map_class_declaration<'a>(child: Node<'a>, source: &'a str) -> (&'a str, SymbolKind, Node<'a>) {
    let name = extract_identifier_text(&child, source).unwrap_or("class");
    let modifiers = parse_class_modifiers(&child, source);
    if modifiers.is_enum {
        return (name, SymbolKind::ENUM, child);
    }
    let mut is_interface = modifiers.is_fun_interface;
    if !is_interface {
        let mut cursor = child.walk();
        for c in child.children(&mut cursor) {
            let k = c.kind();
            if k == "interface" {
                is_interface = true;
                break;
            }
            if k == "class" || k == "class_body" {
                break;
            }
        }
    }
    let kind = if is_interface {
        SymbolKind::INTERFACE
    } else {
        SymbolKind::CLASS
    };
    (name, kind, child)
}

pub fn map_declaration_item_info<'a>(
    child: Node<'a>,
    source: &'a str,
    container: Option<&'a str>,
) -> Option<(&'a str, SymbolKind, Node<'a>)> {
    match child.kind() {
        "class_declaration" => Some(map_class_declaration(child, source)),
        "object_declaration" | "companion_object" => {
            let def = if child.kind() == "object_declaration" {
                "object"
            } else {
                "Companion"
            };
            let name = extract_identifier_text(&child, source).unwrap_or(def);
            Some((name, SymbolKind::OBJECT, child))
        }
        "secondary_constructor" => {
            let name = container
                .and_then(|c| c.rsplit('.').next())
                .unwrap_or("constructor");
            Some((name, SymbolKind::CONSTRUCTOR, child))
        }
        "function_declaration" => {
            let name = extract_identifier_text(&child, source).unwrap_or("fun");
            Some((name, SymbolKind::FUNCTION, child))
        }
        "property_declaration" => {
            let name = extract_identifier_text(&child, source).unwrap_or("val");
            Some((name, SymbolKind::PROPERTY, child))
        }
        "class_parameter" => is_class_parameter_property(&child).then(|| {
            let name = extract_identifier_text(&child, source).unwrap_or("prop");
            (name, SymbolKind::PROPERTY, child)
        }),
        "type_alias" => {
            let name = extract_identifier_text(&child, source).unwrap_or("typealias");
            Some((name, SymbolKind::TYPE_PARAMETER, child))
        }
        "enum_entry" => {
            let name = extract_identifier_text(&child, source).unwrap_or("entry");
            Some((name, SymbolKind::ENUM_MEMBER, child))
        }
        _ => None,
    }
}

pub fn create_declaration_item(
    name: &str,
    kind: SymbolKind,
    decl_node: Node,
    source: &str,
    container: Option<String>,
    imports: &[ImportItem],
    pkg: Option<&str>,
) -> DeclarationItem {
    let item_container = if kind != SymbolKind::INTERFACE {
        if let Some(ext_rec) = extract_extension_receiver(&decl_node, source) {
            Some(resolve_extension_container(ext_rec, imports, pkg))
        } else {
            container
        }
    } else {
        container
    };

    let id_range = find_identifier_node(&decl_node)
        .map(|n| node_to_range(&n))
        .unwrap_or_else(|| node_to_range(&decl_node));

    let sig_and_comments = extract_signature_and_comments(&decl_node, source);
    let signature = sig_and_comments.as_ref().map(|(s, _)| s.clone());
    let mut doc_comment = extract_doc_comment(&decl_node, source);
    if let Some((_, ref extra_comments)) = sig_and_comments {
        doc_comment = merge_extra_comments(doc_comment, extra_comments);
    }

    let is_private = is_private_declaration(&decl_node, source);
    let is_override = has_override_modifier(&decl_node, source);

    let modifiers = parse_class_modifiers(&decl_node, source);
    let mut actual_kind = kind;
    if modifiers.is_enum {
        actual_kind = SymbolKind::ENUM;
    } else if modifiers.is_fun_interface {
        actual_kind = SymbolKind::INTERFACE;
    }

    DeclarationItem::new(
        name,
        actual_kind,
        node_to_range(&decl_node),
        id_range,
        DeclarationMetadata::with_modifiers(
            doc_comment.map(SmolStr::new),
            signature.map(SmolStr::new),
            item_container.as_deref().map(SmolStr::new),
            is_private,
            is_override,
            modifiers,
        ),
    )
}
