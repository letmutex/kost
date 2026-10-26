//! Built-in AST rule enforcing Kotlin canonical modifier ordering (Ktlint rule 33).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces canonical ordering of modifiers according to Kotlin conventions.
pub struct ModifierOrder;

impl AstRule for ModifierOrder {
    fn id(&self) -> &'static str {
        "modifier-order"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["modifiers"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut highest_rank = 0;
        let mut highest_name = "";

        for child in node.children(&mut cursor) {
            let Some((rank, name)) = modifier_rank(child) else {
                continue;
            };

            if rank < highest_rank {
                let message = common::concat_str!(
                    "Modifier '",
                    name,
                    "' should appear before '",
                    highest_name,
                    "'"
                );
                ctx.report(child, message);
            } else if rank > highest_rank {
                highest_rank = rank;
                highest_name = name;
            }
        }
    }
}

fn modifier_rank(node: tree_sitter::Node) -> Option<(usize, &'static str)> {
    if matches!(node.kind(), "annotation" | "single_annotation") {
        return Some((0, "annotation"));
    }

    let keyword = if node.child_count() > 0 {
        node.child(0).map(|c| c.kind()).unwrap_or(node.kind())
    } else {
        node.kind()
    };

    match keyword {
        "public" | "protected" | "private" | "internal" => Some((2, keyword)),
        "expect" | "actual" => Some((3, keyword)),
        "final" | "open" | "abstract" | "sealed" | "const" => Some((4, keyword)),
        "external" => Some((5, keyword)),
        "override" => Some((6, keyword)),
        "lateinit" => Some((7, keyword)),
        "tailrec" => Some((8, keyword)),
        "vararg" => Some((9, keyword)),
        "suspend" => Some((10, keyword)),
        "inner" => Some((11, keyword)),
        "enum" | "annotation" | "fun" | "companion" => Some((12, keyword)),
        "inline" | "value" | "noinline" | "crossinline" => Some((13, keyword)),
        "infix" => Some((14, keyword)),
        "operator" => Some((15, keyword)),
        "data" => Some((16, keyword)),
        _ => None,
    }
}
