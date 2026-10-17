use url::Url;

use super::resolve::resolve_type_by_imports_or_package;
use crate::indexer::WorkspaceIndex;
use crate::inference::model::PrimitiveType;
use crate::inference::typealias::resolve_type_alias_or_import;
use crate::inference::types::standard::StandardInterface;
use crate::inference::types::{KotlinType, TypeArg};
use crate::syntax::symbol_item::DeclarationItem;

fn qualify_class_name(index: &WorkspaceIndex, current_url: &Url, base: &str) -> smol_str::SmolStr {
    let aliased = resolve_type_alias_or_import(index, current_url, base);
    let target = if aliased.is_empty() { base } else { &aliased };
    if target.contains('.') {
        return smol_str::SmolStr::new(target);
    }
    let is_visible = |url: &Url, decl: &DeclarationItem| !decl.is_private || url == current_url;
    let resolved_name = resolve_type_by_imports_or_package(index, current_url, target, &is_visible)
        .unwrap_or_else(|| target.to_string());
    smol_str::SmolStr::new(resolved_name)
}

/// Qualify a KotlinType recursively without string serialization or reparsing.
pub fn qualify_type(index: &WorkspaceIndex, current_url: &Url, ty: &KotlinType) -> KotlinType {
    match ty {
        KotlinType::Primitive(p) => KotlinType::Primitive(*p),
        KotlinType::Nothing => KotlinType::Nothing,
        KotlinType::DynamicOrAny => KotlinType::DynamicOrAny,
        KotlinType::TypeParam(s) => KotlinType::TypeParam(s.clone()),
        KotlinType::Nullable(inner) => qualify_type(index, current_url, inner).make_nullable(),
        KotlinType::Intersection(types) => KotlinType::Intersection(
            types
                .iter()
                .map(|t| qualify_type(index, current_url, t))
                .collect(),
        ),
        KotlinType::Function {
            receiver,
            params,
            return_type,
            is_suspend,
        } => KotlinType::Function {
            receiver: receiver
                .as_ref()
                .map(|r| Box::new(qualify_type(index, current_url, r))),
            params: params
                .iter()
                .map(|p| qualify_type(index, current_url, p))
                .collect(),
            return_type: Box::new(qualify_type(index, current_url, return_type)),
            is_suspend: *is_suspend,
        },
        KotlinType::Class { name, args } => {
            let qualified_args: Vec<TypeArg> = args
                .iter()
                .map(|arg| TypeArg {
                    variance: arg.variance,
                    type_ref: qualify_type(index, current_url, &arg.type_ref),
                })
                .collect();
            let base = name.as_str();
            if base.is_empty() || base == "?" {
                return KotlinType::Class {
                    name: name.clone(),
                    args: qualified_args,
                };
            }
            if qualified_args.is_empty()
                && let Some(prim) = PrimitiveType::from_name(base)
            {
                return KotlinType::Primitive(prim);
            }
            let qualified_name = qualify_class_name(index, current_url, base);
            if qualified_args.is_empty() {
                if let Some(prim) = PrimitiveType::from_name(qualified_name.as_str()) {
                    return KotlinType::Primitive(prim);
                }
                if StandardInterface::from_qualified_name(qualified_name.as_str())
                    == Some(StandardInterface::Number)
                {
                    return KotlinType::NUMBER;
                }
            }
            KotlinType::Class {
                name: qualified_name,
                args: qualified_args,
            }
        }
    }
}
