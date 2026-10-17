use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_unbound_function_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///UnboundRef.kt").unwrap();
    let source = r#"
package com.example

/**
 * Formats a message.
 */
fun formatMessage(msg: String): String = msg

fun main() {
    val fnRef = ::formatMessage
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `formatMessage` in `::formatMessage` (line 9, col 18)
    let pos = Position::new(9, 18);
    let hover =
        find_hover(&index, &url, "formatMessage", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun formatMessage(msg: String): String"));
        assert!(text.contains("Formats a message."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_member_function_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///MemberRef.kt").unwrap();
    let source = r#"
package com.example

class Greeter {
    /**
     * Greets a user.
     */
    fun greet(name: String): String = "Hello " + name
}

fun main() {
    val fnRef = Greeter::greet
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `greet` in `Greeter::greet` (line 11, col 26)
    let pos = Position::new(11, 26);
    let hover = find_hover(&index, &url, "greet", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun greet(name: String): String"));
        assert!(text.contains("Greets a user."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_extension_function_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ExtensionRef.kt").unwrap();
    let source = r#"
package com.example

class Item(val value: Int)

/**
 * Doubles item value.
 */
fun Item.double(): Int = value * 2

fun main() {
    val fnRef = Item::double
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `double` in `Item::double` (line 11, col 23)
    let pos = Position::new(11, 23);
    let hover = find_hover(&index, &url, "double", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun Item.double(): Int"));
        assert!(text.contains("Doubles item value."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_constructor_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ConstructorRef.kt").unwrap();
    let source = r#"
package com.example

/**
 * Represents a person.
 */
class Person(val name: String)

fun main() {
    val factory = ::Person
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `Person` in `::Person` (line 9, col 21)
    let pos = Position::new(9, 21);
    let hover = find_hover(&index, &url, "Person", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("class Person"));
        assert!(text.contains("Represents a person."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_property_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///PropertyRef.kt").unwrap();
    let source = r#"
package com.example

class Account {
    /**
     * Current account balance.
     */
    val balance: Double = 0.0
}

fun main() {
    val propRef = Account::balance
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `balance` in `Account::balance` (line 11, col 28)
    let pos = Position::new(11, 28);
    let hover = find_hover(&index, &url, "balance", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val balance: Double"));
        assert!(text.contains("Current account balance."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_function_reference_inside_call_arguments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///CallArgRef.kt").unwrap();
    let source = r#"
package com.example

fun transform(n: Int): String = n.toString()

fun apply(fn: (Int) -> String): String = fn(42)

fun main() {
    apply(::transform)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `transform` in `apply(::transform)` (line 8, col 13)
    let pos = Position::new(8, 13);
    let hover = find_hover(&index, &url, "transform", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun transform(n: Int): String"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_bound_instance_callable_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///BoundInstanceRef.kt").unwrap();
    let source = r#"
package com.example

class WorkerService {
    /**
     * Executes the task.
     */
    fun executeTask(): Boolean = true
}

fun main() {
    val worker = WorkerService()
    val fnRef = worker::executeTask
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `executeTask` in `worker::executeTask` (line 12, col 26)
    let pos = Position::new(12, 26);
    let hover = find_hover(&index, &url, "executeTask", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun executeTask(): Boolean"));
        assert!(text.contains("Executes the task."));
    } else {
        panic!("unexpected hover contents type");
    }
}
