use tree_sitter::Node;

pub use super::destructuring::{
    lower_multi_variable_declaration, lower_multi_variable_declaration_with_gid,
};
use crate::doc::{Doc, GroupId, IndentKind};
use crate::lower::builders::{format_assign_with_comments_ext, is_block_like_rhs, is_scoping_call};
use crate::lower::context::LowerContext;

#[derive(Default)]
struct PropertyDeclParts<'a> {
    modifier_docs: Vec<Doc<'a>>,
    header_docs: Vec<Doc<'a>>,
    where_doc: Option<Doc<'a>>,
    type_gid: Option<GroupId>,
    init_node: Option<Node<'a>>,
    comments_before_init: Vec<Node<'a>>,
    op_node: Option<Node<'a>>,
    is_after_by: bool,
    is_destructuring: bool,
    accessors: Vec<Doc<'a>>,
}

fn handle_property_delegate<'a>(child: Node<'a>, parts: &mut PropertyDeclParts<'a>) {
    parts.is_after_by = true;
    let mut dc = child.walk();
    for d in child.children(&mut dc) {
        match d.kind() {
            "by" => parts.op_node = Some(d),
            "comment" | "line_comment" | "multiline_comment" => {
                if parts.init_node.is_none() {
                    parts.comments_before_init.push(d);
                }
            }
            _ => {
                if parts.init_node.is_none() {
                    parts.init_node = Some(d);
                }
            }
        }
    }
}

fn parse_property_decl_parts<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> PropertyDeclParts<'a> {
    let mut parts = PropertyDeclParts::default();
    let mut is_after_equals = false;

    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "modifiers" | "annotation" => parts.modifier_docs.push(lower_fn(child, ctx)),
            "context_receiver" | "context_parameters" => {
                crate::rules::comments::push_context_receiver_doc(
                    child,
                    &mut parts.modifier_docs,
                    ctx,
                    lower_fn,
                );
            }
            "val" | "var" | "binding_pattern_kind" => {
                parts.header_docs.push(Doc::smol(common::concat_smol_str!(
                    ctx.node_text(&child),
                    " "
                )));
            }
            "multi_variable_declaration" => {
                parts.is_destructuring = true;
                let (mv_doc, gid) = lower_multi_variable_declaration_with_gid(child, ctx, lower_fn);
                parts.header_docs.push(mv_doc);
                if gid.is_some() {
                    parts.type_gid = gid;
                }
            }
            "variable_declaration" => {
                let (var_doc, gid) = lower_variable_declaration_with_gid(child, ctx, lower_fn);
                parts.header_docs.push(var_doc);
                if gid.is_some() {
                    parts.type_gid = gid;
                }
            }
            "type_parameters" => {
                parts.header_docs.push(lower_fn(child, ctx));
                parts.header_docs.push(Doc::space());
            }
            "=" => (is_after_equals, parts.op_node) = (true, Some(child)),
            "by" => (parts.is_after_by, parts.op_node) = (true, Some(child)),
            "type_constraints" => parts.where_doc = Some(lower_fn(child, ctx)),
            "getter" | "setter" | "backing_field" => parts.accessors.push(lower_fn(child, ctx)),
            "property_delegate" => handle_property_delegate(child, &mut parts),
            "comment" | "line_comment" | "multiline_comment" => {
                if (is_after_equals || parts.is_after_by) && parts.init_node.is_none() {
                    parts.comments_before_init.push(child);
                } else if parts.header_docs.is_empty() {
                    crate::rules::comments::push_modifier_comment(
                        child,
                        prev_child,
                        &mut parts.modifier_docs,
                        ctx,
                        lower_fn,
                    );
                } else {
                    parts.header_docs.push(lower_fn(child, ctx));
                }
            }
            _ => {
                if (is_after_equals || parts.is_after_by) && parts.init_node.is_none() {
                    parts.init_node = Some(child);
                } else {
                    parts.header_docs.push(lower_fn(child, ctx));
                }
            }
        }
        prev_child = Some(child);
    }
    parts
}

fn format_property_with_where<'a>(
    parts: PropertyDeclParts<'a>,
    where_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let prop_header = Doc::concat(parts.header_docs);
    let where_part = Doc::continuation_indent(Doc::concat(vec![Doc::Line, where_doc]));
    let full = if let Some(init) = parts.init_node {
        let op = if parts.is_after_by { "by" } else { "=" };
        let init_part = Doc::concat(vec![
            Doc::text("  "),
            Doc::text(op),
            Doc::space(),
            lower_fn(init, ctx),
        ]);
        Doc::concat(vec![prop_header, where_part, init_part])
    } else {
        Doc::concat(vec![prop_header, where_part])
    };
    let mut final_docs = vec![Doc::group(full)];
    for acc in parts.accessors {
        final_docs.push(Doc::indent(Doc::concat(vec![Doc::HardLine, acc])));
    }
    Doc::concat(final_docs)
}

fn format_property_init_rhs<'a>(
    prop_header: Doc<'a>,
    init: Node<'a>,
    type_gid: Option<GroupId>,
    comments_before_init: &[Node<'a>],
    op_node: Option<Node<'a>>,
    is_after_by: bool,
    is_destructuring: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let rhs_doc = lower_fn(init, ctx);
    if is_after_by && comments_before_init.is_empty() && is_scoping_call(&init) {
        Doc::concat(vec![prop_header, Doc::space(), Doc::text("by "), rhs_doc])
    } else {
        let op = if is_after_by { "by" } else { "=" };
        if let Some(tgid) = type_gid
            && !is_block_like_rhs(&init, ctx)
            && comments_before_init.is_empty()
        {
            let rhs_gid = ctx.next_group_id();
            let brk = Doc::if_group_break(tgid, Doc::HardLine, Doc::Line);
            Doc::concat(vec![
                prop_header,
                Doc::space(),
                Doc::group_with_id(
                    rhs_gid,
                    Doc::concat(vec![
                        Doc::text(op),
                        Doc::indent_if_group_breaks(
                            rhs_gid,
                            IndentKind::Continuation,
                            Doc::concat(vec![brk, rhs_doc]),
                        ),
                    ]),
                ),
            ])
        } else {
            format_assign_with_comments_ext(
                prop_header,
                op,
                op_node,
                comments_before_init,
                init,
                rhs_doc,
                is_destructuring,
                ctx,
                lower_fn,
            )
        }
    }
}

pub fn lower_property_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let parts = parse_property_decl_parts(node, ctx, lower_fn);

    if let Some(wh) = parts.where_doc.clone() {
        return format_property_with_where(parts, wh, ctx, lower_fn);
    }

    let prop_header = Doc::concat(parts.header_docs);

    let base_doc = if let Some(init) = parts.init_node {
        format_property_init_rhs(
            prop_header,
            init,
            parts.type_gid,
            &parts.comments_before_init,
            parts.op_node,
            parts.is_after_by,
            parts.is_destructuring,
            ctx,
            lower_fn,
        )
    } else {
        prop_header
    };

    let doc_with_accessors = if parts.accessors.is_empty() {
        base_doc
    } else {
        let mut full = vec![base_doc];
        full.extend(
            parts
                .accessors
                .into_iter()
                .map(|acc| Doc::indent(Doc::concat(vec![Doc::HardLine, acc]))),
        );
        Doc::concat(full)
    };

    if !parts.modifier_docs.is_empty() {
        let mut full = parts.modifier_docs;
        full.push(doc_with_accessors);
        Doc::group(Doc::concat(full))
    } else {
        doc_with_accessors
    }
}

fn flush_variable_type_docs<'a>(
    parts: &mut Vec<Doc<'a>>,
    type_docs: &mut Vec<Doc<'a>>,
    type_gid: &mut Option<GroupId>,
    ctx: &LowerContext<'a, '_>,
) {
    if !type_docs.is_empty() {
        parts.push(Doc::text(":"));
        let gid = ctx.next_group_id();
        let docs = std::mem::take(type_docs);
        parts.push(Doc::group_with_id(
            gid,
            Doc::indent_if_group_breaks(
                gid,
                IndentKind::Continuation,
                Doc::concat(vec![Doc::Line, Doc::concat(docs)]),
            ),
        ));
        *type_gid = Some(gid);
    }
}

pub fn lower_variable_declaration_with_gid<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (Doc<'a>, Option<GroupId>) {
    let mut parts = Vec::new();
    let mut type_docs = Vec::new();
    let mut type_gid = None;
    let mut after_colon = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ":" {
            after_colon = true;
        } else if kind == "=" {
            after_colon = false;
            flush_variable_type_docs(&mut parts, &mut type_docs, &mut type_gid, ctx);
            parts.push(Doc::text(" = "));
        } else if after_colon {
            let doc = lower_fn(child, ctx);
            type_docs.push(match doc {
                Doc::Group {
                    id: None,
                    doc: inner,
                } => *inner,
                other => other,
            });
        } else {
            parts.push(lower_fn(child, ctx));
            if matches!(kind, "val" | "var" | "binding_pattern_kind" | "annotation") {
                parts.push(Doc::space());
            }
        }
    }

    flush_variable_type_docs(&mut parts, &mut type_docs, &mut type_gid, ctx);
    (Doc::concat(parts), type_gid)
}

pub fn lower_variable_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    lower_variable_declaration_with_gid(node, ctx, lower_fn).0
}
