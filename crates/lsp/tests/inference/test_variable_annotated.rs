use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_expression_type, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_variable_from_trailing_lambda_generic_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LambdaGuardTest.kt").unwrap();
    let code = r#"
package com.example.guard

class Dummy(val text: String)
@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val state = remember {
        Dummy("hello")
    }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(16, 8);
    let inferred = infer_variable_type(&index, &url, "state", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.guard.Dummy"),
        "Trailing lambda calls like remember {{}} should infer Dummy from declaration"
    );
}

#[test]
fn test_infer_variable_inside_annotated_lambda_scope() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ThemeTest.kt").unwrap();
    let code = r#"
package com.example.theme

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun AppTheme(content: @Composable () -> Unit) {}

fun test() {
    AppTheme {
        val state = remember { 10 }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(15, 12);
    let inferred = infer_variable_type(&index, &url, "state", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_variable_from_annotated_function_defaulting_to_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PaddingTest.kt").unwrap();
    let code = r#"
object Dp
object Modifier

@Stable
fun Modifier.padding(start: Dp, top: Dp = Dp, end: Dp = Dp, bottom: Dp = Dp) {}

val modifier = Modifier.padding(Dp)
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "modifier", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Unit"));
    assert_eq!(
        infer_expression_type(&index, &url, "Modifier.padding(Dp)", pos, code, None),
        KotlinType::parse("Unit")
    );
}

#[test]
fn test_infer_unit_literal_and_expressions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnitTest.kt").unwrap();
    let code = r#"
fun doWork() {}
fun doWorkExplicit(): Unit {}

fun test() {
    val u1 = Unit
    val u2 = kotlin.Unit
    val u3 = doWork()
    val u4 = doWorkExplicit()
    val u5 = while (false) {}
    val u6 = for (i in 0..1) {}
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "u1", pos, code, None),
        Some(KotlinType::UNIT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "u2", pos, code, None),
        Some(KotlinType::UNIT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "u3", pos, code, None),
        Some(KotlinType::UNIT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "u4", pos, code, None),
        Some(KotlinType::UNIT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "u5", pos, code, None),
        Some(KotlinType::UNIT)
    );
    assert_eq!(
        infer_variable_type(&index, &url, "u6", pos, code, None),
        Some(KotlinType::UNIT)
    );
}

#[test]
fn test_infer_variable_from_annotated_extension_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DimensionsTest.kt").unwrap();
    let code = r#"
object Dp
object TextUnit

@Stable
inline val Int.dp: Dp
@Stable
inline val Int.sp: TextUnit  { return TextUnit }

val width = 2.dp
val fontSize = 12.sp
"#;
    index.index_source(url.clone(), code);

    let pos_width = Position::new(9, 4);
    let inferred_width = infer_variable_type(&index, &url, "width", pos_width, code, None);
    assert_eq!(inferred_width, KotlinType::parse("Dp"));

    let pos_font_size = Position::new(10, 4);
    let inferred_font_size =
        infer_variable_type(&index, &url, "fontSize", pos_font_size, code, None);
    assert_eq!(inferred_font_size, KotlinType::parse("TextUnit"));
}

#[test]
fn test_infer_variable_from_extension_property_with_parameterized_annotation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DeprecatedTest.kt").unwrap();
    let code = r#"
object Dp

@Deprecated("Use new API")
val Int.deprecatedDp: Dp = Dp

val width = 5.deprecatedDp
"#;
    index.index_source(url.clone(), code);

    let pos_width = Position::new(6, 4);
    let inferred_width = infer_variable_type(&index, &url, "width", pos_width, code, None);
    assert_eq!(inferred_width, KotlinType::parse("Dp"));
}
