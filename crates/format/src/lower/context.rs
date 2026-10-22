use ahash::AHashSet;
use std::cell::Cell;
use tree_sitter::Node;

use crate::doc::GroupId;
use crate::options::FormattingOptions;
use crate::style::StyleProfile;

pub struct LowerContext<'a, 'b> {
    pub source: &'a str,
    pub options: &'b FormattingOptions,
    pub style: StyleProfile,
    group_counter: Cell<usize>,
    pub redundant_semicolons: Option<&'b AHashSet<usize>>,
    pub package_name: Option<&'a str>,
    pub used_references: Option<&'b AHashSet<&'a str>>,
}

impl<'a, 'b> LowerContext<'a, 'b> {
    pub fn new(source: &'a str, options: &'b FormattingOptions) -> Self {
        let style = StyleProfile::from_options(options);
        Self {
            source,
            options,
            style,
            group_counter: Cell::new(0),
            redundant_semicolons: None,
            package_name: None,
            used_references: None,
        }
    }

    pub fn with_redundant_semicolons(mut self, semicolons: &'b AHashSet<usize>) -> Self {
        self.redundant_semicolons = Some(semicolons);
        self
    }

    pub fn with_imports(
        mut self,
        package_name: Option<&'a str>,
        used_references: &'b AHashSet<&'a str>,
    ) -> Self {
        self.package_name = package_name;
        self.used_references = Some(used_references);
        self
    }

    pub fn next_group_id(&self) -> GroupId {
        let id = self.group_counter.get();
        self.group_counter.set(id + 1);
        GroupId(id)
    }

    #[inline]
    pub fn node_text(&self, node: &Node<'_>) -> &'a str {
        let start = node.start_byte();
        let end = node.end_byte();
        if start < end && end <= self.source.len() {
            &self.source[start..end]
        } else {
            ""
        }
    }

    pub fn has_user_newline_between(&self, a: &Node<'_>, b: &Node<'_>) -> bool {
        let start = a.end_byte();
        let end = b.start_byte();
        if start < end && end <= self.source.len() {
            self.source[start..end].contains('\n')
        } else {
            false
        }
    }

    pub fn has_blank_line_between(&self, a: &Node<'_>, b: &Node<'_>) -> bool {
        let start = a.end_byte();
        let end = b.start_byte();
        if start < end && end <= self.source.len() {
            self.source[start..end]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
                >= 2
        } else {
            false
        }
    }

    pub fn has_trailing_comma(&self, list_node: &Node<'_>) -> bool {
        let mut cursor = list_node.walk();
        let children: Vec<_> = list_node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            let kind = child.kind();
            if kind == ")" || kind == "]" || kind == ">" || kind == "}" {
                continue;
            }
            if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                continue;
            }
            return kind == ",";
        }
        false
    }

    pub fn is_redundant_semicolon(&self, node: &Node<'_>) -> bool {
        if let Some(set) = self.redundant_semicolons {
            set.contains(&node.start_byte())
        } else {
            false
        }
    }
}
