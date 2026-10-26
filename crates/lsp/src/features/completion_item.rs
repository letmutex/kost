use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind, SymbolKind, Url,
};

use smol_str::SmolStr;

use crate::indexer::DeclarationItem;
use crate::inference::signature::get_signature_param_types;

pub fn map_symbol_kind(kind: SymbolKind) -> (Option<CompletionItemKind>, bool) {
    match kind {
        SymbolKind::FUNCTION => (Some(CompletionItemKind::METHOD), true),
        SymbolKind::PROPERTY => (Some(CompletionItemKind::PROPERTY), false),
        SymbolKind::ENUM_MEMBER => (Some(CompletionItemKind::ENUM_MEMBER), false),
        SymbolKind::CLASS => (Some(CompletionItemKind::CLASS), false),
        SymbolKind::OBJECT => (Some(CompletionItemKind::MODULE), false),
        SymbolKind::INTERFACE => (Some(CompletionItemKind::INTERFACE), false),
        SymbolKind::TYPE_PARAMETER => (Some(CompletionItemKind::TYPE_PARAMETER), false),
        _ => (Some(CompletionItemKind::VALUE), false),
    }
}

pub fn declaration_to_completion_item(decl: &DeclarationItem) -> CompletionItem {
    let (kind, is_fn) = map_symbol_kind(decl.kind);
    let insert_text = if is_fn {
        let has_params = decl.signature.as_deref().is_none_or(|s| {
            s.find('(')
                .and_then(|o| s.rfind(')').map(|c| o + 1 < c))
                .unwrap_or(true)
        });
        Some(if has_params {
            common::concat_str!(decl.name, "($0)")
        } else {
            common::concat_str!(decl.name, "()")
        })
    } else {
        Some(decl.name.to_string())
    };

    let doc = decl.doc_comment.as_ref().map(|d| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d.to_string(),
        })
    });

    CompletionItem {
        label: decl.name.to_string(),
        kind,
        detail: decl.signature.as_ref().map(|s| s.to_string()),
        documentation: doc,
        insert_text,
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

pub fn declaration_to_callable_reference_item(decl: &DeclarationItem) -> CompletionItem {
    let (kind, _) = map_symbol_kind(decl.kind);
    let doc = decl.doc_comment.as_ref().map(|d| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d.to_string(),
        })
    });

    CompletionItem {
        label: decl.name.to_string(),
        kind,
        detail: decl.signature.as_ref().map(|s| s.to_string()),
        documentation: doc,
        insert_text: Some(decl.name.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        ..Default::default()
    }
}

pub(crate) fn make_callable_reference_item_with_import(
    decl: &DeclarationItem,
    auto_import_pkg: Option<&str>,
    target_url: Option<&Url>,
) -> CompletionItem {
    let mut item = declaration_to_callable_reference_item(decl);
    if let (Some(pkg), Some(url)) = (auto_import_pkg, target_url) {
        let import_data = super::auto_import::AutoImportData::new(
            url.clone(),
            pkg,
            decl.name.as_str(),
            decl.container_name.as_deref(),
        );
        item.data = serde_json::to_value(&import_data).ok();
        item.label_details = Some(CompletionItemLabelDetails {
            detail: None,
            description: Some(pkg.to_string()),
        });
        if item.detail.is_none() {
            item.detail = Some(pkg.to_string());
        }
    }
    item
}

pub(crate) fn make_completion_item_with_import(
    decl: &DeclarationItem,
    auto_import_pkg: Option<&str>,
    target_url: Option<&Url>,
) -> CompletionItem {
    let mut item = declaration_to_completion_item(decl);
    if let (Some(pkg), Some(url)) = (auto_import_pkg, target_url) {
        let import_data = super::auto_import::AutoImportData::new(
            url.clone(),
            pkg,
            decl.name.as_str(),
            decl.container_name.as_deref(),
        );
        item.data = serde_json::to_value(&import_data).ok();
        item.label_details = Some(CompletionItemLabelDetails {
            detail: None,
            description: Some(pkg.to_string()),
        });
        if item.detail.is_none() {
            item.detail = Some(pkg.to_string());
        }
    }
    item
}

pub fn contains_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_lower.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle_lower.len())
        .any(|window| window.eq_ignore_ascii_case(needle_lower.as_bytes()))
}

pub fn starts_with_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_lower.len() > haystack.len() {
        return false;
    }
    haystack.as_bytes()[..needle_lower.len()].eq_ignore_ascii_case(needle_lower.as_bytes())
}

pub fn decl_completion_key(decl: &DeclarationItem) -> String {
    if matches!(
        decl.kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
    ) && let Some(sig) = &decl.signature
    {
        if !sig.contains('\n') {
            return common::concat_str!(decl.name, ":", sig.trim());
        }
        let mut key = String::with_capacity(decl.name.len() + 1 + sig.len());
        key.push_str(&decl.name);
        key.push(':');
        for (i, line) in sig.lines().enumerate() {
            if i > 0 {
                key.push(' ');
            }
            key.push_str(line.trim());
        }
        return key;
    }
    decl.name.to_string()
}

pub struct CandidateContext<'a> {
    pub decl: &'a DeclarationItem,
    pub auto_import_pkg: Option<&'a str>,
    pub target_url: Option<&'a Url>,
    pub limit: usize,
    pub is_callable_ref: bool,
}

impl<'a> CandidateContext<'a> {
    pub fn create_item(&self) -> CompletionItem {
        if self.is_callable_ref {
            make_callable_reference_item_with_import(
                self.decl,
                self.auto_import_pkg,
                self.target_url,
            )
        } else {
            make_completion_item_with_import(self.decl, self.auto_import_pkg, self.target_url)
        }
    }

    pub fn make_key(&self, base: &str) -> String {
        if let Some(pkg) = self.auto_import_pkg {
            common::concat_str!(base, ":", pkg)
        } else {
            base.to_string()
        }
    }
}

pub struct TrackedCallable {
    pub item_index: usize,
    pub signature: Option<SmolStr>,
    pub param_types: Option<Vec<String>>,
}

impl TrackedCallable {
    pub fn param_types(&mut self) -> &[String] {
        self.param_types
            .get_or_insert_with(|| get_signature_param_types(self.signature.as_deref()))
    }
}

pub use super::completion_candidate::CompletionCandidateTracker;
