use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::expr::lower_statements;
use crate::lower::lists::lower_list_with_commas;
use crate::lower::lower_node;

pub fn lower_secondary_constructor<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut modifier_docs = Vec::new();
    let mut prefix_docs = Vec::new();
    let mut params_doc = None;
    let mut delegation_doc = None;
    let mut colon_node = None;
    let mut delegation_node = None;
    let mut statements_node = None;
    let mut has_braces = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "context_receiver" => {
                modifier_docs.push(lower_node(child, ctx));
                modifier_docs.push(Doc::HardLine);
            }
            "modifiers" => {
                modifier_docs.push(lower_node(child, ctx));
            }
            "constructor" => {
                prefix_docs.push(Doc::text("constructor"));
            }
            "function_value_parameters" => {
                params_doc = Some(lower_list_with_commas(child, ctx));
            }
            ":" => {
                colon_node = Some(child);
            }
            "constructor_delegation_call" => {
                delegation_node = Some(child);
                delegation_doc = Some(lower_constructor_delegation_call(child, ctx));
            }
            "{" => {
                has_braces = true;
            }
            "statements" if child.named_child_count() > 0 => {
                statements_node = Some(child);
            }
            _ => {}
        }
    }

    let mut header_parts = prefix_docs;
    if let Some(params) = params_doc {
        header_parts.push(params);
    }
    let header = Doc::Concat(header_parts);
    let delegation_part = format_delegation_part(colon_node, delegation_node, delegation_doc, ctx);
    let head_doc = assemble_secondary_constructor_head(header, modifier_docs, delegation_part);
    attach_secondary_constructor_body(head_doc, has_braces, statements_node, ctx)
}

fn format_delegation_part<'a>(
    colon_node: Option<Node<'a>>,
    delegation_node: Option<Node<'a>>,
    delegation_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Option<Doc<'a>> {
    let colon_has_newline = match (colon_node, delegation_node) {
        (Some(colon), Some(delegation)) => ctx.has_newline_between(&colon, &delegation),
        _ => false,
    };
    delegation_doc.map(|delegation| {
        if colon_has_newline {
            Doc::Concat(vec![
                Doc::space(),
                Doc::text(":"),
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, delegation])),
            ])
        } else {
            Doc::Concat(vec![Doc::space(), Doc::text(": "), delegation])
        }
    })
}

fn assemble_secondary_constructor_head<'a>(
    header: Doc<'a>,
    modifier_docs: Vec<Doc<'a>>,
    delegation_part: Option<Doc<'a>>,
) -> Doc<'a> {
    let mut all_head = vec![header];
    if let Some(del) = delegation_part {
        all_head.push(del);
    }
    let sig_doc = Doc::Group(Box::new(Doc::Concat(all_head)));
    let full_head = if modifier_docs.is_empty() {
        sig_doc
    } else {
        Doc::Concat(vec![Doc::Concat(modifier_docs), sig_doc])
    };
    if full_head.has_hardline() {
        full_head
    } else {
        Doc::Group(Box::new(full_head))
    }
}

fn attach_secondary_constructor_body<'a>(
    head_doc: Doc<'a>,
    has_braces: bool,
    statements_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if !has_braces {
        return head_doc;
    }
    if let Some(stmts) = statements_node {
        Doc::Concat(vec![
            head_doc,
            Doc::space(),
            Doc::text("{"),
            Doc::indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_statements(stmts, ctx),
            ])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    } else {
        Doc::Concat(vec![head_doc, Doc::space(), Doc::text("{}")])
    }
}

fn lower_constructor_delegation_call<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    Doc::Concat(docs)
}
