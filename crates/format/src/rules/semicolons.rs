use ahash::AHashSet;
use tree_sitter::Node;

/// Detects all byte offsets of redundant semicolons that can be safely eliminated.
pub fn find_redundant_semicolons(root: Node<'_>, source: &str) -> AHashSet<usize> {
    if !source.contains(';') {
        return AHashSet::new();
    }
    let mut redundant_offsets = AHashSet::new();
    scan_semicolons(root, &mut redundant_offsets);
    redundant_offsets
}

fn scan_semicolons(node: Node<'_>, redundant: &mut AHashSet<usize>) {
    if node.kind() == ";" {
        if is_redundant_semicolon(node) {
            redundant.insert(node.start_byte());
        }
        return;
    }

    if node.child_count() > 0 {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            scan_semicolons(child, redundant);
        }
    }
}

fn is_enum_redundant_semicolon(semi: Node<'_>) -> bool {
    let mut prev = semi.prev_sibling();
    while let Some(p) = prev {
        if p.kind() == ";" {
            return true;
        }
        prev = p.prev_sibling();
    }
    let mut sibling = semi.next_named_sibling();
    while let Some(s) = sibling {
        let k = s.kind();
        if k == "function_declaration"
            || k == "property_declaration"
            || k == "class_declaration"
            || k == "companion_object"
            || k == "anonymous_initializer"
            || k == "secondary_constructor"
            || k == "object_declaration"
        {
            return false;
        }
        sibling = s.next_named_sibling();
    }
    true
}

fn is_companion_object_ambiguity(semi: Node<'_>, obj: Node<'_>) -> bool {
    let mut cursor = obj.walk();
    let has_body = obj.children(&mut cursor).any(|c| c.kind() == "class_body");
    let next_sibling = if semi.parent() == Some(obj) {
        obj.next_named_sibling()
    } else {
        semi.next_named_sibling()
    };
    !has_body && next_sibling.is_some()
}

fn is_empty_control_body(semi: Node<'_>) -> bool {
    semi.next_named_sibling().is_none()
}

fn is_trailing_call_ambiguity(semi: Node<'_>) -> bool {
    if let Some(next_stmt) = semi.next_named_sibling() {
        let k = next_stmt.kind();
        if k == "lambda_literal" || k == "annotated_lambda" || k == "parenthesized_expression" {
            return true;
        }
        if let Some(first_child) = next_stmt.child(0) {
            let fk = first_child.kind();
            if fk == "lambda_literal" || fk == "parenthesized_expression" {
                return true;
            }
        }
    }
    false
}

fn is_redundant_semicolon(semi: Node<'_>) -> bool {
    let Some(parent) = semi.parent() else {
        return false;
    };
    let parent_kind = parent.kind();

    if parent_kind == "string_literal" || parent_kind == "multiline_string_literal" {
        return false;
    }

    if parent_kind == "enum_class_body" {
        return is_enum_redundant_semicolon(semi);
    }

    if parent_kind == "companion_object" && is_companion_object_ambiguity(semi, parent) {
        return false;
    }

    if parent_kind == "class_body"
        && let Some(prev) = semi.prev_named_sibling()
        && (prev.kind() == "companion_object" || prev.kind() == "object_declaration")
        && is_companion_object_ambiguity(semi, prev)
    {
        return false;
    }

    if (parent_kind == "while_statement"
        || parent_kind == "do_while_statement"
        || parent_kind == "if_expression"
        || parent_kind == "for_statement")
        && is_empty_control_body(semi)
    {
        return false;
    }

    if is_control_structure_empty_body(semi) {
        return false;
    }

    if is_trailing_call_ambiguity(semi) {
        return false;
    }

    true
}

fn is_control_structure_empty_body(semi: Node<'_>) -> bool {
    if let Some(prev) = semi.prev_named_sibling() {
        let pk = prev.kind();
        if pk == "for_statement" || pk == "while_statement" || pk == "if_expression" {
            let mut cursor = prev.walk();
            return !prev
                .children(&mut cursor)
                .any(|c| c.kind() == "control_structure_body");
        }
    }
    false
}
