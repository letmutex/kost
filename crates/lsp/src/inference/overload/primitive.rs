use crate::inference::signature::GenericParamInfo;
use crate::inference::types::standard::StandardInterface;
use crate::inference::types::{KotlinType, PrimitiveType};

pub fn clean_variance(s: &str) -> &str {
    let trimmed = s.trim();
    trimmed
        .strip_prefix("out")
        .or_else(|| trimmed.strip_prefix("in"))
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .unwrap_or(trimmed)
        .trim()
}

pub(crate) fn get_primitive_type(ty: &KotlinType) -> Option<PrimitiveType> {
    match ty.unwrap_nullable_ref() {
        KotlinType::Primitive(prim) => Some(*prim),
        KotlinType::Class { name, args } if args.is_empty() => {
            let simple = name.rsplit('.').next().unwrap_or(name.as_str());
            PrimitiveType::from_name(simple)
        }
        _ => None,
    }
}

pub(crate) fn is_primitive_conforming(
    prim: PrimitiveType,
    bound: &KotlinType,
    type_params: &[GenericParamInfo],
) -> bool {
    let bound_simple = bound.base_name();
    let bound_short = bound_simple.rsplit('.').next().unwrap_or(bound_simple);
    if let Some(iface) = StandardInterface::from_name(bound_short) {
        match iface {
            StandardInterface::Comparable => {
                let args = bound.type_arguments();
                if args.is_empty() {
                    return true;
                }
                let target_arg = &args[0];
                if target_arg.variance == crate::inference::types::Variance::Star {
                    return true;
                }
                let target_ref = target_arg.type_ref.unwrap_nullable_ref();
                if target_ref.is_any() {
                    return true;
                }
                if let Some(target_prim) = get_primitive_type(target_ref) {
                    return prim == target_prim;
                }
                match target_ref {
                    KotlinType::TypeParam(tp_name) => {
                        type_params.iter().any(|p| p.name == tp_name.as_str())
                    }
                    KotlinType::Class { name, args: t_args } if t_args.is_empty() => {
                        let t_short = name.rsplit('.').next().unwrap_or(name.as_str());
                        if type_params.is_empty()
                            && t_short.len() == 1
                            && t_short.chars().all(|c| c.is_ascii_uppercase())
                        {
                            true
                        } else {
                            type_params.iter().any(|p| p.name == t_short)
                        }
                    }
                    _ => false,
                }
            }
            StandardInterface::Number => prim.is_numeric(),
            StandardInterface::CharSequence => prim == PrimitiveType::String,
            StandardInterface::Serializable => true,
            _ => false,
        }
    } else {
        false
    }
}
