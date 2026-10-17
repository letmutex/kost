use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_expression_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_literal_expressions() {
    let index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Literals.kt").unwrap();
    let code = "fun test() {}";
    let pos = Position::new(0, 0);

    assert_eq!(
        infer_expression_type(&index, &url, "\"hello world\"", pos, code, None),
        KotlinType::parse("String")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "true", pos, code, None),
        KotlinType::parse("Boolean")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "false", pos, code, None),
        KotlinType::parse("Boolean")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "12345", pos, code, None),
        KotlinType::parse("Int")
    );
}

#[test]
fn test_infer_chained_safe_call_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SafeCall.kt").unwrap();
    let code = r#"
package com.example.safecall

class Address(val street: String, val city: String)
class User {
    fun getAddress(): Address? = Address("Main St", "Springfield")
}

fun test(user: User?) {
    val c = user?.getAddress()?.city
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 20);
    assert_eq!(
        infer_expression_type(&index, &url, "user?.getAddress()", pos, code, None),
        KotlinType::parse("com.example.safecall.Address?")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "user?.getAddress()?.city", pos, code, None),
        KotlinType::parse("String?")
    );
}

#[test]
fn test_infer_parenthesized_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Paren.kt").unwrap();
    let code = r#"
package com.example.paren

class User(val name: String)

fun test(user: User) {
    val n = ((user)).name
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 15);
    assert_eq!(
        infer_expression_type(&index, &url, "((user))", pos, code, None),
        KotlinType::parse("com.example.paren.User")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "((user)).name", pos, code, None),
        KotlinType::parse("String")
    );
}

#[test]
fn test_infer_elvis_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Elvis.kt").unwrap();
    let code = r#"
package com.example.elvis

class Config(val timeout: Int)

fun test(c1: Config?, c2: Config) {
    val res = (c1 ?: c2).timeout
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 20);
    assert_eq!(
        infer_expression_type(&index, &url, "c1 ?: c2", pos, code, None),
        KotlinType::parse("com.example.elvis.Config")
    );
}

#[test]
fn test_infer_enum_entry_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Enum.kt").unwrap();
    let code = r#"
package com.example.planets

enum class Planet(val mass: Double) {
    EARTH(5.97e24);
    fun surfaceGravity(): Double = mass * 9.8
}

fun test() {
    val m = Planet.EARTH.mass
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 20);
    let inferred_entry = infer_expression_type(&index, &url, "Planet.EARTH", pos, code, None);
    assert_eq!(
        inferred_entry,
        KotlinType::parse("com.example.planets.Planet")
    );
    assert_eq!(
        inferred_entry
            .as_ref()
            .map(|t| t.package_and_simple_name().1),
        Some("Planet")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "Planet.EARTH.mass", pos, code, None),
        KotlinType::parse("Double")
    );
}

#[test]
fn test_infer_labeled_this_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LabeledThis.kt").unwrap();
    let code = r#"
package com.example.labeled

class Window {
    val title: String = "Main"

    inner class Panel {
        val name: String = "Sidebar"

        fun render() {
            val t = this@Window.title
            val n = this@Panel.name
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 25);
    assert_eq!(
        infer_expression_type(&index, &url, "this@Window", pos, code, None),
        KotlinType::parse("com.example.labeled.Window")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "this@Panel", pos, code, None),
        KotlinType::parse("com.example.labeled.Window.Panel")
    );
}

#[test]
fn test_infer_super_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Super.kt").unwrap();
    let code = r#"
package com.example.superrec

open class BaseWorker {
    val id: String = "base"
}

class SpecializedWorker : BaseWorker() {
    fun execute() {
        val x = super.id
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 20);
    assert_eq!(
        infer_expression_type(&index, &url, "super", pos, code, None),
        KotlinType::parse("com.example.superrec.BaseWorker")
    );
}

#[test]
fn test_infer_numeric_literals_with_underscores() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NumLiterals.kt").unwrap();
    let code = "package com.example\n";
    index.index_source(url.clone(), code);

    let pos = Position::default();
    assert_eq!(
        infer_expression_type(&index, &url, "1_000_000", pos, code, None),
        KotlinType::parse("Int"),
        "1_000_000 should infer to Int"
    );
    assert_eq!(
        infer_expression_type(&index, &url, "10_000L", pos, code, None),
        KotlinType::parse("Long"),
        "10_000L should infer to Long"
    );
    assert_eq!(
        infer_expression_type(&index, &url, "3.14_15F", pos, code, None),
        KotlinType::parse("Float"),
        "3.14_15F should infer to Float"
    );
    assert_eq!(
        infer_expression_type(&index, &url, "1_000.0", pos, code, None),
        KotlinType::parse("Double"),
        "1_000.0 should infer to Double"
    );
}

#[test]
fn test_infer_hex_and_binary_literal_expressions() {
    let index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NumericLiterals.kt").unwrap();
    let code = "fun test() {}";
    let pos = Position::new(0, 0);

    // Valid hex and binary
    assert_eq!(
        infer_expression_type(&index, &url, "0x1A", pos, code, None),
        KotlinType::parse("Int")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "0XFF", pos, code, None),
        KotlinType::parse("Int")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "0b1010", pos, code, None),
        KotlinType::parse("Int")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "0B1010", pos, code, None),
        KotlinType::parse("Int")
    );

    // Invalid / bare prefixes must NOT infer as Int
    assert_eq!(
        infer_expression_type(&index, &url, "0x", pos, code, None),
        None,
        "Bare '0x' should not infer as Int"
    );
    assert_eq!(
        infer_expression_type(&index, &url, "0b", pos, code, None),
        None,
        "Bare '0b' should not infer as Int"
    );
    assert_eq!(
        infer_expression_type(&index, &url, "0xzzz", pos, code, None),
        None,
        "Invalid hex '0xzzz' should not infer as Int"
    );
}

#[test]
fn test_infer_parenthesized_multiline_cast_receiver_with_leading_comment() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SpecialCastRec.kt").unwrap();
    let code = r#"
package com.example

interface Worker
class Engineer(val lang: String) : Worker

fun test(w: Worker) {
    val x = (/* comment */ w as
        Engineer).lang
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 12);
    let inferred = infer_expression_type(
        &index,
        &url,
        "(/* comment */ w as\n        Engineer)",
        pos,
        code,
        None,
    );
    assert_eq!(inferred, KotlinType::parse("com.example.Engineer"));
}
