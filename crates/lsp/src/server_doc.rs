use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::Url;

#[derive(Clone, Default)]
pub struct ServerDocuments {
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl ServerDocuments {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_source(&self, url: &Url) -> Option<String> {
        if let Some(text) = self.documents.read().await.get(url) {
            return Some(text.clone());
        }
        if let Ok(path) = url.to_file_path() {
            std::fs::read_to_string(path).ok()
        } else {
            None
        }
    }

    pub async fn insert(&self, uri: Url, text: String) {
        self.documents.write().await.insert(uri, text);
    }

    pub async fn remove_document(&self, uri: &Url) {
        self.documents.write().await.remove(uri);
    }
}
