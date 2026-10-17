use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

use crate::concat_str;
use crate::position::point_to_position;

pub fn extract_import_path<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return source
                .get(child.start_byte()..child.end_byte())
                .map(str::trim);
        }
    }
    None
}

pub struct FileHeaderNodes<'a> {
    pub import_headers: Vec<Node<'a>>,
    pub package_header: Option<Node<'a>>,
    pub last_file_header: Option<Node<'a>>,
    pub last_leading_comment: Option<Node<'a>>,
}

pub fn collect_header_nodes(root: Node<'_>) -> FileHeaderNodes<'_> {
    let mut import_headers = Vec::new();
    let mut package_header = None;
    let mut last_file_header = None;
    let mut last_leading_comment = None;
    let mut in_leading_comments = true;

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "comment" | "line_comment" | "multiline_comment" if in_leading_comments => {
                last_leading_comment = Some(child);
            }
            "shebang_line" | "file_annotation" => {
                in_leading_comments = false;
                last_file_header = Some(child);
            }
            "package_header" => {
                in_leading_comments = false;
                package_header = Some(child);
                last_file_header = Some(child);
            }
            "import_list" => {
                in_leading_comments = false;
                let mut import_cursor = child.walk();
                for import_header_node in child.children(&mut import_cursor) {
                    if import_header_node.kind() == "import_header" {
                        import_headers.push(import_header_node);
                    }
                }
            }
            "import_header" => {
                in_leading_comments = false;
                import_headers.push(child);
            }
            _ => {
                in_leading_comments = false;
            }
        }
    }

    FileHeaderNodes {
        import_headers,
        package_header,
        last_file_header,
        last_leading_comment,
    }
}

pub fn detect_newline(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

pub fn insert_into_import_headers(
    import_headers: &[Node<'_>],
    source: &str,
    target_fqn: &str,
    newline: &str,
) -> (Position, String) {
    for import_header_node in import_headers {
        if let Some(path) = extract_import_path(*import_header_node, source)
            && path > target_fqn
        {
            let start = import_header_node.start_position();
            let pos = Position {
                line: start.row as u32,
                character: 0,
            };
            return (pos, concat_str!("import ", target_fqn, newline));
        }
    }

    let Some(last_import) = import_headers.last() else {
        return (
            Position {
                line: 0,
                character: 0,
            },
            concat_str!("import ", target_fqn, newline),
        );
    };
    let end_row = last_import.end_position().row;
    let line_start = crate::position::position_to_byte_offset(
        Position {
            line: end_row as u32,
            character: 0,
        },
        source,
    );
    let line_slice = &source[line_start..];
    if line_slice.contains('\n') {
        let pos = Position {
            line: (end_row + 1) as u32,
            character: 0,
        };
        (pos, concat_str!("import ", target_fqn, newline))
    } else {
        let char_offset = line_slice.encode_utf16().count() as u32;
        let pos = Position {
            line: end_row as u32,
            character: char_offset,
        };
        (pos, concat_str!(newline, "import ", target_fqn))
    }
}

pub fn insert_after_header(
    node: Node<'_>,
    source: &str,
    target_fqn: &str,
    newline: &str,
) -> (Position, String) {
    let end_row = node.end_position().row;
    let line_start = crate::position::position_to_byte_offset(
        Position {
            line: end_row as u32,
            character: 0,
        },
        source,
    );
    let line_slice = &source[line_start..];
    let after_line = line_slice
        .find('\n')
        .map_or("", |idx| &line_slice[idx + 1..]);
    let has_blank_line = after_line.starts_with('\n')
        || after_line.starts_with("\r\n")
        || after_line.trim_start_matches([' ', '\t']).starts_with('\n')
        || after_line
            .trim_start_matches([' ', '\t'])
            .starts_with("\r\n");

    if line_slice.contains('\n') {
        let pos = Position {
            line: (end_row + 1) as u32,
            character: 0,
        };
        let text = if has_blank_line || after_line.trim().is_empty() {
            concat_str!(newline, "import ", target_fqn, newline)
        } else {
            concat_str!(newline, "import ", target_fqn, newline, newline)
        };
        (pos, text)
    } else {
        let pos = point_to_position(node.end_position());
        (pos, concat_str!(newline, newline, "import ", target_fqn))
    }
}

pub fn find_ast_import_insertion(root: Node, source: &str, target_fqn: &str) -> (Position, String) {
    let headers = collect_header_nodes(root);
    let newline = detect_newline(source);

    if !headers.import_headers.is_empty() {
        return insert_into_import_headers(&headers.import_headers, source, target_fqn, newline);
    }
    if let Some(pkg_node) = headers.package_header {
        return insert_after_header(pkg_node, source, target_fqn, newline);
    }
    if let Some(header_node) = headers.last_file_header {
        return insert_after_header(header_node, source, target_fqn, newline);
    }
    if let Some(comment_node) = headers.last_leading_comment {
        return insert_after_header(comment_node, source, target_fqn, newline);
    }

    let pos = Position {
        line: 0,
        character: 0,
    };
    let text = if source.trim().is_empty() {
        concat_str!("import ", target_fqn, newline)
    } else {
        concat_str!("import ", target_fqn, newline, newline)
    };
    (pos, text)
}
