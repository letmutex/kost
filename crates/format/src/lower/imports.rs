use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::rules::imports::{ImportEntry, clean_identifier_path, parse_import_header};

pub fn lower_package_header<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut pkg_name = String::new();
    let mut comments = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            pkg_name = clean_identifier_path(child, ctx.source);
        } else if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            comments.push(lower_fn(child, ctx));
        }
    }
    if pkg_name.is_empty() {
        let raw = ctx.node_text(&node).trim();
        if raw.is_empty() {
            return Doc::empty();
        }
        let clean = raw.strip_suffix(';').unwrap_or(raw).trim();
        return Doc::text(clean);
    }
    let mut docs = vec![Doc::text("package "), Doc::smol(pkg_name)];
    if !comments.is_empty() {
        docs.push(Doc::EmptyLine);
        for (i, c) in comments.into_iter().enumerate() {
            if i > 0 {
                docs.push(Doc::HardLine);
            }
            docs.push(c);
        }
        docs.push(Doc::HardLine);
    }
    Doc::concat(docs)
}

fn format_import_directive(entry: &ImportEntry) -> String {
    let mut s = common::concat_str!("import ", &entry.full_path);
    if entry.is_wildcard {
        s.push_str(".*");
    }
    if let Some(alias) = &entry.alias {
        s.push_str(" as ");
        s.push_str(alias);
    }
    if let Some(comment) = &entry.trailing_comment {
        s.push(' ');
        if comment.starts_with("/*") && comment.contains('\n') {
            let base_indent = " ".repeat(s.len());
            let mut formatted_comment = String::new();
            for (i, line) in comment.lines().enumerate() {
                if i == 0 {
                    formatted_comment.push_str(line.trim());
                } else {
                    formatted_comment.push('\n');
                    formatted_comment.push_str(&base_indent);
                    formatted_comment.push_str(line.trim());
                }
            }
            s.push_str(&formatted_comment);
        } else {
            s.push_str(comment);
        }
    }
    s
}

fn process_import_header_children<'a>(
    child: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    top_comments: &mut Vec<Doc<'a>>,
    bottom_comments: &mut Vec<(bool, Doc<'a>)>,
    is_last_import: bool,
) -> Option<ImportEntry> {
    let mut code_end_row = child.start_position().row;
    let mut same_line_comment = None;
    let mut separate_comments = Vec::new();

    let mut hc = child.walk();
    for c in child.children(&mut hc) {
        if matches!(c.kind(), "comment" | "line_comment" | "multiline_comment") {
            if c.start_position().row == code_end_row {
                same_line_comment = Some(ctx.node_text(&c).trim().to_string());
            } else {
                let is_kdoc = ctx.node_text(&c).starts_with("/**");
                separate_comments.push((is_kdoc, lower_fn(c, ctx)));
            }
        } else {
            code_end_row = c.end_position().row;
        }
    }

    let empty_set = ahash::AHashSet::new();
    let used_refs = ctx.used_references.unwrap_or(&empty_set);
    let mut entry = parse_import_header(child, ctx.source, ctx.package_name, used_refs)?;
    entry.trailing_comment = same_line_comment;

    for (is_kdoc, comment_doc) in separate_comments {
        if is_last_import {
            bottom_comments.push((is_kdoc, comment_doc));
        } else {
            top_comments.push(comment_doc);
        }
    }

    Some(entry)
}

fn collect_import_list_children<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (Vec<ImportEntry>, Vec<Doc<'a>>, Vec<(bool, Doc<'a>)>) {
    let mut entries = Vec::new();
    let mut top_comments = Vec::new();
    let mut bottom_comments = Vec::new();

    let mut cursor = node.walk();
    let mut last_import_id = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "import_header" {
            last_import_id = Some(child.id());
        }
    }

    let mut cursor = node.walk();
    let mut reached_last_import = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "import_header" {
            let is_last_import = last_import_id.is_some_and(|id| child.id() == id);
            if is_last_import {
                reached_last_import = true;
            }
            if let Some(entry) = process_import_header_children(
                child,
                ctx,
                lower_fn,
                &mut top_comments,
                &mut bottom_comments,
                is_last_import,
            ) {
                entries.push(entry);
            }
        } else if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            let is_kdoc = ctx.node_text(&child).starts_with("/**");
            if !reached_last_import {
                top_comments.push(lower_fn(child, ctx));
            } else {
                bottom_comments.push((is_kdoc, lower_fn(child, ctx)));
            }
        }
    }

    (entries, top_comments, bottom_comments)
}

fn filter_and_sort_imports(
    entries: Vec<ImportEntry>,
    ctx: &LowerContext<'_, '_>,
) -> Vec<ImportEntry> {
    let mut identifier_counts = std::collections::HashMap::new();
    for e in &entries {
        *identifier_counts.entry(e.short_name.clone()).or_insert(0) += 1;
    }

    let mut filtered_entries: Vec<ImportEntry> = entries
        .into_iter()
        .filter(|e| {
            if !ctx.options.remove_unused_imports {
                return true;
            }
            let is_overload = identifier_counts.get(&e.short_name).copied().unwrap_or(0) > 1;
            if e.is_from_same_package && e.alias.is_none() && !is_overload {
                return false;
            }
            e.is_used
        })
        .collect();

    filtered_entries.sort_by_key(|a| a.canonical_key());
    filtered_entries.dedup_by(|a, b| a.canonical_key() == b.canonical_key());
    filtered_entries
}

pub fn lower_import_list<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let (entries, top_comments, bottom_comments) =
        collect_import_list_children(node, ctx, lower_fn);
    if entries.is_empty() && top_comments.is_empty() && bottom_comments.is_empty() {
        return Doc::empty();
    }

    let filtered_entries = filter_and_sort_imports(entries, ctx);
    let has_top_comments = !top_comments.is_empty();
    let mut docs = Vec::new();
    for comment in top_comments {
        docs.push(comment);
        docs.push(Doc::HardLine);
    }

    for (idx, entry) in filtered_entries.iter().enumerate() {
        if idx > 0 {
            docs.push(Doc::HardLine);
        }
        let clean_import = format_import_directive(entry);
        docs.push(Doc::smol(clean_import));
    }

    if !bottom_comments.is_empty() {
        if !filtered_entries.is_empty() || has_top_comments {
            docs.push(Doc::EmptyLine);
        }
        let count = bottom_comments.len();
        for (idx, (is_kdoc, comment_doc)) in bottom_comments.into_iter().enumerate() {
            docs.push(comment_doc);
            if idx + 1 < count || is_kdoc {
                docs.push(Doc::HardLine);
            } else {
                docs.push(Doc::EmptyLine);
            }
        }
    }

    Doc::concat(docs)
}
