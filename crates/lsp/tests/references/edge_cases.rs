use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_overloaded_function_selects_matching_signature() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Math.kt").unwrap();
    let file_a_code = r#"
package com.example

fun calculate(num: Int): Int = num * 2
fun calculate(text: String): String = text + text
"#;

    let file_b_url = Url::parse("file:///project/MathUsage.kt").unwrap();
    let file_b_code = r#"
package com.example

fun run() {
    val a = calculate(10)
    val b = calculate("hello")
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on calculate(num: Int) declaration (line 3, char 4)
    let pos_int = Position::new(3, 4);
    let refs_int = find_references(
        &index,
        &file_a_url,
        "calculate",
        pos_int,
        file_a_code,
        false,
    );

    assert_eq!(refs_int.len(), 1, "Should only match calculate(10)");
    assert_eq!(refs_int[0].range.start.line, 4); // line 4 in File B is calculate(10)

    // Cursor on calculate(text: String) declaration (line 4, char 4)
    let pos_str = Position::new(4, 4);
    let refs_str = find_references(
        &index,
        &file_a_url,
        "calculate",
        pos_str,
        file_a_code,
        false,
    );

    assert_eq!(refs_str.len(), 1, "Should only match calculate(\"hello\")");
    assert_eq!(refs_str[0].range.start.line, 5); // line 5 in File B is calculate("hello")
}

#[test]
fn test_references_extension_function_call_across_files() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/StringExt.kt").unwrap();
    let file_a_code = r#"
package com.example

fun String.customDouble(): String = this + this
"#;

    let file_b_url = Url::parse("file:///project/Usage.kt").unwrap();
    let file_b_code = r#"
package com.example

fun run() {
    val text = "test"
    val doubled = text.customDouble()
}
"#;

    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on customDouble declaration (line 3, char 11)
    let pos = Position::new(3, 11);
    let refs = find_references(&index, &file_a_url, "customDouble", pos, file_a_code, false);

    assert_eq!(refs.len(), 1, "Should find extension call site in File B");
    assert_eq!(refs[0].uri, file_b_url);
    assert_eq!(refs[0].range.start.line, 5);
}

#[test]
fn test_references_variable_used_in_string_template() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TemplateTest.kt").unwrap();
    let code = r#"
fun formatGreeting(name: String): String {
    val message = "Hello, $name!"
    return "${name}: $message"
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on parameter "name" (line 1, char 19)
    let pos = Position::new(1, 19);
    let refs = find_references(&index, &file_url, "name", pos, code, false);

    assert_eq!(
        refs.len(),
        2,
        "Should find both $name and ${{name}} usages inside string templates"
    );
    assert_eq!(refs[0].range.start.line, 2);
    assert_eq!(refs[1].range.start.line, 3);
}

#[test]
fn test_references_triggered_from_interpolated_identifier() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TemplateTest.kt").unwrap();
    let code = r#"
fun formatGreeting(name: String): String {
    val message = "Hello, $name!"
    return "${name}: $message"
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor directly on "$name" inside string template (line 2, char 28)
    let template_pos = Position::new(2, 28);
    let refs = find_references(&index, &file_url, "name", template_pos, code, false);

    assert_eq!(
        refs.len(),
        2,
        "Should find both usages when triggered from inside string template"
    );
    assert_eq!(refs[0].range.start.line, 2);
    assert_eq!(refs[1].range.start.line, 3);
}

#[test]
fn test_references_preserves_lambda_call_with_coerced_unit_return_type() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun schedule(task: () -> Unit) {}
fun schedule(task: () -> Int) {}

fun test() {
    schedule {
        42
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun schedule(task: () -> Unit)` declaration (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &file_url, "schedule", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to schedule(() -> Unit) should match lambda returning Int expression"
    );
}

#[test]
fn test_references_tied_overload_star_import_resolves_targeted_declaration() {
    let mut index = WorkspaceIndex::new();
    let lib_url = Url::parse("file:///project/Lib.kt").unwrap();
    let lib_code = r#"
package com.example.lib

fun cancel(reason: String? = null) {}

interface TaskRunner {
    fun cancel(reason: String? = null)
}
"#;
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

import com.example.lib.*

fun run(runner: TaskRunner) {
    runner.apply {
        cancel("stopped")
    }
}
"#;
    index.index_source(lib_url.clone(), lib_code);
    index.index_source(app_url.clone(), app_code);

    // Cursor at `TaskRunner.cancel` member declaration (line 6, col 8 in Lib.kt)
    let decl_pos = Position::new(6, 8);
    let refs = find_references(&index, &lib_url, "cancel", decl_pos, lib_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to TaskRunner.cancel should match cancel call inside runner.apply"
    );
}

#[test]
fn test_references_member_to_string_matches_only_target_class() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Models.kt").unwrap();
    let code = r#"
package com.example

class User {
    override fun toString(): String = "User"
}

class Product {
    override fun toString(): String = "Product"
}

fun usage(paramUser: User, paramProduct: Product) {
    val localUser: User = User()
    val nullableUser: User? = User()
    val localProduct: Product = Product()

    val fromParam = paramUser.toString()
    val fromLocal = localUser.toString()
    val fromNullable = nullableUser?.toString()
    val fromProduct = localProduct.toString()
    val fromNumber = 42.toString()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on User.toString declaration (line 4, col 17)
    let decl_pos = Position::new(4, 17);
    let refs = find_references(&index, &file_url, "toString", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        3,
        "Finding references to User.toString should match paramUser, localUser, and nullableUser while excluding product and literal"
    );
    assert_eq!(refs[0].range.start.line, 16);
    assert_eq!(refs[1].range.start.line, 17);
    assert_eq!(refs[2].range.start.line, 18);
}

#[test]
fn test_references_companion_object_member_called_on_class_name() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Factory.kt").unwrap();
    let code = r#"
package com.example

class Config {
    companion object {
        fun default(): Config = Config()
    }
}

fun run() {
    val config = Config.default()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor on `fun default(): Config` declaration (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_url, "default", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to Config.Companion.default should find Config.default() call"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_member_called_via_chained_call_without_naming_container() {
    let mut index = WorkspaceIndex::new();
    let models_url = Url::parse("file:///project/Models.kt").unwrap();
    let models_code = r#"
package com.example.models

class User(val name: String)
class Order(val customer: User)
"#;
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"
package com.example.client

import com.example.models.*

fun inspect(order: Order): String {
    return order.customer.name
}
"#;
    index.index_source(models_url.clone(), models_code);
    index.index_source(client_url.clone(), client_code);

    // Cursor on `User.name` declaration (line 3, col 15 in Models.kt)
    let decl_pos = Position::new(3, 15);
    let refs = find_references(&index, &models_url, "name", decl_pos, models_code, false);

    assert_eq!(
        refs.len(),
        1,
        "Finding references to User.name should find order.customer.name in Client.kt"
    );
    assert_eq!(refs[0].uri, client_url);
    assert_eq!(refs[0].range.start.line, 6);
}
