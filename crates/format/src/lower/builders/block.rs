use crate::doc::Doc;

pub struct BlockItem<'a> {
    pub doc: Doc<'a>,
    pub same_line_comment: Option<Doc<'a>>,
    pub blank_line_before: bool,
}

impl<'a> BlockItem<'a> {
    pub fn new(doc: Doc<'a>) -> Self {
        Self {
            doc,
            same_line_comment: None,
            blank_line_before: false,
        }
    }

    pub fn with_comment(mut self, comment: Doc<'a>) -> Self {
        self.same_line_comment = Some(comment);
        self
    }

    pub fn with_blank_line_before(mut self, blank_line: bool) -> Self {
        self.blank_line_before = blank_line;
        self
    }
}

pub fn format_braced_block<'a>(
    items: Vec<BlockItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
) -> Doc<'a> {
    format_braced_block_ext(items, dangling_comments, None, false)
}

pub fn format_braced_block_with_blank_at_end<'a>(
    items: Vec<BlockItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
    blank_at_end: bool,
) -> Doc<'a> {
    format_braced_block_ext(items, dangling_comments, None, blank_at_end)
}

pub fn format_braced_block_ext<'a>(
    items: Vec<BlockItem<'a>>,
    dangling_comments: Vec<Doc<'a>>,
    opening_comment: Option<Doc<'a>>,
    blank_at_end: bool,
) -> Doc<'a> {
    if items.is_empty() && dangling_comments.is_empty() && opening_comment.is_none() {
        return Doc::concat(vec![Doc::text("{"), Doc::text("}")]);
    }

    let mut inner = Vec::new();

    for (i, item) in items.into_iter().enumerate() {
        if i == 0 {
            if item.blank_line_before {
                inner.push(Doc::EmptyLine);
            } else {
                inner.push(Doc::HardLine);
            }
        } else if item.blank_line_before {
            inner.push(Doc::EmptyLine);
        } else {
            inner.push(Doc::HardLine);
        }

        inner.push(item.doc);
        if let Some(comment) = item.same_line_comment {
            inner.push(comment);
        }
    }

    for comment in dangling_comments {
        inner.push(Doc::HardLine);
        inner.push(comment);
    }

    let close_line = if blank_at_end {
        Doc::EmptyLine
    } else {
        Doc::HardLine
    };

    let mut result = vec![Doc::text("{")];
    if let Some(oc) = opening_comment {
        result.push(Doc::space());
        result.push(oc);
    }
    result.push(Doc::indent(Doc::concat(inner)));
    result.push(close_line);
    result.push(Doc::text("}"));

    Doc::concat(result)
}
