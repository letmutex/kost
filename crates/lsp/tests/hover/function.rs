use lsp::features::{find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_multiline_function_with_annotations_and_kdoc() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///OrderService.kt").unwrap();
    let source = r#"
package com.example

/**
 * Processes an incoming customer order.
 * @param orderId unique identifier for order
 * @param amount total cost in USD
 * @return the processed OrderResult
 * @throws IllegalArgumentException if amount is negative
 */
@OptIn(ExperimentalCoroutinesApi::class)
suspend fun processOrder(
    orderId: String,
    amount: Double,
    customer: Customer
): OrderResult {
    return OrderResult(orderId, amount)
}

fun main() {
    processOrder("123", 99.0, Customer())
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 20,
        character: 6,
    };
    let hover =
        find_hover(&index, &url, "processOrder", pos, source).expect("hover should resolve");

    let range = hover.range.expect("hover range should be present");
    assert_eq!(range.start.line, 20);
    assert_eq!(range.end.line, 20);

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("@OptIn(ExperimentalCoroutinesApi::class)"));
        assert!(text.contains("suspend fun processOrder("));
        assert!(text.contains("orderId: String,"));
        assert!(text.contains("amount: Double,"));
        assert!(text.contains("customer: Customer"));
        assert!(text.contains("): OrderResult"));
        assert!(text.contains("Processes an incoming customer order."));
        assert!(text.contains("* **@param** `orderId` — unique identifier for order"));
        assert!(text.contains("* **@param** `amount` — total cost in USD"));
        assert!(text.contains("* **@return** — the processed OrderResult"));
        assert!(text.contains("* **@throws** `IllegalArgumentException` — if amount is negative"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_extension_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Extensions.kt").unwrap();
    let source = r#"
package com.example

/**
 * Capitalizes and trims string.
 */
fun String.customFormat(prefix: String = ""): String {
    return prefix + this.trim()
}

fun test() {
    "hello".customFormat(">> ")
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 11,
        character: 14,
    };
    let hover =
        find_hover(&index, &url, "customFormat", pos, source).expect("hover should resolve");

    let range = hover.range.expect("hover range should be present");
    assert_eq!(range.start.line, 11);
    assert_eq!(range.end.line, 11);

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun String.customFormat(prefix: String = \"\"): String"));
        assert!(text.contains("Capitalizes and trims string."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_member_function_indentation_normalized() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///UserService.kt").unwrap();
    let source = r#"
package com.example

class UserService {
    fun findUser(
        id: String,
        name: String
    ): User {
        return User(id)
    }
}

fun main() {
    val service = UserService()
    service.findUser("1", "Alice")
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 14,
        character: 14,
    };
    let hover = find_hover(&index, &url, "findUser", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let parts: Vec<&str> = text.split("```").collect();
        assert!(parts.len() >= 3);
        let code_block = parts[1].trim().trim_start_matches("kotlin\n");
        let expected = "fun findUser(\n    id: String,\n    name: String\n): User";
        assert_eq!(code_block, expected);
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_function_hover_excludes_parameter_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///PaymentService.kt").unwrap();
    let source = r#"
package com.example

/**
 * Executes a remote payment.
 */
fun pay(
    // Unique payment identifier
    paymentId: String,
    /** Customer token for verification */
    token: String
): Boolean {
    return true
}

fun test() {
    pay("p1", "tok")
}
"#;

    index.index_source(url.clone(), source);

    let pos_fn = Position {
        line: 16,
        character: 6,
    };
    let hover_fn =
        find_hover(&index, &url, "pay", pos_fn, source).expect("hover on function should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_fn.contents {
        assert!(text.contains("Executes a remote payment."));
        assert!(!text.contains("Unique payment identifier"));
        assert!(!text.contains("Customer token for verification"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_parameter_hover_excludes_function_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///PaymentService2.kt").unwrap();
    let source = r#"
package com.example

/**
 * Executes a remote payment.
 */
fun pay(
    // Unique payment identifier
    paymentId: String,
    /** Customer token for verification */
    token: String
): Boolean {
    val pid = paymentId
    val tok = token
    return true
}
"#;

    index.index_source(url.clone(), source);

    // 1. Hover on 'paymentId' (line 11, col 16)
    let pos_param1 = Position {
        line: 11,
        character: 16,
    };
    let hover_param1 = find_hover(&index, &url, "paymentId", pos_param1, source)
        .expect("hover on paymentId should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_param1.contents {
        assert!(text.contains("val paymentId: String"));
        assert!(text.contains("Unique payment identifier"));
        assert!(!text.contains("Executes a remote payment."));
        assert!(!text.contains("Customer token for verification"));
    } else {
        panic!("unexpected hover contents type");
    }

    // 2. Hover on 'token' (line 12, col 16)
    let pos_param2 = Position {
        line: 12,
        character: 16,
    };
    let hover_param2 = find_hover(&index, &url, "token", pos_param2, source)
        .expect("hover on token should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_param2.contents {
        assert!(text.contains("val token: String"));
        assert!(text.contains("Customer token for verification"));
        assert!(!text.contains("Executes a remote payment."));
        assert!(!text.contains("Unique payment identifier"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_local_function_resolution_and_scoping() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalFunction.kt").unwrap();
    let code = r#"
package com.example.local

fun outerFunction() {
    fun localHelper(data: String): String = data.trim()
    val res = localHelper("  abc  ")
}

fun anotherFunction() {
    val leaked = localHelper("test")
}
"#;
    index.index_source(url.clone(), code);

    let pos_inside = Position::new(5, 16);
    let def_inside = find_definition(&index, &url, "localHelper", pos_inside, code);
    assert!(
        def_inside.is_some(),
        "localHelper should resolve inside outerFunction"
    );

    let hover_inside = find_hover(&index, &url, "localHelper", pos_inside, code);
    assert!(
        hover_inside.is_some(),
        "localHelper should have hover inside outerFunction"
    );

    let pos_outside = Position::new(9, 18);
    let def_outside = find_definition(&index, &url, "localHelper", pos_outside, code);
    assert!(
        def_outside.is_none(),
        "localHelper must NOT resolve inside anotherFunction"
    );

    let hover_outside = find_hover(&index, &url, "localHelper", pos_outside, code);
    assert!(
        hover_outside.is_none(),
        "localHelper must NOT have hover inside anotherFunction"
    );
}
