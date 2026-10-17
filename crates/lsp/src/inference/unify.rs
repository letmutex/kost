use super::types::standard::{StandardInterface, is_lub_ignored_name};
use super::types::{KotlinType, PrimitiveType};
use crate::indexer::WorkspaceIndex;

/// Unify two types to their Least Upper Bound (LUB)
pub fn unify_two_types(
    index: &WorkspaceIndex,
    t1: &KotlinType,
    t2: &KotlinType,
) -> Option<KotlinType> {
    if t1 == t2 {
        return Some(t1.clone());
    }
    if t1 == &KotlinType::NOTHING {
        return Some(t2.clone());
    }
    if t2 == &KotlinType::NOTHING {
        return Some(t1.clone());
    }
    let is_nullable = t1.is_nullable() || t2.is_nullable();
    let unnull1 = t1.unwrap_nullable_ref();
    let unnull2 = t2.unwrap_nullable_ref();

    if unnull1 == unnull2 {
        let res = unnull1.clone();
        return Some(res.with_nullability(is_nullable));
    }

    if unnull1 == &KotlinType::NOTHING {
        return Some(unnull2.clone().with_nullability(is_nullable));
    }
    if unnull2 == &KotlinType::NOTHING {
        return Some(unnull1.clone().with_nullability(is_nullable));
    }

    if unnull1.is_any() || unnull2.is_any() {
        return Some(KotlinType::ANY.with_nullability(is_nullable));
    }

    if let (KotlinType::Primitive(p1), KotlinType::Primitive(p2)) = (unnull1, unnull2)
        && p1.is_numeric()
        && p2.is_numeric()
    {
        return Some(KotlinType::NUMBER.with_nullability(is_nullable));
    }

    if let (KotlinType::Primitive(p), KotlinType::Class { name, args })
    | (KotlinType::Class { name, args }, KotlinType::Primitive(p)) = (unnull1, unnull2)
        && unify_primitive_and_class(*p, name, args)
    {
        let res = if matches!(unnull1, KotlinType::Class { .. }) {
            unnull1.clone()
        } else {
            unnull2.clone()
        };
        return Some(res.with_nullability(is_nullable));
    }

    let c1 = unnull1.base_name();
    let c2 = unnull2.base_name();

    unify_by_class_hierarchy(index, unnull1, unnull2, c1, c2, is_nullable)
}

fn unify_by_class_hierarchy(
    index: &WorkspaceIndex,
    unnull1: &KotlinType,
    unnull2: &KotlinType,
    c1: &str,
    c2: &str,
    is_nullable: bool,
) -> Option<KotlinType> {
    let supers1 = index.get_all_supertypes(c1);
    let supers2 = index.get_all_supertypes(c2);

    if c1 != c2 {
        let args1 = unnull1.type_arguments();
        let args2 = unnull2.type_arguments();
        if (args2.is_empty() || args1 == args2) && is_class_subtype_with_supers(&supers1, c1, c2) {
            return Some(unnull2.clone().with_nullability(is_nullable));
        }
        if (args1.is_empty() || args1 == args2) && is_class_subtype_with_supers(&supers2, c2, c1) {
            return Some(unnull1.clone().with_nullability(is_nullable));
        }
    }

    for s in &supers1 {
        if !is_ignored_supertype(s) && supers2.contains(s) {
            let res = KotlinType::parse(s.as_str())
                .unwrap_or_else(|| KotlinType::simple_class(s.as_str()));
            return Some(res.with_nullability(is_nullable));
        }
    }

    Some(KotlinType::ANY.with_nullability(is_nullable))
}

pub fn numeric_widening(p1: PrimitiveType, p2: PrimitiveType) -> Option<PrimitiveType> {
    let r1 = numeric_rank(p1)?;
    let r2 = numeric_rank(p2)?;
    let max_r = r1.max(r2);
    Some(match max_r {
        4 => PrimitiveType::Double,
        3 => PrimitiveType::Float,
        2 => PrimitiveType::Long,
        _ => PrimitiveType::Int,
    })
}

pub fn numeric_rank(p: PrimitiveType) -> Option<u8> {
    match p {
        PrimitiveType::Byte | PrimitiveType::Short => Some(0),
        PrimitiveType::Int => Some(1),
        PrimitiveType::Long => Some(2),
        PrimitiveType::Float => Some(3),
        PrimitiveType::Double => Some(4),
        _ => None,
    }
}

pub fn is_ignored_supertype(name: &str) -> bool {
    is_lub_ignored_name(name)
}

pub fn unify_primitive_and_class(
    p: PrimitiveType,
    name: &str,
    args: &[super::types::TypeArg],
) -> bool {
    let p_name = p.as_str();
    let name_short = name.rsplit('.').next().unwrap_or(name);
    if let Some(iface) = StandardInterface::from_qualified_name(name)
        && iface.conforms_primitive(p)
    {
        if iface == StandardInterface::Comparable && !args.is_empty() {
            let arg_conforms = args.first().is_some_and(|arg| {
                arg.variance == super::types::Variance::Star
                    || arg.type_ref.is_any()
                    || match &arg.type_ref {
                        KotlinType::Primitive(arg_p) => *arg_p == p,
                        _ => false,
                    }
            });
            if !arg_conforms {
                return false;
            }
        }
        return true;
    }
    p_name == name || (!name.contains('.') && p_name == name_short)
}

pub fn is_class_subtype_with_supers(supertypes: &[String], n1: &str, n2: &str) -> bool {
    let n1_clean = n1.split('<').next().unwrap_or(n1).trim();
    let n2_clean = n2.split('<').next().unwrap_or(n2).trim();
    let n1_short = n1_clean.rsplit('.').next().unwrap_or(n1_clean);
    let n2_short = n2_clean.rsplit('.').next().unwrap_or(n2_clean);

    if n1_clean == n2_clean
        || ((!n1_clean.contains('.') || !n2_clean.contains('.')) && n1_short == n2_short)
    {
        return true;
    }
    if StandardInterface::from_qualified_name(n2_clean) == Some(StandardInterface::Any) {
        return true;
    }
    supertypes.iter().any(|s| {
        let s_short = s.rsplit('.').next().unwrap_or(s);
        s == n2_clean || ((!s.contains('.') || !n2_clean.contains('.')) && s_short == n2_short)
    })
}

pub fn is_class_subtype(index: &WorkspaceIndex, n1: &str, n2: &str) -> bool {
    let n1_clean = n1.split('<').next().unwrap_or(n1).trim();
    let supertypes = index.get_all_supertypes(n1_clean);
    is_class_subtype_with_supers(&supertypes, n1, n2)
}
