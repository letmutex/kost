use ahash::AHashMap;
use std::collections::HashSet;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Url};

use smol_str::SmolStr;

use crate::indexer::DeclarationItem;
use crate::inference::signature::{are_param_types_compatible, get_signature_param_types};

use super::completion_item::{CandidateContext, TrackedCallable, decl_completion_key};

#[derive(Default)]
pub struct CompletionCandidateTracker {
    pub items: Vec<CompletionItem>,
    pub is_source: Vec<bool>,
    bytecode_name_counts: AHashMap<SmolStr, usize>,
    source_names: HashSet<String>,
    source_callables: AHashMap<String, Vec<TrackedCallable>>,
    bytecode_callables: AHashMap<String, Vec<TrackedCallable>>,
    bytecode_non_callables: AHashMap<String, Vec<usize>>,
}

impl CompletionCandidateTracker {
    pub fn record(
        &mut self,
        decl: &DeclarationItem,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
        limit: usize,
    ) {
        self.record_with_import(decl, is_source, seen_keys, limit, None, None);
    }

    fn record_candidate_internal(
        &mut self,
        decl: &DeclarationItem,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
        limit: usize,
        auto_import_pkg: Option<&str>,
        target_url: Option<&Url>,
        is_callable_ref: bool,
    ) {
        let ctx = CandidateContext {
            decl,
            auto_import_pkg,
            target_url,
            limit,
            is_callable_ref,
        };
        if matches!(
            decl.kind,
            tower_lsp::lsp_types::SymbolKind::FUNCTION
                | tower_lsp::lsp_types::SymbolKind::METHOD
                | tower_lsp::lsp_types::SymbolKind::CONSTRUCTOR
        ) {
            self.record_callable(&ctx, is_source, seen_keys);
        } else {
            self.record_non_callable(&ctx, is_source, seen_keys);
        }
    }

    pub fn record_with_import(
        &mut self,
        decl: &DeclarationItem,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
        limit: usize,
        auto_import_pkg: Option<&str>,
        target_url: Option<&Url>,
    ) {
        self.record_candidate_internal(
            decl,
            is_source,
            seen_keys,
            limit,
            auto_import_pkg,
            target_url,
            false,
        );
    }

    pub fn record_callable_reference(
        &mut self,
        decl: &DeclarationItem,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
        limit: usize,
    ) {
        self.record_callable_ref_with_import(decl, is_source, seen_keys, limit, None, None);
    }

    pub fn record_callable_ref_with_import(
        &mut self,
        decl: &DeclarationItem,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
        limit: usize,
        auto_import_pkg: Option<&str>,
        target_url: Option<&Url>,
    ) {
        self.record_candidate_internal(
            decl,
            is_source,
            seen_keys,
            limit,
            auto_import_pkg,
            target_url,
            true,
        );
    }

    pub fn push_module(&mut self, label: String) {
        self.items.push(CompletionItem {
            label,
            kind: Some(CompletionItemKind::MODULE),
            ..Default::default()
        });
        self.is_source.push(false);
    }

    pub fn sort_by_label(&mut self) {
        super::completion_sort::sort_by_label(&mut self.items, &mut self.is_source);
    }

    pub fn sort_members(&mut self) {
        super::completion_sort::sort_members(&mut self.items, &mut self.is_source);
    }

    pub fn drain_into(
        mut self,
        mut sub_tracker: Self,
        completions: &mut Vec<CompletionItem>,
        limit: usize,
    ) {
        self.sort_by_label();
        completions.extend(self.items);
        if completions.len() < limit {
            sub_tracker.sort_by_label();
            let remaining = limit - completions.len();
            completions.extend(sub_tracker.items.into_iter().take(remaining));
        }
    }

    #[inline]
    pub fn can_accept_or_replace(&self, name: &str, is_source: bool, limit: usize) -> bool {
        if self.items.len() < limit {
            return true;
        }
        if !is_source {
            return false;
        }
        self.bytecode_name_counts.contains_key(name)
    }

    #[inline]
    pub fn is_saturated(&self, limit: usize) -> bool {
        self.items.len() >= limit && self.bytecode_name_counts.is_empty()
    }

    fn record_callable(
        &mut self,
        ctx: &CandidateContext,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
    ) {
        let call_key = ctx.make_key(ctx.decl.name.as_str());
        let key = ctx.make_key(&decl_completion_key(ctx.decl));

        if is_source {
            self.record_source_callable(ctx, &call_key, key, seen_keys);
        } else {
            self.record_bytecode_callable(ctx, &call_key, key, seen_keys);
        }
    }

    fn record_source_callable(
        &mut self,
        ctx: &CandidateContext,
        call_key: &str,
        key: String,
        seen_keys: &mut HashSet<String>,
    ) {
        let mut src_param_types = None;
        if let Some(bc_list) = self.bytecode_callables.get_mut(call_key) {
            let src_types = src_param_types
                .get_or_insert_with(|| get_signature_param_types(ctx.decl.signature.as_deref()));
            let match_idx = bc_list
                .iter_mut()
                .position(|bc| are_param_types_compatible(bc.param_types(), src_types))
                .or_else(|| {
                    (bc_list.len() == 1 && bc_list[0].param_types().len() == src_types.len())
                        .then_some(0)
                });
            if let Some(pos) = match_idx {
                let bc = bc_list.remove(pos);
                if bc_list.is_empty() {
                    self.bytecode_callables.remove(call_key);
                }
                self.decrement_bytecode_name_count(&ctx.decl.name);
                self.replace_bytecode_item(bc.item_index, ctx.create_item());
                seen_keys.insert(key);
                self.source_callables
                    .entry(call_key.to_string())
                    .or_default()
                    .push(TrackedCallable {
                        item_index: 0,
                        signature: ctx.decl.signature.clone(),
                        param_types: src_param_types,
                    });
                return;
            }
        }
        self.source_callables
            .entry(call_key.to_string())
            .or_default()
            .push(TrackedCallable {
                item_index: 0,
                signature: ctx.decl.signature.clone(),
                param_types: src_param_types,
            });
        if self.items.len() < ctx.limit && seen_keys.insert(key) {
            self.items.push(ctx.create_item());
            self.is_source.push(true);
        }
    }

    fn record_bytecode_callable(
        &mut self,
        ctx: &CandidateContext,
        call_key: &str,
        key: String,
        seen_keys: &mut HashSet<String>,
    ) {
        let mut bc_param_types = None;
        if let Some(src_list) = self.source_callables.get_mut(call_key) {
            let bc_types = bc_param_types
                .get_or_insert_with(|| get_signature_param_types(ctx.decl.signature.as_deref()));
            let has_match = src_list
                .iter_mut()
                .any(|src| are_param_types_compatible(src.param_types(), bc_types));
            if has_match {
                return;
            }
        }
        if self.items.len() < ctx.limit && seen_keys.insert(key) {
            let idx = self.items.len();
            self.bytecode_callables
                .entry(call_key.to_string())
                .or_default()
                .push(TrackedCallable {
                    item_index: idx,
                    signature: ctx.decl.signature.clone(),
                    param_types: bc_param_types,
                });
            *self
                .bytecode_name_counts
                .entry(ctx.decl.name.clone())
                .or_default() += 1;
            self.items.push(ctx.create_item());
            self.is_source.push(false);
        }
    }

    fn record_non_callable(
        &mut self,
        ctx: &CandidateContext,
        is_source: bool,
        seen_keys: &mut HashSet<String>,
    ) {
        let key = ctx.make_key(ctx.decl.name.as_str());
        if is_source {
            self.record_source_non_callable(ctx, key, seen_keys);
        } else {
            self.record_bytecode_non_callable(ctx, key, seen_keys);
        }
    }

    fn record_source_non_callable(
        &mut self,
        ctx: &CandidateContext,
        key: String,
        seen_keys: &mut HashSet<String>,
    ) {
        self.source_names.insert(key.clone());
        if let Some(indices) = self.bytecode_non_callables.get_mut(&key)
            && !indices.is_empty()
        {
            let idx = indices.remove(0);
            if indices.is_empty() {
                self.bytecode_non_callables.remove(&key);
            }
            self.decrement_bytecode_name_count(&ctx.decl.name);
            self.replace_bytecode_item(idx, ctx.create_item());
            seen_keys.insert(key);
            return;
        }
        if self.items.len() < ctx.limit && seen_keys.insert(key) {
            self.items.push(ctx.create_item());
            self.is_source.push(true);
        }
    }

    fn record_bytecode_non_callable(
        &mut self,
        ctx: &CandidateContext,
        key: String,
        seen_keys: &mut HashSet<String>,
    ) {
        if self.source_names.contains(&key) {
            return;
        }
        if self.items.len() < ctx.limit && seen_keys.insert(key.clone()) {
            let idx = self.items.len();
            self.bytecode_non_callables
                .entry(key)
                .or_default()
                .push(idx);
            *self
                .bytecode_name_counts
                .entry(ctx.decl.name.clone())
                .or_default() += 1;
            self.items.push(ctx.create_item());
            self.is_source.push(false);
        }
    }

    fn decrement_bytecode_name_count(&mut self, name: &SmolStr) {
        if let Some(count) = self.bytecode_name_counts.get_mut(name) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.bytecode_name_counts.remove(name);
            }
        }
    }

    fn replace_bytecode_item(&mut self, idx: usize, item: CompletionItem) {
        if self.items.len() > idx {
            self.items[idx] = item;
            if self.is_source.len() > idx {
                self.is_source[idx] = true;
            }
        }
    }
}
