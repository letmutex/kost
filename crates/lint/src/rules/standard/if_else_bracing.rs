//! Built-in AST rule enforcing symmetrical bracing in if-else chains (Ktlint rule 82).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_braced_body;

/// Rule that enforces all branches of an if-else chain to have braces if any branch has braces.
pub struct IfElseBracing;

impl AstRule for IfElseBracing {
    fn id(&self) -> &'static str {
        "if-else-bracing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["if_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if is_else_if_branch(node) {
            return;
        }

        let mut branches: SmallVec<[tree_sitter::Node; 4]> = SmallVec::new();
        let mut has_else = false;
        let mut current_if = Some(node);

        while let Some(if_node) = current_if {
            let (then_branch, else_branch) = get_if_branches(if_node);

            if let Some(then_node) = then_branch {
                branches.push(then_node);
            }

            match else_branch {
                Some(else_node) => {
                    has_else = true;
                    if let Some(nested_if) = find_nested_if(else_node) {
                        current_if = Some(nested_if);
                    } else {
                        branches.push(else_node);
                        current_if = None;
                    }
                }
                None => {
                    current_if = None;
                }
            }
        }

        if !has_else {
            return;
        }

        let any_braced = branches.iter().any(|b| is_braced_body(*b));
        if !any_braced {
            return;
        }

        for branch in &branches {
            if !is_braced_body(*branch) {
                let target_node = find_branch_target(*branch);
                ctx.report(
                    target_node,
                    "All branches of the if statement should be wrapped between braces if at least one branch is wrapped between braces",
                );
            }
        }
    }
}

fn find_branch_target(branch: tree_sitter::Node) -> tree_sitter::Node {
    let mut cursor = branch.walk();
    for child in branch.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            return child;
        }
    }
    branch
}

fn is_else_if_branch(node: tree_sitter::Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "control_structure_body" {
        return false;
    }
    let Some(grandparent) = parent.parent() else {
        return false;
    };
    if grandparent.kind() != "if_expression" {
        return false;
    }

    // Ensure parent is indeed the 'else' branch of grandparent
    let mut cursor = grandparent.walk();
    let mut seen_else_keyword = false;
    for child in grandparent.children(&mut cursor) {
        if child.kind() == "else" {
            seen_else_keyword = true;
        } else if seen_else_keyword && child.id() == parent.id() {
            return true;
        }
    }
    false
}

fn get_if_branches(
    node: tree_sitter::Node,
) -> (Option<tree_sitter::Node>, Option<tree_sitter::Node>) {
    let mut then_node = None;
    let mut else_node = None;
    let mut seen_else = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "else" {
            seen_else = true;
        } else if child.kind() == "control_structure_body" {
            if seen_else {
                else_node = Some(child);
            } else if then_node.is_none() {
                then_node = Some(child);
            }
        }
    }

    (then_node, else_node)
}

fn find_nested_if(body_node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = body_node.walk();
    body_node
        .children(&mut cursor)
        .find(|&child| child.kind() == "if_expression")
}
