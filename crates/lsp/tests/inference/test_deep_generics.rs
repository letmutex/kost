use lsp::indexer::WorkspaceIndex;
use lsp::inference::types::parse_type_signature;
use lsp::inference::{ConstraintSystem, KotlinType, infer_variable_type};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_constraint_system_solves_multi_character_type_parameters() {
    let index = WorkspaceIndex::new();
    let mut system =
        ConstraintSystem::new(&index, vec![SmolStr::new("Item"), SmolStr::new("Result")]);

    let actual_item_list = parse_type_signature("List<String>").unwrap();
    let expected_item_list = parse_type_signature("List<Item>").unwrap();
    assert!(system.add_subtype_constraint(&actual_item_list, &expected_item_list));

    let actual_result = parse_type_signature("Int").unwrap();
    let expected_result = parse_type_signature("Result").unwrap();
    assert!(system.add_subtype_constraint(&actual_result, &expected_result));

    let return_template = parse_type_signature("Pair<Item, Result>").unwrap();
    let solved = system.solve_and_substitute(&return_template);
    assert_eq!(solved.to_string(), "Pair<String, Int>");
}

#[test]
fn test_constraint_system_solves_nested_generic_structures() {
    let index = WorkspaceIndex::new();
    let mut system =
        ConstraintSystem::new(&index, vec![SmolStr::new("Key"), SmolStr::new("Value")]);

    let actual = parse_type_signature("Map<String, List<Int>>").unwrap();
    let template = parse_type_signature("Map<Key, List<Value>>").unwrap();
    assert!(system.add_subtype_constraint(&actual, &template));

    let target = parse_type_signature("Map<Value, Key>").unwrap();
    let solved = system.solve_and_substitute(&target);
    assert_eq!(solved.to_string(), "Map<Int, String>");
}

#[test]
fn test_constraint_system_solves_contravariant_function_arguments() {
    let index = WorkspaceIndex::new();
    let mut system = ConstraintSystem::new(&index, vec![SmolStr::new("Element")]);

    let actual_fn = parse_type_signature("(String) -> Boolean").unwrap();
    let template_fn = parse_type_signature("(Element) -> Boolean").unwrap();
    assert!(system.add_subtype_constraint(&actual_fn, &template_fn));

    let target = parse_type_signature("List<Element>").unwrap();
    let solved = system.solve_and_substitute(&target);
    assert_eq!(solved.to_string(), "List<String>");
}

#[test]
fn test_constraint_system_least_upper_bound_unification() {
    let index = WorkspaceIndex::new();
    let t1 = parse_type_signature("Int").unwrap();
    let t2 = parse_type_signature("Long").unwrap();
    let unified_num = lsp::inference::unify::unify_two_types(&index, &t1, &t2);
    assert_eq!(unified_num, Some(KotlinType::NUMBER));

    let s1 = parse_type_signature("String").unwrap();
    let s2 = parse_type_signature("String?").unwrap();
    let unified_str = lsp::inference::unify::unify_two_types(&index, &s1, &s2);
    assert_eq!(unified_str, Some(KotlinType::STRING.make_nullable()));
}

#[test]
fn test_infer_variable_with_multi_character_generic_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultiCharGeneric.kt").unwrap();
    let code = r#"
package com.example.gen

fun <Element> wrap(e: Element): List<Element> = listOf(e)

fun test() {
    val result = wrap("hello")
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("List<String>"));
}

#[test]
fn test_infer_lambda_parameter_with_multi_character_generic_type_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultiCharLambda.kt").unwrap();
    let code = r#"
package com.example.gen

fun <Item> process(list: List<Item>, action: (Item) -> Unit) {}

fun test(names: List<String>) {
    process(names) {
        val current = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 12);
    let inferred = infer_variable_type(&index, &url, "current", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_constraint_system_solves_deep_nested_triple_generics() {
    let index = WorkspaceIndex::new();
    let mut system = ConstraintSystem::new(
        &index,
        vec![SmolStr::new("K"), SmolStr::new("V"), SmolStr::new("R")],
    );

    let actual = parse_type_signature("Triple<String, List<Int>, Set<Boolean>>").unwrap();
    let template = parse_type_signature("Triple<K, List<V>, Set<R>>").unwrap();
    assert!(system.add_subtype_constraint(&actual, &template));

    let return_template = parse_type_signature("Map<K, Pair<V, R>>").unwrap();
    let solved = system.solve_and_substitute(&return_template);
    assert_eq!(solved.to_string(), "Map<String, Pair<Int, Boolean>>");
}

#[test]
fn test_constraint_system_enforces_upper_bound_satisfied_by_subtype() {
    let index = WorkspaceIndex::new();
    let mut system = ConstraintSystem::new(&index, vec![SmolStr::new("T")]);
    let char_seq = parse_type_signature("CharSequence").unwrap();
    system.add_upper_bound(SmolStr::new("T"), char_seq);

    let actual = parse_type_signature("String").unwrap();
    let param_t = parse_type_signature("T").unwrap();
    assert!(system.add_subtype_constraint(&actual, &param_t));

    let solved = system.solve_and_substitute(&param_t);
    assert_eq!(solved.to_string(), "String");
}

#[test]
fn test_constraint_system_enforces_upper_bound_with_violation() {
    let index = WorkspaceIndex::new();
    let mut system = ConstraintSystem::new(&index, vec![SmolStr::new("T")]);
    let char_seq = parse_type_signature("CharSequence").unwrap();
    system.add_upper_bound(SmolStr::new("T"), char_seq.clone());

    let actual = parse_type_signature("Int").unwrap();
    let param_t = parse_type_signature("T").unwrap();
    assert!(system.add_subtype_constraint(&actual, &param_t));

    let solved = system.solve_and_substitute(&param_t);
    assert_eq!(solved, char_seq);
}

#[test]
fn test_constraint_system_defaults_unconstrained_type_parameter_to_upper_bound() {
    let index = WorkspaceIndex::new();
    let mut system = ConstraintSystem::new(&index, vec![SmolStr::new("T")]);
    let number_bound = parse_type_signature("Number").unwrap();
    system.add_upper_bound(SmolStr::new("T"), number_bound.clone());

    let param_t = parse_type_signature("T").unwrap();
    let solved = system.solve_and_substitute(&param_t);
    assert_eq!(solved, number_bound);
}

#[test]
fn test_infer_generic_call_with_type_param_upper_bound() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BoundedGeneric.kt").unwrap();
    let code = r#"
package com.example.bound

class Container<T : CharSequence>(val item: T)

fun <T : CharSequence> makeContainer(item: T): Container<T> = Container(item)

fun test() {
    val container = makeContainer("hello")
    val item = container.item
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(8, 8);
    let inferred1 = infer_variable_type(&index, &url, "container", pos1, code, None);
    assert_eq!(
        inferred1,
        KotlinType::parse("com.example.bound.Container<String>")
    );

    let pos2 = Position::new(9, 8);
    let inferred2 = infer_variable_type(&index, &url, "item", pos2, code, None);
    assert_eq!(inferred2, KotlinType::parse("String"));
}
