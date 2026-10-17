use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_object_member_called_on_instance_variable_and_parameter() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Logger.kt").unwrap();
    let code = r#"
package com.example

object Logger {
    fun log(message: String) {}
}

fun run(paramLogger: Logger) {
    val localLogger = Logger
    Logger.log("direct")
    localLogger.log("variable")
    paramLogger.log("parameter")
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on `fun log` declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "log", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        3,
        "Finding references to Logger.log should match direct call, variable call, and parameter call"
    );
    assert_eq!(refs[0].range.start.line, 9);
    assert_eq!(refs[1].range.start.line, 10);
    assert_eq!(refs[2].range.start.line, 11);
}

#[test]
fn test_references_named_companion_object_member_called_on_class_name() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Factory.kt").unwrap();
    let code = r#"
package com.example

class Config {
    companion object Factory {
        fun default(): Config = Config()
    }
}

fun run() {
    val config = Config.default()
    val explicit = Config.Factory.default()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on `fun default(): Config` declaration (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_url, "default", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        2,
        "Finding references to Config.Factory.default should match Config.default() and Config.Factory.default()"
    );
    assert_eq!(refs[0].range.start.line, 10);
    assert_eq!(refs[1].range.start.line, 11);
}

#[test]
fn test_references_nested_class_within_private_outer_class_restricted_to_file() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Secret.kt").unwrap();
    let file_a_code = r#"
package com.example

private class Outer {
    class Inner {
        fun secret(): Int = 42
    }
}

fun localCall() {
    val inner = Outer.Inner()
    val value = inner.secret()
}
"#;
    let file_b_url = Url::parse("file:///project/Other.kt").unwrap();
    let file_b_code = r#"
package com.example

class Unrelated {
    fun secret(): Int = 0
}

fun otherCall(unrelated: Unrelated) {
    unrelated.secret()
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on Outer.Inner.secret declaration (line 5, col 12 in Secret.kt)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_a_url, "secret", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Outer.Inner is nested in private Outer, so references must be restricted to Secret.kt"
    );
    assert_eq!(refs[0].uri, file_a_url);
    assert_eq!(refs[0].range.start.line, 11);
}

#[test]
fn test_references_member_on_annotated_type_receiver() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Service.kt").unwrap();
    let code = r#"
package com.example

class Client {
    fun send(): String = "ok"
}

annotation class NonNull

fun run(@NonNull client: Client) {
    val resp = client.send()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on Client.send declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "send", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references should match client.send() when parameter has annotation"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_literal_receiver_matches_standard_interface() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Text.kt").unwrap();
    let code = r#"
package com.example

interface CharSequence {
    val length: Int
}

class CustomSeq : CharSequence {
    override val length: Int = 10
}

fun run(seq: CharSequence) {
    val a = seq.length
    val b = "literal".length
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on CharSequence.length declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "length", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        2,
        "CharSequence.length should match parameter call and string literal receiver call"
    );
    assert_eq!(refs[0].range.start.line, 12);
    assert_eq!(refs[1].range.start.line, 13);
}

#[test]
fn test_references_candidate_pruning_only_treats_matching_return_types_as_producers() {
    let mut index = WorkspaceIndex::new();
    let file_pool_url = Url::parse("file:///project/Pool.kt").unwrap();
    let pool_code = r#"
package com.example

class WorkerPool {
    override fun toString(): String = "WorkerPool()"
}
"#;

    let file_usage_url = Url::parse("file:///project/Usage.kt").unwrap();
    let usage_code = r#"
package com.example

fun inspect(pool: WorkerPool) {
    val text = pool.toString()
}
"#;

    let file_unrelated_url = Url::parse("file:///project/Unrelated.kt").unwrap();
    let unrelated_code = r#"
package com.other

fun report() {
    val number = 42
    val text = number.toString()
}
"#;

    index.index_source(file_pool_url.clone(), pool_code);
    index.index_source(file_usage_url.clone(), usage_code);
    index.index_source(file_unrelated_url.clone(), unrelated_code);

    // Cursor on WorkerPool.toString declaration (line 4, col 17)
    let decl_pos = Position::new(4, 17);
    let refs = find_references(
        &index,
        &file_pool_url,
        "toString",
        decl_pos,
        pool_code,
        false,
    );

    assert_eq!(
        refs.len(),
        1,
        "Only the call on WorkerPool should match, unrelated toString calls are not references"
    );
    assert_eq!(refs[0].uri, file_usage_url);
    assert_eq!(refs[0].range.start.line, 4);
}

#[test]
fn test_references_container_hierarchy_handles_nullable_receiver_type() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NullableText.kt").unwrap();
    let code = r#"
package com.example

interface CharSequence {
    val length: Int
}

fun check(seq: CharSequence?) {
    val len = seq?.length
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on CharSequence.length declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "length", decl_pos, code, false);

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_float_literal_receiver_matches_float_target() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/FloatExt.kt").unwrap();
    let code = r#"
package com.example

fun Float.roundDecimals(): Float = this

fun run() {
    val a = 10.5f.roundDecimals()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on Float.roundDecimals declaration (line 3, col 10)
    let decl_pos = Position::new(3, 10);
    let refs = find_references(&index, &file_url, "roundDecimals", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Float literal receiver (10.5f) must match Float container target"
    );
    assert_eq!(refs[0].range.start.line, 6);
}

#[test]
fn test_references_long_literal_receiver_matches_long_target() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/LongExt.kt").unwrap();
    let code = r#"
package com.example

fun Long.toReadable(): String = ""

fun run() {
    val a = 100L.toReadable()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on Long.toReadable declaration (line 3, col 9)
    let decl_pos = Position::new(3, 9);
    let refs = find_references(&index, &file_url, "toReadable", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Long literal receiver (100L) must match Long container target"
    );
    assert_eq!(refs[0].range.start.line, 6);
}

#[test]
fn test_references_float_literal_receiver_matches_number_interface() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Numbers.kt").unwrap();
    let code = r#"
package com.example

interface Number {
    fun toInt(): Int
}

fun run() {
    val x = 10.5f.toInt()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on Number.toInt declaration (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "toInt", decl_pos, code, false);

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].range.start.line, 8);
}
