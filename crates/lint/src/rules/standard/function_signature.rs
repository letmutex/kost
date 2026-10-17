//! Built-in AST rule enforcing function signature wrapping (Ktlint rule 53).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces formatting of multiline function signatures.
pub struct FunctionSignature;

impl AstRule for FunctionSignature {
    fn id(&self) -> &'static str {
        "function-signature"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_value_parameters" {
                check_function_parameters(child, ctx);
            }
        }
    }
}

fn check_function_parameters(node: tree_sitter::Node, ctx: &mut LintCtx) {
    if node.start_position().row == node.end_position().row {
        return;
    }

    let mut cursor = node.walk();
    let mut lpar_token = None;
    let mut first_parameter = None;

    for child in node.children(&mut cursor) {
        if child.kind() == "(" && lpar_token.is_none() {
            lpar_token = Some(child);
        } else if child.kind() == "parameter" && first_parameter.is_none() {
            first_parameter = Some(child);
            break;
        }
    }

    if let (Some(lpar), Some(first_param)) = (lpar_token, first_parameter)
        && first_param.start_position().row == lpar.start_position().row
    {
        ctx.report(lpar, "Newline expected after opening parenthesis");
    }
}
