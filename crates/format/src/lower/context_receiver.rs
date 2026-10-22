use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub fn lower_context_receiver<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut items = Vec::new();
    let mut comments_before_first = Vec::new();
    let mut has_seen_item = false;
    let mut in_colon = false;
    let mut current_item = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "context" | "(" | ")" => {}
            "," => {
                if !current_item.is_empty() {
                    items.push(Doc::concat(std::mem::take(&mut current_item)));
                }
            }
            ":" => in_colon = true,
            "comment" | "line_comment" | "multiline_comment" => {
                if !has_seen_item {
                    comments_before_first.push(lower_fn(child, ctx));
                } else {
                    current_item.push(Doc::space());
                    current_item.push(lower_fn(child, ctx));
                }
            }
            _ => {
                has_seen_item = true;
                if in_colon {
                    current_item.push(Doc::text(": "));
                    current_item.push(lower_fn(child, ctx));
                    in_colon = false;
                } else {
                    current_item.push(lower_fn(child, ctx));
                }
            }
        }
    }
    if !current_item.is_empty() {
        items.push(Doc::concat(current_item));
    }

    let has_comments = !comments_before_first.is_empty();
    let mut inner = Vec::new();
    for c in comments_before_first {
        inner.push(Doc::HardLine);
        inner.push(c);
    }
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            inner.push(Doc::text(","));
            inner.push(Doc::Line);
        } else if has_comments {
            inner.push(Doc::HardLine);
        }
        inner.push(item);
    }
    inner.push(Doc::text(")"));

    let doc = Doc::concat(vec![Doc::text("context("), Doc::group(Doc::concat(inner))]);
    if has_comments { Doc::group(doc) } else { doc }
}
