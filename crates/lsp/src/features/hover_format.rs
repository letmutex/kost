use tower_lsp::lsp_types::{Location, SymbolKind, Url};
use tree_sitter::Node;

use super::hover::HoverContext;
pub use super::hover_local::{format_implicit_it_hover, format_local_node_markdown};

pub fn format_source_footer(loc: &Location, current_url: &Url) -> Option<String> {
    let uri_str = loc.uri.as_str();

    if let Some(after_jar) = uri_str.strip_prefix("jar:") {
        let mut parts = after_jar.split("!/");
        let archive_url_str = parts.next()?;
        let entry_path = parts.collect::<Vec<_>>().join("!/");

        let archive_url = Url::parse(archive_url_str).ok()?;
        let archive_name = archive_url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or("archive.jar");
        let entry_name = entry_path.rsplit('/').next().unwrap_or(&entry_path);

        Some(format!(
            "\n\n---\n*Defined in `{}` in [{}]({})*",
            entry_name, archive_name, archive_url
        ))
    } else if loc.uri != *current_url {
        let file_name = loc
            .uri
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or("source.kt");
        let line = loc.range.start.line + 1;

        Some(format!(
            "\n\n---\n*Defined in [{}:{}]({}#L{})*",
            file_name, line, loc.uri, line
        ))
    } else {
        None
    }
}

pub fn build_qualifying_path(
    package_name: Option<&str>,
    container_name: Option<&str>,
) -> Option<String> {
    let pkg = package_name.map(|p| p.trim()).filter(|p| !p.is_empty());
    let container = container_name
        .map(|c| c.trim())
        .filter(|c| !c.is_empty() && !c.ends_with("Kt"));

    match (pkg, container) {
        (Some(p), Some(c)) => Some(format!("{}.{}", p, c)),
        (Some(p), None) => Some(p.to_string()),
        (None, Some(c)) => Some(c.to_string()),
        (None, None) => None,
    }
}

pub fn format_markdown_hover(path: Option<&str>, sig: Option<&str>, doc: Option<&str>) -> String {
    let mut markdown = String::new();
    if let Some(s) = sig {
        if let Some(p) = path {
            markdown.push_str(&format!("{}\n\n```kotlin\n{}\n```\n", p, s));
        } else {
            markdown.push_str(&format!("```kotlin\n{}\n```\n", s));
        }
    }
    if let Some(d) = doc
        && !d.is_empty()
    {
        if !markdown.is_empty() {
            markdown.push_str("\n---\n\n");
        }
        markdown.push_str(d);
    }
    markdown
}

pub fn is_extension_signature(sig: &str, symbol_name: &str) -> bool {
    let patterns = [
        format!(".{}(", symbol_name),
        format!(".{}<", symbol_name),
        format!(".{}:", symbol_name),
        format!(".{} ", symbol_name),
        format!(".{}=", symbol_name),
    ];
    patterns.iter().any(|p| sig.contains(p))
}

pub fn format_index_declaration_markdown(
    ctx: HoverContext<'_>,
    loc: &Location,
    symbol_name: &str,
) -> String {
    let Some(file) = ctx.index.files.get(&loc.uri) else {
        return String::new();
    };

    let decl = file
        .declarations
        .iter()
        .find(|d| d.selection_range == loc.range && d.name == symbol_name)
        .or_else(|| {
            file.declarations
                .iter()
                .find(|d| d.range.start == loc.range.start && d.name == symbol_name)
        });

    let Some(decl) = decl else {
        return String::new();
    };

    let container = if decl
        .signature
        .as_ref()
        .is_some_and(|s| is_extension_signature(s, &decl.name))
    {
        None
    } else {
        decl.container_name.as_deref()
    };
    let path = build_qualifying_path(file.package_name.as_deref(), container);

    let mut sig = decl.signature.as_deref().map(|s| s.to_string());
    let is_variable = matches!(decl.kind, SymbolKind::PROPERTY | SymbolKind::VARIABLE)
        || sig.as_ref().is_some_and(|s| {
            s.starts_with("val ") || s.starts_with("var ") || s.starts_with("const val ")
        });

    let trace_rendered = if is_variable && ctx.type_infer_opt.is_enabled() {
        super::hover_augment::infer_variable_and_augment_signature(ctx, &decl.name, &mut sig)
    } else {
        None
    };

    let mut markdown =
        format_markdown_hover(path.as_deref(), sig.as_deref(), decl.doc_comment.as_deref());
    if markdown.is_empty() {
        let sig_fallback = format!(
            "{} {}",
            format!("{:?}", decl.kind).to_lowercase(),
            decl.name
        );
        markdown = format_markdown_hover(path.as_deref(), Some(&sig_fallback), None);
    }
    if let Some(t) = trace_rendered {
        markdown.push_str(&t);
    }
    markdown
}

pub fn extract_package_name_from_ast<'a>(root: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_header"
            && let Ok(text) = child.utf8_text(source.as_bytes())
        {
            let clean = text.lines().next().unwrap_or("").trim();
            if let Some(rest) = clean.strip_prefix("package") {
                let clean = rest.trim().trim_end_matches(';').trim();
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }
    }
    None
}

pub fn find_enclosing_container_name(node: &Node, source: &str) -> Option<String> {
    let mut containers = Vec::new();
    let mut curr = node.parent();
    while let Some(parent) = curr {
        if matches!(
            parent.kind(),
            "class_declaration"
                | "object_declaration"
                | "interface_declaration"
                | "companion_object"
        ) {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if (child.kind() == "type_identifier" || child.kind() == "simple_identifier")
                    && let Ok(name) = child.utf8_text(source.as_bytes())
                {
                    containers.push(name);
                    break;
                }
            }
        }
        curr = parent.parent();
    }
    if containers.is_empty() {
        None
    } else {
        containers.reverse();
        Some(containers.join("."))
    }
}
