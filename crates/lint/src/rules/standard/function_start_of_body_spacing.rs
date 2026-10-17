//! Built-in AST rule enforcing single space before body delimiter (Ktlint rule 29).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

/// Rule that enforces single space before the function body delimiter (`{` or `=`).
pub struct FunctionStartOfBodySpacing;

impl AstRule for FunctionStartOfBodySpacing {
    fn id(&self) -> &'static str {
        "function-start-of-body-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_body"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        check_body(node, source, ctx);
    }
}

fn check_body(body_node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let Some(delimiter) = body_node.child(0) else {
        return;
    };

    let kind = delimiter.kind();
    if kind == "{" {
        match spaces_before(delimiter, source) {
            WhitespaceBefore::Spaces(0) => {
                ctx.report(delimiter, "Missing spacing before '{'");
            }
            WhitespaceBefore::Spaces(count) if count > 1 => {
                ctx.report(delimiter, "Unexpected spacing before '{'");
            }
            WhitespaceBefore::StartOfLine => {
                ctx.report(delimiter, "Unexpected spacing before '{'");
            }
            _ => {}
        }
    } else if kind == "=" {
        match spaces_before(delimiter, source) {
            WhitespaceBefore::Spaces(0) => {
                ctx.report(delimiter, "Missing spacing before '='");
            }
            WhitespaceBefore::Spaces(count) if count > 1 => {
                ctx.report(delimiter, "Unexpected spacing before '='");
            }
            WhitespaceBefore::StartOfLine => {
                ctx.report(delimiter, "Unexpected spacing before '='");
            }
            _ => {}
        }
    }
}
