use tree_sitter::Node;

use super::{ScriptDialect, groovy, groovy_prop, kotlin, util};

pub(crate) fn detect_dialect_with_trees(
    source: &str,
) -> (
    ScriptDialect,
    Option<tree_sitter::Tree>,
    Option<tree_sitter::Tree>,
) {
    let source_bytes = source.as_bytes();
    let kotlin_tree =
        kotlin::KOTLIN_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
    let groovy_tree =
        groovy::GROOVY_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));

    let dialect = match (&kotlin_tree, &groovy_tree) {
        (Some(kt_tree), Some(gr_tree)) => {
            let kt_root = kt_tree.root_node();
            let gr_root = gr_tree.root_node();
            if !kt_root.has_error() && !has_groovy_only_tokens(gr_root, source_bytes) {
                ScriptDialect::Kotlin
            } else if !util::has_unrecoverable_groovy_error(gr_root, source_bytes) {
                ScriptDialect::Groovy
            } else if has_kotlin_specific_ast(kt_root) {
                ScriptDialect::Kotlin
            } else if has_groovy_specific_ast(gr_root, source_bytes) {
                ScriptDialect::Groovy
            } else {
                ScriptDialect::Kotlin
            }
        }
        (None, Some(_)) => ScriptDialect::Groovy,
        _ => ScriptDialect::Kotlin,
    };

    (dialect, kotlin_tree, groovy_tree)
}

fn has_groovy_only_tokens(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "map_literal" | "map_entry" | "labeled_statement" => return true,
        "local_variable_declaration" => {
            if let Some(first) = node.child(0)
                && util::node_text(first, source) == "def"
            {
                return true;
            }
        }
        "command_chain" | "method_invocation" => {
            if groovy_prop::is_ext_invocation(node, source) {
                return true;
            }
            if node.kind() == "command_chain"
                && let Some(rec) = node
                    .child_by_field_name("receiver")
                    .or_else(|| node.child(0))
                && rec.kind() == "identifier"
                && let Some(arg) = node
                    .child_by_field_name("argument")
                    .or_else(|| node.child(1))
                && matches!(
                    arg.kind(),
                    "string_literal" | "number_literal" | "integer_literal"
                )
            {
                return true;
            }
        }
        "assignment_expression" => {
            if let Some(left) = node.child_by_field_name("left").or_else(|| node.child(0))
                && left.kind() == "field_access"
                && let Some(obj) = left.child_by_field_name("object").or_else(|| left.child(0))
            {
                let obj_text = util::node_text(obj, source).trim();
                if obj_text == "ext" || obj_text.ends_with(".ext") {
                    return true;
                }
            }
        }
        "string_literal" if util::node_text(node, source).starts_with('\'') => {
            return true;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_groovy_only_tokens(child, source) {
            return true;
        }
    }
    false
}

fn has_kotlin_specific_ast(node: Node) -> bool {
    if node.kind() == "property_declaration" {
        let mut prop_cursor = node.walk();
        if node
            .children(&mut prop_cursor)
            .any(|child| matches!(child.kind(), "val" | "var"))
        {
            return true;
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_kotlin_specific_ast(child) {
            return true;
        }
    }
    false
}

fn has_groovy_specific_ast(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "command_chain" => {
            if let Some(first) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
                && first.kind() == "identifier"
            {
                let name = util::node_text(first, source).trim();
                if !matches!(
                    name,
                    "val" | "var" | "plugins" | "android" | "dependencies" | "repositories"
                ) {
                    return true;
                }
            }
        }
        "map_literal" | "map_entry" | "labeled_statement" => return true,
        "local_variable_declaration" => {
            if let Some(first) = node.child(0)
                && util::node_text(first, source) == "def"
            {
                return true;
            }
        }
        "string_literal" if util::node_text(node, source).starts_with('\'') => {
            return true;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_groovy_specific_ast(child, source) {
            return true;
        }
    }
    false
}
