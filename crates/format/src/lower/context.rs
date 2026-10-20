use ahash::AHashSet;

use crate::options::FormattingOptions;

pub struct LowerContext<'a, 'o> {
    pub source: &'a str,
    pub options: &'o FormattingOptions,
    pub redundant_semicolons: &'o AHashSet<usize>,
    pub used_references: &'o AHashSet<&'a str>,
    pub package_name: Option<&'o str>,
    pub leaf_override: Option<(usize, tree_sitter::Node<'a>)>,
}

impl<'a, 'o> LowerContext<'a, 'o> {
    pub fn new(
        source: &'a str,
        options: &'o FormattingOptions,
        redundant_semicolons: &'o AHashSet<usize>,
        used_references: &'o AHashSet<&'a str>,
        package_name: Option<&'o str>,
    ) -> Self {
        Self {
            source,
            options,
            redundant_semicolons,
            used_references,
            package_name,
            leaf_override: None,
        }
    }

    pub fn with_leaf_override(&self, elvis_id: usize, node: tree_sitter::Node<'a>) -> Self {
        Self {
            source: self.source,
            options: self.options,
            redundant_semicolons: self.redundant_semicolons,
            used_references: self.used_references,
            package_name: self.package_name,
            leaf_override: Some((elvis_id, node)),
        }
    }

    #[inline]
    pub fn resolve_leaf(&self, node: tree_sitter::Node<'a>) -> tree_sitter::Node<'a> {
        if let Some((id, over)) = self.leaf_override
            && id == node.id()
        {
            over
        } else {
            node
        }
    }

    #[inline]
    pub fn node_text(&self, node: &tree_sitter::Node<'_>) -> &'a str {
        self.source
            .get(node.start_byte()..node.end_byte())
            .unwrap_or("")
    }

    #[inline]
    pub fn is_redundant_semicolon(&self, node: &tree_sitter::Node<'_>) -> bool {
        self.redundant_semicolons.contains(&node.start_byte())
    }

    #[inline]
    pub fn has_blank_line_between(
        &self,
        prev: &tree_sitter::Node<'_>,
        next: &tree_sitter::Node<'_>,
    ) -> bool {
        let start = prev.end_byte();
        let end = next.start_byte();
        if start <= end
            && let Some(between) = self.source.get(start..end)
        {
            let mut count = 0;
            for b in between.bytes() {
                if b == b'\n' {
                    count += 1;
                    if count >= 2 {
                        return true;
                    }
                }
            }
            return false;
        }
        next.start_position()
            .row
            .saturating_sub(prev.end_position().row)
            >= 2
    }

    #[inline]
    pub fn has_newline_between(
        &self,
        prev: &tree_sitter::Node<'_>,
        next: &tree_sitter::Node<'_>,
    ) -> bool {
        let start = prev.end_byte();
        let end = next.start_byte();
        if start <= end
            && let Some(between) = self.source.get(start..end)
        {
            return between.bytes().any(|b| b == b'\n');
        }
        next.start_position().row > prev.end_position().row
    }

    pub fn compute_target_indent(&self, node: &tree_sitter::Node<'_>) -> usize {
        let mut indent_levels = 0;
        let mut curr = *node;
        while let Some(parent) = curr.parent() {
            match parent.kind() {
                "class_body" | "enum_class_body" | "statements" => {
                    indent_levels += 1;
                }
                "function_body" if curr.kind() != "block" && curr.kind() != "statements" => {
                    indent_levels += 1;
                }
                "block" => {
                    if curr.kind() != "statements" {
                        indent_levels += 1;
                    }
                }
                "control_structure_body" => {
                    if curr.kind() != "block"
                        && curr.kind() != "statements"
                        && curr.kind() != "if_expression"
                    {
                        indent_levels += 1;
                    }
                }
                "when_entry"
                    if curr.kind() != "block"
                        && curr.kind() != "statements"
                        && curr.kind() != "when_condition" =>
                {
                    indent_levels += 1;
                }
                "lambda_literal"
                    if curr.kind() != "statements"
                        && curr.kind() != "lambda_parameters"
                        && curr.kind() != "->"
                        && curr.kind() != "{"
                        && curr.kind() != "}" =>
                {
                    indent_levels += 1;
                }
                _ => {}
            }
            curr = parent;
        }
        indent_levels * self.options.block_indent
    }
}
