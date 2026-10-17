use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::trace::Tracer;
use crate::indexer::WorkspaceIndex;

pub struct InferenceContext<'a, 't, 'tr> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub depth: usize,
    pub tracer: &'t mut Tracer<'tr>,
}

impl<'a, 't, 'tr> InferenceContext<'a, 't, 'tr> {
    pub fn new(
        index: &'a WorkspaceIndex,
        current_url: &'a Url,
        pos: Position,
        source: &'a str,
        root: Option<Node<'a>>,
        depth: usize,
        tracer: &'t mut Tracer<'tr>,
    ) -> Self {
        Self {
            index,
            current_url,
            pos,
            source,
            root,
            depth,
            tracer,
        }
    }

    pub fn sub<'s>(&'s mut self) -> InferenceContext<'a, 's, 'tr> {
        InferenceContext {
            index: self.index,
            current_url: self.current_url,
            pos: self.pos,
            source: self.source,
            root: self.root,
            depth: self.depth + 1,
            tracer: self.tracer,
        }
    }

    pub fn sub_with_pos<'s>(&'s mut self, pos: Position) -> InferenceContext<'a, 's, 'tr> {
        InferenceContext {
            index: self.index,
            current_url: self.current_url,
            pos,
            source: self.source,
            root: self.root,
            depth: self.depth + 1,
            tracer: self.tracer,
        }
    }

    pub fn at_pos(mut self, pos: Position) -> Self {
        self.pos = pos;
        self
    }

    pub fn reborrow<'s>(&'s mut self) -> InferenceContext<'a, 's, 'tr> {
        InferenceContext {
            index: self.index,
            current_url: self.current_url,
            pos: self.pos,
            source: self.source,
            root: self.root,
            depth: self.depth,
            tracer: self.tracer,
        }
    }
}
