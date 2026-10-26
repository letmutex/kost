use smol_str::SmolStr;
use tower_lsp::lsp_types::SymbolKind;
use tree_sitter::Node;

use crate::position::node_to_range;
use crate::syntax::modifiers::ClassModifiers;
use crate::syntax::symbol_item::{DeclarationItem, DeclarationMetadata, find_identifier_node};

/// Synthesizes synthetic members for data classes (`copy`, `componentN`) and enum classes (`entries`, `values`, `valueOf`)
pub fn synthesize_class_members(
    node: &Node,
    source: &str,
    class_name: &str,
    modifiers: ClassModifiers,
) -> Vec<DeclarationItem> {
    let mut results = Vec::new();
    let id_range = find_identifier_node(node)
        .map(|n| node_to_range(&n))
        .unwrap_or_else(|| node_to_range(node));

    if modifiers.is_data {
        results.push(DeclarationItem::new(
            "copy",
            SymbolKind::FUNCTION,
            id_range,
            id_range,
            DeclarationMetadata::new(
                Some(
                    common::concat_str!("Generates a copy of this ", class_name, " instance.")
                        .into(),
                ),
                Some(common::concat_str!("fun copy(...): ", class_name).into()),
                Some(SmolStr::new(class_name)),
                false,
            ),
        ));
        synthesize_data_class_components(node, source, class_name, &mut results);
    }

    if modifiers.is_enum {
        for (name, sig, is_fn) in [
            (
                "entries",
                common::concat_str!("val entries: EnumEntries<", class_name, ">"),
                false,
            ),
            (
                "values",
                common::concat_str!("fun values(): Array<", class_name, ">"),
                true,
            ),
            (
                "valueOf",
                common::concat_str!("fun valueOf(value: String): ", class_name),
                true,
            ),
        ] {
            let sk = if is_fn {
                SymbolKind::FUNCTION
            } else {
                SymbolKind::PROPERTY
            };
            results.push(DeclarationItem::new(
                name,
                sk,
                id_range,
                id_range,
                DeclarationMetadata::new(
                    None,
                    Some(SmolStr::new(sig)),
                    Some(SmolStr::new(class_name)),
                    false,
                ),
            ));
        }
    }

    results
}

const COMMON_COMPONENT_NAMES: &[&str] = &[
    "component1",
    "component2",
    "component3",
    "component4",
    "component5",
];

fn synthesize_data_class_components(
    node: &Node,
    source: &str,
    class_name: &str,
    results: &mut Vec<DeclarationItem>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "primary_constructor" {
            let mut param_cursor = child.walk();
            let mut index = 1usize;
            for param in child.children(&mut param_cursor) {
                if param.kind() == "class_parameter" {
                    let mut p_cursor = param.walk();
                    let has_val_or_var = param
                        .children(&mut p_cursor)
                        .any(|c| matches!(c.kind(), "val" | "var" | "binding_pattern_kind"));
                    if has_val_or_var {
                        if let Some(param_type) =
                            crate::parser::extract_type_annotation(&param, source)
                        {
                            let comp_name = if (1..=COMMON_COMPONENT_NAMES.len()).contains(&index) {
                                SmolStr::new_static(COMMON_COMPONENT_NAMES[index - 1])
                            } else {
                                common::concat_smol_str!("component", index.to_string())
                            };
                            let sig =
                                common::concat_smol_str!("fun ", &comp_name, "(): ", param_type);
                            let range = node_to_range(&param);
                            results.push(DeclarationItem::new(
                                comp_name,
                                SymbolKind::FUNCTION,
                                range,
                                range,
                                DeclarationMetadata::new(
                                    None,
                                    Some(sig),
                                    Some(SmolStr::new(class_name)),
                                    false,
                                ),
                            ));
                        }
                        index += 1;
                    }
                }
            }
            break;
        }
    }
}
