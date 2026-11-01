use common::color::{DIM, wrap_styled};
use std::path::Path;

use crate::graph::{GraphNode, SolvedGraph};

/// Renders a SolvedGraph into an ASCII tree hierarchy.
pub struct TreeRenderer;

impl TreeRenderer {
    pub fn render_tree(
        graph: &SolvedGraph,
        project_path: Option<&Path>,
        depth_limit: Option<usize>,
        filter: Option<&str>,
    ) -> String {
        Self::render_tree_styled(
            graph,
            project_path,
            depth_limit,
            filter,
            common::should_colorize_stdout(),
        )
    }

    pub fn render_tree_styled(
        graph: &SolvedGraph,
        project_path: Option<&Path>,
        depth_limit: Option<usize>,
        filter: Option<&str>,
        colorize: bool,
    ) -> String {
        let matching_subtrees: Option<Vec<bool>> = filter.map(|query| {
            let lower_query = query.to_ascii_lowercase();
            (0..graph.nodes.len())
                .map(|node_idx| subtree_matches(graph, node_idx, &lower_query))
                .collect()
        });

        let direct_indices: Vec<usize> = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.is_direct)
            .filter(|&(node_idx, _)| {
                matching_subtrees
                    .as_ref()
                    .is_none_or(|matches| matches[node_idx])
            })
            .map(|(node_idx, _)| node_idx)
            .collect();

        if direct_indices.is_empty() {
            return String::new();
        }

        let mut buffer = String::new();
        let path_str = if let Some(path) = project_path {
            if path.as_os_str().is_empty() {
                ".".to_string()
            } else {
                path.display().to_string()
            }
        } else {
            ".".to_string()
        };
        buffer.push_str(&path_str);
        buffer.push('\n');

        let mut ctx = RenderContext {
            graph,
            depth_limit,
            matching_subtrees: matching_subtrees.as_deref(),
            colorize,
            printed: ahash::AHashSet::default(),
            active_path: ahash::AHashSet::default(),
        };

        let total_direct = direct_indices.len();
        for (idx, &node_idx) in direct_indices.iter().enumerate() {
            let is_last = idx + 1 == total_direct;
            ctx.render_node(&mut buffer, node_idx, "", is_last, 1);
        }

        buffer
    }

    pub fn format_status_tag(is_cached: bool, colorize: bool) -> String {
        let status = if is_cached { "[cached]" } else { "[missing]" };
        wrap_styled(status, DIM, colorize)
    }
}

struct RenderContext<'a> {
    graph: &'a SolvedGraph,
    depth_limit: Option<usize>,
    matching_subtrees: Option<&'a [bool]>,
    colorize: bool,
    printed: ahash::AHashSet<usize>,
    active_path: ahash::AHashSet<usize>,
}

impl<'a> RenderContext<'a> {
    fn render_node(
        &mut self,
        buffer: &mut String,
        node_idx: usize,
        prefix: &str,
        is_last: bool,
        depth: usize,
    ) {
        if let Some(limit) = self.depth_limit
            && depth > limit
        {
            return;
        }

        let Some(node) = self.graph.nodes.get(node_idx) else {
            return;
        };

        self.append_prefix(buffer, prefix, is_last, depth);
        self.append_label(buffer, node);

        if self.active_path.contains(&node_idx) || self.printed.contains(&node_idx) {
            if !node.children.is_empty() {
                buffer.push_str(" (*)");
            }
            buffer.push('\n');
            return;
        }

        self.printed.insert(node_idx);
        self.active_path.insert(node_idx);
        buffer.push('\n');

        if self.depth_limit.is_none_or(|limit| depth < limit) {
            self.render_children(buffer, node, prefix, is_last, depth);
        }

        self.active_path.remove(&node_idx);
    }

    fn append_prefix(&self, buffer: &mut String, prefix: &str, is_last: bool, depth: usize) {
        buffer.push_str(prefix);
        if depth > 0 {
            if is_last {
                buffer.push_str("└── ");
            } else {
                buffer.push_str("├── ");
            }
        }
    }

    fn append_label(&self, buffer: &mut String, node: &GraphNode) {
        buffer.push_str(&node.coordinate.to_string());

        if let Some(ref prev) = node.promoted_from {
            buffer.push_str(" (-> promoted from ");
            buffer.push_str(prev);
            buffer.push(')');
        }

        buffer.push(' ');
        buffer.push_str(&TreeRenderer::format_status_tag(
            node.cached_binary.is_some(),
            self.colorize,
        ));
    }

    fn render_children(
        &mut self,
        buffer: &mut String,
        node: &GraphNode,
        prefix: &str,
        is_last: bool,
        depth: usize,
    ) {
        let child_prefix = if is_last { "    " } else { "│   " };
        let next_prefix = common::concat_str!(prefix, child_prefix);

        let eligible_children: Vec<usize> = if let Some(matches) = self.matching_subtrees {
            node.children
                .iter()
                .copied()
                .filter(|&child_idx| matches.get(child_idx).copied().unwrap_or(false))
                .collect()
        } else {
            node.children.clone()
        };

        let total_children = eligible_children.len();
        for (ci, &child_idx) in eligible_children.iter().enumerate() {
            let child_is_last = ci + 1 == total_children;
            self.render_node(buffer, child_idx, &next_prefix, child_is_last, depth + 1);
        }
    }
}

fn subtree_matches(graph: &SolvedGraph, idx: usize, lower_query: &str) -> bool {
    let mut stack = vec![idx];
    let mut visited = ahash::AHashSet::default();
    while let Some(node_idx) = stack.pop() {
        if !visited.insert(node_idx) {
            continue;
        }
        if let Some(node) = graph.nodes.get(node_idx) {
            if node_matches(&node.coordinate, lower_query) {
                return true;
            }
            stack.extend(node.children.iter().copied());
        }
    }
    false
}

fn node_matches(coord: &crate::coordinate::DependencyCoordinate, lower_query: &str) -> bool {
    coord.group.to_ascii_lowercase().contains(lower_query)
        || coord.name.to_ascii_lowercase().contains(lower_query)
        || coord
            .version
            .as_ref()
            .is_some_and(|v| v.raw().to_ascii_lowercase().contains(lower_query))
}
