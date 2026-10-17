use lsp::indexer::WorkspaceIndex;
use lsp::inference::special_receiver::resolve_special_expression_receiver;
use lsp::inference::trace::Tracer;
use lsp::inference::{KotlinType, infer_scope_receiver};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_apply_this_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Apply.kt").unwrap();
    let code = r#"
package com.example.scope

class Window(var width: Int, var height: Int)

fun setup() {
    val win = Window(800, 600)
    win.apply {
        val w = this
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 18);
    let inferred = infer_scope_receiver(&index, &url, "this", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.scope.Window"));
}

#[test]
fn test_infer_with_this_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/With.kt").unwrap();
    let code = r#"
package com.example.scope

class Window(var width: Int, var height: Int)

fun setup() {
    val win = Window(800, 600)
    with(win) {
        val w = this
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 18);
    let inferred = infer_scope_receiver(&index, &url, "this", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.scope.Window"));
}

#[test]
fn test_infer_let_it_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Let.kt").unwrap();
    let code = r#"
package com.example.scope

class Session(val token: String)

fun test(session: Session) {
    session.let {
        val s = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 18);
    let inferred = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.scope.Session"));
}

#[test]
fn test_infer_also_it_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Also.kt").unwrap();
    let code = r#"
package com.example.scope

class Session(val token: String)

fun test(session: Session) {
    session.also {
        val s = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 18);
    let inferred = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.scope.Session"));
}

#[test]
fn test_infer_extension_this_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtThis.kt").unwrap();
    let code = r#"
package com.example.extthis

class Session(val token: String)

fun Session.validate() {
    val s = this
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 14);
    let inferred = infer_scope_receiver(&index, &url, "this", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.extthis.Session"));
}

#[test]
fn test_resolve_special_expression_receiver_unspaced_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SpecialCastUnspaced.kt").unwrap();
    let code = r#"
package com.example

interface Worker
class Engineer(val lang: String) : Worker
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(0, 0);
    let mut tracer = Tracer::Noop;
    let ctx = lsp::inference::InferenceContext::new(&index, &url, pos, code, None, 0, &mut tracer);
    let inferred = resolve_special_expression_receiver(ctx, "w as(Engineer)");
    assert_eq!(inferred, KotlinType::parse("com.example.Engineer"));
}
