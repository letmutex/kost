use ahash::AHashSet;
use std::sync::LazyLock;
use tree_sitter::{Node, Parser, Tree};

use crate::error::{FormatError, Position};
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::options::{FileType, FormattingOptions};
use crate::printer::Printer;

static KOTLIN_KIND_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let language: tree_sitter::Language =
        unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
    let count = language.node_kind_count();
    let mut names = Vec::with_capacity(count);
    for id in 0..count as u16 {
        names.push(language.node_kind_for_id(id).unwrap_or(""));
    }
    names
});

/// Fast O(1) lookup of a node's kind string by `kind_id` without C-string `strlen` or UTF-8 validation.
#[inline]
pub fn node_kind(node: Node<'_>) -> &'static str {
    let id = node.kind_id() as usize;
    let names = &*KOTLIN_KIND_NAMES;
    if id < names.len() {
        names[id]
    } else if node.is_error() {
        "ERROR"
    } else {
        node.kind()
    }
}

pub struct KotlinParser {
    parser: Parser,
}

impl Default for KotlinParser {
    fn default() -> Self {
        Self::new()
    }
}

impl KotlinParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
        parser
            .set_language(&language)
            .expect("Error loading Kotlin tree-sitter grammar");
        Self { parser }
    }

    pub fn parse(&mut self, text: &str) -> Option<Tree> {
        self.parser.parse(text, None)
    }
}

thread_local! {
    static THREAD_PARSER: std::cell::RefCell<KotlinParser> =
        std::cell::RefCell::new(KotlinParser::new());
}

fn with_parser<F, R>(f: F) -> R
where
    F: FnOnce(&mut KotlinParser) -> R,
{
    THREAD_PARSER.with(|p| f(&mut p.borrow_mut()))
}

fn find_first_error_node<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error()
            && let Some(err) = find_first_error_node(child)
        {
            return Some(err);
        }
    }
    None
}

/// Internal engine dispatch for formatting source code
pub fn format_source(
    source: &str,
    _file_type: FileType,
    options: &FormattingOptions,
) -> Result<String, FormatError> {
    if source.trim().is_empty() {
        return Ok(String::new());
    }

    let tree = with_parser(|p| p.parse(source))
        .ok_or_else(|| FormatError::InternalError("Failed to parse source".into()))?;

    let root = tree.root_node();
    if root.has_error()
        && let Some(err_node) = find_first_error_node(root)
    {
        let point = err_node.start_position();
        return Err(FormatError::ParseError {
            position: Position::new(point.row + 1, point.column + 1),
            message: common::concat_str!("Syntax error near '", err_node.kind(), "'"),
        });
    }

    // Extract package name and check whether imports exist
    let mut package_name = None;
    let mut has_imports = false;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let kind = node_kind(child);
        if kind == "package_header" {
            let mut pkg_cursor = child.walk();
            for pkg_child in child.children(&mut pkg_cursor) {
                if node_kind(pkg_child) == "identifier"
                    && let Some(s) = source.get(pkg_child.start_byte()..pkg_child.end_byte())
                {
                    package_name = Some(s.trim());
                    break;
                }
            }
        } else if kind == "import_list" {
            has_imports = true;
            break;
        }
    }

    let used_references: AHashSet<&str> = if has_imports {
        crate::rules::imports::collect_used_references(root, source)
    } else {
        AHashSet::new()
    };
    let redundant_semicolons = crate::rules::semicolons::find_redundant_semicolons(root, source);

    let ctx = LowerContext::new(source, options)
        .with_redundant_semicolons(&redundant_semicolons)
        .with_imports(package_name, &used_references);

    let doc = lower_node(root, &ctx);
    if std::env::var("KOST_DEBUG_DOC").is_ok() && source.contains("blablablablFoobar") {
        eprintln!("{}", crate::doc::dump::dump(&doc, 0));
    }
    let mut printer = Printer::new(options);
    let formatted = printer.print_with_capacity(&doc, source.len() + 64);

    Ok(formatted)
}
