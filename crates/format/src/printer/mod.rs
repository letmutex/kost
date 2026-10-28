pub mod fits;
pub mod state;

use ahash::AHashMap;

use crate::doc::{Doc, GroupId, IndentKind};
use crate::options::FormattingOptions;
use crate::rules::comments::normalize_line_comment;
use fits::{FitItem, fits};
pub use state::Mode;
use state::{PrintItem, PrintState};

pub struct Printer<'a> {
    options: &'a FormattingOptions,
    group_modes: AHashMap<GroupId, Mode>,
    debug_trace: bool,
}

impl<'a> Printer<'a> {
    pub fn new(options: &'a FormattingOptions) -> Self {
        let debug_trace = std::env::var("KOST_DEBUG_DOC").is_ok();
        Self {
            options,
            group_modes: AHashMap::default(),
            debug_trace,
        }
    }

    pub fn print<'d>(&mut self, doc: &'d Doc<'a>) -> String {
        self.print_with_capacity(doc, 1024)
    }

    pub fn print_with_capacity<'d>(&mut self, doc: &'d Doc<'a>, capacity: usize) -> String {
        let mut state = PrintState::new(capacity);
        let mut stack = Vec::with_capacity(64);
        let mut scratch = Vec::with_capacity(64);

        stack.push(PrintItem {
            indent: 0,
            mode: Mode::Broken,
            doc,
            preserve_trailing: false,
        });

        while let Some(item) = stack.pop() {
            self.print_item(item, &mut stack, &mut state, &mut scratch);
        }

        if !state.line_suffixes.is_empty() {
            let suffixes = std::mem::take(&mut state.line_suffixes);
            for s in suffixes {
                state.push_char(' ');
                state.push_str(&s);
            }
        }

        while state.out.ends_with(' ') || state.out.ends_with('\t') {
            state.out.pop();
        }

        if self.options.insert_final_newline && !state.out.ends_with('\n') {
            state.out.push('\n');
        }

        state.out
    }

    fn print_item<'d>(
        &mut self,
        item: PrintItem<'d, 'a>,
        stack: &mut Vec<PrintItem<'d, 'a>>,
        state: &mut PrintState,
        scratch: &mut Vec<FitItem<'d, 'a>>,
    ) {
        state.preserve_trailing = item.preserve_trailing;
        match item.doc {
            Doc::Text(s) => state.push_str(s),
            Doc::Smol(s) => state.push_str(s.as_str()),
            Doc::Line => {
                if item.mode == Mode::Flat {
                    state.push_char(' ');
                } else if state.consecutive_newlines > 0 {
                    state.pending_indent = Some(item.indent);
                } else {
                    state.emit_newline(item.indent);
                }
            }
            Doc::SoftLine => {
                if item.mode == Mode::Broken {
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
            Doc::EmptyLine => state.emit_empty_line(item.indent),
            Doc::Group { id, doc: inner } => {
                self.handle_group(*id, inner.as_ref(), item.indent, stack, state, scratch);
            }
            Doc::LineComment {
                text,
                is_trailing,
                align_to_start_col,
            } => {
                self.print_line_comment(
                    text.as_str(),
                    *is_trailing,
                    *align_to_start_col,
                    item.indent,
                    state,
                );
            }
            Doc::LineSuffix(s) => state.line_suffixes.push(s.clone()),
            _ => self.print_composite_item(item, stack),
        }
    }

    fn resolve_indent_if_break(
        &self,
        group_id: Option<GroupId>,
        kind: IndentKind,
        item_mode: Mode,
    ) -> usize {
        let is_broken = group_id
            .and_then(|gid| self.group_modes.get(&gid).copied())
            .unwrap_or(item_mode)
            == Mode::Broken;
        if is_broken {
            match kind {
                IndentKind::Block => self.options.block_indent,
                IndentKind::Continuation => self.options.continuation_indent,
            }
        } else {
            0
        }
    }

    fn print_composite_item<'d>(
        &self,
        item: PrintItem<'d, 'a>,
        stack: &mut Vec<PrintItem<'d, 'a>>,
    ) {
        match item.doc {
            Doc::Indent(inner) => stack.push(PrintItem {
                indent: item.indent + self.options.block_indent,
                mode: item.mode,
                doc: inner.as_ref(),
                preserve_trailing: item.preserve_trailing,
            }),
            Doc::ContinuationIndent(inner) => stack.push(PrintItem {
                indent: item.indent + self.options.continuation_indent,
                mode: item.mode,
                doc: inner.as_ref(),
                preserve_trailing: item.preserve_trailing,
            }),
            Doc::FitsFirstLine(inner) => stack.push(PrintItem {
                indent: item.indent,
                mode: Mode::Broken,
                doc: inner.as_ref(),
                preserve_trailing: item.preserve_trailing,
            }),
            Doc::PreserveTrailing(inner) => stack.push(PrintItem {
                indent: item.indent,
                mode: item.mode,
                doc: inner.as_ref(),
                preserve_trailing: true,
            }),
            Doc::IndentIfBreak {
                group_id,
                kind,
                doc: inner,
            } => {
                let extra = self.resolve_indent_if_break(*group_id, *kind, item.mode);
                stack.push(PrintItem {
                    indent: item.indent + extra,
                    mode: item.mode,
                    doc: inner.as_ref(),
                    preserve_trailing: item.preserve_trailing,
                });
            }
            Doc::IfBreak {
                group_id,
                broken,
                flat,
            } => {
                let is_broken = group_id
                    .and_then(|gid| self.group_modes.get(&gid).copied())
                    .unwrap_or(item.mode)
                    == Mode::Broken;
                let next_doc = if is_broken {
                    broken.as_ref()
                } else {
                    flat.as_ref()
                };
                stack.push(PrintItem {
                    indent: item.indent,
                    mode: item.mode,
                    doc: next_doc,
                    preserve_trailing: item.preserve_trailing,
                });
            }
            Doc::Concat(items) => {
                for it in items.iter().rev() {
                    stack.push(PrintItem {
                        indent: item.indent,
                        mode: item.mode,
                        doc: it,
                        preserve_trailing: item.preserve_trailing,
                    });
                }
            }
            _ => {}
        }
    }

    fn handle_group<'d>(
        &mut self,
        id: Option<GroupId>,
        inner: &'d Doc<'a>,
        indent: usize,
        stack: &mut Vec<PrintItem<'d, 'a>>,
        state: &PrintState,
        scratch: &mut Vec<FitItem<'d, 'a>>,
    ) {
        let col = state.effective_column();
        let remaining = (self.options.max_width as isize) - (col as isize);
        let is_flat = remaining >= 0
            && fits(
                inner,
                stack,
                &self.group_modes,
                remaining,
                indent,
                self.options,
                scratch,
            );
        let mode = if is_flat { Mode::Flat } else { Mode::Broken };
        if let Some(gid) = id {
            self.group_modes.insert(gid, mode);
        }
        if self.debug_trace {
            eprintln!(
                "Group{:?} mode={:?} at col={} remaining={}",
                id, mode, state.current_column, remaining
            );
        }
        stack.push(PrintItem {
            indent,
            mode,
            doc: inner,
            preserve_trailing: state.preserve_trailing,
        });
    }

    fn print_line_comment(
        &self,
        text: &str,
        is_trailing: bool,
        align_to_start_col: bool,
        indent: usize,
        state: &mut PrintState,
    ) {
        let start_col = if is_trailing {
            if state.out.ends_with(' ') {
                state.current_column
            } else {
                state.current_column + 1
            }
        } else {
            indent
        };
        let cont_indent = if align_to_start_col {
            start_col
        } else {
            indent
        };
        let lines = normalize_line_comment(text, start_col, cont_indent, self.options.max_width);

        if is_trailing && !state.out.ends_with(' ') {
            state.push_char(' ');
        }
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                state.emit_newline(cont_indent);
            }
            state.push_str(line.as_str());
        }
        state.emit_newline(indent);
    }
}
