use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::KotlinType;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn implicit_receiver_generic_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ImplicitReceiverGeneric.kt").unwrap();
    let code = r#"
package test.receiver

class Scope {
    fun <T> compute(expr: String): T = TODO()
}

fun <R> withScope(block: Scope.() -> R): R = TODO()

fun test() {
    withScope {
        val result = compute<Int>("1 + 2")
        val x = 1
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(12, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::INT));
}

#[test]
fn implicit_receiver_member_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ImplicitReceiverMember.kt").unwrap();
    let code = r#"
package test.receiver

class Buffer
class Worker {
    fun execute(code: String): Buffer = TODO()
}

fun <R> runWorker(block: Worker.() -> R): R = TODO()

fun test() {
    runWorker {
        val buffer = execute("1 + 2")
        val x = 1
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(13, 8);
    let inferred = infer_variable_type(&index, &url, "buffer", pos, code, None);
    let expected = KotlinType::simple_class("test.receiver.Buffer");
    assert_eq!(inferred, Some(expected));
}

#[test]
fn enclosing_generic_extension_receiver_qualifies_type_arguments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtGenericThis.kt").unwrap();
    let code = r#"
package test.receiver

class Item
class Box<T>

fun Box<Item>.inspect() {
    val selfBox = this
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 4);
    let inferred = infer_variable_type(&index, &url, "selfBox", pos, code, None);
    let expected = KotlinType::class(
        "test.receiver.Box",
        vec![lsp::inference::types::TypeArg::invariant(
            KotlinType::simple_class("test.receiver.Item"),
        )],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn unimported_extension_lambda_not_resolved_via_container_lookup() {
    let mut index = WorkspaceIndex::new();
    let hidden_url = Url::parse("file:///project/HiddenExt.kt").unwrap();
    let hidden_code = r#"
package hidden.pkg

import test.receiver.Worker

fun Worker.secretRun(block: (Int) -> Unit) {}
"#;
    index.index_source(hidden_url, hidden_code);

    let url = Url::parse("file:///project/MainWorker.kt").unwrap();
    let code = r#"
package test.receiver

class Worker

fun test(w: Worker) {
    w.secretRun { param ->
        val p = param
        val x = 1
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &url, "p", pos, code, None);
    assert_eq!(inferred, None);
}
