//! Built-in AST rule enforcing class signature formatting (Ktlint rule 47).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

/// Rule that enforces formatting of multiline class headers and constructor parameters.
pub struct ClassSignature;

impl AstRule for ClassSignature {
    fn id(&self) -> &'static str {
        "class-signature"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "primary_constructor" | "class_parameters") {
                check_primary_constructor(child, source, ctx);
            }
        }
    }
}

fn check_primary_constructor(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if node.start_position().row == node.end_position().row {
        return;
    }

    let mut parameters: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
    let mut closing_parenthesis = None;

    collect_constructor_items(node, &mut parameters, &mut closing_parenthesis);

    for parameter in parameters {
        if spaces_before(parameter, source) != WhitespaceBefore::StartOfLine {
            ctx.report(parameter, "Parameter should start on a newline");
        }
    }

    if let Some(rpar) = closing_parenthesis
        && spaces_before(rpar, source) != WhitespaceBefore::StartOfLine
    {
        ctx.report(rpar, "Newline expected before closing parenthesis");
    }
}

fn collect_constructor_items<'a>(
    node: tree_sitter::Node<'a>,
    parameters: &mut SmallVec<[tree_sitter::Node<'a>; 8]>,
    closing_parenthesis: &mut Option<tree_sitter::Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_parameter" => parameters.push(child),
            "class_parameters" => {
                let mut inner_cursor = child.walk();
                for inner_child in child.children(&mut inner_cursor) {
                    if inner_child.kind() == "class_parameter" {
                        parameters.push(inner_child);
                    } else if inner_child.kind() == ")" {
                        *closing_parenthesis = Some(inner_child);
                    }
                }
            }
            ")" => *closing_parenthesis = Some(child),
            _ => {}
        }
    }
}
