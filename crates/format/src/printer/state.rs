use smol_str::SmolStr;

use crate::doc::Doc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Flat,
    Broken,
}

#[derive(Debug, Clone)]
pub struct PrintItem<'d, 'a> {
    pub indent: usize,
    pub mode: Mode,
    pub doc: &'d Doc<'a>,
    pub preserve_trailing: bool,
}

pub struct PrintState {
    pub out: String,
    pub current_column: usize,
    pub pending_indent: Option<usize>,
    pub line_suffixes: Vec<SmolStr>,
    pub consecutive_newlines: usize,
    pub preserve_trailing: bool,
}

impl PrintState {
    pub fn new(capacity: usize) -> Self {
        Self {
            out: String::with_capacity(capacity.max(128)),
            current_column: 0,
            pending_indent: None,
            line_suffixes: Vec::new(),
            consecutive_newlines: 0,
            preserve_trailing: false,
        }
    }

    const SPACES: &'static str =
        "                                                                                ";

    pub fn flush_indent(&mut self) {
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

    pub fn push_str(&mut self, s: &str) {
        self.flush_indent();
        self.out.push_str(s);
        let len = if s.is_ascii() {
            s.len()
        } else {
            s.chars().count()
        };
        self.current_column += len;
        self.consecutive_newlines = 0;
    }

    pub fn push_char(&mut self, c: char) {
        self.flush_indent();
        self.out.push(c);
        self.current_column += 1;
        self.consecutive_newlines = 0;
    }

    pub fn effective_column(&self) -> usize {
        if self.current_column == 0
            && let Some(ind) = self.pending_indent
        {
            ind
        } else {
            self.current_column
        }
    }

    pub fn emit_newline(&mut self, next_indent: usize) {
        if !self.line_suffixes.is_empty() {
            let suffixes = std::mem::take(&mut self.line_suffixes);
            for s in suffixes {
                self.push_char(' ');
                self.push_str(&s);
            }
        }
        if !self.preserve_trailing {
            while self.out.ends_with(' ') || self.out.ends_with('\t') {
                self.out.pop();
            }
        }
        self.out.push('\n');
        self.current_column = 0;
        self.pending_indent = Some(next_indent);
        self.consecutive_newlines += 1;
    }

    pub fn emit_empty_line(&mut self, next_indent: usize) {
        while !self.out.is_empty() && self.consecutive_newlines < 2 {
            self.emit_newline(next_indent);
        }
        self.pending_indent = Some(next_indent);
    }
}
