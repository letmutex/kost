use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_expression_type, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_extension_infix_expression_and_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///InfixExt.kt").unwrap();
    let code = r#"
infix fun Int.withUnit(unit: String): String = "$this $unit"

fun test() {
    val str = 1 withUnit "ms"
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "str", pos, code, None),
        KotlinType::parse("String")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "1 withUnit \"ms\"", pos, code, None),
        KotlinType::parse("String")
    );
}

#[test]
fn test_infer_member_infix_and_single_expression_chain() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Modifier.kt").unwrap();
    let code = r#"
object Dp
object Modifier {
    infix fun then(other: Modifier): Modifier {
        return other
    }
}

fun Modifier.padding(horizontal: Dp = Dp, vertical: Dp = Dp) =
    this then this

fun Modifier.padding(all: Dp) =
    this then this

fun test() {
    val modifier = Modifier.padding(Dp)
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(15, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "modifier", pos, code, None),
        KotlinType::parse("Modifier")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "Modifier.padding(Dp)", pos, code, None),
        KotlinType::parse("Modifier")
    );

    let padding_pos = Position::new(9, 6);
    assert_eq!(
        infer_expression_type(&index, &url, "this then this", padding_pos, code, None),
        KotlinType::parse("Modifier")
    );
}

#[test]
fn test_infer_member_infix_with_call_operand_and_single_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Modifier.kt").unwrap();
    let code = r#"
object Dp
class PaddingElement(val all: Dp)
object Modifier {
    infix fun then(other: Any): Modifier {
        return this
    }
}

fun Modifier.padding(all: Dp) =
    this then
        PaddingElement(
            all = all,
        )

fun test() {
    val modifier = Modifier.padding(Dp)
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(16, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "modifier", pos, code, None),
        KotlinType::parse("Modifier")
    );
}

#[test]
fn test_infer_generic_infix_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///GenericInfix.kt").unwrap();
    let code = r#"
class Pair<A, B>(val first: A, val second: B)
infix fun <A, B> A.to(that: B): Pair<A, B> = Pair(this, that)

fun test() {
    val pair = 1 to "hello"
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "pair", pos, code, None),
        KotlinType::parse("Pair<Int, String>")
    );
    assert_eq!(
        infer_expression_type(&index, &url, "1 to \"hello\"", pos, code, None),
        KotlinType::parse("Pair<Int, String>")
    );
}

#[test]
fn test_infer_chained_infix_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ChainInfix.kt").unwrap();
    let code = r#"
infix fun String.withPrefix(prefix: String): String = prefix + this

fun test() {
    val res = "world" withPrefix "hello " withPrefix ">> "
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "res", pos, code, None),
        KotlinType::parse("String")
    );
    assert_eq!(
        infer_expression_type(
            &index,
            &url,
            "\"world\" withPrefix \"hello \" withPrefix \">> \"",
            pos,
            code,
            None
        ),
        KotlinType::parse("String")
    );
}

#[test]
fn test_infer_infix_called_as_method() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///InfixMethod.kt").unwrap();
    let code = r#"
infix fun Int.withUnit(unit: String): String = "$this $unit"

fun test() {
    val strDot = 1.withUnit("ms")
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "strDot", pos, code, None),
        KotlinType::parse("String")
    );
}

#[test]
fn test_infix_expression_with_comment_trivia_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///InfixComment.kt").unwrap();
    let code = r#"
infix fun Int.withUnit(unit: String): String = "$this $unit"

fun test() {
    val str = 1 /* unit */ withUnit "ms"
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(4, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "str", pos, code, None),
        KotlinType::parse("String")
    );
}
