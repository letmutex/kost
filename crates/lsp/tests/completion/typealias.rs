use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_and_definition_on_typealias() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TypeAliasMemberTest.kt").unwrap();
    let code = r#"
package com.example.aliases

class Account(val accountNumber: String, val balance: Double) {
    fun closeAccount(): Boolean = true
}

typealias UserAccount = Account

fun test(account: UserAccount) {
    account.
    val b = account.balance
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion on account. (typed as UserAccount) should offer Account members:
    let pos_dot = Position::new(10, 12);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"accountNumber".to_string()),
        "account. should offer 'accountNumber' from underlying Account class"
    );
    assert!(
        labels.contains(&"balance".to_string()),
        "account. should offer 'balance' from underlying Account class"
    );
    assert!(
        labels.contains(&"closeAccount".to_string()),
        "account. should offer 'closeAccount' from underlying Account class"
    );

    // 2. Definition of account.balance:
    let pos_bal = Position::new(11, 22);
    let def = find_definition(&index, &url, "balance", pos_bal, code);
    assert!(
        def.is_some(),
        "account.balance should resolve definition on underlying Account class"
    );
}
