use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_filter_private_members_from_outside() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Account.kt").unwrap();
    let model_code = r#"
package com.example.account

class Account(
    val publicId: String,
    private val secretKey: String
) {
    fun getBalance(): Double = 100.0
    private fun internalEncrypt(): String = "enc"
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.account.Account

fun test(acc: Account) {
    acc.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let completions = find_completions(&index, &main_url, Position::new(6, 8), main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"publicId".to_string()));
    assert!(labels.contains(&"getBalance".to_string()));
    assert!(
        !labels.contains(&"secretKey".to_string()),
        "Private property secretKey should be filtered out"
    );
    assert!(
        !labels.contains(&"internalEncrypt".to_string()),
        "Private method internalEncrypt should be filtered out"
    );
}

#[test]
fn test_allow_private_members_with_this() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Account.kt").unwrap();
    let model_code = r#"
package com.example.account

class Account(
    val publicId: String,
    private val secretKey: String
) {
    fun getBalance(): Double {
        this.
        return 100.0
    }
    private fun internalEncrypt(): String = "enc"
}
"#;
    index.index_source(model_url.clone(), model_code);

    let completions = find_completions(&index, &model_url, Position::new(8, 13), model_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(labels.contains(&"publicId".to_string()));
    assert!(
        labels.contains(&"secretKey".to_string()),
        "Private property secretKey should be visible on this."
    );
    assert!(labels.contains(&"getBalance".to_string()));
    assert!(
        labels.contains(&"internalEncrypt".to_string()),
        "Private method internalEncrypt should be visible on this."
    );
}

#[test]
fn test_named_companion_object_members_accessible_on_class_name() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NamedCompanion.kt").unwrap();
    let code = r#"
package com.example.db

class Database {
    companion object Factory {
        fun create(): Database = Database()
        val DEFAULT_PORT: Int = 5432
    }
}

fun test() {
    Database.
    val db = Database.create()
    val port = Database.DEFAULT_PORT
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(11, 13);
    let completions = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"create".to_string()),
        "Database. should offer named companion factory method create()"
    );
    assert!(
        labels.contains(&"DEFAULT_PORT".to_string()),
        "Database. should offer named companion property DEFAULT_PORT"
    );

    let pos_create = Position::new(12, 23);
    let def_create = find_definition(&index, &url, "create", pos_create, code);
    assert!(
        def_create.is_some(),
        "Database.create() should resolve definition"
    );

    let pos_port = Position::new(13, 24);
    let def_port = find_definition(&index, &url, "DEFAULT_PORT", pos_port, code);
    assert!(
        def_port.is_some(),
        "Database.DEFAULT_PORT should resolve definition"
    );
}

#[test]
fn test_single_expression_function_return_type_member_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SingleExpr.kt").unwrap();
    let code = r#"
package com.example.expr

class Address(val city: String, val zipCode: String)
class User(val address: Address, val username: String)

class UserService {
    fun getUser() = User(Address("Tokyo", "100-0001"), "Alice")
}

fun test(service: UserService) {
    service.getUser().
    service.getUser().address.
}
"#;
    index.index_source(url.clone(), code);

    let pos_user = Position::new(11, 22);
    let user_comps = find_completions(&index, &url, pos_user, code);
    let user_labels: Vec<String> = user_comps.into_iter().map(|c| c.label).collect();
    assert!(
        user_labels.contains(&"address".to_string()),
        "service.getUser(). should offer 'address' from inferred User return type"
    );
    assert!(
        user_labels.contains(&"username".to_string()),
        "service.getUser(). should offer 'username' from inferred User return type"
    );

    let pos_addr = Position::new(12, 30);
    let addr_comps = find_completions(&index, &url, pos_addr, code);
    let addr_labels: Vec<String> = addr_comps.into_iter().map(|c| c.label).collect();
    assert!(
        addr_labels.contains(&"city".to_string()),
        "service.getUser().address. should offer 'city' from Address"
    );
    assert!(
        addr_labels.contains(&"zipCode".to_string()),
        "service.getUser().address. should offer 'zipCode' from Address"
    );
}

#[test]
fn test_member_completion_on_function_returning_spaced_generic() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SpacedGenericReturnTest.kt").unwrap();
    let code = r#"
package com.example.mapping

class Map<K, V>(val size: Int)

class ServiceRegistry {
    fun getServices(): Map<String, String> = Map(0)
}

fun test(registry: ServiceRegistry) {
    registry.getServices().
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(10, 27);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"size".to_string()),
        "registry.getServices(). should resolve Map return type instead of failing on 'Map<String,'"
    );
}
