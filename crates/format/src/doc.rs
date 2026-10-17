use smol_str::SmolStr;

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
    Group(Box<Doc<'a>>),
    /// Conditional fragment based on whether the enclosing group is broken or flat
    IfBreak {
        broken: Box<Doc<'a>>,
        flat: Box<Doc<'a>>,
    },
    /// Trailing inline comment deferred to the end of the line
    LineSuffix(SmolStr),
    /// Lookahead width reservation (deducted in flat-fit lookahead, no-op when emitted)
    ReserveWidth(usize),
    /// Sequence of document nodes
    Concat(Vec<Doc<'a>>),
}

impl<'a> Doc<'a> {
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
    pub fn reserve_width(n: usize) -> Self {
        Doc::ReserveWidth(n)
    }

    #[inline]
    pub fn group(doc: Doc<'a>) -> Self {
        Doc::Group(Box::new(doc))
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
    pub fn if_break(broken: Doc<'a>, flat: Doc<'a>) -> Self {
        Doc::IfBreak {
            broken: Box::new(broken),
            flat: Box::new(flat),
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
            Doc::HardLine | Doc::EmptyLine => true,
            Doc::Concat(docs) => docs.iter().any(|d| d.has_hardline()),
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.has_hardline()
            }
            Doc::IfBreak { flat, .. } => flat.has_hardline(),
            _ => false,
        }
    }

    pub fn ends_with_newline(&self) -> bool {
        match self {
            Doc::HardLine | Doc::EmptyLine => true,
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_newline()),
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.ends_with_newline()
            }
            _ => false,
        }
    }

    pub fn ends_with_space(&self) -> bool {
        match self {
            Doc::Text(s) => s.ends_with(' '),
            Doc::Smol(s) => s.ends_with(' '),
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_space()),
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.ends_with_space()
            }
            _ => false,
        }
    }

    pub fn ends_with_line(&self) -> bool {
        match self {
            Doc::Line | Doc::HardLine | Doc::EmptyLine | Doc::SoftLine => true,
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_line()),
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.ends_with_line()
            }
            _ => false,
        }
    }

    pub fn ends_with_brace(&self) -> bool {
        match self {
            Doc::Text(s) => s.ends_with('}'),
            Doc::Smol(s) => s.ends_with('}'),
            Doc::Concat(docs) => docs.last().is_some_and(|d| d.ends_with_brace()),
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.ends_with_brace()
            }
            Doc::IfBreak { broken, flat } => broken.ends_with_brace() || flat.ends_with_brace(),
            _ => false,
        }
    }
}
