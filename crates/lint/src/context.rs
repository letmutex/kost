//! Traversal context and lexical scope tracking during AST walk.

use crate::condition::{PropertyBag, PropertyLookup};
use crate::registry::RuleRegistry;
use crate::suppression::SuppressionLocator;
use crate::types::{
    CommentType, Diagnostic, PropertyKey, PropertyValue, Scope, Severity, TextRange,
};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// A frame on the scope stack representing an open lexical boundary.
#[derive(Debug, Clone)]
pub struct ScopeFrame {
    pub scope: Scope,
    pub range: TextRange,
    pub properties: PropertyBag,
    pub property_ranges: SmallVec<[(PropertyKey, TextRange); 2]>,
}

impl ScopeFrame {
    pub fn new(scope: Scope, range: TextRange) -> Self {
        Self {
            scope,
            range,
            properties: PropertyBag::new(),
            property_ranges: SmallVec::new(),
        }
    }

    pub fn get_property_range(&self, key: PropertyKey) -> Option<TextRange> {
        self.property_ranges
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, range)| *range)
    }
}

/// Tracks nested lexical scopes during the single-pass AST traversal.
#[derive(Debug, Default)]
pub struct ScopeTracker {
    pub stack: Vec<ScopeFrame>,
}

impl ScopeTracker {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, scope: Scope, range: TextRange) {
        self.stack.push(ScopeFrame::new(scope, range));
    }

    pub fn pop(&mut self) -> Option<ScopeFrame> {
        self.stack.pop()
    }

    pub fn current_mut(&mut self) -> Option<&mut ScopeFrame> {
        self.stack.last_mut()
    }

    pub fn current(&self) -> Option<&ScopeFrame> {
        self.stack.last()
    }

    pub fn insert_property(&mut self, key: PropertyKey, value: PropertyValue) {
        if let Some(frame) = self.stack.last_mut() {
            frame.properties.insert(key, value);
        }
    }
}

/// Resolves properties across the current scope and enclosing ancestor scopes on the stack.
pub struct StackPropertyResolver<'a> {
    pub current: &'a ScopeFrame,
    pub tracker: &'a ScopeTracker,
}

impl<'a> PropertyLookup for StackPropertyResolver<'a> {
    #[inline]
    fn get_property(&self, key: PropertyKey) -> Option<&PropertyValue> {
        if key.scope() == self.current.scope {
            return self.current.properties.get(key);
        }
        for frame in self.tracker.stack.iter().rev() {
            if frame.scope == key.scope() {
                return frame.properties.get(key);
            }
        }
        None
    }
}

/// Traversal execution context holding source bytes, active rules, and collected diagnostics.
pub struct LintCtx<'a> {
    pub source: &'a [u8],
    pub registry: &'a RuleRegistry,
    pub suppression_locator: &'a SuppressionLocator,
    pub scope_tracker: ScopeTracker,
    pub diagnostics: Vec<Diagnostic>,
    pub active_rule_index: usize,
    pub active_severity: Severity,
    pub active_ignores_suppressions: bool,
    pub max_indent_spaces: i64,
    pub max_indent_range: Option<TextRange>,
    pub file_functions_count: i64,
    pub last_indent_row: isize,
    pub file_path: Option<SmolStr>,
    pub multiline_string_ranges: Vec<(usize, usize)>,
    pub multiline_string_and_comment_ranges: Vec<(usize, usize)>,
    pub max_line_length_exclusions: Vec<(usize, usize)>,
    pub all_string_and_comment_ranges: Vec<(usize, usize)>,
    pub in_string_literal: bool,
    pub last_comment: Option<(CommentType, usize, usize)>,
    pub has_test_import: Option<bool>,
    pub ancestors: Vec<tree_sitter::Node<'a>>,
    pub prev_sibling: Option<tree_sitter::Node<'a>>,
    pub editorconfig: Option<&'a common::editorconfig::ResolvedEditorConfig>,
    pub disabled_ast_rules: [u64; 4],
}

impl<'a> LintCtx<'a> {
    pub fn new(
        source: &'a [u8],
        registry: &'a RuleRegistry,
        suppression_locator: &'a SuppressionLocator,
    ) -> Self {
        Self {
            source,
            registry,
            suppression_locator,
            scope_tracker: ScopeTracker::new(),
            diagnostics: Vec::new(),
            active_rule_index: 0,
            active_severity: Severity::Warning,
            active_ignores_suppressions: false,
            max_indent_spaces: 0,
            max_indent_range: None,
            file_functions_count: 0,
            last_indent_row: -1,
            file_path: None,
            multiline_string_ranges: Vec::new(),
            multiline_string_and_comment_ranges: Vec::new(),
            max_line_length_exclusions: Vec::new(),
            all_string_and_comment_ranges: Vec::new(),
            in_string_literal: false,
            last_comment: None,
            has_test_import: None,
            ancestors: Vec::with_capacity(64),
            prev_sibling: None,
            editorconfig: None,
            disabled_ast_rules: [0u64; 4],
        }
    }

    pub fn with_file_path(mut self, file_path: Option<&str>) -> Self {
        self.file_path = file_path.map(SmolStr::new);
        self
    }

    pub fn with_editorconfig(
        mut self,
        editorconfig: Option<&'a common::editorconfig::ResolvedEditorConfig>,
    ) -> Self {
        self.editorconfig = editorconfig;
        if let Some(config) = editorconfig {
            for (index, rule) in self.registry.ast_rules.iter().enumerate() {
                if !config.is_rule_enabled(rule.id(), true) {
                    self.set_rule_disabled(index, true);
                }
            }
        }
        self
    }

    #[inline]
    pub fn is_rule_disabled(&self, index: usize) -> bool {
        debug_assert!(index < 256, "rule index exceeds bitset capacity");
        let word_idx = index / 64;
        let bit_idx = index % 64;
        word_idx < 4 && (self.disabled_ast_rules[word_idx] & (1 << bit_idx)) != 0
    }

    #[inline]
    pub fn set_rule_disabled(&mut self, index: usize, disabled: bool) {
        debug_assert!(index < 256, "rule index exceeds bitset capacity");
        let word_idx = index / 64;
        let bit_idx = index % 64;
        if word_idx < 4 {
            if disabled {
                self.disabled_ast_rules[word_idx] |= 1 << bit_idx;
            } else {
                self.disabled_ast_rules[word_idx] &= !(1 << bit_idx);
            }
        }
    }

    #[inline]
    pub fn trim_trailing_whitespace(&self) -> bool {
        self.editorconfig
            .and_then(|ec| ec.trim_trailing_whitespace)
            .unwrap_or(true)
    }

    #[inline]
    pub fn indent_style(&self) -> common::editorconfig::IndentStyle {
        self.editorconfig
            .and_then(|ec| ec.indent_style)
            .unwrap_or(common::editorconfig::IndentStyle::Space)
    }

    #[inline]
    pub fn max_line_length(&self) -> Option<common::editorconfig::LineLengthSetting> {
        self.editorconfig.and_then(|ec| ec.max_line_length)
    }

    #[inline]
    pub fn insert_final_newline(&self) -> bool {
        self.editorconfig
            .and_then(|ec| ec.insert_final_newline)
            .unwrap_or(true)
    }

    #[inline]
    pub fn active_report_id(&self) -> &SmolStr {
        &self.registry.ast_report_ids[self.active_rule_index]
    }

    /// Returns the immediate parent node in the active AST traversal stack.
    #[inline]
    pub fn parent(&self) -> Option<tree_sitter::Node<'a>> {
        let len = self.ancestors.len();
        if len >= 2 {
            Some(self.ancestors[len - 2])
        } else {
            None
        }
    }

    /// Returns the ancestor node at the given level up the active AST traversal stack:
    /// - `0`: current node (equivalent to `ancestors.last()`)
    /// - `1`: immediate parent (equivalent to `ctx.parent()`)
    /// - `2`: grandparent
    #[inline]
    pub fn ancestor(&self, levels_up: usize) -> Option<tree_sitter::Node<'a>> {
        let len = self.ancestors.len();
        if len > levels_up {
            Some(self.ancestors[len - 1 - levels_up])
        } else {
            None
        }
    }

    /// Returns the sibling immediately preceding the current node in its parent's child list.
    #[inline]
    pub fn prev_sibling(&self) -> Option<tree_sitter::Node<'a>> {
        self.prev_sibling
    }

    /// Reports a violation on the full span of an AST node.
    #[inline]
    pub fn report(&mut self, node: tree_sitter::Node, message: impl Into<String>) {
        self.report_range(TextRange::from_node(node), message);
    }

    /// Reports a violation on a precise text span.
    #[inline]
    pub fn report_range(&mut self, range: TextRange, message: impl Into<String>) {
        if self.active_severity == Severity::None {
            return;
        }
        let Some(report_id) = self.registry.ast_report_ids.get(self.active_rule_index) else {
            return;
        };
        if !self.active_ignores_suppressions
            && self.suppression_locator.is_suppressed(report_id, &range)
        {
            return;
        }
        self.diagnostics.push(Diagnostic {
            rule_id: report_id.clone(),
            severity: self.active_severity,
            range,
            message: message.into(),
        });
    }
}
