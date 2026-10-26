use lsp::syntax::signature::extract_type_arguments_list;
use lsp::syntax::signature::{
    extract_functional_input_types, extract_functional_return_type, find_param_colon,
    find_parameter_list_bounds, parse_signature_type_params, parse_single_param,
    split_type_arguments,
};
use smol_str::SmolStr;

#[test]
fn test_parse_signature_type_params_with_where_clause_and_default_parameter_value() {
    let params = parse_signature_type_params(
        "fun <T> copy(item: T, count: Int = 1): Unit where T : CharSequence",
    );
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert_eq!(params[0].bounds, vec![SmolStr::new("CharSequence")]);
}

#[test]
fn test_find_param_colon_ignores_double_colon_callable_reference() {
    assert_eq!(find_param_colon("action: () -> Unit = Foo::bar"), Some(6));
    assert_eq!(find_param_colon("Foo::bar"), None);
}

#[test]
fn test_parse_signature_type_params_ignores_double_colon_class_in_annotation() {
    let params =
        parse_signature_type_params("@OptIn(Marker::class) fun List<String>.process(): Unit");
    assert_eq!(
        params,
        vec![],
        "Extension receiver generics must not be treated as class type params due to Marker::class"
    );
}

#[test]
fn test_split_type_arguments_with_comma_inside_string_literal() {
    let args = split_type_arguments("\"foo,bar\", Baz");
    assert_eq!(args, vec!["\"foo,bar\"", "Baz"]);
}

#[test]
fn test_extract_type_arguments_list_from_functional_receiver_and_parameter_types() {
    let args1 = extract_type_arguments_list("(List<String>) -> Unit");
    assert_eq!(
        args1,
        vec!["String"],
        "Functional parameter List<String> should extract [String], but rfind('>') matched the function arrow"
    );

    let args2 = extract_type_arguments_list("List<String>.() -> Int");
    assert_eq!(
        args2,
        vec!["String"],
        "Functional receiver List<String> should extract [String], but rfind('>') matched the function arrow"
    );
}

#[test]
fn test_split_type_arguments_with_square_brackets_array_literal() {
    let args = split_type_arguments("[A, B], C");
    assert_eq!(
        args,
        vec!["[A, B]", "C"],
        "Square brackets in array literals should not be split at internal commas"
    );
}

#[test]
fn test_parse_signature_type_params_ignores_parameter_named_where() {
    let params = parse_signature_type_params("fun <T> test(where: Int, T: String): Unit");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert_eq!(
        params[0].bounds,
        Vec::<SmolStr>::new(),
        "Parameter named 'where' inside parameter list must not be parsed as a where-clause"
    );
}

#[test]
fn test_extract_type_arguments_list_with_greater_than_inside_annotation_string() {
    let args = extract_type_arguments_list("Map<@Anno(\">\") String, Int>");
    assert_eq!(
        args,
        vec!["@Anno(\">\") String", "Int"],
        "Angle brackets inside string literals must not close generic type arguments list prematurely"
    );
}

#[test]
fn test_parse_signature_type_params_with_closing_paren_in_default_string_and_parameter_named_where()
{
    let params = parse_signature_type_params(
        "fun <T> test(sep: String = \")\", where: Int): Unit where T : Number",
    );
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert_eq!(params[0].bounds, vec![SmolStr::new("Number")]);
}

#[test]
fn test_parse_single_param_ignores_vararg_in_default_string_literal() {
    let param = parse_single_param("message: String = \"error: vararg mismatch\"")
        .expect("Should parse parameter with default string literal");
    assert_eq!(param.name, "message");
    assert_eq!(param.type_name, "String");
    assert!(
        !param.is_vararg,
        "Substring 'vararg' inside default string value must not mark parameter as vararg"
    );
}

#[test]
fn test_extract_functional_input_types_with_nested_and_modifiers() {
    assert_eq!(
        extract_functional_input_types("((x: Int) -> String) -> Unit"),
        vec!["(x: Int) -> String".to_string()]
    );
    assert_eq!(
        extract_functional_input_types("(a: (Int) -> Unit, b: () -> String) -> Unit"),
        vec!["(Int) -> Unit".to_string(), "() -> String".to_string()]
    );
    assert_eq!(
        extract_functional_input_types("(action: suspend (Int) -> Unit) -> Unit"),
        vec!["suspend (Int) -> Unit".to_string()]
    );
    assert_eq!(
        extract_functional_input_types("(suspend (Int) -> Unit) -> Unit"),
        vec!["suspend (Int) -> Unit".to_string()]
    );
    assert_eq!(
        extract_functional_input_types("(Int, suspend (String) -> Unit) -> Unit"),
        vec!["Int".to_string(), "suspend (String) -> Unit".to_string()]
    );
    assert_eq!(
        extract_functional_input_types("(suspend (Int) -> String)?"),
        vec!["Int".to_string()]
    );
}

#[test]
fn test_extract_functional_return_type_nested_and_modifiers() {
    assert_eq!(
        extract_functional_return_type("(Int) -> (String) -> Unit"),
        Some("(String) -> Unit".to_string())
    );
    assert_eq!(
        extract_functional_return_type("(Int) -> (String)"),
        Some("String".to_string())
    );
    assert_eq!(
        extract_functional_return_type("((Int) -> String)?"),
        Some("String".to_string())
    );
    assert_eq!(
        extract_functional_return_type("(T) -> R?"),
        Some("R?".to_string())
    );
    assert_eq!(
        extract_functional_return_type("() -> String?"),
        Some("String?".to_string())
    );
    assert_eq!(
        extract_functional_return_type("(suspend (Int) -> String)?"),
        Some("String".to_string())
    );
    assert_eq!(
        extract_functional_return_type("() -> (suspend (Int) -> Unit)"),
        Some("suspend (Int) -> Unit".to_string())
    );
    assert_eq!(
        extract_functional_return_type("(Int) -> suspend (String) -> Unit"),
        Some("suspend (String) -> Unit".to_string())
    );
    assert_eq!(
        extract_functional_return_type("() -> suspend () -> Unit"),
        Some("suspend () -> Unit".to_string())
    );
}

#[test]
fn test_find_parameter_list_bounds_extracts_nested_and_empty_ranges() {
    let empty_sig = "fun execute(): Boolean";
    let (open_idx, close_idx) = find_parameter_list_bounds(empty_sig).unwrap();
    assert_eq!(&empty_sig[open_idx..=close_idx], "()");

    let nested_sig = "fun process(handler: (Int) -> String, count: Int): Unit";
    let (nested_open, nested_close) = find_parameter_list_bounds(nested_sig).unwrap();
    assert_eq!(
        &nested_sig[nested_open..=nested_close],
        "(handler: (Int) -> String, count: Int)"
    );

    let no_parens = "dispatch";
    assert_eq!(find_parameter_list_bounds(no_parens), None);
}

#[test]
fn test_extract_param_type_by_index_strips_default_value() {
    let sig = "fun runTest(expected: ((Throwable) -> Boolean)? = null, count: Int = 1, block: Scope.() -> Unit)";
    assert_eq!(
        lsp::syntax::signature::extract_param_type_by_index(sig, 0),
        Some("((Throwable) -> Boolean)?".to_string())
    );
    assert_eq!(
        lsp::syntax::signature::extract_param_type_by_index(sig, 1),
        Some("Int".to_string())
    );
    assert_eq!(
        lsp::syntax::signature::extract_param_type_by_index(sig, 2),
        Some("Scope.() -> Unit".to_string())
    );
}

#[test]
fn test_extract_param_type_by_index_ignores_colon_in_annotation_string() {
    let sig = r#"fun execute(@Named("tag:custom") id: String = "val:1")"#;
    assert_eq!(
        lsp::syntax::signature::extract_param_type_by_index(sig, 0),
        Some("String".to_string())
    );
}
