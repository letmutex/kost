use ahash::AHashSet;
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::expression::resolve_full_type_name;
pub use super::scope_lambda::{get_builtin_scope_function, resolve_enclosing_lambda_type};
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::{extract_identifier_text, find_node_at_position};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeReceiverKind {
    ClassBody,
    ExtensionFunction,
    LambdaReceiver,
}

#[derive(Debug, Clone)]
pub struct ScopeReceiver {
    pub type_ref: KotlinType,
    pub kind: ScopeReceiverKind,
    pub label: Option<SmolStr>,
    pub node_start_byte: usize,
}

pub struct ScopeContext<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub receivers: Vec<ScopeReceiver>,
}

impl<'a> ScopeContext<'a> {
    pub fn new(index: &'a WorkspaceIndex, current_url: &'a Url) -> Self {
        Self {
            index,
            current_url,
            receivers: Vec::new(),
        }
    }

    pub fn push_receiver(
        &mut self,
        type_ref: KotlinType,
        kind: ScopeReceiverKind,
        label: Option<SmolStr>,
        node_start_byte: usize,
    ) {
        self.receivers.push(ScopeReceiver {
            type_ref,
            kind,
            label,
            node_start_byte,
        });
    }

    pub fn current_this(&self) -> Option<&KotlinType> {
        self.receivers.first().map(|r| &r.type_ref)
    }

    pub fn find_this_by_label(&self, label: &str) -> Option<&KotlinType> {
        self.receivers
            .iter()
            .find(|r| r.label.as_deref() == Some(label))
            .map(|r| &r.type_ref)
    }

    pub fn collect_visible_container_names(&self) -> AHashSet<String> {
        let mut names = AHashSet::default();
        for r in &self.receivers {
            let base = r.type_ref.base_name();
            let short = base.rsplit('.').next().unwrap_or(base);
            names.insert(base.to_string());
            names.insert(short.to_string());
            for sup in self.index.get_all_supertypes(base) {
                let s_short = sup.rsplit('.').next().unwrap_or(&sup);
                names.insert(sup.clone());
                names.insert(s_short.to_string());
            }
        }
        names
    }
}

fn push_enclosing_receiver<'a>(
    ctx: &mut ScopeContext<'a>,
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    curr: Node<'a>,
    pos: Position,
    source: &str,
    root: Node<'a>,
    depth: usize,
) {
    let k = curr.kind();
    if k == "lambda_literal" {
        if let Some((
            KotlinType::Function {
                receiver: Some(rec),
                ..
            },
            label,
        )) = resolve_enclosing_lambda_type(
            index,
            current_url,
            curr,
            pos,
            source,
            Some(root),
            depth + 1,
        ) {
            ctx.push_receiver(
                *rec,
                ScopeReceiverKind::LambdaReceiver,
                label,
                curr.start_byte(),
            );
        }
    } else if k == "function_declaration" {
        if let Some(ext_rec) =
            crate::syntax::ast_util::find_enclosing_extension_receiver(curr, source)
        {
            let full = resolve_full_type_name(index, current_url, &ext_rec);
            if let Some(k_ty) = super::types::parse_type_signature(&full) {
                let fn_name = extract_identifier_text(&curr, source).map(SmolStr::new);
                ctx.push_receiver(
                    k_ty,
                    ScopeReceiverKind::ExtensionFunction,
                    fn_name,
                    curr.start_byte(),
                );
            }
        }
    } else if matches!(
        k,
        "class_declaration" | "object_declaration" | "companion_object"
    ) {
        let class_name =
            extract_identifier_text(&curr, source).unwrap_or(if k == "companion_object" {
                "Companion"
            } else {
                ""
            });
        if !class_name.is_empty() {
            let full = resolve_full_type_name(index, current_url, class_name);
            if let Some(k_ty) = super::types::parse_type_signature(&full) {
                ctx.push_receiver(
                    k_ty,
                    ScopeReceiverKind::ClassBody,
                    Some(SmolStr::new(class_name)),
                    curr.start_byte(),
                );
            }
        }
    }
}

pub fn build_scope_context<'a>(
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    pos: Position,
    source: &str,
    root: Node<'a>,
    depth: usize,
) -> ScopeContext<'a> {
    let mut ctx = ScopeContext::new(index, current_url);
    let target = match find_node_at_position(root, pos) {
        Some(n) => n,
        None => return ctx,
    };

    let mut current = Some(target);
    while let Some(curr) = current {
        push_enclosing_receiver(&mut ctx, index, current_url, curr, pos, source, root, depth);
        current = curr.parent();
    }

    ctx
}
