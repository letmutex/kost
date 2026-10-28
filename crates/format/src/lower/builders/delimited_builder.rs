use tree_sitter::Node;

use crate::doc::Doc;
use crate::engine::node_kind;
use crate::lower::builders::assign_like::format_assign_like;
use crate::lower::builders::delimited_list::ListItem;
use crate::lower::context::LowerContext;

fn find_last_element_start_byte(node: Node<'_>) -> Option<usize> {
    let mut last_byte = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !matches!(
            node_kind(child),
            "(" | ")"
                | "["
                | "]"
                | "<"
                | ">"
                | ","
                | "comment"
                | "line_comment"
                | "multiline_comment"
        ) {
            last_byte = Some(child.start_byte());
        }
    }
    last_byte
}

#[derive(Default)]
pub struct DelimitedListBuilder<'a> {
    pub items: Vec<ListItem<'a>>,
    current_docs: Vec<Doc<'a>>,
    pub dangling_comments: Vec<Doc<'a>>,
    pub open: &'static str,
    pub close: &'static str,
    pub has_trailing_comma: bool,
    pub has_user_newline: bool,
    has_equals: bool,
    eq_rhs_node: Option<Node<'a>>,
    eq_lhs_docs: Vec<Doc<'a>>,
    prev_child: Option<Node<'a>>,
}

impl<'a> DelimitedListBuilder<'a> {
    pub fn new() -> Self {
        Self {
            open: "(",
            close: ")",
            ..Default::default()
        }
    }

    pub fn build_from_node(
        node: Node<'a>,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) -> Self {
        let mut builder = Self::new();
        let mut last_element_byte: Option<Option<usize>> = None;
        let mut cursor = node.walk();
        let mut children_iter = node.children(&mut cursor).peekable();

        while let Some(child) = children_iter.next() {
            if let Some(prev) = builder.prev_child
                && !builder.has_user_newline
            {
                builder.has_user_newline = ctx.has_user_newline_between(&prev, &child);
            }
            let kind = node_kind(child);
            if !matches!(
                kind,
                ")" | "]" | ">" | "}" | "comment" | "line_comment" | "multiline_comment"
            ) {
                builder.has_trailing_comma = kind == ",";
            }
            let next_sibling = children_iter.peek().copied();
            let is_dangling_comment =
                if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                    let last_byte = *last_element_byte
                        .get_or_insert_with(|| find_last_element_start_byte(node));
                    last_byte.is_none_or(|last| child.start_byte() > last)
                } else {
                    false
                };
            builder.handle_child_with_next(child, next_sibling, is_dangling_comment, ctx, lower_fn);
        }
        builder.flush(ctx, lower_fn);
        builder
    }

    pub fn handle_child(
        &mut self,
        child: Node<'a>,
        index: usize,
        last_element_idx: Option<usize>,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) {
        let next_sibling = child.next_sibling();
        let is_dangling = last_element_idx.is_none_or(|last| index > last);
        self.handle_child_with_next(child, next_sibling, is_dangling, ctx, lower_fn);
    }

    pub fn handle_child_with_next(
        &mut self,
        child: Node<'a>,
        next_sibling: Option<Node<'a>>,
        is_dangling_comment: bool,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) {
        let kind = node_kind(child);
        match kind {
            "(" | "[" | "<" => {
                self.open = match kind {
                    "[" => "[",
                    "<" => "<",
                    _ => "(",
                };
            }
            ")" | "]" | ">" => {
                self.close = match kind {
                    "]" => "]",
                    ">" => ">",
                    _ => ")",
                };
            }
            "," => self.flush(ctx, lower_fn),
            "comment" | "line_comment" | "multiline_comment" => {
                self.handle_comment(child, next_sibling, is_dangling_comment, ctx, lower_fn);
            }
            "=" if !self.has_equals => {
                self.has_equals = true;
                self.eq_lhs_docs = std::mem::take(&mut self.current_docs);
            }
            _ if self.has_equals && self.eq_rhs_node.is_none() => {
                self.eq_rhs_node = Some(child);
            }
            "parameter_modifiers" | "modifiers" => {
                self.current_docs.push(lower_fn(child, ctx));
            }
            "val" | "var" | "binding_pattern_kind" => {
                let kw = ctx.node_text(&child);
                self.current_docs
                    .push(Doc::smol(common::concat_smol_str!(kw, " ")));
            }
            _ => self.current_docs.push(lower_fn(child, ctx)),
        }
        self.prev_child = Some(child);
    }

    fn handle_comment(
        &mut self,
        child: Node<'a>,
        next_sibling: Option<Node<'a>>,
        is_dangling: bool,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) {
        let child_kind = node_kind(child);
        if ctx.style.align_wrapped_trailing_comments
            && child_kind == "line_comment"
            && let Some(prev) = self
                .prev_child
                .and_then(crate::rules::comments::find_trailing_line_comment)
            && crate::rules::comments::should_merge_wrapped_comment(&prev, &child, ctx)
        {
            let text = ctx.node_text(&child);
            if let Some(last_cur) = self.current_docs.last_mut()
                && crate::rules::comments::try_append_to_trailing_line_comment(last_cur, text)
            {
                return;
            }
            if let Some(last_dangling) = self.dangling_comments.last_mut()
                && crate::rules::comments::try_append_to_trailing_line_comment(last_dangling, text)
            {
                return;
            }
            if let Some(last_item) = self.items.last_mut()
                && let Some(ref mut slc) = last_item.same_line_comment
                && crate::rules::comments::try_append_to_trailing_line_comment(slc, text)
            {
                return;
            }
        }
        let is_same_line = self
            .prev_child
            .is_some_and(|p| p.end_position().row == child.start_position().row);

        if is_dangling {
            if is_same_line {
                if !self.current_docs.is_empty() || self.has_equals {
                    self.flush(ctx, lower_fn);
                }
                if !self.items.is_empty() {
                    self.attach_same_line_comment(child, ctx, lower_fn);
                    return;
                }
            }
            self.dangling_comments.push(lower_fn(child, ctx));
            return;
        }

        if is_same_line && !self.items.is_empty() && self.current_docs.is_empty() {
            self.attach_same_line_comment(child, ctx, lower_fn);
        } else if is_same_line && !self.current_docs.is_empty() {
            if !self
                .current_docs
                .last()
                .is_some_and(|d| d.ends_with_space())
            {
                self.current_docs.push(Doc::space());
            }
            self.current_docs.push(lower_fn(child, ctx));
        } else {
            if !self.current_docs.is_empty()
                && !self
                    .current_docs
                    .last()
                    .is_some_and(|d| d.ends_with_newline())
            {
                self.current_docs.push(Doc::HardLine);
            }
            self.current_docs.push(lower_fn(child, ctx));
            if let Some(next) = next_sibling
                && (child_kind == "line_comment" || ctx.has_user_newline_between(&child, &next))
            {
                self.current_docs.push(Doc::HardLine);
            }
        }
    }

    fn attach_same_line_comment(
        &mut self,
        child: Node<'a>,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) {
        let last = self.items.last_mut().unwrap();
        let c_doc = if child.kind() == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        if let Some(existing) = last.same_line_comment.take() {
            last.same_line_comment = Some(Doc::concat(vec![existing, Doc::space(), c_doc]));
        } else {
            last.same_line_comment = Some(Doc::concat(vec![Doc::space(), c_doc]));
        }
        if child.kind() == "line_comment" {
            last.force_newline_after = true;
        }
    }

    pub fn flush(
        &mut self,
        ctx: &LowerContext<'a, '_>,
        lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    ) {
        if self.has_equals
            && let Some(rhs) = self.eq_rhs_node.take()
        {
            let lhs_doc = Doc::concat(std::mem::take(&mut self.eq_lhs_docs));
            let rhs_doc = lower_fn(rhs, ctx);
            let doc = format_assign_like(lhs_doc, "=", rhs, rhs_doc, ctx);
            self.items.push(ListItem::new(doc));
            self.current_docs.clear();
            self.has_equals = false;
            return;
        }
        self.has_equals = false;
        if !self.current_docs.is_empty() {
            let doc = Doc::concat(std::mem::take(&mut self.current_docs));
            self.items.push(ListItem::new(doc));
        }
    }
}
