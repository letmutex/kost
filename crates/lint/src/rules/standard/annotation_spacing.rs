//! Built-in AST rule enforcing annotation spacing and prohibiting blank lines (Ktlint rule 13).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces proper spacing around declaration annotations.
pub struct AnnotationSpacing;

impl AstRule for AnnotationSpacing {
    fn id(&self) -> &'static str {
        "annotation-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["modifiers"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !matches!(child.kind(), "annotation" | "single_annotation") {
                continue;
            }

            let mut following = child.next_sibling().or_else(|| node.next_sibling());
            while let Some(next_node) = following {
                if matches!(next_node.kind(), "parenthesized_expression" | "ERROR") {
                    following = next_node.next_sibling();
                } else {
                    break;
                }
            }
            let Some(next) = following else {
                continue;
            };

            let is_next_annotation = matches!(next.kind(), "annotation" | "single_annotation");

            if child.end_position().row == next.start_position().row {
                let Some(span) = source.get(child.end_byte()..next.start_byte()) else {
                    continue;
                };
                if span != b" " {
                    if let Some(parent) = node.parent()
                        && parent.kind() == "primary_constructor"
                        && span.strip_prefix(b" constructor").is_some_and(|rest| {
                            rest.iter()
                                .all(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
                        })
                    {
                        continue;
                    }
                    let msg = if is_next_annotation {
                        "Expected a single space between annotations"
                    } else {
                        "Expected a single space after annotation"
                    };
                    ctx.report(next, msg);
                }
            } else if next.start_position().row > child.end_position().row + 1 {
                let msg = if is_next_annotation {
                    "Unexpected blank line(s) between annotations"
                } else {
                    "Unexpected blank line(s) after annotation"
                };
                let target = if is_next_annotation { next } else { child };
                ctx.report(target, msg);
            }
        }
    }
}
