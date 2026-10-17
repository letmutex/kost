use lsp::WorkspaceIndex;
use lsp::inference::GenericParamInfo;
use lsp::inference::overload::is_subtype;
use lsp::inference::types::{
    KotlinType, TypeArg, Variance, parse_type_signature, parse_type_signature_with_params,
};

#[test]
fn test_kotlin_type_primitive_parsing_and_display() {
    let int_type = KotlinType::INT;
    assert_eq!(int_type.to_string(), "Int");
    assert_eq!(int_type.base_name(), "Int");
    assert!(int_type.is_primitive());
    assert!(!int_type.is_nullable());

    let parsed = parse_type_signature("Boolean").expect("Should parse Boolean");
    assert_eq!(parsed, KotlinType::BOOLEAN);
    assert_eq!(parsed.to_string(), "Boolean");
}

#[test]
fn test_kotlin_type_nullable_primitive_and_class() {
    let parsed_null_int = parse_type_signature("Int?").expect("Should parse Int?");
    assert!(parsed_null_int.is_nullable());
    assert_eq!(parsed_null_int.to_string(), "Int?");
    assert_eq!(parsed_null_int.base_name(), "Int");
    assert_eq!(parsed_null_int.make_non_null(), KotlinType::INT);

    let parsed_null_user = parse_type_signature("com.example.User?").expect("Should parse User?");
    assert!(parsed_null_user.is_nullable());
    assert_eq!(parsed_null_user.to_string(), "com.example.User?");
    assert_eq!(parsed_null_user.base_name(), "com.example.User");
    assert_eq!(
        parsed_null_user.package_and_simple_name(),
        (Some("com.example"), "User")
    );
}

#[test]
fn test_kotlin_type_nested_generics_with_variance() {
    let code = "Map<String, List<out Number>>";
    let parsed = parse_type_signature(code).expect("Should parse nested generic");
    assert_eq!(parsed.to_string(), "Map<String, List<out Number>>");
    assert_eq!(parsed.base_name(), "Map");

    let args = parsed.type_arguments();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].variance, Variance::Invariant);
    assert_eq!(args[0].type_ref, KotlinType::STRING);

    assert_eq!(args[1].variance, Variance::Invariant);
    let inner_list = &args[1].type_ref;
    assert_eq!(inner_list.base_name(), "List");
    let inner_args = inner_list.type_arguments();
    assert_eq!(inner_args.len(), 1);
    assert_eq!(inner_args[0].variance, Variance::Out);
    assert_eq!(inner_args[0].type_ref, KotlinType::simple_class("Number"));

    let star_code = "List<*>";
    let parsed_star = parse_type_signature(star_code).expect("Should parse star projection");
    assert_eq!(parsed_star.to_string(), "List<*>");
    assert_eq!(parsed_star.type_arguments()[0].variance, Variance::Star);
}

#[test]
fn test_kotlin_type_function_type_with_receiver_and_suspend() {
    let simple_fn = parse_type_signature("() -> Unit").expect("Should parse simple function");
    assert_eq!(simple_fn.to_string(), "() -> Unit");
    assert!(simple_fn.is_function());

    let multi_fn = parse_type_signature("(Int, String) -> Boolean")
        .expect("Should parse multi-param function");
    assert_eq!(multi_fn.to_string(), "(Int, String) -> Boolean");

    let receiver_fn =
        parse_type_signature("String.(Int) -> Boolean").expect("Should parse receiver function");
    assert_eq!(receiver_fn.to_string(), "String.(Int) -> Boolean");

    let suspend_fn =
        parse_type_signature("suspend () -> Unit").expect("Should parse suspend function");
    assert_eq!(suspend_fn.to_string(), "suspend () -> Unit");

    let nullable_fn =
        parse_type_signature("((Int) -> String)?").expect("Should parse nullable function type");
    assert!(nullable_fn.is_nullable());
    assert_eq!(nullable_fn.to_string(), "((Int) -> String)?");
}

#[test]
fn test_parse_function_type_with_named_parameters() {
    let suspend_named = parse_type_signature_with_params("suspend (value: T) -> R", &["T", "R"])
        .expect("Should parse suspend function with named parameter");
    assert_eq!(suspend_named.to_string(), "suspend (T) -> R");
    assert!(suspend_named.is_function());

    let multi_named =
        parse_type_signature_with_params("(key: K, value: V) -> Boolean", &["K", "V"])
            .expect("Should parse multi named parameters");
    assert_eq!(multi_named.to_string(), "(K, V) -> Boolean");

    let receiver_named = parse_type_signature_with_params("Flow<T>.(value: T) -> R", &["T", "R"])
        .expect("Should parse receiver with named parameter");
    assert_eq!(receiver_named.to_string(), "Flow<T>.(T) -> R");

    let special_named = parse_type_signature_with_params("(_: Int, `val`: String) -> Unit", &[])
        .expect("Should parse underscore and backtick parameter names");
    assert_eq!(special_named.to_string(), "(Int, String) -> Unit");

    let non_function_named = parse_type_signature("(value: Int)");
    assert_eq!(non_function_named, None);

    let suspend_ident = parse_type_signature("suspendState")
        .expect("Should parse identifier starting with suspend as class");
    assert_eq!(suspend_ident.to_string(), "suspendState");
}

#[test]
fn test_kotlin_type_intersection_types() {
    let parsed = parse_type_signature_with_params("T & Any", &["T"])
        .expect("Should parse intersection type");
    assert_eq!(parsed.to_string(), "T & Any");
    match parsed {
        KotlinType::Intersection(types) => {
            assert_eq!(types.len(), 2);
            assert_eq!(types[0], KotlinType::type_param("T"));
            assert_eq!(types[1], KotlinType::ANY);
        }
        _ => panic!("Expected intersection type"),
    }
}

#[test]
fn test_kotlin_type_nullability_and_parsing_roundtrip() {
    let kt = KotlinType::class(
        "List",
        vec![TypeArg::invariant(KotlinType::simple_class("User"))],
    )
    .make_nullable();

    assert!(kt.is_nullable());
    assert_eq!(kt.to_string(), "List<User>?");

    let parsed = KotlinType::parse("List<User>?").expect("Should parse");
    assert_eq!(parsed, kt);
    assert!(parsed.is_nullable());
    assert_eq!(parsed.to_string(), "List<User>?");
}

#[test]
fn test_kotlin_type_nullability_aware_equality() {
    let non_null = KotlinType::parse("Int").unwrap();
    assert_eq!(non_null.to_string(), "Int");
    assert_ne!(non_null, KotlinType::parse("Int?").unwrap());

    let nullable = KotlinType::parse("Int?").unwrap();
    assert_eq!(nullable.to_string(), "Int?");
    assert_ne!(nullable, KotlinType::parse("Int").unwrap());
}

#[test]
fn test_kotlin_type_to_short_string() {
    let int_type = KotlinType::INT;
    assert_eq!(int_type.to_short_string(), "Int");

    let user_type = KotlinType::parse("com.example.User").unwrap();
    assert_eq!(user_type.to_short_string(), "User");

    let state_type = KotlinType::parse("androidx.compose.runtime.MutableState<Int>").unwrap();
    assert_eq!(state_type.to_short_string(), "MutableState<Int>");

    let map_type =
        KotlinType::parse("java.util.Map<java.lang.String, java.util.List<com.example.User>>")
            .unwrap();
    assert_eq!(map_type.to_short_string(), "Map<String, List<User>>");

    let nullable_state = KotlinType::parse("androidx.compose.runtime.MutableState<Int>?").unwrap();
    assert_eq!(nullable_state.to_short_string(), "MutableState<Int>?");

    let fn_type = KotlinType::parse("(com.example.User) -> kotlin.Boolean").unwrap();
    assert_eq!(fn_type.to_short_string(), "(User) -> Boolean");

    let star_type = KotlinType::parse("java.util.List<*>").unwrap();
    assert_eq!(star_type.to_short_string(), "List<*>");

    let out_type = KotlinType::parse("java.util.List<out kotlin.CharSequence>").unwrap();
    assert_eq!(out_type.to_short_string(), "List<out CharSequence>");

    let ext_fn_type = KotlinType::parse("com.example.User.() -> kotlin.Unit").unwrap();
    assert_eq!(ext_fn_type.to_short_string(), "User.() -> Unit");

    let nullable_ext_fn = KotlinType::parse("com.example.User?.() -> kotlin.Unit").unwrap();
    assert_eq!(nullable_ext_fn.to_short_string(), "(User?).() -> Unit");

    assert_eq!(TypeArg::STAR.to_short_string(), "*");
}

#[test]
fn test_algebraic_primitive_interface_conformance() {
    let index = WorkspaceIndex::new();

    let int_ty = KotlinType::INT;
    let str_ty = KotlinType::STRING;
    let comp_int = KotlinType::parse("Comparable<Int>").unwrap();
    let comp_str = KotlinType::parse("Comparable<String>").unwrap();
    let num_ty = KotlinType::parse("Number").unwrap();
    let char_seq = KotlinType::parse("CharSequence").unwrap();
    let serializable = KotlinType::parse("java.io.Serializable").unwrap();

    assert!(is_subtype(&int_ty, &comp_int, &index, &[]));
    assert!(!is_subtype(&int_ty, &comp_str, &index, &[]));
    assert!(is_subtype(&int_ty, &num_ty, &index, &[]));
    assert!(!is_subtype(&int_ty, &char_seq, &index, &[]));
    assert!(is_subtype(&int_ty, &serializable, &index, &[]));

    assert!(is_subtype(&str_ty, &comp_str, &index, &[]));
    assert!(!is_subtype(&str_ty, &comp_int, &index, &[]));
    assert!(!is_subtype(&str_ty, &num_ty, &index, &[]));
    assert!(is_subtype(&str_ty, &char_seq, &index, &[]));
    assert!(is_subtype(&str_ty, &serializable, &index, &[]));
}

#[test]
fn test_algebraic_subtyping_nullability_and_variance() {
    let index = WorkspaceIndex::new();

    let int_ty = KotlinType::INT;
    let nullable_int = KotlinType::parse("Int?").unwrap();
    assert!(is_subtype(&int_ty, &nullable_int, &index, &[]));
    assert!(!is_subtype(&nullable_int, &int_ty, &index, &[]));

    let nothing_ty = KotlinType::NOTHING;
    assert!(is_subtype(&nothing_ty, &int_ty, &index, &[]));
    assert!(is_subtype(&nothing_ty, &nullable_int, &index, &[]));

    let list_str = KotlinType::parse("List<String>").unwrap();
    let list_char_seq = KotlinType::parse("List<CharSequence>").unwrap();
    let list_star = KotlinType::parse("List<*>").unwrap();
    assert!(is_subtype(&list_str, &list_char_seq, &index, &[]));
    assert!(is_subtype(&list_str, &list_star, &index, &[]));

    let t_param = GenericParamInfo {
        name: smol_str::SmolStr::new("T"),
        bounds: vec![KotlinType::parse("Number").unwrap()],
        is_out: false,
    };
    let t_ty = KotlinType::type_param("T");
    let num_ty = KotlinType::parse("Number").unwrap();
    let str_ty = KotlinType::STRING;
    assert!(is_subtype(
        &t_ty,
        &num_ty,
        &index,
        std::slice::from_ref(&t_param)
    ));
    assert!(!is_subtype(
        &t_ty,
        &str_ty,
        &index,
        std::slice::from_ref(&t_param)
    ));
}

#[test]
fn test_is_subtype_cyclic_hierarchy_terminates() {
    let mut index = WorkspaceIndex::new();
    index
        .class_supertypes
        .insert("LoopA".into(), vec!["LoopB".into()]);
    index
        .class_supertypes
        .insert("LoopB".into(), vec!["LoopA".into()]);

    let ty_a = KotlinType::parse("LoopA").unwrap();
    let ty_target = KotlinType::parse("Target").unwrap();

    assert!(
        !is_subtype(&ty_a, &ty_target, &index, &[]),
        "Cyclic supertype hierarchy should terminate safely and return false"
    );
}

#[test]
fn test_is_subtype_self_referential_bound_terminates() {
    let mut index = WorkspaceIndex::new();
    index.class_supertypes.insert(
        "CopyableThrowable".into(),
        vec!["CopyableThrowable<T>".into()],
    );

    let ty = KotlinType::parse("CopyableThrowable").unwrap();
    let ty_target = KotlinType::parse("Target").unwrap();

    assert!(
        !is_subtype(&ty, &ty_target, &index, &[]),
        "Self-referential supertype bound should terminate safely and return false"
    );
}

#[test]
fn test_is_subtype_diamond_inheritance_generic_target() {
    let mut index = WorkspaceIndex::new();
    // Diamond: D -> B, C; B -> Mid<Int>; C -> Mid<String>
    index
        .class_supertypes
        .insert("D".into(), vec!["B".into(), "C".into()]);
    index
        .class_supertypes
        .insert("B".into(), vec!["Mid<Int>".into()]);
    index
        .class_supertypes
        .insert("C".into(), vec!["Mid<String>".into()]);

    let ty_d = KotlinType::parse("D").unwrap();
    let ty_mid_str = KotlinType::parse("Mid<String>").unwrap();

    assert!(
        is_subtype(&ty_d, &ty_mid_str, &index, &[]),
        "Diamond inheritance should find matching branch via backtracking"
    );
}
