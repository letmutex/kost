use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub fn lower_backing_field<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut in_colon = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "field" {
            docs.push(Doc::text("field"));
        } else if kind == ":" {
            in_colon = true;
        } else if in_colon {
            docs.push(Doc::text(": "));
            docs.push(lower_fn(child, ctx));
            in_colon = false;
        } else if kind == "=" {
            docs.push(Doc::text(" = "));
        } else if matches!(kind, "modifiers" | "annotation") {
            docs.push(lower_fn(child, ctx));
            docs.push(Doc::space());
        } else {
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}
