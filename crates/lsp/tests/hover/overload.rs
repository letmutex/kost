use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_top_level_overload_by_primitive_argument() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/OverloadHover.kt").unwrap();
    let code = r#"
package com.example.test

fun render(value: Int): String = "Int: $value"
fun render(value: Double): String = "Double: $value"
fun render(value: String): String = "String: $value"

fun test() {
    val a = render(10)
    val b = render(3.14)
    val c = render("hello")
}
"#;
    index.index_source(url.clone(), code);

    // Hover on `render(10)` at line 8, col 14
    let pos_a = Position::new(8, 14);
    let hover_a = find_hover(&index, &url, "render", pos_a, code)
        .expect("Hover for render(10) should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_a.contents {
        assert!(
            text.contains("fun render(value: Int): String"),
            "Hover for render(10) should show Int overload: {}",
            text
        );
    }

    // Hover on `render(3.14)` at line 9, col 14
    let pos_b = Position::new(9, 14);
    let hover_b = find_hover(&index, &url, "render", pos_b, code)
        .expect("Hover for render(3.14) should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_b.contents {
        assert!(
            text.contains("fun render(value: Double): String"),
            "Hover for render(3.14) should show Double overload: {}",
            text
        );
    }

    // Hover on `render("hello")` at line 10, col 14
    let pos_c = Position::new(10, 14);
    let hover_c = find_hover(&index, &url, "render", pos_c, code)
        .expect("Hover for render(\"hello\") should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_c.contents {
        assert!(
            text.contains("fun render(value: String): String"),
            "Hover for render(\"hello\") should show String overload: {}",
            text
        );
    }
}

#[test]
fn test_hover_top_level_overload_by_arity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArityHover.kt").unwrap();
    let code = r#"
package com.example.test

fun transform(item: String): String = item
fun transform(item: String, prefix: String): String = "$prefix$item"

fun test() {
    val r1 = transform("test")
    val r2 = transform("test", ">> ")
}
"#;
    index.index_source(url.clone(), code);

    let pos_r1 = Position::new(7, 14);
    let hover_r1 = find_hover(&index, &url, "transform", pos_r1, code)
        .expect("Hover on transform(\"test\") should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_r1.contents {
        assert!(
            text.contains("fun transform(item: String): String"),
            "Hover on 1-arg transform should show 1-arg signature: {}",
            text
        );
    }

    let pos_r2 = Position::new(8, 14);
    let hover_r2 = find_hover(&index, &url, "transform", pos_r2, code)
        .expect("Hover on transform(\"test\", \">> \") should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_r2.contents {
        assert!(
            text.contains("fun transform(item: String, prefix: String): String"),
            "Hover on 2-arg transform should show 2-arg signature: {}",
            text
        );
    }
}

#[test]
fn test_hover_member_overload_by_named_arguments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NamedHover.kt").unwrap();
    let code = r#"
package com.example.test

class Dp
class Modifier

class Layout {
    fun padding(all: Dp): String = "all"
    fun padding(horizontal: Dp = Dp(), vertical: Dp = Dp()): Modifier = Modifier()
}

fun test(l: Layout) {
    val m1 = l.padding(Dp())
    val m2 = l.padding(horizontal = Dp(), vertical = Dp())
}
"#;
    index.index_source(url.clone(), code);

    let pos_m1 = Position::new(12, 16);
    let hover_m1 = find_hover(&index, &url, "padding", pos_m1, code)
        .expect("Hover on padding(Dp()) should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_m1.contents {
        assert!(
            text.contains("fun padding(all: Dp): String"),
            "Hover on 1-arg padding should show all: Dp: {}",
            text
        );
    }

    let pos_m2 = Position::new(13, 16);
    let hover_m2 = find_hover(&index, &url, "padding", pos_m2, code)
        .expect("Hover on padding(horizontal = ..., vertical = ...) should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_m2.contents {
        assert!(
            text.contains("fun padding(horizontal: Dp = Dp(), vertical: Dp = Dp()): Modifier"),
            "Hover on named args padding should show horizontal/vertical overload: {}",
            text
        );
    }
}

#[test]
fn test_hover_extension_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionOverload.kt").unwrap();
    let code = r#"
package com.example.test

fun String.pad(length: Int): String = this
fun String.pad(length: Int, char: Char): String = this

fun test() {
    val s1 = "abc".pad(5)
    val s2 = "abc".pad(5, '0')
}
"#;
    index.index_source(url.clone(), code);

    let pos_s1 = Position::new(7, 20);
    let hover_s1 =
        find_hover(&index, &url, "pad", pos_s1, code).expect("Hover on 1-arg pad should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_s1.contents {
        assert!(
            text.contains("fun String.pad(length: Int): String"),
            "Hover on 1-arg pad should match 1-arg overload: {}",
            text
        );
    }

    let pos_s2 = Position::new(8, 20);
    let hover_s2 =
        find_hover(&index, &url, "pad", pos_s2, code).expect("Hover on 2-arg pad should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_s2.contents {
        assert!(
            text.contains("fun String.pad(length: Int, char: Char): String"),
            "Hover on 2-arg pad should match 2-arg overload: {}",
            text
        );
    }
}

#[test]
fn test_hover_partial_named_argument_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AreaHoverOverload.kt").unwrap();
    let code = r#"
package com.example.test

fun area(size: Int): Double = size.toDouble() * size.toDouble()
fun area(width: Int, height: Int): Int = width * height

fun test() {
    area(width = 5)
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let hover = find_hover(&index, &url, "area", pos, code)
        .expect("Hover on area(width = 5) should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("fun area(width: Int, height: Int): Int"),
            "Hover on area(width = 5) should match area(width, height): {}",
            text
        );
    }
}

#[test]
fn test_hover_zero_argument_call_prioritizes_parameterless_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ZeroArgHover.kt").unwrap();
    let code = r#"
package com.example.test

fun reset(timeout: Long, hard: Boolean): Unit {}
fun reset(): Unit {}

fun test() {
    reset()
}
"#;
    index.index_source(url.clone(), code);

    // Hover on `reset()` at line 7, col 4
    let pos = Position::new(7, 4);
    let hover =
        find_hover(&index, &url, "reset", pos, code).expect("Hover on reset() should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("fun reset(): Unit"),
            "Hover on 0-arg call reset() should show parameterless overload, got: {}",
            text
        );
    }
}
