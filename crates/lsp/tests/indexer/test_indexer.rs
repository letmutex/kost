use lsp::features::{find_completions, find_definition};
use lsp::indexer::{
    DeclarationItem, DeclarationMetadata, FileSymbols, IndexSegment, SegmentKey, WorkspaceIndex,
};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Range, SymbolKind, Url};

#[test]
fn test_cross_file_navigation() {
    let mut index = WorkspaceIndex::new();
    let file1_url = Url::parse("file:///project/Model.kt").unwrap();
    let file1_code = r#"
package com.example.model

class User(val id: String)
"#;
    let file2_url = Url::parse("file:///project/Repo.kt").unwrap();
    let file2_code = r#"
package com.example.repo

import com.example.model.User

class UserRepo {
    fun get(): User = User("1")
}
"#;
    index.index_source(file1_url.clone(), file1_code);
    index.index_source(file2_url.clone(), file2_code);

    // Navigate to User from Repo.kt
    let def = find_definition(&index, &file2_url, "User", Position::new(6, 16), file2_code);
    assert!(def.is_some());
    assert_eq!(def.unwrap().uri, file1_url);
}

#[test]
fn test_goto_definition_lands_on_symbol_line_not_annotation_line() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/AnnotatedService.kt").unwrap();
    let code = r#"
package com.example

@OptIn(ExperimentalCoroutinesApi::class)
@Serializable
@Deprecated("Use new service")
class AnnotatedService(
    @SerializedName("key")
    val apiKey: String
) {
    @OptIn(ExperimentalCoroutinesApi::class)
    @Composable
    suspend fun executeTask() {
    }
}

fun main() {
    val service = AnnotatedService("test")
    service.executeTask()
}
"#;
    index.index_source(file_url.clone(), code);

    // 1. Go to definition of AnnotatedService:
    let def_class = find_definition(
        &index,
        &file_url,
        "AnnotatedService",
        Position::new(17, 20),
        code,
    );
    assert!(def_class.is_some());
    let loc_class = def_class.unwrap();
    assert_eq!(loc_class.range.start.line, 6); // Line 6 contains "class AnnotatedService", not line 3 "@OptIn"

    // 2. Go to definition of executeTask:
    let def_fn = find_definition(
        &index,
        &file_url,
        "executeTask",
        Position::new(18, 14),
        code,
    );
    assert!(def_fn.is_some());
    let loc_fn = def_fn.unwrap();
    assert_eq!(loc_fn.range.start.line, 12); // Line 12 contains "suspend fun executeTask", not line 10 "@OptIn"
}

#[test]
fn test_import_alias_definition_and_resolution() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/ModelClient.kt").unwrap();
    let model_code = r#"
package com.example.model

class Client(val clientId: String) {
    fun connect(): Boolean = true
}
"#;
    let service_url = Url::parse("file:///project/ClientService.kt").unwrap();
    let service_code = r#"
package com.example.service

import com.example.model.Client as AppClient

fun test(client: AppClient) {
    client.
    val id = client.clientId
}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(service_url.clone(), service_code);

    // 1. Go to definition on AppClient type reference at line 5, col 19:
    let pos_type = Position::new(5, 19);
    let def_type = find_definition(&index, &service_url, "AppClient", pos_type, service_code);
    assert!(
        def_type.is_some(),
        "AppClient type reference should resolve to definition"
    );
    assert_eq!(
        def_type.unwrap().uri,
        model_url,
        "AppClient definition should point to ModelClient.kt"
    );

    // 2. Member completion on client. (typed as AppClient):
    let pos_dot = Position::new(6, 11);
    let comps = find_completions(&index, &service_url, pos_dot, service_code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"clientId".to_string()),
        "client. on AppClient should offer property 'clientId'"
    );
    assert!(
        labels.contains(&"connect".to_string()),
        "client. on AppClient should offer method 'connect'"
    );
}

#[test]
fn test_explicit_import_disambiguation_with_same_class_name() {
    let mut index = WorkspaceIndex::new();
    let internal_url = Url::parse("file:///project/internal/User.kt").unwrap();
    let internal_code = r#"
package com.example.internal

class User(val internalToken: String)
"#;
    let api_url = Url::parse("file:///project/api/User.kt").unwrap();
    let api_code = r#"
package com.example.api

class User(val apiToken: String)
"#;
    let main_url = Url::parse("file:///project/main/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.api.User

fun test(u: User) {
    val t = u.apiToken
}
"#;
    // Index internal.User first, then api.User:
    index.index_source(internal_url, internal_code);
    index.index_source(api_url.clone(), api_code);
    index.index_source(main_url.clone(), main_code);

    // Go to definition of User at line 5, col 14 in Main.kt:
    let pos_user = Position::new(5, 14);
    let def = find_definition(&index, &main_url, "User", pos_user, main_code);
    assert!(def.is_some(), "User should resolve definition");
    assert_eq!(
        def.unwrap().uri,
        api_url,
        "User should resolve to com.example.api.User, not com.example.internal.User"
    );
}

#[test]
fn test_segment_add_and_invalidate_by_key() {
    let mut index = WorkspaceIndex::new();

    // 1. Create a segment for a library
    let lib_url = Url::parse("jar:file:///libs/retrofit-2.9.0.jar!/Retrofit.class").unwrap();
    let mut lib_symbols = FileSymbols::default();
    lib_symbols.declarations.push(DeclarationItem::new(
        "Retrofit",
        SymbolKind::CLASS,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature("class Retrofit"),
    ));
    lib_symbols.declarations.push(DeclarationItem::new(
        "create",
        SymbolKind::METHOD,
        Range::default(),
        Range::default(),
        DeclarationMetadata::new(
            None,
            Some(SmolStr::new("fun create(): Any")),
            Some(SmolStr::new("Retrofit")),
            false,
        ),
    ));

    let seg_key = SegmentKey::artifact("com.squareup.retrofit2:retrofit:2.9.0");
    let mut segment = IndexSegment::new(seg_key.clone());
    segment.insert_file_symbols(lib_url.clone(), lib_symbols);

    index.add_segment(segment);

    assert!(index.has_segment(&seg_key));
    assert!(index.name_to_decls.contains_key("Retrofit"));
    assert!(index.container_to_decls.contains_key("Retrofit"));
    assert_eq!(index.files.len(), 1);

    // 2. Invalidate the segment by key
    let invalidated = index.invalidate_segment(&seg_key);
    assert!(invalidated);
    assert!(!index.has_segment(&seg_key));
    assert!(!index.name_to_decls.contains_key("Retrofit"));
    assert!(!index.container_to_decls.contains_key("Retrofit"));
    assert_eq!(index.files.len(), 0);
}

#[test]
fn test_segment_parallel_construction_and_query() {
    let mut symbols = Vec::new();
    for i in 0..50 {
        let url = Url::parse(&format!("jar:file:///libs/lib.jar!/Class{i}.class")).unwrap();
        let mut fs = FileSymbols::default();
        fs.declarations.push(DeclarationItem::new(
            format!("Class{i}"),
            SymbolKind::CLASS,
            Range::default(),
            Range::default(),
            DeclarationMetadata::signature(format!("class Class{i}")),
        ));
        symbols.push((url, fs));
    }

    let key = SegmentKey::from("parallel_stage");
    let segment = IndexSegment::from_symbols_parallel(key.clone(), symbols);

    let mut index = WorkspaceIndex::new();
    index.add_segment(segment);

    assert!(index.has_segment(&key));
    assert!(index.name_to_decls.contains_key("Class0"));
    assert!(index.name_to_decls.contains_key("Class49"));
    assert_eq!(index.files.len(), 50);
}

#[test]
fn test_is_subclass_of_matches_nullable_class_type() {
    let mut index = WorkspaceIndex::new();
    index
        .class_supertypes
        .insert("Child".to_string(), vec!["Parent".to_string()]);
    assert!(index.is_subclass_of("Child?", "Parent"));
    assert!(index.is_subclass_of("Child", "Parent?"));
    assert!(index.is_subclass_of("Child?", "Parent?"));
}
