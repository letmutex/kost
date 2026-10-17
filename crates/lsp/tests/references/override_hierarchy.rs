use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_interface_method_and_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/InterfaceOverride.kt").unwrap();
    let code = r#"
package com.example

interface Base {
    fun doSomething()
}

class BaseImpl : Base {
    override fun doSomething() {}
}
"#;
    index.index_source(file_url.clone(), code);

    // 1. Cursor at Base.doSomething declaration (line 4, col 8)
    let base_decl_pos = Position::new(4, 8);
    let refs_from_base =
        find_references(&index, &file_url, "doSomething", base_decl_pos, code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "Base.doSomething references should include interface declaration and BaseImpl override"
    );
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 8));

    // 2. Cursor at BaseImpl.doSomething override (line 8, col 17)
    let impl_decl_pos = Position::new(8, 17);
    let refs_from_impl =
        find_references(&index, &file_url, "doSomething", impl_decl_pos, code, true);

    assert_eq!(
        refs_from_impl.len(),
        2,
        "BaseImpl.doSomething references should include override and overridden Base declaration"
    );
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 8));
}

#[test]
fn test_references_abstract_class_method_and_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AbstractOverride.kt").unwrap();
    let code = r#"
package com.example

abstract class AbstractWorker {
    abstract fun process()
}

class ConcreteWorker : AbstractWorker() {
    override fun process() {}
}
"#;
    index.index_source(file_url.clone(), code);

    // 1. Cursor at AbstractWorker.process declaration (line 4, col 17)
    let abstract_pos = Position::new(4, 17);
    let refs_from_abstract =
        find_references(&index, &file_url, "process", abstract_pos, code, true);

    assert_eq!(
        refs_from_abstract.len(),
        2,
        "AbstractWorker.process references should include abstract declaration and ConcreteWorker override"
    );
    assert!(
        refs_from_abstract
            .iter()
            .any(|loc| loc.range.start.line == 4)
    );
    assert!(
        refs_from_abstract
            .iter()
            .any(|loc| loc.range.start.line == 8)
    );

    // 2. Cursor at ConcreteWorker.process override (line 8, col 17)
    let concrete_pos = Position::new(8, 17);
    let refs_from_concrete =
        find_references(&index, &file_url, "process", concrete_pos, code, true);

    assert_eq!(
        refs_from_concrete.len(),
        2,
        "ConcreteWorker.process references should include override and overridden AbstractWorker declaration"
    );
    assert!(
        refs_from_concrete
            .iter()
            .any(|loc| loc.range.start.line == 4)
    );
    assert!(
        refs_from_concrete
            .iter()
            .any(|loc| loc.range.start.line == 8)
    );
}

#[test]
fn test_references_open_class_method_and_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/OpenOverride.kt").unwrap();
    let code = r#"
package com.example

open class BaseService {
    open fun execute() {}
}

class CustomService : BaseService() {
    override fun execute() {}
}
"#;
    index.index_source(file_url.clone(), code);

    // 1. Cursor at BaseService.execute declaration (line 4, col 13)
    let base_pos = Position::new(4, 13);
    let refs_from_base = find_references(&index, &file_url, "execute", base_pos, code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "BaseService.execute references should include open declaration and CustomService override"
    );
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 8));

    // 2. Cursor at CustomService.execute override (line 8, col 17)
    let custom_pos = Position::new(8, 17);
    let refs_from_custom = find_references(&index, &file_url, "execute", custom_pos, code, true);

    assert_eq!(
        refs_from_custom.len(),
        2,
        "CustomService.execute references should include override and overridden BaseService declaration"
    );
    assert!(refs_from_custom.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_custom.iter().any(|loc| loc.range.start.line == 8));
}

#[test]
fn test_references_multilevel_inheritance_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/MultilevelOverride.kt").unwrap();
    let code = r#"
package com.example

interface Component {
    fun render()
}

open class BaseComponent : Component {
    override fun render() {}
}

class LeafComponent : BaseComponent() {
    override fun render() {}
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at root Component.render declaration (line 4, col 8)
    let root_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "render", root_pos, code, true);

    assert_eq!(
        refs.len(),
        3,
        "Component.render references should include interface, intermediate override, and leaf override"
    );
    assert!(refs.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs.iter().any(|loc| loc.range.start.line == 8));
    assert!(refs.iter().any(|loc| loc.range.start.line == 12));

    // Cursor at intermediate BaseComponent.render override (line 8, col 17)
    let intermediate_pos = Position::new(8, 17);
    let refs_intermediate =
        find_references(&index, &file_url, "render", intermediate_pos, code, true);
    assert_eq!(refs_intermediate.len(), 3);

    // Cursor at leaf LeafComponent.render override (line 12, col 17)
    let leaf_pos = Position::new(12, 17);
    let refs_leaf = find_references(&index, &file_url, "render", leaf_pos, code, true);
    assert_eq!(refs_leaf.len(), 3);
}

#[test]
fn test_references_override_property_declaration() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/PropertyOverride.kt").unwrap();
    let code = r#"
package com.example

interface Config {
    val maxRetries: Int
}

class DefaultConfig : Config {
    override val maxRetries: Int = 3
}
"#;
    index.index_source(file_url.clone(), code);

    // 1. Cursor at Config.maxRetries declaration (line 4, col 8)
    let base_pos = Position::new(4, 8);
    let refs_from_base = find_references(&index, &file_url, "maxRetries", base_pos, code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "Config.maxRetries references should include interface property and DefaultConfig override"
    );
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 8));

    // 2. Cursor at DefaultConfig.maxRetries override (line 8, col 17)
    let impl_pos = Position::new(8, 17);
    let refs_from_impl = find_references(&index, &file_url, "maxRetries", impl_pos, code, true);

    assert_eq!(
        refs_from_impl.len(),
        2,
        "DefaultConfig.maxRetries references should include override and overridden Config property declaration"
    );
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 8));
}

#[test]
fn test_references_override_excludes_unrelated_same_arity_overload() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/OverloadOverride.kt").unwrap();
    let code = r#"
package com.example

open class Processor {
    open fun process(name: String) {}
    open fun process(count: Int) {}
}

class CustomProcessor : Processor() {
    override fun process(name: String) {}
}
"#;
    index.index_source(file_url.clone(), code);

    // Finding references on Processor.process(count: Int) at line 5, col 13
    let count_pos = Position::new(5, 13);
    let refs_count = find_references(&index, &file_url, "process", count_pos, code, true);

    assert_eq!(
        refs_count.len(),
        1,
        "Processor.process(count: Int) should only find itself, not CustomProcessor.process(name: String)"
    );
}

#[test]
fn test_references_override_excludes_any_parameter_mismatch() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AnyMismatchOverride.kt").unwrap();
    let code = r#"
package com.example

open class Handler {
    open fun handle(msg: Any) {}
    open fun handle(count: Int) {}
}

class CustomHandler : Handler() {
    override fun handle(count: Int) {}
}
"#;
    index.index_source(file_url.clone(), code);

    let any_pos = Position::new(4, 13);
    let refs_any = find_references(&index, &file_url, "handle", any_pos, code, true);

    assert_eq!(
        refs_any.len(),
        1,
        "Handler.handle(msg: Any) should only find itself, not CustomHandler.handle(count: Int)"
    );
}

#[test]
fn test_references_annotated_override_declaration() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AnnotatedOverride.kt").unwrap();
    let code = r#"
package com.example

interface Service {
    fun execute()
}

class CustomService : Service {
    @Deprecated("old")
    override fun execute() {}
}
"#;
    index.index_source(file_url.clone(), code);

    let base_pos = Position::new(4, 8);
    let refs_from_base = find_references(&index, &file_url, "execute", base_pos, code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "Service.execute references should include interface declaration and annotated override"
    );
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 9));

    let impl_pos = Position::new(9, 17);
    let refs_from_impl = find_references(&index, &file_url, "execute", impl_pos, code, true);

    assert_eq!(
        refs_from_impl.len(),
        2,
        "Annotated override execute references should include override and interface declaration"
    );
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_impl.iter().any(|loc| loc.range.start.line == 9));
}
