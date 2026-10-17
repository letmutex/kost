use lsp::features::find_hover;
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, WorkspaceIndex};
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Range, SymbolKind, Url};

fn extract_code_block(text: &str) -> String {
    text.replace("```kotlin\n", "")
        .replace("```", "")
        .trim_end()
        .to_string()
}

#[test]
fn test_hover_top_level_function_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Utils.kt").unwrap();
    let source = r#"package com.example.utils

/** Formats a given name. */
fun formatName(name: String): String = name.trim()

fun main() {
    formatName(" Alice ")
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(6, 6);
    let hover = find_hover(&index, &url, "formatName", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.utils\n\nfun formatName(name: String): String = name.trim()\n\n\n---\n\n Formats a given name."
        );
        assert!(text.contains("Formats a given name."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_class_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Models.kt").unwrap();
    let source = r#"package com.example.models

class UserProfile(val id: String)

fun main() {
    val u = UserProfile("123")
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(5, 14);
    let hover = find_hover(&index, &url, "UserProfile", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert!(code.starts_with("com.example.models\n\nclass UserProfile("));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_member_function_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Services.kt").unwrap();
    let source = r#"package com.example.services

class OrderManager {
    fun cancelOrder(orderId: String): Boolean = true
}

fun main() {
    val manager = OrderManager()
    manager.cancelOrder("999")
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(8, 14);
    let hover = find_hover(&index, &url, "cancelOrder", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.services.OrderManager\n\nfun cancelOrder(orderId: String): Boolean = true"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_member_property_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Config.kt").unwrap();
    let source = r#"package com.example.config

class ServerConfig {
    val port: Int = 8080
}

fun main() {
    val cfg = ServerConfig()
    val p = cfg.port
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(8, 17);
    let hover = find_hover(&index, &url, "port", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.config.ServerConfig\n\nval port: Int = 8080"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_nested_class_and_member_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Outer.kt").unwrap();
    let source = r#"package com.example.nested

class Outer {
    class Inner {
        fun doSomething(): Unit {}
    }
}

fun main() {
    val inner = Outer.Inner()
    inner.doSomething()
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(10, 12);
    let hover = find_hover(&index, &url, "doSomething", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.nested.Outer.Inner\n\nfun doSomething(): Unit"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_extension_function_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Extensions.kt").unwrap();
    let source = r#"package com.example.ext

class CustomItem {
    val title: String = "item"
}

fun CustomItem.slugify(): String = this.title

fun main() {
    val item = CustomItem()
    item.slugify()
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(10, 11);
    let hover = find_hover(&index, &url, "slugify", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.ext\n\nfun CustomItem.slugify(): String = this.title"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_bytecode_function_strips_kt_facade() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///libs/utils.jar!/com/example/lib/TextKt.class").unwrap();

    let mut fs = FileSymbols {
        package_name: Some("com.example.lib".to_string()),
        ..Default::default()
    };
    let decl = std::sync::Arc::new(DeclarationItem::new(
        "capitalizeWords",
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::new(
            None,
            Some("fun capitalizeWords(input: String): String".into()),
            Some("TextKt".into()),
            false,
        ),
    ));
    fs.declarations.push((*decl).clone());
    index.files.insert(class_url.clone(), fs);
    index
        .name_to_decls
        .entry("capitalizeWords".into())
        .or_default()
        .push((std::sync::Arc::new(class_url.clone()), decl));

    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_source = r#"package com.example.app
import com.example.lib.capitalizeWords

fun main() {
    capitalizeWords("test")
}
"#;
    index.index_source(main_url.clone(), main_source);

    let pos = Position::new(4, 6);
    let hover = find_hover(&index, &main_url, "capitalizeWords", pos, main_source)
        .expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(
            code,
            "com.example.lib\n\nfun capitalizeWords(input: String): String\
            \n\n\n\n---\n*Defined in `TextKt.class` in [utils.jar](file:///libs/utils.jar)*"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_local_variable_and_parameter_have_no_package_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Local.kt").unwrap();
    let source = r#"package com.example.local

fun process(threshold: Int): Int {
    val totalCount = threshold * 2
    return totalCount
}
"#;
    index.index_source(url.clone(), source);

    // Hover on local variable totalCount
    let pos_var = Position::new(4, 13);
    let hover_var = find_hover(&index, &url, "totalCount", pos_var, source)
        .expect("local variable hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_var.contents {
        let code = extract_code_block(&text);
        assert_eq!(code, "val totalCount = threshold * 2");
        assert!(!code.contains("com.example.local"));
    } else {
        panic!("unexpected hover contents type");
    }

    // Hover on parameter threshold
    let pos_param = Position::new(2, 14);
    let hover_param = find_hover(&index, &url, "threshold", pos_param, source)
        .expect("parameter hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_param.contents {
        let code = extract_code_block(&text);
        assert_eq!(code, "val threshold: Int");
        assert!(!code.contains("com.example.local"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_default_package_top_level_has_no_header() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Root.kt").unwrap();
    let source = r#"
fun rootHelper(): String = "root"

fun main() {
    rootHelper()
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(4, 6);
    let hover = find_hover(&index, &url, "rootHelper", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let code = extract_code_block(&text);
        assert_eq!(code, "fun rootHelper(): String = \"root\"");
        assert!(!code.contains('.'));
    } else {
        panic!("unexpected hover contents type");
    }
}
