use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_parenthesized_variable_completion_and_definition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UserTest.kt").unwrap();
    let code = r#"
package com.example.paren

class User(val name: String) {
    fun greet(): String = "Hello $name"
}

fun test(u: User) {
    (u).
    val n = (u).name
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(8, 8);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"name".to_string()),
        "(u). should offer property 'name'"
    );
    assert!(
        labels.contains(&"greet".to_string()),
        "(u). should offer method 'greet'"
    );

    let pos_name = Position::new(9, 17);
    let def = find_definition(&index, &url, "name", pos_name, code);
    assert!(
        def.is_some(),
        "(u).name should resolve to definition of 'name'"
    );
}

#[test]
fn test_cast_expression_completion_and_definition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CastTest.kt").unwrap();
    let code = r#"
package com.example.cast

open class Animal
class Dog(val breed: String) : Animal() {
    fun bark(): String = "Woof"
}

fun test(animal: Animal) {
    (animal as Dog).
    val b = (animal as Dog).breed
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(9, 20);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"breed".to_string()),
        "(animal as Dog). should offer property 'breed'"
    );
    assert!(
        labels.contains(&"bark".to_string()),
        "(animal as Dog). should offer method 'bark'"
    );

    let pos_breed = Position::new(10, 29);
    let def = find_definition(&index, &url, "breed", pos_breed, code);
    assert!(
        def.is_some(),
        "(animal as Dog).breed should resolve to definition of 'breed'"
    );
}

#[test]
fn test_elvis_expression_completion_and_definition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ElvisTest.kt").unwrap();
    let code = r#"
package com.example.elvis

class Account(val balance: Double) {
    fun deposit(amount: Double): Double = balance + amount
}

fun test(acc: Account?, defaultAcc: Account) {
    (acc ?: defaultAcc).
    val b = (acc ?: defaultAcc).balance
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(8, 24);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"balance".to_string()),
        "(acc ?: defaultAcc). should offer property 'balance'"
    );
    assert!(
        labels.contains(&"deposit".to_string()),
        "(acc ?: defaultAcc). should offer method 'deposit'"
    );

    let pos_bal = Position::new(9, 33);
    let def = find_definition(&index, &url, "balance", pos_bal, code);
    assert!(
        def.is_some(),
        "(acc ?: defaultAcc).balance should resolve to definition of 'balance'"
    );
}
