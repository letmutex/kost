//! Built-in AST rule enforcing curly braces around when entry bodies (Ktlint rule 90).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_braced_body;

/// Rule that requires braces around when entry bodies if any entry is braced or multiline.
pub struct WhenEntryBracing;

impl AstRule for WhenEntryBracing {
    fn id(&self) -> &'static str {
        "when-entry-bracing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["when_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut entries: SmallVec<[(tree_sitter::Node, tree_sitter::Node); 8]> = SmallVec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "when_entry"
                && let Some((arrow_node, body_node)) = find_arrow_and_body(child)
            {
                entries.push((arrow_node, body_node));
            }
        }

        if entries.is_empty() {
            return;
        }

        let has_trigger = entries
            .iter()
            .any(|(arrow, body)| is_braced_body(*body) || is_multiline_entry(*arrow, *body));

        if !has_trigger {
            return;
        }

        for (_, body) in entries {
            if !is_braced_body(body) {
                let target_node = find_when_entry_target(body);
                ctx.report(
                    target_node,
                    "Body of when entry should be surrounded by braces if any when entry body is surrounded by braces or has a multiline body",
                );
            }
        }
    }
}

fn find_when_entry_target(body: tree_sitter::Node) -> tree_sitter::Node {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            return child;
        }
    }
    body
}

fn find_arrow_and_body(
    when_entry: tree_sitter::Node,
) -> Option<(tree_sitter::Node, tree_sitter::Node)> {
    let mut arrow = None;
    let mut body = None;

    let mut cursor = when_entry.walk();
    for child in when_entry.children(&mut cursor) {
        if child.kind() == "->" {
            arrow = Some(child);
        } else if child.kind() == "control_structure_body" {
            body = Some(child);
        }
    }

    match (arrow, body) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

fn is_multiline_entry(arrow_node: tree_sitter::Node, body_node: tree_sitter::Node) -> bool {
    arrow_node.start_position().row != body_node.end_position().row
}
