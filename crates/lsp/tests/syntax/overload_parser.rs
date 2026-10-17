use lsp::syntax::ast_call_view::{find_top_level_eq, parse_call_arg};
use lsp::syntax::signature::{
    parse_signature_params, parse_signature_type_params, parse_single_param, split_type_arguments,
};

#[test]
fn test_parse_call_arg_empty_name_does_not_treat_as_named() {
    let arg1 = parse_call_arg("= 5");
    assert_eq!(
        arg1.name, None,
        "Leading '=' should not produce an empty parameter name"
    );
    assert_eq!(arg1.expr, "= 5");

    let arg2 = parse_call_arg("=5");
    assert_eq!(
        arg2.name, None,
        "Leading '=' without spaces should not produce an empty parameter name"
    );
    assert_eq!(arg2.expr, "=5");

    let arg3 = parse_call_arg(" = 5");
    assert_eq!(
        arg3.name, None,
        "Leading space before '=' should not produce an empty parameter name"
    );
    assert_eq!(arg3.expr, "= 5");
}

#[test]
fn test_parse_single_param_with_annotations_and_modifiers() {
    let p1 = parse_single_param("@Deprecated val name: String")
        .expect("Should parse annotated parameter");
    assert_eq!(
        p1.name, "name",
        "Parameter name should be extracted without annotations/modifiers"
    );
    assert_eq!(p1.type_name, "String");

    let p2 = parse_single_param("@Composable content: () -> Unit")
        .expect("Should parse composable lambda parameter");
    assert_eq!(
        p2.name, "content",
        "Parameter name should be extracted without @Composable annotation"
    );
    assert_eq!(p2.type_name, "() -> Unit");

    let p3 = parse_single_param("override val width: Int = 0")
        .expect("Should parse override modifier parameter");
    assert_eq!(
        p3.name, "width",
        "Parameter name should be extracted without override modifier"
    );
    assert_eq!(p3.type_name, "Int");
    assert!(p3.has_default);
}

#[test]
fn test_parse_signature_params_function_type_extension_receiver() {
    let params1 = parse_signature_params("fun (() -> Unit).invoke(arg: Int): Unit");
    assert_eq!(
        params1.len(),
        1,
        "Should parse the parameters of the function, not the function-type receiver"
    );
    assert_eq!(params1[0].name, "arg");
    assert_eq!(params1[0].type_name, "Int");

    let params2 = parse_signature_params("fun (String, Int).format(prefix: String): String");
    assert_eq!(
        params2.len(),
        1,
        "Should parse the parameters after the receiver and function identifier"
    );
    assert_eq!(params2[0].name, "prefix");
    assert_eq!(params2[0].type_name, "String");
}

#[test]
fn test_parse_signature_params_qualified_return_and_generics() {
    let params1 = parse_signature_params("fun <T> List<T>.process(x: Int): java.lang.String");
    assert_eq!(params1.len(), 1);
    assert_eq!(params1[0].name, "x");
    assert_eq!(params1[0].type_name, "Int");

    let params2 = parse_signature_params("fun (() -> Unit).invoke(arg: Int): java.lang.String");
    assert_eq!(params2.len(), 1);
    assert_eq!(params2[0].name, "arg");
    assert_eq!(params2[0].type_name, "Int");

    let params3 = parse_signature_params("fun getHandler(): () -> Unit");
    assert_eq!(params3.len(), 0);

    let params4 =
        parse_signature_params("fun <T : Comparable<T>> sort(items: List<T>): java.util.List<T>");
    assert_eq!(params4.len(), 1);
    assert_eq!(params4[0].name, "items");
    assert_eq!(params4[0].type_name, "List<T>");
}

#[test]
fn test_parse_single_param_with_use_site_annotation() {
    let p1 = parse_single_param(r#"@param:Named("config") timeout: Long"#)
        .expect("Should parse parameter with use-site annotation");
    assert_eq!(
        p1.name, "timeout",
        "Parameter name should be 'timeout', not '@param'"
    );
    assert_eq!(p1.type_name, "Long");

    let p2 = parse_single_param(r#"@field:SerializedName("user_id") id: String = """#)
        .expect("Should parse parameter with @field use-site annotation");
    assert_eq!(p2.name, "id", "Parameter name should be 'id', not '@field'");
    assert_eq!(p2.type_name, "String");
    assert!(p2.has_default);
}

#[test]
fn test_find_top_level_eq_ignores_compound_operators_and_strings() {
    assert_eq!(
        find_top_level_eq("a == b"),
        None,
        "Equality operator '==' must not be treated as top-level parameter default '='"
    );
    assert_eq!(
        find_top_level_eq("a === b"),
        None,
        "Identity operator '===' must not be treated as top-level parameter default '='"
    );
    assert_eq!(
        find_top_level_eq("\"name=value\""),
        None,
        "'=' inside a string literal must not be treated as top-level parameter default '='"
    );
}

#[test]
fn test_parse_signature_params_with_function_type_generic_constraint() {
    let sig = "fun <T : () -> Unit, R : (String) -> Int> transform(item: R): String";
    let params = parse_signature_params(sig);
    assert_eq!(params.len(), 1, "Should find exactly 1 parameter 'item'");
    assert_eq!(params[0].name, "item");
    assert_eq!(params[0].type_name, "R");
}

#[test]
fn test_parse_call_arg_does_not_treat_leading_digit_as_named_parameter() {
    let arg = parse_call_arg("123 = 456");
    assert_eq!(
        arg.name, None,
        "Leading digit in '123 = 456' must not be parsed as a named parameter"
    );
    assert_eq!(arg.expr, "123 = 456");
}

#[test]
fn test_parse_call_arg_accepts_backtick_escaped_identifier() {
    let arg = parse_call_arg("`when` = true");
    assert_eq!(
        arg.name.as_deref(),
        Some("when"),
        "Backtick-escaped keyword parameter '`when`' should be parsed with name 'when'"
    );
    assert_eq!(arg.expr, "true");

    let arg_in = parse_call_arg("`in` = 42");
    assert_eq!(
        arg_in.name.as_deref(),
        Some("in"),
        "Backtick-escaped keyword parameter '`in`' should be parsed with name 'in'"
    );
    assert_eq!(arg_in.expr, "42");
}

#[test]
fn test_parse_signature_type_params_ignores_extension_receiver_generics() {
    let params = parse_signature_type_params("fun List<String>.sum(): Int");
    assert!(
        params.is_empty(),
        "Non-generic extension function on List<String> must not parse 'String' as a type parameter"
    );
}

#[test]
fn test_parse_signature_type_params_ignores_supertype_arguments_in_class_signature() {
    let params = parse_signature_type_params("class StringList : ArrayList<String>()");
    assert!(
        params.is_empty(),
        "Non-generic class extending ArrayList<String> must not parse 'String' as a type parameter"
    );

    let params_iface = parse_signature_type_params("interface StringHandler : Handler<String>");
    assert!(
        params_iface.is_empty(),
        "Non-generic interface extending Handler<String> must not parse 'String' as a type parameter"
    );
}

#[test]
fn test_parse_signature_type_params_with_function_name_containing_keyword_substring() {
    let params = parse_signature_type_params("fun get_val(item: String): Unit");
    assert!(
        params.is_empty(),
        "Function named 'get_val' must not match 'val' keyword for type parameters"
    );
}

#[test]
fn test_parse_signature_type_params_from_typealias() {
    let params = parse_signature_type_params("typealias Handler<T> = (T) -> Unit");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
}

#[test]
fn test_parse_signature_type_params_with_keyword_in_annotation() {
    let params = parse_signature_type_params("@Suppress(\"fun\") fun <T> execute(): Unit");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
}

#[test]
fn test_parse_signature_type_params_with_multiline_where_clause() {
    let params =
        parse_signature_type_params("fun <T> execute(item: T): Unit\n    where T : CharSequence");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert_eq!(
        params[0].bounds,
        vec![smol_str::SmolStr::new("CharSequence")]
    );
}

#[test]
fn test_split_type_arguments_with_nested_function_type_arrow() {
    let args = split_type_arguments("Wrapper<() -> Int, String>, Boolean");
    assert_eq!(args, vec!["Wrapper<() -> Int, String>", "Boolean"]);
}

#[test]
fn test_parse_signature_type_params_with_function_type_bound_containing_parentheses() {
    let params_class = parse_signature_type_params("class Handler<T : () -> Unit>(val action: T)");
    assert_eq!(params_class.len(), 1);
    assert_eq!(params_class[0].name, "T");

    let params_iface = parse_signature_type_params("interface Callback<T : (String) -> Unit>");
    assert_eq!(params_iface.len(), 1);
    assert_eq!(params_iface[0].name, "T");
}

#[test]
fn test_parse_signature_type_params_with_where_keyword_inside_annotation_string() {
    let sig = "@Annotation(\"where logic lives\") fun <T> run(x: T): Unit where T : Number";
    let params = parse_signature_type_params(sig);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert_eq!(params[0].bounds, vec![smol_str::SmolStr::new("Number")]);
}

#[test]
fn test_parse_signature_type_params_ignores_where_identifier_in_function_body() {
    let sig = "fun <T> execute(): Unit { val where = 1 }";
    let params = parse_signature_type_params(sig);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert!(
        params[0].bounds.is_empty(),
        "Identifier 'where' inside function body must not be parsed as where-clause"
    );
}

#[test]
fn test_parse_signature_type_params_with_greater_than_inside_annotation_string() {
    let sig = "fun <@Anno(\">\") T> execute(): Unit";
    let params = parse_signature_type_params(sig);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
}
