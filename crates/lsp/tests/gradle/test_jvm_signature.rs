use lsp::gradle::jvm_type::{
    jvm_method_sig_to_kotlin, parse_jvm_class_signature, parse_jvm_method_signature,
    parse_jvm_type_signature,
};

#[test]
fn test_parse_jvm_type_signature_primitive_and_array() {
    assert_eq!(parse_jvm_type_signature("I").as_deref(), Some("Int"));
    assert_eq!(
        parse_jvm_type_signature("[I").as_deref(),
        Some("Array<Int>")
    );
    assert_eq!(
        parse_jvm_type_signature("[Ljava/lang/String;").as_deref(),
        Some("Array<String>")
    );
}

#[test]
fn test_parse_jvm_type_signature_generics_and_wildcards() {
    assert_eq!(parse_jvm_type_signature("TT;").as_deref(), Some("T"));
    assert_eq!(
        parse_jvm_type_signature("Ljava/util/List<TT;>;").as_deref(),
        Some("List<T>")
    );
    assert_eq!(
        parse_jvm_type_signature("Ljava/util/Map<TK;Ljava/util/List<TV;>;>;").as_deref(),
        Some("Map<K, List<V>>")
    );
    assert_eq!(
        parse_jvm_type_signature("Lkotlin/reflect/KProperty<*>;").as_deref(),
        Some("KProperty<*>")
    );
    assert_eq!(
        parse_jvm_type_signature("Ljava/lang/Class<+Ljava/lang/Number;>;").as_deref(),
        Some("Class<out Number>")
    );
    assert_eq!(
        parse_jvm_type_signature("Ljava/lang/Class<-Ljava/lang/Number;>;").as_deref(),
        Some("Class<in Number>")
    );
}

#[test]
fn test_parse_jvm_method_signature_generic_function() {
    let sig = "<T:Ljava/lang/Object;>([TT;)Ljava/util/List<TT;>;";
    let parsed = parse_jvm_method_signature(sig).expect("Should parse method signature");
    assert_eq!(parsed.type_params, vec!["T"]);
    assert_eq!(parsed.params, vec!["Array<T>"]);
    assert_eq!(parsed.return_type, "List<T>");

    let (kotlin_sig, rec) = jvm_method_sig_to_kotlin("listOf", &parsed, false);
    assert_eq!(kotlin_sig, "fun <T> listOf(arg1: Array<T>): List<T>");
    assert_eq!(rec, None);
}

#[test]
fn test_parse_jvm_method_signature_extension_function() {
    let sig = "<T:Ljava/lang/Object;>(Lcom/example/Box<TT;>;)TT;";
    let parsed = parse_jvm_method_signature(sig).expect("Should parse extension signature");
    assert_eq!(parsed.type_params, vec!["T"]);
    assert_eq!(parsed.params, vec!["Box<T>"]);
    assert_eq!(parsed.return_type, "T");

    let (kotlin_sig, rec) = jvm_method_sig_to_kotlin("item", &parsed, true);
    assert_eq!(kotlin_sig, "fun <T> Box<T>.item(): T");
    assert_eq!(rec.as_deref(), Some("Box<T>"));
}

#[test]
fn test_parse_jvm_method_signature_class_member_with_type_var() {
    let sig = "(Ljava/lang/Object;Lkotlin/reflect/KProperty<*>;)TT;";
    let parsed = parse_jvm_method_signature(sig).expect("Should parse member signature");
    assert!(parsed.type_params.is_empty());
    assert_eq!(parsed.params, vec!["Object", "KProperty<*>"]);
    assert_eq!(parsed.return_type, "T");

    let (kotlin_sig, rec) = jvm_method_sig_to_kotlin("getValue", &parsed, false);
    assert_eq!(
        kotlin_sig,
        "fun getValue(arg1: Object, arg2: KProperty<*>): T"
    );
    assert_eq!(rec, None);
}

#[test]
fn test_parse_jvm_class_signature_with_generics() {
    let sig = "<T:Ljava/lang/Object;>Ljava/lang/Object;Ljava/lang/Comparable<TT;>;";
    let parsed = parse_jvm_class_signature(sig).expect("Should parse class signature");
    assert_eq!(parsed.type_params, vec!["T"]);
    assert_eq!(parsed.supertypes, vec!["Comparable<T>"]);
}
