use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_cross_file_override_hierarchy() {
    let mut index = WorkspaceIndex::new();
    let base_url = Url::parse("file:///project/Worker.kt").unwrap();
    let base_code = r#"
package com.example

interface Worker {
    fun execute()
}
"#;
    index.index_source(base_url.clone(), base_code);

    let impl_url = Url::parse("file:///project/WorkerImpl.kt").unwrap();
    let impl_code = r#"
package com.example

class WorkerImpl : Worker {
    override fun execute() {}
}
"#;
    index.index_source(impl_url.clone(), impl_code);

    // 1. Finding references on Worker.execute in Worker.kt (line 4, col 8)
    let base_pos = Position::new(4, 8);
    let refs_from_base = find_references(&index, &base_url, "execute", base_pos, base_code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "Worker.execute should find declaration in Worker.kt and override in WorkerImpl.kt"
    );
    assert!(refs_from_base.iter().any(|loc| loc.uri == base_url));
    assert!(refs_from_base.iter().any(|loc| loc.uri == impl_url));

    // 2. Finding references on WorkerImpl.execute in WorkerImpl.kt (line 4, col 17)
    let impl_pos = Position::new(4, 17);
    let refs_from_impl = find_references(&index, &impl_url, "execute", impl_pos, impl_code, true);

    assert_eq!(
        refs_from_impl.len(),
        2,
        "WorkerImpl.execute should find override in WorkerImpl.kt and interface declaration in Worker.kt"
    );
    assert!(refs_from_impl.iter().any(|loc| loc.uri == base_url));
    assert!(refs_from_impl.iter().any(|loc| loc.uri == impl_url));
}
