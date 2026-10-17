use std::sync::Arc;
use url::Url;

pub use crate::syntax::symbol_item::{
    DeclarationItem, DeclarationMetadata, FileSymbols, ImportItem,
};

pub type DeclarationRef = (Arc<Url>, Arc<DeclarationItem>);

/// Check if a URL represents a Kotlin source file (.kt / .kts) rather than compiled bytecode (.class)
#[inline]
pub fn is_source_url(url: &Url) -> bool {
    !url.as_str().ends_with(".class")
}
