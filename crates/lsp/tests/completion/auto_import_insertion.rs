use lsp::features::auto_import::{find_ast_import_insertion, insert_into_import_headers};
use lsp::syntax::parser::parse_source;

#[test]
fn test_auto_import_insertion_alphabetical_order() {
    let code = r#"package com.example.client

import com.aaa.Alpha
import com.zzz.Omega

class Client
"#;

    let tree = parse_source(code).unwrap();
    let (pos, text) = find_ast_import_insertion(tree.root_node(), code, "com.mmm.Middle");

    assert_eq!(pos.line, 3, "should insert before com.zzz.Omega at line 3");
    assert_eq!(pos.character, 0);
    assert_eq!(text, "import com.mmm.Middle\n");
}

#[test]
fn test_auto_import_insertion_at_end_of_import_list() {
    let code = r#"package com.example.client

import com.aaa.Alpha
import com.bbb.Beta

class Client
"#;

    let tree = parse_source(code).unwrap();
    let (pos, text) = find_ast_import_insertion(tree.root_node(), code, "com.zzz.Omega");

    assert_eq!(pos.line, 4, "should insert at next line after last import");
    assert_eq!(text, "import com.zzz.Omega\n");
}

#[test]
fn test_auto_import_insertion_after_package_header() {
    let code = r#"package com.example.client

class Client
"#;

    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");

    assert_eq!(pos.line, 1, "should insert on line after package header");
    assert!(text.contains("import com.example.service.OrderProcessor"));
}

#[test]
fn test_auto_import_insertion_at_file_start_without_package() {
    let code = r#"fun main() {
    println("hello")
}
"#;

    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");

    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 0);
    assert_eq!(text, "import com.example.service.OrderProcessor\n\n");
}

#[test]
fn test_auto_import_insertion_preserves_trailing_comment() {
    let code =
        "package com.example.client\n\nimport com.aaa.Alpha // trailing comment\n\nclass Client\n";
    let tree = parse_source(code).unwrap();
    let (pos, text) = find_ast_import_insertion(tree.root_node(), code, "com.zzz.Omega");

    assert_eq!(
        pos.line, 3,
        "must insert on the line after the trailing comment"
    );
    assert_eq!(pos.character, 0);
    assert_eq!(text, "import com.zzz.Omega\n");
}

#[test]
fn test_auto_import_insertion_crlf_line_endings() {
    let code = "package com.example.client\r\n\r\nimport com.aaa.Alpha\r\n\r\nclass Client\r\n";
    let tree = parse_source(code).unwrap();
    let (_, text) = find_ast_import_insertion(tree.root_node(), code, "com.zzz.Omega");
    assert!(
        text.contains("\r\n"),
        "should use CRLF on CRLF files: {:?}",
        text
    );
}

#[test]
fn test_auto_import_insertion_after_leading_file_comments() {
    let code = "/* Copyright (c) 2026 */\n\nfun main() {}\n";
    let tree = parse_source(code).unwrap();
    let (pos, _) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");
    assert!(
        pos.line > 0,
        "must insert below copyright comment, not at (0, 0)"
    );
}

#[test]
fn test_auto_import_insertion_preserves_blank_lines_around_import() {
    let code = "package com.example.client\n\nclass Client\n";
    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);
    assert_eq!(
        text, "\nimport com.example.service.OrderProcessor\n",
        "must insert blank line before import and trailing newline after import"
    );
}

#[test]
fn test_auto_import_insertion_creates_blank_lines_when_missing() {
    let code = "package com.example.client\nclass Client\n";
    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);
    assert_eq!(
        text, "\nimport com.example.service.OrderProcessor\n\n",
        "must insert blank line before import and blank line after import when no blank line exists"
    );
}

#[test]
fn test_auto_import_insertion_empty_headers_slice_fallback() {
    let (pos, text) =
        insert_into_import_headers(&[], "", "com.example.service.OrderProcessor", "\n");
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 0);
    assert_eq!(text, "import com.example.service.OrderProcessor\n");
}

#[test]
fn test_auto_import_insertion_after_shebang() {
    let code = "#!/usr/bin/env kotlin\n\nfun main() {}\n";
    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);
    assert!(text.contains("import com.example.service.OrderProcessor"));
}

#[test]
fn test_auto_import_insertion_after_file_annotation() {
    let code = "@file:JvmName(\"MainUtil\")\n\nfun main() {}\n";
    let tree = parse_source(code).unwrap();
    let (pos, text) =
        find_ast_import_insertion(tree.root_node(), code, "com.example.service.OrderProcessor");
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);
    assert!(text.contains("import com.example.service.OrderProcessor"));
}

#[test]
fn test_auto_import_insertion_at_eof_without_trailing_newline_with_comment() {
    let code = "import com.example.service.A // trailing note   ";
    let tree = parse_source(code).unwrap();
    let (pos, text) = find_ast_import_insertion(tree.root_node(), code, "com.example.service.Z");
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, code.len() as u32);
    assert_eq!(text, "\nimport com.example.service.Z");
}
