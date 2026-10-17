use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_remember_trailing_lambda_without_function_declaration_does_not_infer_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val shouldNotAppear: String)

fun test() {
    val state = remember {
        Dummy("hello")
    }
    state.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(9, 10);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        !labels.contains(&"shouldNotAppear".to_string()),
        "Should not infer type when there is no function declaration for remember {{}}"
    );
}

#[test]
fn test_remember_with_args_and_trailing_lambda_without_function_declaration_does_not_infer_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val shouldNotAppear: String)

fun test() {
    val state = remember("key") {
        Dummy("hello")
    }
    state.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(9, 10);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        !labels.contains(&"shouldNotAppear".to_string()),
        "Should not infer type when there is no function declaration for remember(key) {{}}"
    );
}

#[test]
fn test_remember_without_function_declaration_does_not_resolve_definition_or_hover() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val shouldNotAppear: String)

fun test() {
    val state = remember {
        Dummy("hello")
    }
    val x = state.shouldNotAppear
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(9, 18);
    let def = find_definition(&index, &main_url, "shouldNotAppear", pos, main_code);
    assert!(
        def.is_none(),
        "Should not resolve definition on member of type inferred from remember {{}} without decl"
    );

    let hover = find_hover(&index, &main_url, "shouldNotAppear", pos, main_code);
    assert!(
        hover.is_none(),
        "Should not resolve hover on member of type inferred from remember {{}} without decl"
    );
}

#[test]
fn test_remember_trailing_lambda_with_function_declaration_infers_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val message: String)

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val state = remember {
        Dummy("hello")
    }
    state.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(17, 10);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"message".to_string()),
        "Should infer type from trailing lambda call like remember {{}}"
    );
}

#[test]
fn test_remember_with_args_and_trailing_lambda_with_function_declaration_infers_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val message: String)

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val state = remember("key") {
        Dummy("hello")
    }
    state.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(17, 10);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"message".to_string()),
        "Should infer type from remember(key) {{}} with trailing lambda"
    );
}

#[test]
fn test_remember_with_function_declaration_resolves_definition_or_hover() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val message: String)

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val state = remember {
        Dummy("hello")
    }
    val x = state.message
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(17, 18);
    let def = find_definition(&index, &main_url, "message", pos, main_code);
    assert!(
        def.is_some(),
        "Should resolve definition on member of type inferred from remember {{}}"
    );

    let hover = find_hover(&index, &main_url, "message", pos, main_code);
    assert!(
        hover.is_some(),
        "Should resolve hover on member of type inferred from remember {{}}"
    );
}

#[test]
fn test_arbitrary_function_with_trailing_lambda_infers_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class Dummy(val message: String)

fun <T> runBlocking(block: () -> T): T = block()

fun test() {
    val res = runBlocking {
        Dummy("hello")
    }
    res.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(11, 8);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"message".to_string()),
        "Should infer type from arbitrary call with trailing lambda like runBlocking {{}}"
    );
}

#[test]
fn test_direct_constructor_and_function_call_still_infers_type() {
    let mut index = WorkspaceIndex::new();
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

class User(val name: String)

fun createUser(): User = User("Alice")

fun test() {
    val u1 = User("Bob")
    val u2 = createUser()
    u1.
    u2.
}
"#;
    index.index_source(main_url.clone(), main_code);

    let completions1 = find_completions(&index, &main_url, Position::new(10, 7), main_code);
    let labels1: Vec<String> = completions1.into_iter().map(|c| c.label).collect();
    assert!(
        labels1.contains(&"name".to_string()),
        "Direct constructor call should still infer User"
    );

    let completions2 = find_completions(&index, &main_url, Position::new(11, 7), main_code);
    let labels2: Vec<String> = completions2.into_iter().map(|c| c.label).collect();
    assert!(
        labels2.contains(&"name".to_string()),
        "Normal function call without trailing lambda should still infer User"
    );
}
