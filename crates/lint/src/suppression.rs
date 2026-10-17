//! Ktlint suppression locator and hint evaluation.

use crate::types::TextRange;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// A single active suppression window and its targeted rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppressionHint {
    pub range: TextRange,
    pub disabled_rules: SmallVec<[SmolStr; 2]>,
}

impl SuppressionHint {
    pub fn matches_rule(&self, rule_id: &str) -> bool {
        if self.disabled_rules.is_empty() {
            return true;
        }
        self.disabled_rules.iter().any(|disabled| {
            if disabled == "ktlint" || disabled == "ktlint:suppress-all-rules" {
                true
            } else if let Some(target) = disabled.strip_prefix("ktlint:") {
                target == rule_id
            } else {
                false
            }
        })
    }

    pub fn covers(&self, line: u32, col: u32) -> bool {
        let start = (self.range.start_line, self.range.start_col);
        let end = (self.range.end_line, self.range.end_col);
        let point = (line, col);
        point >= start && point <= end
    }
}

/// Interval collection of all suppressions in a file.
#[derive(Debug, Default, Clone)]
pub struct SuppressionLocator {
    hints: Vec<SuppressionHint>,
}

impl SuppressionLocator {
    pub fn empty() -> Self {
        Self { hints: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    pub fn is_suppressed(&self, rule_id: &str, range: &TextRange) -> bool {
        if self.hints.is_empty() {
            return false;
        }
        self.hints.iter().any(|hint| {
            hint.covers(range.start_line, range.start_col) && hint.matches_rule(rule_id)
        })
    }

    pub fn build(root_node: tree_sitter::Node, source: &[u8]) -> Self {
        if !source.windows(8).any(|window| window == b"Suppress") {
            return Self::empty();
        }
        let mut hints = Vec::new();
        let mut cursor = root_node.walk();
        collect_suppression_hints(&mut cursor, source, &mut hints);
        Self { hints }
    }
}

fn collect_suppression_hints(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    hints: &mut Vec<SuppressionHint>,
) {
    let node = cursor.node();
    let kind = node.kind();
    if kind == "file_annotation" {
        if is_suppress_annotation(node, source) {
            let mut rules = SmallVec::new();
            collect_ktlint_rules(node, source, &mut rules);
            if !rules.is_empty() {
                hints.push(SuppressionHint {
                    range: TextRange::new(1, 1, u32::MAX, u32::MAX),
                    disabled_rules: rules,
                });
            }
        }
    } else if matches!(kind, "annotation" | "single_annotation")
        && is_suppress_annotation(node, source)
    {
        let mut rules = SmallVec::new();
        collect_ktlint_rules(node, source, &mut rules);
        if rules.is_empty() {
            let next_sib = node.next_named_sibling().or_else(|| {
                if let Some(parent) = node.parent()
                    && parent.kind() == "modifiers"
                {
                    parent.next_named_sibling()
                } else {
                    None
                }
            });
            if let Some(sibling) = next_sib
                && matches!(
                    sibling.kind(),
                    "parenthesized_expression" | "ERROR" | "value_arguments"
                )
            {
                collect_ktlint_rules(sibling, source, &mut rules);
            }
        }
        if !rules.is_empty() {
            let target = find_annotation_target_node(node);
            hints.push(SuppressionHint {
                range: TextRange::from_node(target),
                disabled_rules: rules,
            });
        }
    }

    if cursor.goto_first_child() {
        loop {
            collect_suppression_hints(cursor, source, hints);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn is_suppress_annotation(node: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "type_identifier" | "simple_identifier")
            && child.utf8_text(source) == Ok("Suppress")
        {
            return true;
        }
        if matches!(child.kind(), "user_type" | "constructor_invocation")
            && is_suppress_annotation(child, source)
        {
            return true;
        }
    }
    false
}

fn collect_ktlint_rules(
    node: tree_sitter::Node,
    source: &[u8],
    rules: &mut SmallVec<[SmolStr; 2]>,
) {
    if node.kind() == "string_literal" {
        if let Ok(raw) = node.utf8_text(source) {
            let trimmed = raw.trim_matches(|c| c == '(' || c == ')' || c == '"' || c == ' ');
            if trimmed == "ktlint" || trimmed.starts_with("ktlint:") {
                rules.push(SmolStr::new(trimmed));
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ktlint_rules(child, source, rules);
    }
}

fn find_target_sibling(annotation: tree_sitter::Node) -> tree_sitter::Node {
    let mut current = annotation.next_named_sibling();
    while let Some(sibling) = current {
        match sibling.kind() {
            "annotation"
            | "single_annotation"
            | "parenthesized_expression"
            | "line_comment"
            | "multiline_comment"
            | "comment" => {
                current = sibling.next_named_sibling();
            }
            _ => return sibling,
        }
    }
    annotation
}

fn find_annotation_target_node(annotation: tree_sitter::Node) -> tree_sitter::Node {
    let mut current = annotation;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            "modifiers" => {
                if let Some(declaration) = parent.parent() {
                    return declaration;
                }
                return parent;
            }
            "annotated_expression" => {
                return parent;
            }
            "source_file" | "statements" => {
                return find_target_sibling(current);
            }
            _ => {
                current = parent;
            }
        }
    }
    annotation
}

#[cfg(test)]
mod tests {
    use super::SuppressionLocator;

    #[test]
    fn test_suppression_hints_multi_rule() {
        let code = r#"@Suppress("ktlint:standard:no-semi", "ktlint:statistics:too-many-parameters")
fun suppressedMultiple(a: Int, b: Int, c: Int, d: Int, e: Int, f: Int, g: Int, h: Int) {
    enum class Inner { X, Y; }
}
"#;
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
        parser.set_language(&language).unwrap();
        let tree = parser.parse(code.as_bytes(), None).unwrap();
        let locator = SuppressionLocator::build(tree.root_node(), code.as_bytes());
        assert_eq!(locator.hints.len(), 1);
        assert_eq!(locator.hints[0].disabled_rules.len(), 2);
        assert!(locator.hints[0].matches_rule("standard:no-semi"));
        assert!(locator.hints[0].matches_rule("statistics:too-many-parameters"));
    }
}
