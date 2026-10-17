use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_on_variable_inferred_from_function_call() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val id: String, val name: String) {
    fun getDisplayName(): String = name
}

fun createUser(): User = User("1", "Alice")
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.createUser

fun test() {
    val user = createUser()
    user.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(7, 9), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"id".to_string()),
        "Should contain prop 'id'"
    );
    assert!(
        labels.contains(&"name".to_string()),
        "Should contain prop 'name'"
    );
    assert!(
        labels.contains(&"getDisplayName".to_string()),
        "Should contain method 'getDisplayName'"
    );
}

#[test]
fn test_member_definition_and_hover_on_variable_inferred_from_function_call() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val id: String, val name: String)

fun createUser(): User = User("1", "Alice")
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.createUser

fun test() {
    val user = createUser()
    val n = user.name
}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 17);
    let def = find_definition(&index, &main_url, "name", pos, main_code);
    assert!(
        def.is_some(),
        "Definition of 'user.name' on inferred variable should resolve"
    );
    assert_eq!(def.unwrap().uri, model_url);

    let hover = find_hover(&index, &main_url, "name", pos, main_code);
    assert!(
        hover.is_some(),
        "Hover on 'user.name' on inferred variable should resolve"
    );
}

#[test]
fn test_member_completion_on_variable_inferred_from_method_call() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class Profile(val bio: String, val avatarUrl: String)
class AccountService {
    fun getProfile(): Profile = Profile("bio", "https://img")
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.AccountService

fun test(service: AccountService) {
    val profile = service.getProfile()
    profile.
}
"#;
    index.index_source(service_url, service_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(7, 12), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"bio".to_string()),
        "Should contain prop 'bio'"
    );
    assert!(
        labels.contains(&"avatarUrl".to_string()),
        "Should contain prop 'avatarUrl'"
    );
}

#[test]
fn test_member_completion_and_definition_on_cast_variable() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

interface Vehicle
class Car(val horsepower: Int) : Vehicle
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.Vehicle
import com.example.model.Car

fun test(v: Vehicle) {
    val car = v as Car
    car.horsepower
}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(main_url.clone(), main_code);

    let pos_def = Position::new(8, 8);
    let def = find_definition(&index, &main_url, "horsepower", pos_def, main_code);
    assert!(
        def.is_some(),
        "Definition on 'car.horsepower' after cast should resolve"
    );
    assert_eq!(def.unwrap().uri, model_url);

    let completions = find_completions(&index, &main_url, Position::new(8, 8), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"horsepower".to_string()),
        "Should contain 'horsepower'"
    );
}

#[test]
fn test_member_completion_and_definition_on_safe_cast_variable() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

interface Worker
class Engineer(val language: String) : Worker
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.Worker
import com.example.model.Engineer

fun test(w: Worker) {
    val eng = w as? Engineer
    eng?.language
}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(main_url.clone(), main_code);

    let pos_def = Position::new(8, 9);
    let def = find_definition(&index, &main_url, "language", pos_def, main_code);
    assert!(
        def.is_some(),
        "Definition on 'eng?.language' after safe cast should resolve"
    );
    assert_eq!(def.unwrap().uri, model_url);

    let completions = find_completions(&index, &main_url, Position::new(8, 9), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"language".to_string()),
        "Should contain 'language'"
    );
}

#[test]
fn test_member_completion_on_local_variable_alias() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class Config(val timeout: Int, val retries: Int)
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.Config

fun test() {
    val c1 = Config(30, 3)
    val c2 = c1
    c2.
}
"#;
    index.index_source(service_url, service_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(8, 7), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"timeout".to_string()),
        "Should contain prop 'timeout'"
    );
    assert!(
        labels.contains(&"retries".to_string()),
        "Should contain prop 'retries'"
    );
}

#[test]
fn test_member_completion_on_variable_with_same_name_as_class() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example.pkg1

class f1 {}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.example.pkg2

class Foo {
    fun run() {}
}

val f1 = Foo()

fun test() {
    f1.
}
"#;
    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let completions = find_completions(&index, &file_b_url, Position::new(10, 7), file_b_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"run".to_string()),
        "Should suggest Foo.run on variable f1 despite class f1 existing in workspace"
    );
}

#[test]
fn test_member_completion_private_constructor_parameters_do_not_leak_as_supertypes() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ServiceTest.kt").unwrap();
    let code = r#"
class OrderRepo {
    val orders: Int = 100
    fun addOrder() {}
}

class FeedRepo {
    val feedCount: Int = 100
    fun getFeedList()
}

class AppService(
    private val orderRepo: OrderRepo,
    private val feedRepo: FeedRepo,
) {
    fun compose() {}
}

val orderRepo = OrderRepo()
val feedRepo = FeedRepo()
val service = AppService(orderRepo, feedRepo)

fun test() {
    service.
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(23, 12), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert_eq!(labels, vec!["compose".to_string()]);
}
