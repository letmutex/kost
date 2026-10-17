use ahash::AHashMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smol_str::SmolStr;
use std::sync::Arc;
use url::Url;

use super::container::index_container_variants;
use super::types::{DeclarationItem, DeclarationRef, FileSymbols};
use tower_lsp::lsp_types::SymbolKind;

fn is_type_alias_private(declarations: &[DeclarationItem], alias_name: &str) -> bool {
    declarations.iter().any(|d| {
        d.name.as_str() == alias_name && d.kind == SymbolKind::TYPE_PARAMETER && d.is_private
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SegmentKey(pub SmolStr);

impl SegmentKey {
    pub const WORKSPACE: &'static str = "workspace";
    pub const FAST_PATH: &'static str = "fast_path";
    pub const DIRECT_DEPS: &'static str = "direct_deps";
    pub const TRANSITIVE: &'static str = "transitive";

    pub fn fast_path() -> Self {
        Self(SmolStr::new_inline(Self::FAST_PATH))
    }

    pub fn direct_deps() -> Self {
        Self(SmolStr::new_inline(Self::DIRECT_DEPS))
    }

    pub fn transitive() -> Self {
        Self(SmolStr::new_inline(Self::TRANSITIVE))
    }

    pub fn artifact(coord: &str) -> Self {
        Self(SmolStr::new(coord))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for SegmentKey {
    fn default() -> Self {
        Self(SmolStr::new_inline("default"))
    }
}

impl<T: AsRef<str>> From<T> for SegmentKey {
    fn from(s: T) -> Self {
        Self(SmolStr::new(s.as_ref()))
    }
}

impl std::fmt::Display for SegmentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Default)]
pub struct IndexSegment {
    pub key: SegmentKey,
    pub files: AHashMap<Url, FileSymbols>,
    pub name_to_decls: AHashMap<SmolStr, Vec<DeclarationRef>>,
    pub container_to_decls: AHashMap<SmolStr, Vec<DeclarationRef>>,
    pub top_level_decls: Vec<DeclarationRef>,
    pub class_supertypes: AHashMap<String, Vec<String>>,
    pub type_aliases: AHashMap<String, String>,
}

impl IndexSegment {
    pub fn new(key: SegmentKey) -> Self {
        Self {
            key,
            files: AHashMap::default(),
            name_to_decls: AHashMap::default(),
            container_to_decls: AHashMap::default(),
            top_level_decls: Vec::new(),
            class_supertypes: AHashMap::default(),
            type_aliases: AHashMap::default(),
        }
    }

    pub fn insert_file_symbols(&mut self, url: Url, file_symbols: FileSymbols) {
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

    pub fn merge(&mut self, other: IndexSegment) {
        self.files.extend(other.files);
        for (k, mut v) in other.name_to_decls {
            self.name_to_decls.entry(k).or_default().append(&mut v);
        }
        for (k, mut v) in other.container_to_decls {
            self.container_to_decls.entry(k).or_default().append(&mut v);
        }
        self.top_level_decls.extend(other.top_level_decls);
        self.class_supertypes.extend(other.class_supertypes);
        self.type_aliases.extend(other.type_aliases);
    }

    /// Parallel Rayon map-reduce construction across all CPU cores.
    pub fn from_symbols_parallel(key: SegmentKey, symbols: Vec<(Url, FileSymbols)>) -> Self {
        if symbols.is_empty() {
            return Self::new(key);
        }

        let mut segment = symbols
            .into_par_iter()
            .fold(
                || IndexSegment::new(key.clone()),
                |mut acc, (url, fs)| {
                    acc.insert_file_symbols(url, fs);
                    acc
                },
            )
            .reduce(
                || IndexSegment::new(key.clone()),
                |mut a, b| {
                    a.merge(b);
                    a
                },
            );

        segment.key = key;
        segment
    }
}
