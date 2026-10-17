use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_lex_context_outside_comment_or_string_after_cjk_identifier() {
    let source = "val 是 = 1";
    let pos = Position::new(0, 5);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string_text(pos, source));
}

#[test]
fn test_completion_finds_identifiers_after_multibyte_string_literal() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TestMultibyte.kt").unwrap();
    let file_code = r#"fun test() {
    val message = "你好，世界"
    val target = 1
    tar
}"#;
    index.index_source(file_url.clone(), file_code);

    let pos = Position::new(3, 7);
    let items = find_completions(&index, &file_url, pos, file_code);
    let labels: Vec<String> = items.into_iter().map(|c| c.label).collect();
    assert!(labels.contains(&"target".to_string()));
}

#[test]
fn test_call_argument_finds_enclosing_callee_after_cjk_identifier() {
    let source = "val 是 = foo(1)";
    let pos = Position::new(0, 13);
    let res = lsp::features::call_argument::find_enclosing_call_name(pos, source);
    assert_eq!(res, Some((None, "foo".to_string())));
}

#[test]
fn test_completion_context_scope_after_cjk_identifier() {
    let source = "val 是 = 1";
    let pos = Position::new(0, 5);
    let ctx = lsp::features::completion_context::determine_completion_context(pos, source);
    assert!(matches!(
        ctx,
        lsp::features::completion_context::CompletionContext::Scope { .. }
    ));
}

#[test]
fn test_signature_parser_call_context_after_cjk_identifier() {
    let source = "val 是 = foo(1";
    let pos = Position::new(0, 14);
    let ctx = lsp::features::signature_parser::find_enclosing_call_context(pos, source);
    assert!(ctx.is_some());
    let call = ctx.unwrap();
    assert_eq!(call.callee, "foo");
    assert_eq!(call.active_parameter, 0);
}

#[test]
fn test_position_to_byte_offset_multibyte_character_boundary() {
    let source = "val 是 = 1";
    let pos = Position::new(0, 5);
    let offset = lsp::syntax::position::position_to_byte_offset(pos, source);
    assert_eq!(offset, 7, "Byte offset should skip full 3 bytes of '是'");
    assert!(
        source.is_char_boundary(offset),
        "Offset must land on char boundary"
    );
}

#[test]
fn test_position_to_byte_offset_astral_plane_emoji() {
    let source = "val 😀 = 1";
    let pos = Position::new(0, 6);
    let offset = lsp::syntax::position::position_to_byte_offset(pos, source);
    assert_eq!(offset, 8, "Byte offset should skip full 4 bytes of '😀'");
    assert!(
        source.is_char_boundary(offset),
        "Offset must land on char boundary"
    );
}

#[test]
fn test_position_to_byte_offset_split_surrogate_floor() {
    let source = "val 😀 = 1";
    let pos = Position::new(0, 5);
    let offset = lsp::syntax::position::position_to_byte_offset(pos, source);
    assert_eq!(
        offset, 4,
        "Halfway surrogate should floor to start of emoji"
    );
}

#[test]
fn test_position_to_byte_offset_crlf_with_multibyte() {
    let source = "val 是 = 1\r\nval a = 2";
    let pos = Position::new(1, 4);
    let offset = lsp::syntax::position::position_to_byte_offset(pos, source);
    assert_eq!(offset, 17);
    assert_eq!(&source[offset..offset + 1], "a");
}

#[test]
fn test_lex_context_block_comment_boundaries() {
    let source = "/* block */ val x = 1";
    let tree = lsp::syntax::parser::parse_source(source).unwrap();
    let root = Some(tree.root_node());

    let pos_inside = Position::new(0, 3);
    assert!(lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_inside, source
    ));

    let pos_after = Position::new(0, 11);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_after, source
    ));

    let pos_start = Position::new(0, 0);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_start, source
    ));
}

#[test]
fn test_lex_context_line_comment_boundaries() {
    let source = "// line comment\nval x = 1";
    let tree = lsp::syntax::parser::parse_source(source).unwrap();
    let root = Some(tree.root_node());

    let pos_start = Position::new(0, 0);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_start, source
    ));

    let pos_inside = Position::new(0, 5);
    assert!(lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_inside, source
    ));

    let pos_end = Position::new(0, 15);
    assert!(lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_end, source
    ));

    let pos_next_line = Position::new(1, 0);
    assert!(
        !lsp::features::lex_context::is_inside_comment_or_string(root, pos_next_line, source),
        "Start of line following a line comment must not be considered inside the comment"
    );
}

#[test]
fn test_lex_context_nested_expression_in_string_template() {
    let source = "val s = \"${foo({ bar })}\"";
    let pos = Position::new(0, 17);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string_text(pos, source));
}

#[test]
fn test_lex_context_nested_block_comments() {
    let source = "/* /* nested */ still comment */ val x = 1";
    let pos = Position::new(0, 17);
    assert!(lsp::features::lex_context::is_inside_comment_or_string_text(pos, source));
}

#[test]
fn test_lex_context_string_literal_boundaries() {
    let source = "val s = \"hello\"";
    let tree = lsp::syntax::parser::parse_source(source).unwrap();
    let root = Some(tree.root_node());

    let pos_inside = Position::new(0, 10);
    assert!(lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_inside, source
    ));

    let pos_after = Position::new(0, 15);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_after, source
    ));

    let pos_before = Position::new(0, 8);
    assert!(!lsp::features::lex_context::is_inside_comment_or_string(
        root, pos_before, source
    ));
}
