use lsp::features::{find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_secondary_constructor_named_argument_definition() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SecondaryConstructorTest.kt").unwrap();
    let code = r#"
package com.example.conn

class ConnectionConfig {
    val host: String
    val port: Int

    /**
     * Creates a connection configuration.
     * @param host Target server hostname
     * @param port Target server port
     */
    constructor(host: String, port: Int) {
        this.host = host
        this.port = port
    }
}

fun test() {
    val config = ConnectionConfig(host = "localhost", port = 9090)
}
"#;
    index.index_source(url.clone(), code);

    let pos_host = Position::new(19, 35);
    let def = find_definition(&index, &url, "host", pos_host, code);
    assert!(
        def.is_some(),
        "Named argument 'host' targeting secondary constructor should resolve definition"
    );

    let def_loc = def.unwrap();
    assert_eq!(def_loc.range.start.line, 12);
}

#[test]
fn test_secondary_constructor_named_argument_hover() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SecondaryConstructorTest.kt").unwrap();
    let code = r#"
package com.example.conn

class ConnectionConfig {
    val host: String
    val port: Int

    /**
     * Creates a connection configuration.
     * @param host Target server hostname
     * @param port Target server port
     */
    constructor(host: String, port: Int) {
        this.host = host
        this.port = port
    }
}

fun test() {
    val config = ConnectionConfig(host = "localhost", port = 9090)
}
"#;
    index.index_source(url.clone(), code);

    let pos_host = Position::new(19, 35);
    let hover = find_hover(&index, &url, "host", pos_host, code);
    assert!(
        hover.is_some(),
        "Named argument 'host' targeting secondary constructor should resolve hover"
    );
}
