use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionList, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
    InitializedParams, Location, ReferenceParams, SignatureHelp, SignatureHelpParams,
    SymbolInformation, Url, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

use crate::config::ServerConfig;
use crate::features::extract_document_symbols;
use crate::indexer::WorkspaceIndex;
use crate::parser::{extract_identifier_text, find_node_at_position, parse_source};
use crate::server_doc::ServerDocuments;
use crate::server_init::{handle_initialize, handle_initialized};

pub struct Backend {
    client: Client,
    index: Arc<RwLock<WorkspaceIndex>>,
    documents: ServerDocuments,
    config: Arc<RwLock<ServerConfig>>,
    workspace_paths: RwLock<Vec<PathBuf>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            index: Arc::new(RwLock::new(WorkspaceIndex::new())),
            documents: ServerDocuments::new(),
            config: Arc::new(RwLock::new(ServerConfig::default())),
            workspace_paths: RwLock::new(Vec::new()),
        }
    }

    async fn get_source(&self, url: &Url) -> Option<String> {
        self.documents.get_source(url).await
    }

    async fn update_document(&self, uri: Url, text: String) {
        self.documents.insert(uri.clone(), text.clone()).await;
        self.index.write().await.index_source(uri, &text);
    }

    async fn parse_node_and_identifier(
        &self,
        uri: &Url,
        pos: tower_lsp::lsp_types::Position,
    ) -> Option<(String, tree_sitter::Tree, String)> {
        let source = self.get_source(uri).await?;
        let tree = parse_source(&source)?;
        let node = find_node_at_position(tree.root_node(), pos)?;
        let symbol_name = extract_identifier_text(&node, &source)?.to_string();
        Some((source, tree, symbol_name))
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        Ok(handle_initialize(&self.config, &self.workspace_paths, params).await)
    }

    async fn initialized(&self, _: InitializedParams) {
        handle_initialized(
            &self.client,
            &self.index,
            &self.config,
            &self.workspace_paths,
        )
        .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.update_document(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.update_document(params.text_document.uri, change.text)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.update_document(uri, text).await;
        } else if let Some(text) = self.get_source(&uri).await {
            self.index.write().await.index_source(uri, &text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .remove_document(&params.text_document.uri)
            .await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let start = std::time::Instant::now();
        let uri = params.text_document.uri;
        let Some(source) = self.get_source(&uri).await else {
            return Ok(None);
        };

        let Some(tree) = parse_source(&source) else {
            return Ok(None);
        };

        let symbols = extract_document_symbols(tree.root_node(), &source);
        if self.config.read().await.interaction_logging.is_enabled() {
            crate::logger::log!(
                "interaction",
                "document_symbol: found {} symbols in {:?}",
                symbols.len(),
                start.elapsed()
            );
        }
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let start = std::time::Instant::now();
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some((source, tree, symbol_name)) = self.parse_node_and_identifier(&uri, pos).await
        else {
            return Ok(None);
        };
        let root = tree.root_node();

        let index = self.index.read().await;
        let loc = crate::features::find_definition_with_root(
            &index,
            &uri,
            &symbol_name,
            pos,
            &source,
            Some(root),
        );

        if self.config.read().await.interaction_logging.is_enabled() {
            crate::logger::log!(
                "interaction",
                "goto_definition: '{}' at {}:{} resolved={} in {:?}",
                symbol_name,
                pos.line,
                pos.character,
                loc.is_some(),
                start.elapsed()
            );
        }

        Ok(loc.map(GotoDefinitionResponse::Scalar))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let start = std::time::Instant::now();
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some((source, tree, symbol_name)) = self.parse_node_and_identifier(&uri, pos).await
        else {
            return Ok(None);
        };
        let root = tree.root_node();

        let cfg = self.config.read().await;
        let type_infer_opt = cfg.variable_hover_type_inference;
        let log_enabled = cfg.interaction_logging.is_enabled();
        drop(cfg);

        let index = self.index.read().await;
        let hover = crate::features::find_hover_with_config(
            &index,
            &uri,
            &symbol_name,
            pos,
            &source,
            Some(root),
            type_infer_opt,
        );

        if log_enabled {
            crate::logger::log!(
                "interaction",
                "hover: '{}' at {}:{} resolved={} in {:?}",
                symbol_name,
                pos.line,
                pos.character,
                hover.is_some(),
                start.elapsed()
            );
        }

        Ok(hover)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let start = std::time::Instant::now();
        let index = self.index.read().await;
        let legacy_symbols =
            crate::features::search_workspace_symbol_information(&index, &params.query);

        if self.config.read().await.interaction_logging.is_enabled() {
            crate::logger::log!(
                "interaction",
                "workspace_symbol: query='{}' found {} symbols in {:?}",
                params.query,
                legacy_symbols.len(),
                start.elapsed()
            );
        }

        Ok(Some(legacy_symbols))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let start = std::time::Instant::now();
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let Some(source) = self.get_source(&uri).await else {
            return Ok(None);
        };

        let tree = parse_source(&source);
        let root = tree.as_ref().map(|t| t.root_node());

        let cfg = self.config.read().await;
        let auto_import = cfg.auto_import;
        let log_enabled = cfg.interaction_logging.is_enabled();
        drop(cfg);

        let index = self.index.read().await;
        let items = crate::features::find_completions_with_config(
            &index,
            &uri,
            pos,
            &source,
            root,
            auto_import,
        );
        let is_incomplete = items.len() >= 1000;

        if log_enabled {
            crate::logger::log!(
                "interaction",
                "completion: returned {} items (incomplete={}) at {}:{} in {:?}",
                items.len(),
                is_incomplete,
                pos.line,
                pos.character,
                start.elapsed()
            );
        }

        Ok(Some(CompletionResponse::List(CompletionList {
            is_incomplete,
            items,
        })))
    }

    async fn completion_resolve(&self, params: CompletionItem) -> Result<CompletionItem> {
        let is_auto_import = self.config.read().await.auto_import;
        if is_auto_import
            && let Some(data) = crate::features::extract_auto_import_data(&params)
            && let Some(source) = self.get_source(&data.target_uri).await
        {
            return Ok(crate::features::resolve_completion_item(
                params, &source, &data,
            ));
        }
        Ok(params)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(source) = self.get_source(&uri).await else {
            return Ok(None);
        };
        let index = self.index.read().await;
        let result = crate::features::find_signature_help(&index, &uri, pos, &source);
        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let start = std::time::Instant::now();
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;

        let Some((source, tree, symbol_name)) = self.parse_node_and_identifier(&uri, pos).await
        else {
            return Ok(None);
        };
        let root = tree.root_node();

        let log_level = self.config.read().await.interaction_logging;
        let index = self.index.read().await;
        let (locations, slowest_occurrences) = crate::features::find_references_profiled(
            &index,
            &uri,
            &symbol_name,
            pos,
            &source,
            Some(root),
            include_decl,
            log_level.is_verbose(),
        );
        drop(index);

        if log_level.is_enabled() {
            crate::features::log_references_interaction(
                &symbol_name,
                pos,
                locations.len(),
                start.elapsed(),
                &slowest_occurrences,
                log_level.is_verbose(),
            );
        }

        Ok(Some(locations))
    }
}
