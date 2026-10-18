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

    pub fn flat_len(&self) -> usize {
        match self {
            Doc::Text(s) => s.len(),
            Doc::Smol(s) => s.len(),
            Doc::Line => 1,
            Doc::SoftLine | Doc::HardLine | Doc::EmptyLine => 0,
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.flat_len()
            }
            Doc::IfBreak { flat, .. } => flat.flat_len(),
            Doc::LineSuffix(s) => s.len(),
            Doc::ReserveWidth(n) => *n,
            Doc::Concat(docs) => docs.iter().map(|d| d.flat_len()).sum(),
        }
    }

    pub fn last_line_len(&self) -> usize {
        match self {
            Doc::Text(s) => match s.rfind('\n') {
                Some(i) => s[i + 1..].len(),
                None => s.len(),
            },
            Doc::Smol(s) => match s.rfind('\n') {
                Some(i) => s[i + 1..].len(),
                None => s.len(),
            },
            Doc::HardLine | Doc::EmptyLine => 0,
            Doc::Line => 1,
            Doc::SoftLine => 0,
            Doc::ReserveWidth(_) => 0,
            Doc::LineSuffix(_) => 0,
            Doc::Indent(inner) | Doc::ContinuationIndent(inner) | Doc::Group(inner) => {
                inner.last_line_len()
            }
            Doc::IfBreak { broken, flat } => {
                if broken.has_hardline() {
                    broken.last_line_len()
                } else {
                    flat.last_line_len()
                }
            }
            Doc::Concat(docs) => {
                let mut len = 0;
                for d in docs.iter().rev() {
                    let has_break = match d {
                        Doc::IfBreak { broken, .. } => broken.has_hardline(),
                        other => other.has_hardline(),
                    };
                    if has_break {
                        len += d.last_line_len();
                        break;
                    }
                    let last = d.last_line_len();
                    let flat = d.flat_len();
                    if last != flat {
                        len += last;
                        break;
                    }
                    len += flat;
                }
                len
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_last_line_len() {
        assert_eq!(Doc::text("hello").last_line_len(), 5);
        assert_eq!(Doc::text("hello\nworld").last_line_len(), 5);
        assert_eq!(
            Doc::Concat(vec![Doc::text("first\nsecond")]).last_line_len(),
            6
        );
        let concat = Doc::Concat(vec![Doc::text("first"), Doc::HardLine, Doc::text("second")]);
        assert_eq!(concat.last_line_len(), 6);
        let if_break_concat = Doc::Concat(vec![
            Doc::text("prefix"),
            Doc::if_break(
                Doc::Concat(vec![Doc::HardLine, Doc::text("x")]),
                Doc::text("y"),
            ),
        ]);
        assert_eq!(if_break_concat.last_line_len(), 1);
    }

    #[test]
    fn test_doc_flat_len_variants() {
        assert_eq!(Doc::text("hello").flat_len(), 5);
        assert_eq!(Doc::Line.flat_len(), 1);
        assert_eq!(Doc::HardLine.flat_len(), 0);
        assert_eq!(Doc::SoftLine.flat_len(), 0);
        assert_eq!(Doc::EmptyLine.flat_len(), 0);
        assert_eq!(Doc::ReserveWidth(4).flat_len(), 4);
        assert_eq!(Doc::line_suffix(" // trailing").flat_len(), 12);

        let group = Doc::Group(Box::new(Doc::text("group")));
        assert_eq!(group.flat_len(), 5);

        let indent = Doc::indent(Doc::text("indent"));
        assert_eq!(indent.flat_len(), 6);

        let cont_indent = Doc::continuation_indent(Doc::text("cont"));
        assert_eq!(cont_indent.flat_len(), 4);

        let if_break = Doc::if_break(Doc::text("broken_branch"), Doc::text("flat_branch"));
        assert_eq!(if_break.flat_len(), 11);

        let concat = Doc::Concat(vec![
            Doc::text("val"),
            Doc::Line,
            Doc::text("x"),
            Doc::space(),
            Doc::text("="),
            Doc::space(),
            Doc::text("1"),
        ]);
        assert_eq!(concat.flat_len(), 9);
    }
}
