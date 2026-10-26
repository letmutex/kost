use ahash::AHashMap;
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Range, SymbolKind};

use super::class_hierarchy::{apply_class_signature, collect_supertypes};
use super::class_metadata::{
    is_kotlin_file_facade, is_skippable_class_file, read_class_attributes,
};
use super::constant_pool::{CpEntry, get_class_name, parse_constant_pool};
use super::jvm_signature::{
    jvm_method_sig_to_kotlin, parse_jvm_method_signature, parse_jvm_type_signature,
};
use super::jvm_type::{
    descriptor_to_type, extract_extension_info_from_desc, get_utf8, is_extension_method,
    method_descriptor_to_sig, read_u16, read_u32,
};
use crate::syntax::modifiers::ClassModifiers;
use crate::syntax::symbol_item::{DeclarationItem, DeclarationMetadata, FileSymbols};

pub fn parse_class_file_symbols(bytes: &[u8]) -> Option<FileSymbols> {
    if bytes.len() < 10 || bytes[0..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return None;
    }

    let mut cursor = 8;
    let cp_count = read_u16(bytes, &mut cursor)? as usize;
    if cp_count == 0 {
        return None;
    }

    let constant_pool = parse_constant_pool(bytes, &mut cursor, cp_count)?;
    let access_flags = read_u16(bytes, &mut cursor)?;
    let this_class_idx = read_u16(bytes, &mut cursor)? as usize;
    let super_class_idx = read_u16(bytes, &mut cursor)? as usize;

    let full_class_name = get_class_name(&constant_pool.entries, this_class_idx)?;
    let (package_name, simple_class_name) = split_package_and_class(&full_class_name);

    let (kind, modifiers, is_interface) = determine_class_kind_and_modifiers(access_flags);

    let mut file_symbols = FileSymbols {
        package_name,
        imports: Vec::new(),
        declarations: Vec::new(),
        class_supertypes: AHashMap::default(),
        type_aliases: AHashMap::default(),
    };

    collect_supertypes(
        bytes,
        &mut cursor,
        &constant_pool.entries,
        super_class_idx,
        simple_class_name,
        &mut file_symbols,
    );
    add_class_declaration(
        simple_class_name,
        is_interface,
        kind,
        modifiers,
        &mut file_symbols,
    );
    collect_fields(
        bytes,
        &mut cursor,
        &constant_pool.entries,
        simple_class_name,
        &mut file_symbols,
    );
    collect_methods(
        bytes,
        &mut cursor,
        &constant_pool.entries,
        simple_class_name,
        constant_pool.has_this_receiver,
        &mut file_symbols,
    );

    let class_attrs = read_class_attributes(
        bytes,
        &mut cursor,
        &constant_pool.entries,
        constant_pool.has_kotlin_metadata,
    );
    if is_skippable_class_file(class_attrs.metadata_k) {
        return None;
    }
    apply_class_signature(simple_class_name, class_attrs.signature, &mut file_symbols);

    if is_kotlin_file_facade(class_attrs.metadata_k) {
        adjust_file_facade_symbols(simple_class_name, &mut file_symbols);
    }

    Some(file_symbols)
}

fn adjust_file_facade_symbols(simple_class_name: &str, file_symbols: &mut FileSymbols) {
    file_symbols.class_supertypes.clear();
    file_symbols.declarations.retain_mut(|decl| {
        if decl.name == simple_class_name
            && matches!(decl.kind, SymbolKind::CLASS | SymbolKind::INTERFACE)
        {
            return false;
        }
        if decl.container_name.as_deref() == Some(simple_class_name) {
            decl.container_name = None;
        }
        true
    });
}

fn determine_class_kind_and_modifiers(access_flags: u16) -> (SymbolKind, ClassModifiers, bool) {
    let is_interface = (access_flags & 0x0200) != 0;
    let is_annotation = (access_flags & 0x2000) != 0;
    let is_enum = (access_flags & 0x4000) != 0;
    let kind = if is_enum {
        SymbolKind::ENUM
    } else if is_interface {
        SymbolKind::INTERFACE
    } else {
        SymbolKind::CLASS
    };
    let modifiers = ClassModifiers {
        is_enum,
        is_annotation,
        ..Default::default()
    };
    (kind, modifiers, is_interface)
}

fn split_package_and_class(full_name: &str) -> (Option<String>, &str) {
    match full_name.rfind('/') {
        Some(pos) => (
            Some(full_name[..pos].replace('/', ".")),
            &full_name[pos + 1..],
        ),
        None => (None, full_name),
    }
}

fn add_class_declaration(
    simple_name: &str,
    is_interface: bool,
    kind: SymbolKind,
    modifiers: ClassModifiers,
    file_symbols: &mut FileSymbols,
) {
    let default_range = Range::default();
    let class_signature = common::concat_str!(
        if is_interface { "interface" } else { "open" },
        " class ",
        simple_name
    );
    file_symbols.declarations.push(DeclarationItem::new(
        simple_name,
        kind,
        default_range,
        default_range,
        DeclarationMetadata::with_modifiers(
            None,
            Some(class_signature.into()),
            None,
            false,
            false,
            modifiers,
        ),
    ));
}

fn collect_fields(
    bytes: &[u8],
    cursor: &mut usize,
    cp: &[CpEntry],
    container: &str,
    symbols: &mut FileSymbols,
) {
    let default_range = Range::default();
    let Some(fields_count) = read_u16(bytes, cursor) else {
        return;
    };

    for _ in 0..fields_count {
        let f_flags = read_u16(bytes, cursor).unwrap_or(0);
        let name_idx = read_u16(bytes, cursor).unwrap_or(0) as usize;
        let desc_idx = read_u16(bytes, cursor).unwrap_or(0) as usize;
        let is_pub = (f_flags & 0x0001) != 0 || (f_flags & 0x0004) != 0;

        let sig_attr = read_signature_attribute(bytes, cursor, cp);

        if is_pub
            && let (Some(name), Some(desc)) = (get_utf8(cp, name_idx), get_utf8(cp, desc_idx))
            && !name.contains('$')
            && !name.contains('-')
        {
            let field_type = sig_attr
                .and_then(parse_jvm_type_signature)
                .unwrap_or_else(|| descriptor_to_type(desc));
            let sig = common::concat_str!("val ", name, ": ", field_type);
            symbols.declarations.push(DeclarationItem::new(
                name,
                SymbolKind::PROPERTY,
                default_range,
                default_range,
                DeclarationMetadata::new(
                    None,
                    Some(sig.into()),
                    Some(SmolStr::new(container)),
                    false,
                ),
            ));
        }
    }
}

fn format_method_sig_and_container(
    name: &str,
    desc: &str,
    container: &str,
    is_ext: bool,
    sig_attr: Option<&str>,
) -> (String, SmolStr) {
    if let Some(method_sig) = sig_attr.and_then(parse_jvm_method_signature) {
        if is_ext {
            let rec_qualified = extract_extension_info_from_desc(desc)
                .map(|(q, _)| q)
                .unwrap_or_else(|| container.to_string());
            let (sig_str, _) = jvm_method_sig_to_kotlin(name, &method_sig, true);
            (sig_str, SmolStr::new(rec_qualified))
        } else {
            let (sig_str, _) = jvm_method_sig_to_kotlin(name, &method_sig, false);
            (sig_str, SmolStr::new(container))
        }
    } else if is_ext {
        if let Some((rec_qualified, rec_sig)) = extract_extension_info_from_desc(desc) {
            let rec_simple = rec_sig.split('.').next().unwrap_or("");
            let suffix = rec_sig.split('.').nth(1).unwrap_or("()");
            (
                common::concat_str!("fun ", rec_simple, ".", name, suffix),
                SmolStr::new(rec_qualified),
            )
        } else {
            (
                common::concat_str!("fun ", name, method_descriptor_to_sig(desc)),
                SmolStr::new(container),
            )
        }
    } else {
        (
            common::concat_str!("fun ", name, method_descriptor_to_sig(desc)),
            SmolStr::new(container),
        )
    }
}

fn collect_methods(
    bytes: &[u8],
    cursor: &mut usize,
    cp: &[CpEntry],
    container: &str,
    has_this_receiver: bool,
    symbols: &mut FileSymbols,
) {
    let default_range = Range::default();
    let Some(methods_count) = read_u16(bytes, cursor) else {
        return;
    };

    for _ in 0..methods_count {
        let m_flags = read_u16(bytes, cursor).unwrap_or(0);
        let name_idx = read_u16(bytes, cursor).unwrap_or(0) as usize;
        let desc_idx = read_u16(bytes, cursor).unwrap_or(0) as usize;
        let is_pub = (m_flags & 0x0001) != 0 || (m_flags & 0x0004) != 0;
        let is_static = (m_flags & 0x0008) != 0;

        let method_name = get_utf8(cp, name_idx);
        let method_desc = get_utf8(cp, desc_idx);
        let is_valid = is_pub
            && method_name.is_some_and(|name| {
                !name.contains('$') && !name.contains('-') && name != "<clinit>" && name != "<init>"
            });

        let is_ext = has_this_receiver
            && is_static
            && is_valid
            && method_desc.is_some_and(|desc| desc.starts_with('(') && !desc.starts_with("()"))
            && is_extension_method(bytes, *cursor, cp);

        let sig_attr = read_signature_attribute(bytes, cursor, cp);

        if is_valid && let (Some(name), Some(desc)) = (method_name, method_desc) {
            let (sig, method_container) =
                format_method_sig_and_container(name, desc, container, is_ext, sig_attr);
            symbols.declarations.push(DeclarationItem::new(
                name,
                SymbolKind::FUNCTION,
                default_range,
                default_range,
                DeclarationMetadata::new(None, Some(sig.into()), Some(method_container), false),
            ));
        }
    }
}

fn read_signature_attribute<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    cp: &'a [CpEntry],
) -> Option<&'a str> {
    let attr_count = read_u16(bytes, cursor)? as usize;
    let mut sig = None;
    for _ in 0..attr_count {
        let name_idx = read_u16(bytes, cursor)? as usize;
        let length = read_u32(bytes, cursor)? as usize;
        if *cursor + length > bytes.len() {
            return None;
        }
        if get_utf8(cp, name_idx) == Some("Signature") && length == 2 {
            let mut sig_cur = *cursor;
            if let Some(sig_idx) = read_u16(bytes, &mut sig_cur)
                && let Some(s) = get_utf8(cp, sig_idx as usize)
            {
                sig = Some(s);
            }
        }
        *cursor += length;
    }
    sig
}
