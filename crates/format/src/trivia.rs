use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
    KDoc,
}

#[derive(Debug, Clone)]
pub struct CommentItem<'a> {
    pub node: Node<'a>,
    pub kind: CommentKind,
    pub has_blank_line_before: bool,
    pub has_newline_before: bool,
}

#[derive(Debug, Clone)]
pub struct ChildWithTrivia<'a> {
    pub leading_comments: Vec<CommentItem<'a>>,
    pub child: Node<'a>,
    pub same_line_trailing_comment: Option<CommentItem<'a>>,
}

pub struct SyntaxCursor<'a> {
    pub items: Vec<ChildWithTrivia<'a>>,
    pub dangling_comments: Vec<CommentItem<'a>>,
}

#[inline]
pub fn is_comment(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "comment" | "line_comment" | "multiline_comment"
    )
}

pub fn classify_comment<'a>(
    node: Node<'a>,
    source: &'a str,
    prev_end_byte: usize,
) -> CommentItem<'a> {
    let text = &source[node.start_byte()..node.end_byte()];
    let kind = if text.starts_with("/**") {
        CommentKind::KDoc
    } else if text.starts_with("//") {
        CommentKind::Line
    } else {
        CommentKind::Block
    };

    let gap = if node.start_byte() >= prev_end_byte {
        &source[prev_end_byte..node.start_byte()]
    } else {
        ""
    };

    let newline_count = gap.bytes().filter(|&b| b == b'\n').count();
    let has_blank_line_before = newline_count >= 2;
    let has_newline_before = newline_count >= 1;

    CommentItem {
        node,
        kind,
        has_blank_line_before,
        has_newline_before,
    }
}

impl<'a> SyntaxCursor<'a> {
    /// Constructs a cursor by analyzing all named and trivia children of a node.
    pub fn from_named_children(parent: Node<'a>, source: &'a str) -> Self {
        let mut items = Vec::new();
        let mut pending_leading = Vec::new();
        let mut prev_end = parent.start_byte();
        let mut cursor = parent.walk();

        let children: Vec<_> = parent.children(&mut cursor).collect();

        let mut i = 0;
        while i < children.len() {
            let child = children[i];
            if is_comment(&child) {
                let comment_item = classify_comment(child, source, prev_end);
                prev_end = child.end_byte();
                pending_leading.push(comment_item);
                i += 1;
            } else if child.is_named() {
                let leading = std::mem::take(&mut pending_leading);
                prev_end = child.end_byte();

                // Check if immediately following child is a same-line trailing comment
                let mut same_line_trailing = None;
                if i + 1 < children.len() && is_comment(&children[i + 1]) {
                    let next_comment = children[i + 1];
                    let gap = &source[child.end_byte()..next_comment.start_byte()];
                    if !gap.contains('\n') {
                        same_line_trailing =
                            Some(classify_comment(next_comment, source, child.end_byte()));
                        prev_end = next_comment.end_byte();
                        i += 1; // Consume trailing comment
                    }
                }

                items.push(ChildWithTrivia {
                    leading_comments: leading,
                    child,
                    same_line_trailing_comment: same_line_trailing,
                });
                i += 1;
            } else {
                // Anonymous syntax token (e.g. '{', '}', ';')
                prev_end = child.end_byte();
                i += 1;
            }
        }

        Self {
            items,
            dangling_comments: pending_leading,
        }
    }
}
