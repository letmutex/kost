pub mod dump;

use smol_str::SmolStr;

/// Explicit identifier for dependent group layout decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub usize);

/// Type of indentation to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentKind {
    Block,
    Continuation,
}

/// Algebraic Document Intermediate Representation (Doc IR) for Kotlin code formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc<'a> {
    /// Borrowed text slice from the original source
    Text(&'a str),
    /// Small heap/inline string for synthesized tokens (operators, commas, keywords)
    Smol(SmolStr),
    /// Space when enclosing group is flat; newline when broken
    Line,
    /// Nothing when flat; newline when broken
    SoftLine,
    /// Unconditional newline; forces parent groups to break
    HardLine,
    /// Blank line (enforces at most one empty line between blocks)
    EmptyLine,
    /// Block indent (+block_indent spaces)
    Indent(Box<Doc<'a>>),
    /// Continuation indent (+continuation_indent spaces)
    ContinuationIndent(Box<Doc<'a>>),
    /// Layout group: rendered flat if it fits within max_width; broken otherwise
    Group {
        id: Option<GroupId>,
        doc: Box<Doc<'a>>,
    },
    /// Block-like RHS expression: tests only its opening line against max_width
    FitsFirstLine(Box<Doc<'a>>),
    /// Zero-clone conditional indent: adds indent ONLY when target group breaks
    IndentIfBreak {
        group_id: Option<GroupId>,
        kind: IndentKind,
        doc: Box<Doc<'a>>,
    },
    /// Conditional fragment based on whether the enclosing group is broken or flat
    IfBreak {
        group_id: Option<GroupId>,
        broken: Box<Doc<'a>>,
        flat: Box<Doc<'a>>,
    },
    /// Print-time line comment reflowed by Printer using exact runtime column
    LineComment {
        text: SmolStr,
        is_trailing: bool,
        align_to_start_col: bool,
    },
    /// Trailing inline comment or suffix string deferred to the end of the line
    LineSuffix(SmolStr),
    /// Preserves verbatim trailing whitespace on lines within this document
    PreserveTrailing(Box<Doc<'a>>),
    /// Sequence of document nodes
    Concat(Vec<Doc<'a>>),
}

impl<'a> Doc<'a> {
    #[inline]
    pub fn preserve_trailing(doc: Doc<'a>) -> Self {
        Doc::PreserveTrailing(Box::new(doc))
    }

    #[inline]
    pub fn text(s: &'a str) -> Self {
        Doc::Text(s)
    }

    #[inline]
    pub fn smol(s: impl Into<SmolStr>) -> Self {
        Doc::Smol(s.into())
    }

    #[inline]
    pub fn space() -> Self {
        Doc::Text(" ")
    }

    #[inline]
    pub fn empty() -> Self {
        Doc::Concat(Vec::new())
    }

    #[inline]
    pub fn group(doc: Doc<'a>) -> Self {
        Doc::Group {
            id: None,
            doc: Box::new(doc),
        }
    }

    #[inline]
    pub fn group_with_id(id: GroupId, doc: Doc<'a>) -> Self {
        Doc::Group {
            id: Some(id),
            doc: Box::new(doc),
        }
    }

    #[inline]
    pub fn fits_first_line(doc: Doc<'a>) -> Self {
        Doc::FitsFirstLine(Box::new(doc))
    }

    #[inline]
    pub fn indent(doc: Doc<'a>) -> Self {
        Doc::Indent(Box::new(doc))
    }

    #[inline]
    pub fn continuation_indent(doc: Doc<'a>) -> Self {
        Doc::ContinuationIndent(Box::new(doc))
    }

    #[inline]
    pub fn indent_if_break(kind: IndentKind, doc: Doc<'a>) -> Self {
        Doc::IndentIfBreak {
            group_id: None,
            kind,
            doc: Box::new(doc),
        }
    }

    #[inline]
    pub fn indent_if_group_breaks(id: GroupId, kind: IndentKind, doc: Doc<'a>) -> Self {
        Doc::IndentIfBreak {
            group_id: Some(id),
            kind,
            doc: Box::new(doc),
        }
    }

    #[inline]
    pub fn if_break(broken: Doc<'a>, flat: Doc<'a>) -> Self {
        Doc::IfBreak {
            group_id: None,
            broken: Box::new(broken),
            flat: Box::new(flat),
        }
    }

    #[inline]
    pub fn if_group_break(id: GroupId, broken: Doc<'a>, flat: Doc<'a>) -> Self {
        Doc::IfBreak {
            group_id: Some(id),
            broken: Box::new(broken),
            flat: Box::new(flat),
        }
    }

    #[inline]
    pub fn line_comment(
        text: impl Into<SmolStr>,
        is_trailing: bool,
        align_to_start_col: bool,
    ) -> Self {
        Doc::LineComment {
            text: text.into(),
            is_trailing,
            align_to_start_col,
        }
    }

    #[inline]
    pub fn line_suffix(s: impl Into<SmolStr>) -> Self {
        Doc::LineSuffix(s.into())
    }

    #[inline]
    pub fn concat(docs: Vec<Doc<'a>>) -> Self {
        Doc::Concat(docs)
    }

    pub fn has_hardline(&self) -> bool {
        match self {
            Doc::HardLine | Doc::EmptyLine | Doc::LineComment { .. } => true,
            Doc::Concat(docs) => docs.iter().any(|d| d.has_hardline()),
            Doc::Indent(inner)
            | Doc::ContinuationIndent(inner)
            | Doc::Group { doc: inner, .. }
            | Doc::FitsFirstLine(inner)
            | Doc::IndentIfBreak { doc: inner, .. } => inner.has_hardline(),
            Doc::IfBreak { flat, .. } => flat.has_hardline(),
            _ => false,
        }
    }

    pub fn ends_with_newline(&self) -> bool {
        match self {
            Doc::HardLine | Doc::EmptyLine | Doc::LineComment { .. } => true,
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_newline()),
            Doc::Indent(inner)
            | Doc::ContinuationIndent(inner)
            | Doc::Group { doc: inner, .. }
            | Doc::FitsFirstLine(inner)
            | Doc::IndentIfBreak { doc: inner, .. } => inner.ends_with_newline(),
            _ => false,
        }
    }

    pub fn ends_with_space(&self) -> bool {
        match self {
            Doc::Text(s) => s.ends_with(' '),
            Doc::Smol(s) => s.ends_with(' '),
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_space()),
            Doc::Indent(inner)
            | Doc::ContinuationIndent(inner)
            | Doc::Group { doc: inner, .. }
            | Doc::FitsFirstLine(inner)
            | Doc::IndentIfBreak { doc: inner, .. } => inner.ends_with_space(),
            _ => false,
        }
    }

    pub fn ends_with_brace(&self) -> bool {
        match self {
            Doc::Text(s) => s.ends_with('}'),
            Doc::Smol(s) => s.ends_with('}'),
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_brace()),
            Doc::Indent(inner)
            | Doc::ContinuationIndent(inner)
            | Doc::Group { doc: inner, .. }
            | Doc::FitsFirstLine(inner)
            | Doc::IndentIfBreak { doc: inner, .. } => inner.ends_with_brace(),
            _ => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Doc::Text(s) => s.is_empty(),
            Doc::Smol(s) => s.is_empty(),
            Doc::Concat(docs) => docs.iter().all(|d| d.is_empty()),
            Doc::Indent(inner)
            | Doc::ContinuationIndent(inner)
            | Doc::Group { doc: inner, .. }
            | Doc::FitsFirstLine(inner)
            | Doc::IndentIfBreak { doc: inner, .. } => inner.is_empty(),
            _ => false,
        }
    }

    pub fn dump(&self, indent: usize) -> String {
        dump::dump(self, indent)
    }
}
