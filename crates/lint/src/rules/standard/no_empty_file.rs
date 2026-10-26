//! Built-in AST rule checking for non-empty Kotlin source files (Ktlint rule 12).

use std::path::Path;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that detects empty Kotlin files containing no executable code or declarations.
pub struct NoEmptyFile;

impl AstRule for NoEmptyFile {
    fn id(&self) -> &'static str {
        "no-empty-file"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if let Some(file_path_str) = ctx.file_path.as_deref() {
            let file_path = Path::new(file_path_str);
            if file_path.extension().and_then(|s| s.to_str()) != Some("kt") {
                return;
            }
        }

        let mut has_code = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !matches!(
                child.kind(),
                "shebang_line"
                    | "line_comment"
                    | "multiline_comment"
                    | "package_header"
                    | "import_list"
                    | "file_annotation"
            ) {
                has_code = true;
                break;
            }
        }

        if !has_code {
            let file_name = ctx
                .file_path
                .as_deref()
                .and_then(|p| Path::new(p).file_name().and_then(|s| s.to_str()))
                .unwrap_or("file");
            let message = common::concat_str!("File '", file_name, "' should not be empty");
            ctx.report(node, message);
        }
    }
}
