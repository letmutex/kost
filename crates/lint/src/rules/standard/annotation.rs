//! Built-in AST rule enforcing annotation formatting (Ktlint rule 80).

use crate::context::LintCtx;
use crate::rule::AstRule;
use smallvec::SmallVec;

/// Rule enforcing proper line wrapping and placement of annotations.
pub struct Annotation;

impl AstRule for Annotation {
    fn id(&self) -> &'static str {
        "annotation"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["modifiers"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if let Some(parent) = ctx.parent()
            && matches!(parent.kind(), "parameter" | "class_parameter")
        {
            return;
        }

        let mut annotations = SmallVec::<[tree_sitter::Node; 4]>::new();
        collect_annotations(node, &mut annotations);

        if annotations.is_empty() {
            return;
        }

        let last_index = annotations.len() - 1;
        for (i, &current) in annotations.iter().enumerate() {
            let has_arguments = annotation_has_arguments(current);

            if has_arguments
                && i > 0
                && annotations[i - 1].end_position().row == current.start_position().row
            {
                ctx.report(current, "Expected newline before annotation");
            }

            if i == last_index {
                let following = current.next_sibling().or_else(|| node.next_sibling());
                if (has_arguments || annotations.len() > 1)
                    && let Some(next) = following
                    && next.start_position().row == current.end_position().row
                {
                    ctx.report(current, "Expected newline after last annotation");
                }
            } else if has_arguments
                && annotations[i + 1].start_position().row == current.end_position().row
            {
                ctx.report(annotations[i + 1], "Expected newline before annotation");
            }
        }
    }
}

fn collect_annotations<'a>(
    node: tree_sitter::Node<'a>,
    list: &mut SmallVec<[tree_sitter::Node<'a>; 4]>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "annotation" | "single_annotation") {
            list.push(child);
        }
    }
}

fn annotation_has_arguments(node: tree_sitter::Node) -> bool {
    if matches!(node.kind(), "value_arguments" | "value_argument_list") {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if annotation_has_arguments(child) {
            return true;
        }
    }
    false
}
