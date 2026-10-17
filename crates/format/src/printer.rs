use smol_str::SmolStr;

use crate::doc::Doc;
use crate::options::FormattingOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Broken,
}

struct PrintItem<'d, 'a> {
    indent: usize,
    mode: Mode,
    doc: &'d Doc<'a>,
}

fn account_text_span(s: &str, mut max_col: isize, _max_width: usize) -> Option<isize> {
    if s.contains('\n') {
        return None;
    }
    let len = if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    };
    max_col -= len as isize;
    if max_col < 0 { None } else { Some(max_col) }
}

fn fits<'d, 'a>(
    doc: &'d Doc<'a>,
    mut max_col: isize,
    max_width: usize,
    start_indent: usize,
    block_indent: usize,
    continuation_indent: usize,
    scratch: &mut Vec<(&'d Doc<'a>, usize)>,
) -> bool {
    scratch.clear();
    scratch.push((doc, start_indent));
    while let Some((item, current_indent)) = scratch.pop() {
        if max_col < 0 {
            return false;
        }
        match item {
            Doc::Text(s) => match account_text_span(s, max_col, max_width) {
                Some(rem) => max_col = rem,
                None => return false,
            },
            Doc::Smol(s) => match account_text_span(s, max_col, max_width) {
                Some(rem) => max_col = rem,
                None => return false,
            },
            Doc::Line => max_col -= 1,
            Doc::SoftLine => {}
            Doc::HardLine | Doc::EmptyLine => return false,
            Doc::Indent(inner) => {
                scratch.push((inner.as_ref(), current_indent + block_indent));
            }
            Doc::ContinuationIndent(inner) => {
                scratch.push((inner.as_ref(), current_indent + continuation_indent));
            }
            Doc::Group(inner) => {
                scratch.push((inner.as_ref(), current_indent));
            }
            Doc::IfBreak { flat, .. } => {
                scratch.push((flat.as_ref(), current_indent));
            }
            Doc::LineSuffix(s) => {
                let len = if s.is_ascii() {
                    s.len()
                } else {
                    s.chars().count()
                };
                max_col -= len as isize;
            }
            Doc::ReserveWidth(n) => {
                max_col -= *n as isize;
            }
            Doc::Concat(items) => {
                for it in items.iter().rev() {
                    scratch.push((it, current_indent));
                }
            }
        }
    }
    max_col >= 0
}

struct PrintState {
    out: String,
    current_column: usize,
    pending_indent: Option<usize>,
    line_suffixes: Vec<SmolStr>,
    consecutive_newlines: usize,
}

impl PrintState {
    fn new(capacity: usize) -> Self {
        Self {
            out: String::with_capacity(capacity.max(128)),
            current_column: 0,
            pending_indent: None,
            line_suffixes: Vec::new(),
            consecutive_newlines: 0,
        }
    }
    const SPACES: &'static str =
        "                                                                                ";

    fn flush_indent(&mut self) {
        if let Some(indent) = self.pending_indent.take()
            && indent > 0
        {
            if indent <= Self::SPACES.len() {
                self.out.push_str(&Self::SPACES[..indent]);
            } else {
                self.out.reserve(indent);
                for _ in 0..indent {
                    self.out.push(' ');
                }
            }
            self.current_column = indent;
        }
    }

    fn emit_newline(&mut self, next_indent: usize) {
        self.pending_indent = None;
        if !self.line_suffixes.is_empty() {
            for suffix in self.line_suffixes.drain(..) {
                if !self.out.ends_with(' ') {
                    self.out.push(' ');
                }
                self.out.push_str(suffix.as_str());
            }
        }
        self.out.push('\n');
        self.current_column = 0;
        self.pending_indent = Some(next_indent);
        self.consecutive_newlines += 1;
    }

    fn append_text(&mut self, s: &str) {
        self.flush_indent();
        self.consecutive_newlines = 0;
        self.out.push_str(s);
        if s.contains('\n') {
            let last_nl = s.rfind('\n').unwrap();
            let tail = &s[last_nl + 1..];
            self.current_column = if tail.is_ascii() {
                tail.len()
            } else {
                tail.chars().count()
            };
        } else {
            self.current_column += if s.is_ascii() {
                s.len()
            } else {
                s.chars().count()
            };
        }
    }
}

pub struct Printer<'o> {
    options: &'o FormattingOptions,
}

impl<'o> Printer<'o> {
    pub fn new(options: &'o FormattingOptions) -> Self {
        Self { options }
    }

    fn handle_line_break(&self, state: &mut PrintState, mode: Mode, indent: usize) {
        if mode == Mode::Flat {
            state.flush_indent();
            state.consecutive_newlines = 0;
            state.out.push(' ');
            state.current_column += 1;
        } else {
            state.emit_newline(indent);
        }
    }

    fn handle_group<'d, 'a>(
        &self,
        inner: &'d Doc<'a>,
        state: &PrintState,
        mode: Mode,
        indent: usize,
        stack: &mut Vec<PrintItem<'d, 'a>>,
        scratch: &mut Vec<(&'d Doc<'a>, usize)>,
    ) {
        let next_mode = if mode == Mode::Flat {
            Mode::Flat
        } else {
            let effective_col = state.pending_indent.unwrap_or(state.current_column);
            let remaining = (self.options.max_width as isize) - (effective_col as isize);
            if remaining > 0
                && fits(
                    inner,
                    remaining,
                    self.options.max_width,
                    indent,
                    self.options.block_indent,
                    self.options.continuation_indent,
                    scratch,
                )
            {
                Mode::Flat
            } else {
                Mode::Broken
            }
        };
        stack.push(PrintItem {
            indent,
            mode: next_mode,
            doc: inner,
        });
    }

    fn print_item<'d, 'a>(
        &self,
        item: PrintItem<'d, 'a>,
        state: &mut PrintState,
        stack: &mut Vec<PrintItem<'d, 'a>>,
        scratch: &mut Vec<(&'d Doc<'a>, usize)>,
    ) {
        match item.doc {
            Doc::Text(s) => state.append_text(s),
            Doc::Smol(s) => state.append_text(s.as_str()),
            Doc::Line => self.handle_line_break(state, item.mode, item.indent),
            Doc::SoftLine => {
                if item.mode != Mode::Flat {
                    if state.consecutive_newlines > 0 {
                        state.pending_indent = Some(item.indent);
                    } else {
                        state.emit_newline(item.indent);
                    }
                }
            }
            Doc::HardLine => {
                if state.consecutive_newlines > 0 {
                    state.pending_indent = Some(item.indent);
                } else {
                    state.emit_newline(item.indent);
                }
            }
            Doc::EmptyLine => {
                while !state.out.is_empty() && state.consecutive_newlines < 2 {
                    state.emit_newline(item.indent);
                }
            }
            Doc::Indent(inner) => stack.push(PrintItem {
                indent: item.indent + self.options.block_indent,
                mode: item.mode,
                doc: inner.as_ref(),
            }),
            Doc::ContinuationIndent(inner) => stack.push(PrintItem {
                indent: item.indent + self.options.continuation_indent,
                mode: item.mode,
                doc: inner.as_ref(),
            }),
            Doc::Group(inner) => self.handle_group(
                inner.as_ref(),
                state,
                item.mode,
                item.indent,
                stack,
                scratch,
            ),
            Doc::IfBreak { broken, flat } => {
                let target = if item.mode == Mode::Flat {
                    flat
                } else {
                    broken
                };
                stack.push(PrintItem {
                    indent: item.indent,
                    mode: item.mode,
                    doc: target.as_ref(),
                });
            }
            Doc::LineSuffix(s) => state.line_suffixes.push(s.clone()),
            Doc::ReserveWidth(_) => {}
            Doc::Concat(items) => {
                for it in items.iter().rev() {
                    stack.push(PrintItem {
                        indent: item.indent,
                        mode: item.mode,
                        doc: it,
                    });
                }
            }
        }
    }

    pub fn print<'d, 'a>(&self, doc: &'d Doc<'a>, capacity_hint: usize) -> String {
        let mut state = PrintState::new(capacity_hint);
        let mut scratch = Vec::with_capacity(32);
        let mut stack = vec![PrintItem {
            indent: 0,
            mode: Mode::Broken,
            doc,
        }];

        while let Some(item) = stack.pop() {
            self.print_item(item, &mut state, &mut stack, &mut scratch);
        }

        if !state.line_suffixes.is_empty() {
            for suffix in state.line_suffixes.drain(..) {
                if !state.out.ends_with(' ') {
                    state.out.push(' ');
                }
                state.out.push_str(suffix.as_str());
            }
        }

        if !state.out.is_empty() && !state.out.ends_with('\n') {
            state.out.push('\n');
        }

        state.out
    }
}
