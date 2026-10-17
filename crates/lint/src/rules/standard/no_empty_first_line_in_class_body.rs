//! Built-in AST rule forbidding empty first line in class bodies (Ktlint rule 72).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that flags redundant empty lines immediately after the opening brace of a class body.
pub struct NoEmptyFirstLineInClassBody;

impl AstRule for NoEmptyFirstLineInClassBody {
    fn id(&self) -> &'static str {
        "no-empty-first-line-in-class-body"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["class_body", "enum_class_body"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(open_brace) = find_opening_brace(node) else {
            return;
        };

        let mut brace_row = open_brace.end_position().row;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "{" || kind == ";" {
                continue;
            }
            if child.start_position().row == brace_row {
                if !matches!(kind, "line_comment" | "multiline_comment" | "comment") {
                    break;
                }
                brace_row = brace_row.max(child.end_position().row);
                continue;
            }
            if child.start_position().row > brace_row {
                if kind != "}" && child.start_position().row > brace_row + 1 {
                    ctx.report(child, "Redundant empty line at start of class body");
                }
                break;
            }
        }
    }
}

fn find_opening_brace(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|&child| child.kind() == "{")
}
