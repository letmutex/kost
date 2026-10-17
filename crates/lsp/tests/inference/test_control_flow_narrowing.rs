use lsp::indexer::WorkspaceIndex;
use lsp::inference::types::KotlinType;
use lsp::inference::unify::is_class_subtype;
use lsp::inference::{infer_variable_type, unify_types};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_when_entry_smart_cast_affects_branch_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenSmartCast2.kt").unwrap();
    let code = r#"
package com.example.control

class Box(val size: Int)

fun test(x: Any) {
    val res = when (x) {
        is Box -> x.size
        else -> "default"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::ANY));
}

#[test]
fn test_unify_dynamic_or_any_with_concrete_type_yields_any() {
    let index = WorkspaceIndex::new();
    let lub = unify_types(&index, &[KotlinType::DynamicOrAny, KotlinType::STRING]);
    assert_eq!(lub, Some(KotlinType::ANY));
}

#[test]
fn test_unify_subtype_and_direct_supertype_yields_supertype() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Subsumption.kt").unwrap();
    let code = r#"
package com.example.sub

open class Parent
class Child : Parent()

fun test(cond: Boolean, c: Child, p: Parent) {
    val res = if (cond) c else p
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(
        inferred,
        Some(KotlinType::simple_class("com.example.sub.Parent"))
    );
}

#[test]
fn test_branch_intermediate_declaration_respects_explicit_annotation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BranchAnnot.kt").unwrap();
    let code = r#"
package com.example.control

fun test(cond: Boolean) {
    val res = if (cond) {
        val x: Number = 42
        x
    } else {
        100
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::NUMBER));
}

#[test]
fn test_subjectless_when_narrows_condition_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SubjectlessWhen.kt").unwrap();
    let code = r#"
package com.example.control

class Box(val size: Int)

fun test(x: Any) {
    val res = when {
        x is Box -> x.size
        else -> "default"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::ANY));
}

#[test]
fn test_when_subject_val_does_not_leak_to_outer_scope() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenLeak.kt").unwrap();
    let code = r#"
package com.example.control

class Box(val size: Int)

fun test(cond: Boolean, box: Box) {
    val res = if (cond) {
        val size = "outer string"
        val inner = when (val size = box.size) {
            else -> size.toString()
        }
        size
    } else {
        "fallback"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_when_entry_mutable_var_not_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenVar.kt").unwrap();
    let code = r#"
package com.example.control

class Box(val size: Int)

fun test(initial: Any) {
    var target = initial
    val res = when (target) {
        is Box -> target
        else -> null
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    // target is mutable var, so is Box is NOT smart cast; target remains Any, unifying with null yields Any?
    assert_eq!(inferred, Some(KotlinType::ANY.make_nullable()));
}

#[test]
fn test_when_multi_condition_all_narrowing_computes_lub() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenMulti.kt").unwrap();
    let code = r#"
package com.example.control

open class Animal
class Cat : Animal()
class Dog : Animal()

fun test(pet: Any) {
    val res = when (pet) {
        is Cat, is Dog -> pet
        else -> null
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(
        inferred,
        Some(KotlinType::simple_class("com.example.control.Animal").make_nullable())
    );
}

#[test]
fn test_when_multi_condition_with_non_narrowing_alternative_disables_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WhenMultiNonNarrow.kt").unwrap();
    let code = r#"
package com.example.control

class Box(val size: Int)

fun check(v: Any): Boolean = true

fun test(x: Any) {
    val res = when (x) {
        is Box, check(x) -> x.size
        else -> "default"
    }
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    // Because check(x) is an arbitrary expression that doesn't narrow, smart cast on x is disabled, x.size fails to resolve, falls back to "default"
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn test_generic_class_unification_does_not_blindly_subsume_incompatible_arguments() {
    let index = WorkspaceIndex::new();
    let t1 = lsp::parse_type_signature("List<Int>").unwrap();
    let t2 = lsp::parse_type_signature("List<String>").unwrap();
    let lub = unify_types(&index, &[t1, t2]);
    // Should NOT be List<String> or List<Int>; falls back to Any when conflicting
    assert_eq!(lub, Some(KotlinType::ANY));
}

#[test]
fn test_generic_subclass_to_non_generic_supertype_unification() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Hierarchy.kt").unwrap();
    let code = r#"
package com.example

open class Base
class GenericDerived<T> : Base()
"#;
    index.index_source(url, code);
    let t1 = lsp::parse_type_signature("com.example.GenericDerived<String>").unwrap();
    let t2 = lsp::parse_type_signature("com.example.Base").unwrap();
    let lub = unify_types(&index, &[t1, t2]);
    assert_eq!(lub, Some(KotlinType::simple_class("com.example.Base")));
}

#[test]
fn test_cross_package_identical_class_name_subtyping() {
    let index = WorkspaceIndex::new();
    assert!(!is_class_subtype(
        &index,
        "com.foo.Config",
        "com.bar.Config"
    ));
    assert!(!is_class_subtype(
        &index,
        "com.bar.Config",
        "com.foo.Config"
    ));
    let t1 = lsp::parse_type_signature("com.foo.Config").unwrap();
    let t2 = lsp::parse_type_signature("com.bar.Config").unwrap();
    let lub = unify_types(&index, &[t1, t2]);
    assert_eq!(lub, Some(KotlinType::ANY));
}

#[test]
fn test_subsequent_var_does_not_invalidate_prior_smart_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SubsequentVar.kt").unwrap();
    let code = r#"
package com.example

class Box(val size: Int)

fun test(x: Any) {
    val res = if (x is Box) {
        x.size
    } else {
        0
    }
    var x = 42
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_if_branch_with_labeled_return_evaluates_to_alternative_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LabeledReturn.kt").unwrap();
    let code = r#"
package com.example

fun compute(): Int {
    val res = if (true) return@compute 42 else 0
    return res
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn test_mismatched_labeled_return_does_not_inherit_function_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MismatchedLabeledReturn.kt").unwrap();
    let code = r#"
package com.example

fun compute(): Int {
    val res = if (true) return@lambda "hello" else "world"
    return 0
}
"#;
    index.index_source(url.clone(), code);
    let pos = Position::new(4, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}
