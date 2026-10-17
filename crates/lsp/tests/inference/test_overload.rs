use lsp::features::completion::find_completions;
use lsp::features::hover::find_hover;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_infer_top_level_overload_by_primitive_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/OverloadPrimitive.kt").unwrap();
    let code = r#"
package com.example.test

fun format(x: Int): String = "Int: $x"
fun format(x: Double): Double = x

fun test() {
    val a = format(10)
    val b = format(3.14)
}
"#;
    index.index_source(url.clone(), code);

    let pos_a = Position::new(7, 8);
    let inferred_a = infer_variable_type(&index, &url, "a", pos_a, code, None);
    assert_eq!(inferred_a, KotlinType::parse("String"));

    let pos_b = Position::new(8, 8);
    let inferred_b = infer_variable_type(&index, &url, "b", pos_b, code, None);
    assert_eq!(inferred_b, KotlinType::parse("Double"));
}

#[test]
fn test_infer_top_level_overload_by_arity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/OverloadArity.kt").unwrap();
    let code = r#"
package com.example.test

class SingleResult(val id: Int)
class MultiResult(val id: Int, val tag: String)

fun process(id: Int): SingleResult = SingleResult(id)
fun process(id: Int, tag: String): MultiResult = MultiResult(id, tag)

fun test() {
    val r1 = process(1)
    val r2 = process(1, "tag")
}
"#;
    index.index_source(url.clone(), code);

    let pos_r1 = Position::new(10, 8);
    let inferred_r1 = infer_variable_type(&index, &url, "r1", pos_r1, code, None);
    assert_eq!(
        inferred_r1,
        KotlinType::parse("com.example.test.SingleResult")
    );

    let pos_r2 = Position::new(11, 8);
    let inferred_r2 = infer_variable_type(&index, &url, "r2", pos_r2, code, None);
    assert_eq!(
        inferred_r2,
        KotlinType::parse("com.example.test.MultiResult")
    );
}

#[test]
fn test_infer_member_overload_by_named_arguments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/OverloadNamed.kt").unwrap();
    let code = r#"
package com.example.test

class Modifier
class Dp

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

    let pos_m1 = Position::new(12, 8);
    let inferred_m1 = infer_variable_type(&index, &url, "m1", pos_m1, code, None);
    assert_eq!(inferred_m1, KotlinType::parse("String"));

    let pos_m2 = Position::new(13, 8);
    let inferred_m2 = infer_variable_type(&index, &url, "m2", pos_m2, code, None);
    assert_eq!(inferred_m2, KotlinType::parse("com.example.test.Modifier"));
}

#[test]
fn test_hover_overloaded_function_call_matches_active_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/HoverOverload.kt").unwrap();
    let code = r#"
package com.example.test

fun calculate(value: Int): String = "int"
fun calculate(value: Double): Double = 0.0

fun test() {
    val x = calculate(3.14)
}
"#;
    index.index_source(url.clone(), code);

    let hover_pos = Position::new(7, 14);
    let hover = find_hover(&index, &url, "calculate", hover_pos, code).expect("Hover should exist");

    let HoverContents::Scalar(MarkedString::String(md)) = hover.contents else {
        panic!("Expected scalar markdown hover content");
    };

    assert!(
        md.contains("fun calculate(value: Double): Double"),
        "Hover markdown should display the Double overload: {}",
        md
    );
}

#[test]
fn test_completion_member_preserves_distinct_overloads() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompletionOverload.kt").unwrap();
    let code = r#"
package com.example.test

class Dp
class Modifier {
    fun padding(all: Dp): Modifier = this
    fun padding(horizontal: Dp, vertical: Dp): Modifier = this
}

fun test(m: Modifier) {
    m.pad
}
"#;
    index.index_source(url.clone(), code);

    let completion_pos = Position::new(10, 9);
    let items = find_completions(&index, &url, completion_pos, code);
    let padding_items: Vec<_> = items.iter().filter(|i| i.label == "padding").collect();

    assert_eq!(
        padding_items.len(),
        2,
        "Should contain both padding overloads in completions"
    );
    assert!(
        padding_items
            .iter()
            .any(|i| i.detail.as_deref().is_some_and(|d| d.contains("all: Dp")))
    );
    assert!(padding_items.iter().any(|i| {
        i.detail
            .as_deref()
            .is_some_and(|d| d.contains("horizontal: Dp"))
    }));
}

#[test]
fn test_partial_named_argument_overload_matching() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AreaOverload.kt").unwrap();
    let code = r#"
package com.example.test

fun area(size: Int): Double = size.toDouble() * size.toDouble()
fun area(width: Int, height: Int): Int = width * height

fun test() {
    val r = area(width = 5)
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "r", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("Int"),
        "area(width = 5) should resolve to area(width, height): Int"
    );
}

#[test]
fn test_infer_overload_fqn_vs_simple_name_compatibility() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FqnOverload.kt").unwrap();
    let code = r#"
package com.example.test

class Dp(val value: Int)
class Px(val value: Int)

fun pad(d: Dp): String = "dp"
fun pad(p: Px): Int = 0

fun test() {
    val r1 = pad(Dp(5))
    val r2 = pad(Px(10))
}
"#;
    index.index_source(url.clone(), code);

    let pos_r1 = Position::new(10, 8);
    let inferred_r1 = infer_variable_type(&index, &url, "r1", pos_r1, code, None);
    assert_eq!(
        inferred_r1,
        KotlinType::parse("String"),
        "pad(Dp(5)) should resolve to pad(d: Dp): String"
    );

    let pos_r2 = Position::new(11, 8);
    let inferred_r2 = infer_variable_type(&index, &url, "r2", pos_r2, code, None);
    assert_eq!(
        inferred_r2,
        KotlinType::parse("Int"),
        "pad(Px(10)) should resolve to pad(p: Px): Int"
    );
}

#[test]
fn test_infer_overload_positional_after_named_argument() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MixedNamedOverload.kt").unwrap();
    let code = r#"
package com.example.test

fun render(width: Int, height: Double): String = "dims"
fun render(width: Int, title: String): Int = 42

fun test() {
    val x = render(title = "banner", 100)
}
"#;
    index.index_source(url.clone(), code);

    let pos_x = Position::new(7, 8);
    let inferred_x = infer_variable_type(&index, &url, "x", pos_x, code, None);
    assert_eq!(
        inferred_x,
        KotlinType::parse("Int"),
        "render(title = \"banner\", 100) should resolve to render(width: Int, title: String): Int"
    );
}

#[test]
fn test_infer_extension_overload_by_receiver_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionOverload.kt").unwrap();
    let code = r#"
class Text
class Block

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()

fun <T, R> T.map(transform: (T) -> R): R = transform(this)

val block = Text().map {  Block() }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 4);
    let inferred = infer_variable_type(&index, &url, "block", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Block"));
}

#[test]
fn test_infer_extension_overload_prefers_specific_receiver_over_generic() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionSpecificity.kt").unwrap();
    let code = r#"
class Text
class Block
interface List<E> : Iterable<E>

fun <T, R> T.map(transform: (T) -> R): R = transform(this)
fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()

fun test(items: List<Text>) {
    val blocks = items.map { Block() }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "blocks", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("List<Block>"));
}

#[test]
fn test_infer_extension_overload_bounded_receiver_preferred() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionBounded.kt").unwrap();
    let code = r#"
fun <T> T.process(block: () -> Unit): Int = 0
fun <T : CharSequence> T.process(block: () -> Unit): String = ""

val res = "hello".process { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}
