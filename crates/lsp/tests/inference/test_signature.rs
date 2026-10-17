use lsp::indexer::{DeclarationItem, DeclarationMetadata};
use lsp::inference::{KotlinType, SignatureInfo, TypeArg, are_param_types_compatible};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Range, SymbolKind};

#[test]
fn test_signature_info_generic_type_bounds_parsed_as_kotlin_types() {
    let sig = "fun <T : Comparable<T>> sort(list: List<T>): List<T>";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse successfully");

    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "T");
    assert!(!info.type_params[0].is_out);

    let expected_bound = KotlinType::class(
        "Comparable",
        vec![TypeArg::invariant(KotlinType::TypeParam(SmolStr::new("T")))],
    );
    assert_eq!(info.type_params[0].bounds, vec![expected_bound]);

    let expected_list = KotlinType::class(
        "List",
        vec![TypeArg::invariant(KotlinType::TypeParam(SmolStr::new("T")))],
    );
    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "list");
    assert_eq!(info.params[0].kotlin_type, Some(expected_list.clone()));
    assert_eq!(info.return_type, Some(expected_list));
}

#[test]
fn test_signature_info_extension_and_functional_receiver_parsed() {
    let sig = "fun <T, R> T.map(transform: (T) -> R): R";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse extension function");

    assert_eq!(info.type_params.len(), 2);
    assert_eq!(info.type_params[0].name, "T");
    assert_eq!(info.type_params[1].name, "R");

    assert_eq!(
        info.receiver,
        Some(KotlinType::TypeParam(SmolStr::new("T")))
    );

    let expected_fn = KotlinType::function(
        None,
        vec![KotlinType::TypeParam(SmolStr::new("T"))],
        KotlinType::TypeParam(SmolStr::new("R")),
        false,
    );
    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "transform");
    assert_eq!(info.params[0].kotlin_type, Some(expected_fn));
    assert_eq!(
        info.return_type,
        Some(KotlinType::TypeParam(SmolStr::new("R")))
    );
}

#[test]
fn test_signature_info_functional_receiver_block_parsed() {
    let sig = "fun <T> T.apply(block: T.() -> Unit): T";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse apply receiver block");

    assert_eq!(
        info.receiver,
        Some(KotlinType::TypeParam(SmolStr::new("T")))
    );

    let expected_block = KotlinType::function(
        Some(KotlinType::TypeParam(SmolStr::new("T"))),
        vec![],
        KotlinType::UNIT,
        false,
    );
    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "block");
    assert_eq!(info.params[0].kotlin_type, Some(expected_block));
    assert_eq!(
        info.return_type,
        Some(KotlinType::TypeParam(SmolStr::new("T")))
    );
}

#[test]
fn test_signature_info_where_clause_multiple_bounds_parsed() {
    let sig = "fun <T> copy(item: T): T where T : CharSequence, T : java.io.Serializable";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse where-clause bounds");

    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "T");
    assert_eq!(
        info.type_params[0].bounds,
        vec![
            KotlinType::simple_class("CharSequence"),
            KotlinType::simple_class("java.io.Serializable"),
        ]
    );
}

#[test]
fn test_signature_info_from_declaration_item_conversion() {
    let sig = "fun <E> List<E>.filter(predicate: (E) -> Boolean): List<E>";
    let decl = DeclarationItem::new(
        "filter",
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature(sig),
    );

    let info = SignatureInfo::from_decl(&decl)
        .expect("from_decl should parse valid declaration signature");
    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "E");

    let expected_list = KotlinType::class(
        "List",
        vec![TypeArg::invariant(KotlinType::TypeParam(SmolStr::new("E")))],
    );
    assert_eq!(info.receiver, Some(expected_list.clone()));

    let expected_pred = KotlinType::function(
        None,
        vec![KotlinType::TypeParam(SmolStr::new("E"))],
        KotlinType::BOOLEAN,
        false,
    );
    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "predicate");
    assert_eq!(info.params[0].kotlin_type, Some(expected_pred));
    assert_eq!(info.return_type, Some(expected_list));
}

#[test]
fn test_signature_info_vararg_and_default_parameters_parsed() {
    let sig = "fun <T> format(prefix: String = \"\", vararg items: T): String";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse defaults and varargs");

    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0].name, "prefix");
    assert!(info.params[0].has_default);
    assert!(!info.params[0].is_vararg);
    assert_eq!(info.params[0].kotlin_type, Some(KotlinType::STRING));

    assert_eq!(info.params[1].name, "items");
    assert!(!info.params[1].has_default);
    assert!(info.params[1].is_vararg);
    assert_eq!(
        info.params[1].kotlin_type,
        Some(KotlinType::TypeParam(SmolStr::new("T")))
    );
    assert_eq!(info.return_type, Some(KotlinType::STRING));
}

#[test]
fn test_signature_info_out_variance_type_param_parsed() {
    let sig = "fun <out T : Number> produce(): T";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse out variance");

    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "T");
    assert!(info.type_params[0].is_out);
    assert_eq!(
        info.type_params[0].bounds,
        vec![KotlinType::simple_class("Number")]
    );
}

#[test]
fn test_signature_info_annotated_function_and_functional_parameter() {
    let sig = r#"@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()"#;
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse remember");
    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "T");
    assert_eq!(info.params.len(), 4);
    assert!(info.params[0].has_default);
    assert!(info.params[1].has_default);
    assert!(info.params[2].has_default);
    assert_eq!(info.params[3].name, "calculation");
    assert_eq!(
        info.params[3].kotlin_type,
        Some(KotlinType::function(
            None,
            Vec::new(),
            KotlinType::TypeParam(SmolStr::new("T")),
            false
        ))
    );
    assert_eq!(
        info.return_type,
        Some(KotlinType::TypeParam(SmolStr::new("T")))
    );

    let parsed_ann = lsp::inference::types::parse_type_signature("@Composable () -> Unit");
    assert_eq!(
        parsed_ann,
        Some(KotlinType::function(
            None,
            Vec::new(),
            KotlinType::UNIT,
            false
        ))
    );
}

#[test]
fn test_signature_info_annotations_do_not_trigger_modifiers() {
    let sig = "fun <@without T> process(@not_vararg x: T): Output<@without Any>";
    let info = SignatureInfo::parse(sig).expect("SignatureInfo should parse with annotations");
    assert_eq!(info.type_params.len(), 1);
    assert_eq!(info.type_params[0].name, "T");
    // Should NOT be marked as out variance just because annotation contains "without" (which contains "out")
    assert!(!info.type_params[0].is_out);

    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "x");
    // Should NOT be marked as vararg just because annotation contains "not_vararg"
    assert!(!info.params[0].is_vararg);
}

#[test]
fn test_signature_info_suspend_function_types_in_declarations() {
    let fun_sig = "fun execute(): suspend () -> Unit";
    let fun_info = SignatureInfo::parse(fun_sig).expect("Should parse fun with suspend return");
    let ret = fun_info.return_type.expect("Expected return type");
    if let KotlinType::Function { is_suspend, .. } = ret {
        assert!(is_suspend, "Function return type should be suspend");
    } else {
        panic!("Expected Function return type");
    }

    let prop_sig = "val handler: suspend () -> Unit";
    let prop_info =
        SignatureInfo::parse(prop_sig).expect("Should parse property with suspend return");
    let prop_ret = prop_info
        .return_type
        .expect("Expected property return type");
    if let KotlinType::Function { is_suspend, .. } = prop_ret {
        assert!(is_suspend, "Property return type should be suspend");
    } else {
        panic!("Expected Function property type");
    }

    let generic_sig = "fun <T : suspend () -> Unit> runTask(task: T): Unit";
    let gen_info =
        SignatureInfo::parse(generic_sig).expect("Should parse generic with suspend bound");
    assert_eq!(gen_info.type_params.len(), 1);
    assert_eq!(gen_info.type_params[0].bounds.len(), 1);
    if let KotlinType::Function { is_suspend, .. } = &gen_info.type_params[0].bounds[0] {
        assert!(*is_suspend, "Generic bound should be suspend function");
    } else {
        panic!("Expected Function type in generic bound");
    }

    let where_sig = "fun <T> runTask(task: T): Unit where T : suspend () -> Unit";
    let where_info =
        SignatureInfo::parse(where_sig).expect("Should parse where-clause suspend bound");
    assert_eq!(where_info.type_params.len(), 1);
    assert_eq!(where_info.type_params[0].bounds.len(), 1);
    if let KotlinType::Function { is_suspend, .. } = &where_info.type_params[0].bounds[0] {
        assert!(*is_suspend, "Where-clause bound should be suspend function");
    } else {
        panic!("Expected Function type in where-clause bound");
    }
}

#[test]
fn test_param_types_compatible_multi_parameter_and_nested_functional_types() {
    let t1 = vec!["(com.foo.A, com.bar.B) -> Unit".to_string()];
    let t2 = vec!["(com.baz.X, com.bar.B) -> Unit".to_string()];
    assert!(
        !are_param_types_compatible(&t1, &t2),
        "Incompatible first parameter types A and X should not match"
    );

    let t3 = vec!["(String, Double) -> Unit".to_string()];
    let t4 = vec!["(String, Double) -> Unit".to_string()];
    assert!(
        are_param_types_compatible(&t3, &t4),
        "Identical multi-parameter functional types should match"
    );

    let t5 = vec!["(String, Any) -> Unit".to_string()];
    let t6 = vec!["(String, Double) -> Unit".to_string()];
    assert!(
        are_param_types_compatible(&t5, &t6),
        "Any in parameter of functional type should match concrete type"
    );

    let t7 = vec!["(Int) -> Unit".to_string()];
    let t8 = vec!["(Int, Double) -> Unit".to_string()];
    assert!(
        !are_param_types_compatible(&t7, &t8),
        "Different arity lambdas should not match"
    );

    let t9 = vec!["(Int) -> (String) -> Unit".to_string()];
    let t10 = vec!["(Int) -> (String) -> Unit".to_string()];
    assert!(
        are_param_types_compatible(&t9, &t10),
        "Higher order functions with nested return arrow should match"
    );
}

#[test]
fn test_param_types_compatible_nullable_and_receiver_functional_types() {
    let t1 = vec!["((Int) -> String)?".to_string()];
    let t2 = vec!["((Int) -> Any)?".to_string()];
    assert!(
        are_param_types_compatible(&t1, &t2),
        "Nullable functional types should match when return type matches Any"
    );

    let t3 = vec!["suspendScope.() -> Unit".to_string()];
    let t4 = vec!["suspendScope.() -> Unit".to_string()];
    assert!(
        are_param_types_compatible(&t3, &t4),
        "Receiver starting with suspend should not have its prefix corrupted"
    );

    let t5 = vec!["(value: String, count: Int) -> Unit".to_string()];
    let t6 = vec!["(String, Int) -> Unit".to_string()];
    assert!(
        are_param_types_compatible(&t5, &t6),
        "Named parameters in functional type should match unnamed parameters"
    );

    let t7 = vec!["(Receiver?).() -> String".to_string()];
    let t8 = vec!["Receiver?.() -> Any".to_string()];
    assert!(
        are_param_types_compatible(&t7, &t8),
        "Parenthesized nullable receiver functional type should match unparenthesized nullable receiver"
    );
}
