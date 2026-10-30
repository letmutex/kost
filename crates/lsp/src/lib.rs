#![deny(clippy::wildcard_imports)]
#![deny(clippy::too_many_arguments)]

pub mod config;
pub mod features;
pub mod gradle;
pub mod indexer;
pub mod inference;
pub mod logger;
pub mod progress;
pub mod server;
pub mod server_doc;
pub mod server_init;
pub mod syntax;
pub mod util;

pub use config::{IndexingLogLevel, InteractionLogLevel, ServerConfig, VariableHoverTypeInference};
pub use features::{
    OccurrenceTiming, extract_document_symbols, find_completions, find_completions_with_root,
    find_definition, find_definition_with_root, find_hover, find_hover_with_config,
    find_hover_with_root, find_references, find_references_profiled, find_references_with_root,
    find_signature_help, search_workspace,
};
pub use gradle::{GradleConfig, IndexingSummary, MultiStageIndexer};
pub use indexer::WorkspaceIndex;
pub use inference::{
    KotlinType, PrimitiveType, SignatureInfo, TypeArg, Variance, parse_type_signature,
    parse_type_signature_with_params,
};
pub use progress::ProgressReporter;
pub use server::Backend;
pub use syntax::symbol_item::{DeclarationItem, FileSymbols, ImportItem};

pub async fn run_stdio_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = tower_lsp::LspService::new(Backend::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}

pub(crate) use syntax::body_masker;
pub(crate) use syntax::doc;
pub(crate) use syntax::parser;
pub(crate) use syntax::position;
pub(crate) use syntax::signature;
pub(crate) use syntax::symbols;
