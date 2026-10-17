use tree_sitter::{Node, Parser, Tree};

use crate::error::{FormatError, Position};
use crate::options::{FileType, FormattingOptions};

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
            message: crate::concat_str!("Syntax error near '", err_node.kind(), "'"),
        });
    }

    // Extract package name if present
    let mut package_name = None;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_header" {
            let mut pkg_cursor = child.walk();
            for pkg_child in child.children(&mut pkg_cursor) {
                if pkg_child.kind() == "identifier"
                    && let Some(s) = source.get(pkg_child.start_byte()..pkg_child.end_byte())
                {
                    package_name = Some(s.trim());
                    break;
                }
            }
            break;
        }
    }

    // Collect used references and redundant semicolons
    let used_references = crate::rules::imports::collect_used_references(root, source);
    let redundant_semicolons = crate::rules::semicolons::find_redundant_semicolons(root, source);

    let ctx = crate::lower::context::LowerContext::new(
        source,
        options,
        &redundant_semicolons,
        &used_references,
        package_name,
    );

    let doc = crate::lower::lower_source_file(root, &ctx);
    let printer = crate::printer::Printer::new(options);
    let formatted = printer.print(&doc, source.len());

    Ok(formatted)
}
