//! Built-in AST rule forbidding empty first line in method blocks (Ktlint rule 73).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that flags redundant empty lines immediately after the opening brace of a method block.
pub struct NoEmptyFirstLineInMethodBlock;

impl AstRule for NoEmptyFirstLineInMethodBlock {
    fn id(&self) -> &'static str {
        "no-empty-first-line-in-method-block"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_body"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some((container, open_brace)) = find_brace_container(node) else {
            return;
        };

        let mut brace_row = open_brace.end_position().row;
        let mut cursor = container.walk();

        for child in container.children(&mut cursor) {
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
                    ctx.report(child, "Redundant empty line at start of method block");
                }
                break;
            }
        }
    }
}

fn find_brace_container(node: tree_sitter::Node) -> Option<(tree_sitter::Node, tree_sitter::Node)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" {
            return Some((node, child));
        }
        if child.kind() == "block" {
            let mut block_cursor = child.walk();
            for block_child in child.children(&mut block_cursor) {
                if block_child.kind() == "{" {
                    return Some((child, block_child));
                }
            }
        }
    }
    None
}
