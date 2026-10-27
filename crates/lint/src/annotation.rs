//! AST annotation extraction utilities for scope properties.

use smallvec::SmallVec;
use smol_str::SmolStr;

/// Extracts all annotation names present in declaration modifiers.
pub fn extract_annotations(node: tree_sitter::Node, source: &[u8]) -> SmallVec<[SmolStr; 4]> {
    let mut annotations = SmallVec::new();
    let Some(modifiers) = node
        .children(&mut node.walk())
        .find(|child| child.kind() == "modifiers")
    else {
        return annotations;
    };

    let mut mod_cursor = modifiers.walk();
    for modifier in modifiers.children(&mut mod_cursor) {
        if modifier.kind() != "annotation" {
            continue;
        }

        let mut ann_cursor = modifier.walk();
        for ann_child in modifier.children(&mut ann_cursor) {
            if let Some(user_type) = find_annotation_type(ann_child)
                && let Ok(full_name) = user_type.utf8_text(source)
            {
                let trimmed = full_name.trim();
                push_annotation(&mut annotations, trimmed);
                let short_name = trimmed.rsplit('.').next().unwrap_or(trimmed);
                if short_name != trimmed {
                    push_annotation(&mut annotations, short_name);
                }
            }
        }
    }

    annotations
}

fn find_annotation_type(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if node.kind() == "user_type" {
        Some(node)
    } else if node.kind() == "constructor_invocation" {
        node.named_child(0)
            .filter(|child| child.kind() == "user_type")
    } else {
        None
    }
}

fn push_annotation(annotations: &mut SmallVec<[SmolStr; 4]>, name: &str) {
    let name_smol = SmolStr::new(name);
    if !annotations.contains(&name_smol) {
        let at_name = common::concat_smol_str!("@", name);
        annotations.push(name_smol);
        annotations.push(at_name);
    }
}
