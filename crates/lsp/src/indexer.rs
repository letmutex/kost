mod container;
mod scanner;
pub mod segment;
mod types;
mod walker;

use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;
use std::path::Path;
use std::sync::Arc;
use tower_lsp::lsp_types::SymbolKind;
use url::Url;

use container::{index_container_variants, remove_container_variants};

pub use segment::{IndexSegment, SegmentKey};
pub use types::{
    DeclarationItem, DeclarationMetadata, DeclarationRef, FileSymbols, ImportItem, is_source_url,
};

use crate::parser::KotlinParser;
use crate::symbols::parse_file_symbols;

pub struct WorkspaceIndex {
    pub parser: KotlinParser,
    pub files: AHashMap<Url, FileSymbols>,
    pub sources: AHashMap<Url, String>,
    pub name_to_decls: AHashMap<SmolStr, Vec<DeclarationRef>>,
    pub container_to_decls: AHashMap<SmolStr, Vec<DeclarationRef>>,
    pub top_level_decls: Vec<DeclarationRef>,
    pub class_supertypes: AHashMap<String, Vec<String>>,
    pub type_aliases: AHashMap<String, String>,
    pub segments: AHashMap<SegmentKey, Vec<Url>>,
}

impl Default for WorkspaceIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceIndex {
    pub fn new() -> Self {
        Self {
            parser: KotlinParser::new(),
            files: AHashMap::default(),
            sources: AHashMap::default(),
            name_to_decls: AHashMap::default(),
            container_to_decls: AHashMap::default(),
            top_level_decls: Vec::new(),
            class_supertypes: AHashMap::default(),
            type_aliases: AHashMap::default(),
            segments: AHashMap::default(),
        }
    }

    pub fn add_segment(&mut self, segment: IndexSegment) {
        if self.segments.contains_key(&segment.key) {
            self.invalidate_segment(&segment.key);
        }

        for url in segment.files.keys() {
            if self.files.contains_key(url) {
                self.remove_file(url);
            }
        }

        let key = segment.key;
        let file_urls: Vec<Url> = segment.files.keys().cloned().collect();

        self.files.extend(segment.files);
        for (k, mut v) in segment.name_to_decls {
            self.name_to_decls.entry(k).or_default().append(&mut v);
        }
        for (k, mut v) in segment.container_to_decls {
            self.container_to_decls.entry(k).or_default().append(&mut v);
        }
        self.top_level_decls.extend(segment.top_level_decls);
        self.class_supertypes.extend(segment.class_supertypes);
        self.type_aliases.extend(segment.type_aliases);

        self.segments.insert(key, file_urls);
    }

    pub fn invalidate_segment(&mut self, key: &SegmentKey) -> bool {
        if let Some(urls) = self.segments.remove(key) {
            for url in &urls {
                self.remove_file(url);
            }
            true
        } else {
            false
        }
    }

    pub fn has_segment(&self, key: &SegmentKey) -> bool {
        self.segments.contains_key(key)
    }

    pub fn insert_batch(&mut self, batch: Vec<(Url, FileSymbols)>) {
        self.files.reserve(batch.len());
        self.top_level_decls.reserve(batch.len() * 2);
        for (url, file_symbols) in batch {
            self.insert_file_symbols(url, file_symbols);
        }
    }

    pub fn insert_file_symbols(&mut self, url: Url, file_symbols: FileSymbols) {
        if !self.files.is_empty() && self.files.contains_key(&url) {
            self.remove_file(&url);
        }

        let shared_url = Arc::new(url.clone());
        let pkg = file_symbols.package_name.as_deref();
        for decl in &file_symbols.declarations {
            let shared_decl = Arc::new(decl.clone());
            self.name_to_decls
                .entry(decl.name.clone())
                .or_default()
                .push((Arc::clone(&shared_url), Arc::clone(&shared_decl)));

            if let Some(ref container) = decl.container_name {
                index_container_variants(
                    &mut self.container_to_decls,
                    container,
                    pkg,
                    &shared_url,
                    &shared_decl,
                );
            } else if !decl.is_private {
                self.top_level_decls
                    .push((Arc::clone(&shared_url), Arc::clone(&shared_decl)));
            }
        }

        for (k, v) in &file_symbols.class_supertypes {
            self.class_supertypes.insert(k.clone(), v.clone());
        }
        for (k, v) in &file_symbols.type_aliases {
            if !is_type_alias_private(&file_symbols.declarations, k) {
                self.type_aliases.insert(k.clone(), v.clone());
            }
        }

        self.files.insert(url, file_symbols);
    }

    pub fn index_source(&mut self, url: Url, source: &str) {
        if let Some(symbols) = parse_file_symbols(&mut self.parser, source) {
            self.insert_file_symbols(url.clone(), symbols);
        }
        self.sources.insert(url, source.to_string());
    }

    pub fn remove_file(&mut self, url: &Url) {
        self.sources.remove(url);
        if let Some(fs) = self.files.remove(url) {
            let pkg = fs.package_name.as_deref();
            for k in fs.class_supertypes.keys() {
                self.class_supertypes.remove(k);
            }
            for k in fs.type_aliases.keys() {
                self.type_aliases.remove(k);
            }
            for decl in &fs.declarations {
                if let Some(list) = self.name_to_decls.get_mut(&decl.name) {
                    list.retain(|(u, _)| u.as_ref() != url);
                    if list.is_empty() {
                        self.name_to_decls.remove(&decl.name);
                    }
                }
                if let Some(ref container) = decl.container_name {
                    remove_container_variants(&mut self.container_to_decls, container, pkg, url);
                }
            }
            self.top_level_decls.retain(|(u, _)| u.as_ref() != url);
        }
    }

    pub fn get_file_source(&self, url: &Url) -> Option<String> {
        if let Some(src) = self.sources.get(url) {
            return Some(src.clone());
        }
        url.to_file_path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .or_else(|| crate::util::archive::read_jar_url_content(url.as_str()))
    }

    pub fn is_subclass_of(&self, sub_class: &str, target_super: &str) -> bool {
        let clean_start = sub_class
            .split(['<', '?'])
            .next()
            .unwrap_or(sub_class)
            .trim();
        let clean_target = target_super
            .split(['<', '?'])
            .next()
            .unwrap_or(target_super)
            .trim();
        if crate::syntax::hierarchy::containers_match(clean_start, clean_target) {
            return true;
        }

        let mut queue = vec![clean_start];
        let mut visited = AHashSet::new();
        visited.insert(clean_start);

        while let Some(curr) = queue.pop() {
            let supers_opt = self.class_supertypes.get(curr).or_else(|| {
                curr.rsplit('.')
                    .next()
                    .filter(|&short| short != curr)
                    .and_then(|short| self.class_supertypes.get(short))
            });
            if let Some(supers) = supers_opt {
                for s in supers {
                    let clean_s = s.split(['<', '?']).next().unwrap_or(s).trim();
                    if crate::syntax::hierarchy::containers_match(clean_s, clean_target) {
                        return true;
                    }
                    if visited.insert(clean_s) {
                        queue.push(clean_s);
                    }
                }
            }
        }
        false
    }

    pub fn get_all_supertypes(&self, class_name: &str) -> Vec<String> {
        let clean_start = class_name.split('<').next().unwrap_or(class_name).trim();
        let mut results = Vec::new();
        let mut queue = vec![clean_start.to_string()];
        let mut visited = std::collections::HashSet::new();
        visited.insert(clean_start.to_string());

        while let Some(curr) = queue.pop() {
            let clean_curr = curr.split('<').next().unwrap_or(&curr).trim();
            let supers_opt = self.class_supertypes.get(clean_curr).or_else(|| {
                clean_curr
                    .rsplit('.')
                    .next()
                    .filter(|&short| short != clean_curr)
                    .and_then(|short| self.class_supertypes.get(short))
            });
            if let Some(supers) = supers_opt {
                for s in supers {
                    let clean_s = s.split('<').next().unwrap_or(s).trim();
                    if visited.insert(clean_s.to_string()) {
                        results.push(s.clone());
                        queue.push(s.clone());
                    }
                }
            }
        }
        results
    }

    pub fn scan_directory(&mut self, root: &Path) {
        scanner::scan_directory(self, root);
    }
}

pub(crate) fn is_type_alias_private(declarations: &[DeclarationItem], alias_name: &str) -> bool {
    declarations.iter().any(|d| {
        d.name.as_str() == alias_name && d.kind == SymbolKind::TYPE_PARAMETER && d.is_private
    })
}
