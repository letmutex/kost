use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, SignatureHelp, SignatureInformation, Url};
use tree_sitter::Node;

use super::signature_info::{
    SignatureMatchContext, build_signature_information, is_callable_kind,
    select_best_signature_and_param,
};
use super::signature_parser::find_enclosing_call_context;

use crate::indexer::{DeclarationItem, WorkspaceIndex, is_source_url};
use crate::inference::signature::{
    are_param_types_compatible, get_decl_param_types, parse_signature_params,
};
use crate::parser::parse_source;

pub fn find_signature_help(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
) -> Option<SignatureHelp> {
    let tree = parse_source(source);
    let root = tree.as_ref().map(|t| t.root_node());
    let call_ctx = find_enclosing_call_context(pos, source)?;

    let candidates = resolve_signature_candidates(
        index,
        current_url,
        &call_ctx.callee,
        call_ctx.receiver.as_deref(),
        pos,
        source,
        root,
    );

    if candidates.is_empty() {
        return None;
    }

    let mut signatures: Vec<SignatureInformation> = candidates
        .iter()
        .filter_map(|(decl, _)| {
            let mut info = build_signature_information(decl, call_ctx.active_parameter)?;
            if let Some(ref name) = call_ctx.active_arg_name {
                let sig_str = decl.signature.as_deref().unwrap_or("");
                let params = parse_signature_params(sig_str);
                if let Some(p_idx) = params.iter().position(|p| p.name == *name) {
                    info.active_parameter = Some(p_idx as u32);
                }
            }
            Some(info)
        })
        .collect();

    if signatures.is_empty() {
        return None;
    }

    let match_ctx = SignatureMatchContext {
        index,
        current_url,
        pos,
        source,
        root,
    };
    let (active_sig, active_param) =
        select_best_signature_and_param(&candidates, &signatures, &call_ctx, &match_ctx);

    if let Some(sig) = signatures.get_mut(active_sig as usize) {
        sig.active_parameter = Some(active_param);
    }

    Some(SignatureHelp {
        signatures,
        active_signature: Some(active_sig),
        active_parameter: Some(active_param),
    })
}

fn normalize_container(container: Option<&str>) -> &str {
    match container {
        Some(c) if c.ends_with("Kt") => "",
        Some(c) => c,
        None => "",
    }
}

fn decl_key(decl: &DeclarationItem) -> String {
    let sig_str = decl.signature.as_deref().unwrap_or("");
    let clean_sig = sig_str
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ");
    let norm_container = normalize_container(decl.container_name.as_deref());
    format!("{}:{}:{}", decl.name, norm_container, clean_sig)
}

fn find_bytecode_match(
    results: &[(Arc<DeclarationItem>, bool)],
    decl: &DeclarationItem,
    norm_container: &str,
    param_types: &[String],
) -> Option<usize> {
    let type_idx = results.iter().enumerate().find_map(|(i, (d, src))| {
        (!*src
            && d.name == decl.name
            && normalize_container(d.container_name.as_deref()) == norm_container
            && are_param_types_compatible(&get_decl_param_types(d), param_types))
        .then_some(i)
    });
    if type_idx.is_some() {
        return type_idx;
    }
    // Arity fallback: only when exactly one bytecode entry matches by arity
    let arity_matches: Vec<usize> = results
        .iter()
        .enumerate()
        .filter_map(|(i, (d, src))| {
            (!*src
                && d.name == decl.name
                && normalize_container(d.container_name.as_deref()) == norm_container
                && get_decl_param_types(d).len() == param_types.len())
            .then_some(i)
        })
        .collect();
    if arity_matches.len() == 1 {
        Some(arity_matches[0])
    } else {
        None
    }
}

struct CandidateCollector<'a> {
    index: &'a WorkspaceIndex,
    results: Vec<(Arc<DeclarationItem>, bool)>,
    seen: AHashMap<String, usize>,
    matched_src: AHashSet<usize>,
}

impl<'a> CandidateCollector<'a> {
    fn new(index: &'a WorkspaceIndex) -> Self {
        Self {
            index,
            results: Vec::new(),
            seen: AHashMap::default(),
            matched_src: AHashSet::default(),
        }
    }

    fn add(&mut self, decl: &Arc<DeclarationItem>, is_src: bool) {
        let key = decl_key(decl);
        let norm_container = normalize_container(decl.container_name.as_deref());
        let param_types = get_decl_param_types(decl);

        if let Some(&idx) = self.seen.get(&key) {
            if !self.results[idx].1 && is_src {
                self.results[idx] = (Arc::clone(decl), true);
            }
            self.matched_src.insert(idx);
            return;
        }

        if is_src {
            if let Some(idx) =
                find_bytecode_match(&self.results, decl, norm_container, &param_types)
            {
                self.results[idx] = (Arc::clone(decl), true);
                self.seen.insert(key, idx);
                self.matched_src.insert(idx);
                return;
            }
        } else {
            let type_match = self.results.iter().enumerate().find_map(|(i, (d, src))| {
                (*src
                    && d.name == decl.name
                    && normalize_container(d.container_name.as_deref()) == norm_container
                    && are_param_types_compatible(&get_decl_param_types(d), &param_types))
                .then_some(i)
            });
            if let Some(idx) = type_match {
                self.matched_src.insert(idx);
                return;
            }

            let arity_matches: Vec<usize> = self
                .results
                .iter()
                .enumerate()
                .filter_map(|(i, (d, src))| {
                    (*src
                        && !self.matched_src.contains(&i)
                        && d.name == decl.name
                        && normalize_container(d.container_name.as_deref()) == norm_container
                        && get_decl_param_types(d).len() == param_types.len())
                    .then_some(i)
                })
                .collect();
            if arity_matches.len() == 1 {
                self.matched_src.insert(arity_matches[0]);
                return;
            }
        }

        let idx = self.results.len();
        self.seen.insert(key, idx);
        self.results.push((Arc::clone(decl), is_src));
    }

    fn collect_receiver(
        &mut self,
        current_url: &Url,
        callee: &str,
        r: &str,
        pos: Position,
        source: &str,
        root: Option<Node>,
    ) {
        let rec_type = crate::features::receiver_type::resolve_receiver_type(
            self.index,
            current_url,
            r,
            pos,
            source,
            root,
        );
        if let Some(ref rt) = rec_type {
            let simple_rt = rt.rsplit('.').next().unwrap_or(rt);
            let mut containers = vec![rt.as_str()];
            if simple_rt != rt {
                containers.push(simple_rt);
            }
            for c in containers {
                if let Some(decls) = self.index.container_to_decls.get(&SmolStr::new(c)) {
                    for (url, d) in decls {
                        if d.name == callee && is_callable_kind(d.kind) {
                            self.add(d, is_source_url(url));
                        }
                    }
                }
            }
        }
        if let Some(decls) = self.index.name_to_decls.get(&SmolStr::new(callee)) {
            for (url, d) in decls {
                if is_callable_kind(d.kind)
                    && let Some(ref c) = d.container_name
                    && (rec_type.as_deref() == Some(c.as_str())
                        || rec_type
                            .as_deref()
                            .is_some_and(|rt| rt.ends_with(c.as_str())))
                {
                    self.add(d, is_source_url(url));
                }
            }
        }
    }

    fn collect_unqualified(&mut self, current_url: &Url, callee: &str) {
        if let Some(file_syms) = self.index.files.get(current_url) {
            let is_src = is_source_url(current_url);
            for d in &file_syms.declarations {
                if d.name == callee && is_callable_kind(d.kind) {
                    let arc_d = Arc::new(d.clone());
                    self.add(&arc_d, is_src);
                }
            }
        }
        if let Some(decls) = self.index.name_to_decls.get(&SmolStr::new(callee)) {
            for (url, d) in decls {
                if is_callable_kind(d.kind) {
                    self.add(d, is_source_url(url));
                }
            }
        }
    }

    fn finish(mut self) -> Vec<(Arc<DeclarationItem>, bool)> {
        self.results.sort_by_key(|(_, is_src)| !*is_src);
        self.results
    }
}

fn resolve_signature_candidates(
    index: &WorkspaceIndex,
    current_url: &Url,
    callee: &str,
    receiver: Option<&str>,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Vec<(Arc<DeclarationItem>, bool)> {
    let mut collector = CandidateCollector::new(index);
    if let Some(r) = receiver {
        collector.collect_receiver(current_url, callee, r, pos, source, root);
    } else {
        collector.collect_unqualified(current_url, callee);
    }
    collector.finish()
}
