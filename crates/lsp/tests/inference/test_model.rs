use lsp::inference::types::Variance;
use lsp::inference::{KotlinType, PrimitiveType};
use smol_str::SmolStr;

#[test]
fn test_inferred_type_primitives() {
    let int_type = KotlinType::INT;
    assert_eq!(int_type.to_string(), "Int");
    assert_eq!(int_type, KotlinType::Primitive(PrimitiveType::Int));
    assert!(int_type.is_primitive());
    assert_eq!(int_type.to_container_key(), SmolStr::new("Int"));
    assert_eq!(int_type.to_companion_key(), SmolStr::new("Int.Companion"));
    assert_eq!(int_type.to_string(), "Int");

    let str_type = KotlinType::STRING.with_nullability(true);
    assert!(str_type.is_nullable());
    assert_eq!(str_type.to_string(), "String?");
}

#[test]
fn test_inferred_type_classes() {
    let user_type = KotlinType::parse("com.example.User").unwrap();
    assert_eq!(user_type.to_string(), "com.example.User");
    assert!(!user_type.is_primitive());
    assert_eq!(
        user_type.package_and_simple_name(),
        (Some("com.example"), "User")
    );
    assert_eq!(
        user_type.to_companion_key(),
        SmolStr::new("com.example.User.Companion")
    );

    let generic_box = KotlinType::parse("Box<String>").unwrap();
    assert_eq!(generic_box.to_string(), "Box<String>");
    assert_eq!(generic_box.base_name(), "Box");
}

#[test]
fn test_inferred_type_comparisons() {
    let t = KotlinType::simple_class("CustomType");
    assert_eq!(t.to_string(), "CustomType");
    assert_eq!(t, KotlinType::simple_class("CustomType"));
    assert_eq!(t.base_name(), "CustomType");
}

#[test]
fn test_parenthesized_function_type_with_modifiers() {
    let suspend_paren = KotlinType::parse("(suspend () -> Unit)")
        .expect("Should parse parenthesized suspend function type");
    assert!(suspend_paren.is_function());
    assert_eq!(suspend_paren.to_string(), "suspend () -> Unit");

    let suspend_nullable = KotlinType::parse("(suspend () -> Unit)?")
        .expect("Should parse nullable parenthesized suspend function type");
    assert!(suspend_nullable.is_nullable());
    assert_eq!(suspend_nullable.to_string(), "(suspend () -> Unit)?");

    let composable_paren = KotlinType::parse("(@Composable () -> Unit)")
        .expect("Should parse parenthesized annotated function type");
    assert!(composable_paren.is_function());
    assert_eq!(composable_paren.to_string(), "() -> Unit");

    let composable_nullable = KotlinType::parse("(@Composable (Int) -> String)?")
        .expect("Should parse nullable parenthesized annotated function type");
    assert!(composable_nullable.is_nullable());
    assert_eq!(composable_nullable.to_string(), "((Int) -> String)?");
}

#[test]
fn test_qualified_type_nested_type_arguments() {
    let qualified = KotlinType::parse("Outer<Int>.Inner<String>")
        .expect("Should parse qualified type with type arguments on outer and inner classes");
    assert_eq!(qualified.base_name(), "Outer.Inner");
    assert_eq!(qualified.to_string(), "Outer.Inner<Int, String>");
}

#[test]
fn test_type_projection_annotations_do_not_trigger_variance() {
    let ty = KotlinType::parse("List<@without String>")
        .expect("Should parse projection with annotation");
    assert_eq!(ty.type_arguments().len(), 1);
    assert_eq!(ty.type_arguments()[0].variance, Variance::Invariant);

    let ty_in = KotlinType::parse("List<@instance String>")
        .expect("Should parse projection with annotation");
    assert_eq!(ty_in.type_arguments().len(), 1);
    assert_eq!(ty_in.type_arguments()[0].variance, Variance::Invariant);
}

#[test]
fn test_nested_suspend_function_types() {
    let return_suspend = KotlinType::parse("() -> suspend () -> Unit")
        .expect("Should parse higher-order function returning suspend function");
    assert_eq!(return_suspend.to_string(), "() -> suspend () -> Unit");
    if let KotlinType::Function { return_type, .. } = return_suspend {
        assert!(return_type.is_function());
        if let KotlinType::Function { is_suspend, .. } = *return_type {
            assert!(is_suspend, "Nested return function type should be suspend");
        } else {
            panic!("Expected Function return type");
        }
    } else {
        panic!("Expected Function type");
    }

    let param_suspend = KotlinType::parse("(suspend () -> Unit) -> Unit")
        .expect("Should parse higher-order function taking unnamed suspend function parameter");
    assert_eq!(param_suspend.to_string(), "(suspend () -> Unit) -> Unit");
    if let KotlinType::Function { params, .. } = param_suspend {
        assert_eq!(params.len(), 1);
        if let KotlinType::Function { is_suspend, .. } = &params[0] {
            assert!(
                *is_suspend,
                "Unnamed parameter function type should be suspend"
            );
        } else {
            panic!("Expected Function parameter type");
        }
    } else {
        panic!("Expected Function type");
    }

    let list_suspend = KotlinType::parse("List<suspend () -> Unit>")
        .expect("Should parse generic container with suspend function type argument");
    assert_eq!(list_suspend.to_string(), "List<suspend () -> Unit>");
    let args = list_suspend.type_arguments();
    assert_eq!(args.len(), 1);
    if let KotlinType::Function { is_suspend, .. } = &args[0].type_ref {
        assert!(*is_suspend, "Type argument function type should be suspend");
    } else {
        panic!("Expected Function type in type argument");
    }
}
