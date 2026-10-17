use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_on_variable() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/User.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val id: String, val name: String) {
    fun getDisplayName(): String = name
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.User

fun test() {
    val user: User = User("1", "Alice")
    user.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(7, 9), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"id".to_string()),
        "Should contain constructor prop 'id'"
    );
    assert!(
        labels.contains(&"name".to_string()),
        "Should contain constructor prop 'name'"
    );
    assert!(
        labels.contains(&"getDisplayName".to_string()),
        "Should contain method 'getDisplayName'"
    );
}

#[test]
fn test_member_completion_inferred_constructor() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class UserService {
    fun findUser(id: String): String = id
    val version: String = "1.0"
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.UserService

fun test() {
    val service = UserService()
    service.
}
"#;
    index.index_source(service_url, service_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(7, 12), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"findUser".to_string()));
    assert!(labels.contains(&"version".to_string()));
}

#[test]
fn test_member_completion_extension_functions() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/User.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val name: String)

fun User.greet(): String = "Hello $name"
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.User

fun test(user: User) {
    user.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(6, 9), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"name".to_string()));
    assert!(labels.contains(&"greet".to_string()));
}

#[test]
fn test_member_completion_chained_calls() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Address(val city: String, val zipCode: String)
class User(val id: String, val address: Address)
class UserService {
    fun getUser(): User = User("1", Address("Tokyo", "100"))
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.UserService

fun test(service: UserService) {
    service.getUser().address.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(6, 30), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"city".to_string()));
    assert!(labels.contains(&"zipCode".to_string()));
}

#[test]
fn test_member_completion_enum_and_companion() {
    let mut index = WorkspaceIndex::new();
    let types_url = Url::parse("file:///project/Types.kt").unwrap();
    let types_code = r#"
package com.example.types

enum class Status {
    ACTIVE,
    INACTIVE,
    PENDING
}

class Config {
    companion object {
        fun loadDefault(): Config = Config()
        val DEFAULT_TIMEOUT: Int = 30
    }
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.types.Status
import com.example.types.Config

fun test() {
    Status.
    Config.
}
"#;
    index.index_source(types_url, types_code);
    index.index_source(main_url.clone(), main_code);

    let status_comps = find_completions(&index, &main_url, Position::new(7, 11), main_code);
    let status_labels: Vec<String> = status_comps.into_iter().map(|c| c.label).collect();
    assert!(status_labels.contains(&"ACTIVE".to_string()));
    assert!(status_labels.contains(&"INACTIVE".to_string()));

    let config_comps = find_completions(&index, &main_url, Position::new(8, 11), main_code);
    let config_labels: Vec<String> = config_comps.into_iter().map(|c| c.label).collect();
    assert!(config_labels.contains(&"loadDefault".to_string()));
    assert!(config_labels.contains(&"DEFAULT_TIMEOUT".to_string()));
}
