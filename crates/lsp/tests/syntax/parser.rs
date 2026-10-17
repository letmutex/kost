use lsp::features::completion_context::extract_receiver_and_partial;
use lsp::syntax::hierarchy::{extract_class_supertypes, extract_type_alias_target};
use lsp::syntax::parser::{KotlinParser, extract_return_type_from_signature, parse_source};
use lsp::syntax::signature::extract_receiver_type_from_signature;
use lsp::syntax::symbols::parse_file_symbols;

#[test]
fn test_extract_receiver_and_partial() {
    assert_eq!(
        extract_receiver_and_partial("user."),
        Some(("user".to_string(), "".to_string()))
    );
    assert_eq!(
        extract_receiver_and_partial("user.na"),
        Some(("user".to_string(), "na".to_string()))
    );
    assert_eq!(
        extract_receiver_and_partial("val x = service.getUser()."),
        Some(("service.getUser()".to_string(), "".to_string()))
    );
    assert_eq!(
        extract_receiver_and_partial("user?.name"),
        Some(("user".to_string(), "name".to_string()))
    );
    assert_eq!(extract_receiver_and_partial("val user = 10"), None);
}

#[test]
fn test_extract_return_type_from_signature() {
    assert_eq!(
        extract_return_type_from_signature("fun getUser(id: String): User"),
        Some("User".to_string())
    );
    assert_eq!(
        extract_return_type_from_signature("val name: String = \"Alice\""),
        Some("String".to_string())
    );
    assert_eq!(
        extract_return_type_from_signature("fun process(data: Data)"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("fun log(msg: String): Unit"),
        Some("Unit".to_string())
    );
    assert_eq!(
        extract_return_type_from_signature(
            "fun getUser(): User = User(\"1\", Address(\"Tokyo\", \"100\"))"
        ),
        Some("User".to_string())
    );
}

#[test]
fn test_extract_return_type_with_spaced_generics() {
    assert_eq!(
        extract_return_type_from_signature("fun getMap(): Map<String, User>"),
        Some("Map<String, User>".to_string()),
        "extract_return_type_from_signature should extract clean generic return type"
    );
}

#[test]
fn test_extract_return_type_functional_type() {
    assert_eq!(
        extract_return_type_from_signature("fun getHandler(): () -> Unit"),
        Some("() -> Unit".to_string()),
        "extract_return_type_from_signature should extract functional return type"
    );
    assert_eq!(
        extract_return_type_from_signature(
            "fun getCallback(): (Int, String) -> Boolean = { a, b -> true }"
        ),
        Some("(Int, String) -> Boolean".to_string()),
        "extract_return_type_from_signature should extract multi-param functional return type"
    );
}

#[test]
fn test_extract_return_type_from_signature_ignores_modified_class_and_interface() {
    assert_eq!(
        extract_return_type_from_signature("open class Base(val item: String) : Super"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature(
            "abstract class AbstractService<T>(val item: Int) : BaseService<T>"
        ),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("sealed interface Api : SuperApi"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("fun interface Action"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("internal class Helper(val x: Int)"),
        None
    );
}

#[test]
fn test_extract_receiver_type_from_extension_signature() {
    // Standard extension function and property
    assert_eq!(
        extract_receiver_type_from_signature("fun String.firstChar(): Char"),
        Some("String".to_string())
    );
    assert_eq!(
        extract_receiver_type_from_signature("val Int.dp: Dp"),
        Some("Int".to_string())
    );

    // Annotations with arguments and use-site targets
    assert_eq!(
        extract_receiver_type_from_signature("@Deprecated(\"use other\") val Int.dp: Dp"),
        Some("Int".to_string())
    );
    assert_eq!(
        extract_receiver_type_from_signature("@receiver:Stable inline val Int.dp: Dp"),
        Some("Int".to_string())
    );
    assert_eq!(
        extract_receiver_type_from_signature(
            "@androidx.annotation.FloatRange(from = 0.0) val Float.dp: Float"
        ),
        Some("Float".to_string())
    );

    // Function type receivers
    assert_eq!(
        extract_receiver_type_from_signature("fun (() -> Unit).start()"),
        Some("(() -> Unit)".to_string())
    );

    // Multiline and multi-space keywords
    assert_eq!(
        extract_receiver_type_from_signature("inline val\n    Int.dp: Dp"),
        Some("Int".to_string())
    );
    assert_eq!(
        extract_receiver_type_from_signature("val   Int.dp: Dp"),
        Some("Int".to_string())
    );

    // Generic extension property
    assert_eq!(
        extract_receiver_type_from_signature("val <T> List<T>.first: T"),
        Some("List<T>".to_string())
    );

    // Return type containing dots
    assert_eq!(
        extract_receiver_type_from_signature("val Int.dp: some.pkg.Dp"),
        Some("Int".to_string())
    );

    // Non-extension backtick identifier containing dots should not extract receiver
    assert_eq!(
        extract_receiver_type_from_signature("val `user.name`: String"),
        None
    );
}

#[test]
fn test_extract_receiver_type_with_complex_annotations_and_generics() {
    // Escaped quotes in annotation strings
    assert_eq!(
        extract_receiver_type_from_signature(r#"@Deprecated("use \"other\"") val Int.dp: Dp"#),
        Some("Int".to_string())
    );
    assert_eq!(
        extract_receiver_type_from_signature(r#"@Deprecated("use \" other") val Int.dp: Dp"#),
        Some("Int".to_string())
    );

    // Generic parameter bound with function type arrow
    assert_eq!(
        extract_receiver_type_from_signature("fun <T : () -> Unit> T.start()"),
        Some("T".to_string())
    );

    // Nullable function type receiver
    assert_eq!(
        extract_receiver_type_from_signature("fun (() -> Unit)?.start()"),
        Some("(() -> Unit)?".to_string())
    );

    // Generic receiver with function type argument containing arrow
    assert_eq!(
        extract_receiver_type_from_signature("fun Producer<() -> Unit>.produce()"),
        Some("Producer<() -> Unit>".to_string())
    );
}

#[test]
fn test_extract_return_type_class_with_modifiers_returns_none() {
    assert_eq!(
        extract_return_type_from_signature("open class Person(val name: String) : Base()"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("abstract class Animal : Living"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("sealed interface Action"),
        None
    );
    assert_eq!(
        extract_return_type_from_signature("inner class Item(val id: Int)"),
        None
    );
}

#[test]
fn test_extract_class_supertypes_excludes_where_clause_bounds() {
    let code1 = "interface CopyableThrowable<T> where T : Throwable, T : CopyableThrowable<T>";
    let tree1 = parse_source(code1).unwrap();
    let root1 = tree1.root_node();
    let mut cursor1 = root1.walk();
    let class_node1 = root1
        .children(&mut cursor1)
        .find(|n| n.kind() == "class_declaration")
        .expect("class_declaration node should exist");

    let supers1 = extract_class_supertypes(&class_node1, code1);
    assert!(
        supers1.is_empty(),
        "Where clause bounds must not be extracted as class supertypes: {:?}",
        supers1
    );

    let code2 = "class CustomError<T> : BaseError() where T : Throwable, T : CustomError<T>";
    let tree2 = parse_source(code2).unwrap();
    let root2 = tree2.root_node();
    let mut cursor2 = root2.walk();
    let class_node2 = root2
        .children(&mut cursor2)
        .find(|n| n.kind() == "class_declaration")
        .expect("class_declaration node should exist");

    let supers2 = extract_class_supertypes(&class_node2, code2);
    assert_eq!(
        supers2,
        vec!["BaseError".to_string()],
        "Only BaseError should be extracted as supertype, excluding where clause bounds: {:?}",
        supers2
    );
}

#[test]
fn test_extract_class_supertypes_with_where_like_name() {
    let code = "class Service : Somewhere<Data>() where T : Constraint";
    let tree = parse_source(code).unwrap();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let class_node = root
        .children(&mut cursor)
        .find(|n| n.kind() == "class_declaration")
        .expect("class_declaration node should exist");

    let supers = extract_class_supertypes(&class_node, code);
    assert_eq!(
        supers,
        vec!["Somewhere<Data>".to_string()],
        "Somewhere<Data> should be extracted without where clause bounds: {:?}",
        supers
    );
}

#[test]
fn test_extract_type_alias_target_with_inline_comment() {
    let code = "typealias Callback = /* inline doc */ () -> Unit";
    let tree = parse_source(code).unwrap();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let alias_node = root
        .children(&mut cursor)
        .find(|n| n.kind() == "type_alias")
        .expect("type_alias node should exist");

    let target = extract_type_alias_target(&alias_node, code);
    assert_eq!(target.as_deref(), Some("() -> Unit"));
}

#[test]
fn test_parse_import_alias_with_inline_comment() {
    let mut parser = KotlinParser::new();
    let code = "import com.example.Foo as /* inline doc */ RenamedFoo";
    let symbols = parse_file_symbols(&mut parser, code).expect("symbols should parse");
    assert_eq!(symbols.imports.len(), 1);
    assert_eq!(symbols.imports[0].path, "com.example.Foo");
    assert_eq!(symbols.imports[0].alias.as_deref(), Some("RenamedFoo"));
}

#[test]
fn test_extract_type_alias_target_with_suspend_modifier() {
    let code = "typealias Handler = suspend () -> Unit";
    let tree = parse_source(code).unwrap();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let alias_node = root
        .children(&mut cursor)
        .find(|n| n.kind() == "type_alias")
        .expect("type_alias node should exist");

    let target = extract_type_alias_target(&alias_node, code);
    assert_eq!(target.as_deref(), Some("suspend () -> Unit"));
}
