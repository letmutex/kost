use super::constant_pool::{CpEntry, get_class_name};
use super::jvm_signature::parse_jvm_class_signature;
use super::jvm_type::read_u16;
use crate::indexer::FileSymbols;

pub(crate) fn collect_supertypes(
    bytes: &[u8],
    cursor: &mut usize,
    cp: &[CpEntry],
    super_idx: usize,
    simple_name: &str,
    symbols: &mut FileSymbols,
) {
    let pkg = symbols.package_name.clone();
    let mut add_super = |raw_name: &str| {
        if raw_name == "java/lang/Object" {
            return;
        }
        let dot_super = raw_name.replace('/', ".");
        let simple_super = raw_name.rsplit('/').next().unwrap_or(raw_name);
        symbols
            .class_supertypes
            .entry(simple_name.to_string())
            .or_default()
            .push(simple_super.to_string());
        if let Some(ref p) = pkg
            && !p.is_empty()
        {
            symbols
                .class_supertypes
                .entry(crate::concat_str!(p, ".", simple_name))
                .or_default()
                .push(dot_super);
        }
    };

    if let Some(super_name) = get_class_name(cp, super_idx) {
        add_super(&super_name);
    }

    if let Some(iface_count) = read_u16(bytes, cursor) {
        for _ in 0..iface_count {
            if let Some(iface_idx) = read_u16(bytes, cursor)
                && let Some(iface_name) = get_class_name(cp, iface_idx as usize)
            {
                add_super(&iface_name);
            }
        }
    }
}

pub(crate) fn apply_class_signature(
    simple_class_name: &str,
    class_sig_attr: Option<&str>,
    symbols: &mut FileSymbols,
) {
    let Some(class_sig) = class_sig_attr.and_then(parse_jvm_class_signature) else {
        return;
    };
    let pkg = symbols.package_name.clone();
    for supertype in class_sig.supertypes {
        if supertype == "Object" || supertype == "Any" {
            continue;
        }
        let simple_super = supertype.split('<').next().unwrap_or(&supertype);
        let simple_super = simple_super.rsplit('.').next().unwrap_or(simple_super);
        let super_with_simple_name = if supertype.contains('<') {
            let args = &supertype[supertype.find('<').unwrap()..];
            crate::concat_str!(simple_super, args)
        } else {
            simple_super.to_string()
        };

        let entry = symbols
            .class_supertypes
            .entry(simple_class_name.to_string())
            .or_default();
        if let Some(pos) = entry.iter().position(|s| s == simple_super) {
            entry[pos] = super_with_simple_name.clone();
        } else if !entry.contains(&super_with_simple_name) {
            entry.push(super_with_simple_name.clone());
        }

        if let Some(ref p) = pkg
            && !p.is_empty()
        {
            let dot_super = if supertype.contains('<') {
                let bare = supertype.split('<').next().unwrap_or(&supertype);
                let args = &supertype[supertype.find('<').unwrap()..];
                crate::concat_str!(bare.replace('/', "."), args)
            } else {
                supertype.replace('/', ".")
            };
            let full_entry = symbols
                .class_supertypes
                .entry(crate::concat_str!(p, ".", simple_class_name))
                .or_default();
            let raw_dot = dot_super.split('<').next().unwrap_or(&dot_super);
            if let Some(pos) = full_entry.iter().position(|s| s == raw_dot) {
                full_entry[pos] = dot_super.clone();
            } else if !full_entry.contains(&dot_super) {
                full_entry.push(dot_super);
            }
        }
    }
}
