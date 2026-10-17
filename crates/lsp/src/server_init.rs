use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::Client;
use tower_lsp::lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeParams, InitializeResult, MessageType,
    OneOf, ServerCapabilities, ServerInfo, SignatureHelpOptions, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, WorkspaceFolder,
};

use crate::config::{IndexingLogLevel, ServerConfig};
use crate::indexer::WorkspaceIndex;
use crate::progress::ProgressReporter;

pub fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: Some(vec![".".to_string(), "?.".to_string(), ":".to_string()]),
            ..Default::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string(), "<".to_string()]),
            retrigger_characters: Some(vec![",".to_string()]),
            work_done_progress_options: Default::default(),
        }),
        ..Default::default()
    }
}

pub fn extract_workspace_paths(
    folders: Option<Vec<WorkspaceFolder>>,
    root_uri: Option<Url>,
) -> Vec<PathBuf> {
    if let Some(folders) = folders {
        folders
            .into_iter()
            .filter_map(|folder| folder.uri.to_file_path().ok())
            .collect()
    } else if let Some(root_uri) = root_uri {
        root_uri.to_file_path().ok().into_iter().collect()
    } else {
        Vec::new()
    }
}

pub fn spawn_workspace_indexer(
    workspace_paths: Vec<PathBuf>,
    index: Arc<RwLock<WorkspaceIndex>>,
    indexing_log: IndexingLogLevel,
    client: Client,
    progress_supported: bool,
) {
    if workspace_paths.is_empty() {
        return;
    }
    tokio::spawn(async move {
        for path in workspace_paths {
            let path_clone = path.clone();
            let local_symbols = tokio::task::spawn_blocking(move || {
                let mut local_idx = WorkspaceIndex::new();
                local_idx.scan_directory(&path_clone);
                local_idx.files
            })
            .await
            .unwrap_or_default();

            {
                let mut idx = index.write().await;
                for (url, file_symbols) in local_symbols {
                    idx.insert_file_symbols(url, file_symbols);
                }
            }

            let progress = if progress_supported {
                ProgressReporter::new(client.clone())
            } else {
                ProgressReporter::noop()
            };
            let handle = crate::gradle::MultiStageIndexer::spawn_with_progress(
                path,
                crate::gradle::GradleConfig::new().with_indexing_logging(indexing_log),
                Arc::clone(&index),
                progress,
            );
            let _ = handle.await;
        }
    });
}

pub async fn handle_initialize(
    config: &RwLock<ServerConfig>,
    workspace_paths: &RwLock<Vec<PathBuf>>,
    params: InitializeParams,
) -> InitializeResult {
    let mut parsed_config = ServerConfig::from_init_options(params.initialization_options.as_ref());
    parsed_config.work_done_progress_supported = params
        .capabilities
        .window
        .as_ref()
        .and_then(|window| window.work_done_progress)
        .unwrap_or(false);
    *config.write().await = parsed_config;

    let paths = extract_workspace_paths(params.workspace_folders, params.root_uri);
    *workspace_paths.write().await = paths;

    InitializeResult {
        server_info: Some(ServerInfo {
            name: "kost".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        capabilities: build_server_capabilities(),
    }
}

pub async fn handle_initialized(
    client: &Client,
    index: &Arc<RwLock<WorkspaceIndex>>,
    config: &RwLock<ServerConfig>,
    workspace_paths: &RwLock<Vec<PathBuf>>,
) {
    client
        .log_message(
            MessageType::INFO,
            "Kotlin Static LSP (Native/Tree-sitter) initialized",
        )
        .await;

    let paths = std::mem::take(&mut *workspace_paths.write().await);
    if !paths.is_empty() {
        let server_cfg = *config.read().await;
        spawn_workspace_indexer(
            paths,
            Arc::clone(index),
            server_cfg.indexing_logging,
            client.clone(),
            server_cfg.work_done_progress_supported,
        );
    }
}
