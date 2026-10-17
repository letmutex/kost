use tower_lsp::lsp_types::Url;

use super::context::InferenceContext;
use super::qualification::{resolve_full_type_name, resolve_type_by_imports_or_package};
use super::trace::Tracer;
use super::types::KotlinType;
use super::variable::resolve_local_receiver_kt_traced;
use crate::indexer::{DeclarationItem, WorkspaceIndex};

pub fn resolve_named_symbol_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    clean: &str,
) -> Option<KotlinType> {
    if let Some(local_type) = resolve_local_receiver_kt_traced(
        ctx.index,
        ctx.current_url,
        clean,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
        ctx.tracer,
    ) {
        ctx.tracer
            .record_step(|| crate::concat_str!("Resolved local symbol `", clean, "`"));
        return Some(local_type);
    }

    let enclosing_classes = super::scope_function::collect_visible_containers_depth(
        ctx.index, ctx.root, ctx.pos, ctx.source, ctx.depth,
    );
    let is_visible = |url: &Url, decl: &DeclarationItem| {
        if decl.is_private && url != ctx.current_url {
            return false;
        }
        decl.container_name
            .as_ref()
            .is_none_or(|c| enclosing_classes.contains(c.as_str()))
    };

    if let Some(type_name) =
        resolve_type_by_imports_or_package(ctx.index, ctx.current_url, clean, &is_visible)
    {
        ctx.tracer.record_step(|| {
            crate::concat_str!(
                "Resolved symbol `",
                clean,
                "` via imports or package to `",
                &type_name,
                "`"
            )
        });
        return super::types::parse_type_signature(&type_name);
    }

    resolve_from_declarations(ctx.index, clean, is_visible, ctx.tracer)
        .or_else(|| resolve_builtin_primitive(clean, ctx.tracer))
}

fn resolve_from_declarations(
    index: &WorkspaceIndex,
    clean: &str,
    is_visible: impl Fn(&Url, &DeclarationItem) -> bool,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    index.name_to_decls.get(clean).and_then(|decls| {
        decls.iter().find_map(|(url, d)| {
            if !is_visible(url.as_ref(), d) {
                return None;
            }
            let ret = if let Some(sig_info) = super::signature::SignatureInfo::from_decl(d) {
                sig_info.return_type.as_ref().map(|t| t.to_string())
            } else {
                d.signature
                    .as_deref()
                    .and_then(crate::syntax::signature::extract_return_type_from_signature)
            };
            ret.and_then(|s| {
                let full = resolve_full_type_name(index, url.as_ref(), &s);
                tracer.record_step(|| {
                    crate::concat_str!(
                        "Resolved declaration `",
                        clean,
                        "` with return type `",
                        &full,
                        "`"
                    )
                });
                super::types::parse_type_signature(&full)
            })
        })
    })
}

fn resolve_builtin_primitive(clean: &str, tracer: &mut Tracer) -> Option<KotlinType> {
    crate::inference::model::PrimitiveType::from_name(clean).map(|prim| {
        tracer.record_step(|| crate::concat_str!("Resolved built-in primitive `", clean, "`"));
        if prim == crate::inference::model::PrimitiveType::Nothing {
            KotlinType::Nothing
        } else {
            KotlinType::Primitive(prim)
        }
    })
}
