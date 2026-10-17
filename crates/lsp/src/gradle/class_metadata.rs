use super::constant_pool::{CpEntry, get_integer, get_utf8};
use super::jvm_type::{read_u8, read_u16, read_u32};

pub(crate) struct ClassAttributes<'a> {
    pub(crate) signature: Option<&'a str>,
    pub(crate) metadata_k: Option<u32>,
}

pub const KOTLIN_KIND_CLASS: u32 = 1;
pub const KOTLIN_KIND_FILE_FACADE: u32 = 2;
pub const KOTLIN_KIND_SYNTHETIC_CLASS: u32 = 3;
pub const KOTLIN_KIND_MULTIFILE_FACADE: u32 = 4;
pub const KOTLIN_KIND_MULTIFILE_PART: u32 = 5;

pub fn is_kotlin_file_facade(metadata_k: Option<u32>) -> bool {
    matches!(
        metadata_k,
        Some(KOTLIN_KIND_FILE_FACADE | KOTLIN_KIND_MULTIFILE_FACADE)
    )
}

pub fn is_skippable_class_file(metadata_k: Option<u32>) -> bool {
    matches!(
        metadata_k,
        Some(KOTLIN_KIND_SYNTHETIC_CLASS | KOTLIN_KIND_MULTIFILE_PART)
    )
}

fn skip_element_value(bytes: &[u8], cur: &mut usize) -> Option<()> {
    let tag = read_u8(bytes, cur)?;
    match tag {
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' | b'c' => {
            *cur += 2;
        }
        b'e' => {
            *cur += 4;
        }
        b'@' => {
            skip_annotation(bytes, cur)?;
        }
        b'[' => {
            let num_values = read_u16(bytes, cur)? as usize;
            for _ in 0..num_values {
                skip_element_value(bytes, cur)?;
            }
        }
        _ => return None,
    }
    if *cur > bytes.len() {
        return None;
    }
    Some(())
}

fn skip_annotation(bytes: &[u8], cur: &mut usize) -> Option<()> {
    read_u16(bytes, cur)?;
    let num_pairs = read_u16(bytes, cur)? as usize;
    for _ in 0..num_pairs {
        read_u16(bytes, cur)?;
        skip_element_value(bytes, cur)?;
    }
    Some(())
}

fn parse_kotlin_metadata_kind(bytes: &[u8], cp: &[CpEntry]) -> Option<u32> {
    let mut cur = 0;
    let num_annotations = read_u16(bytes, &mut cur)? as usize;
    for _ in 0..num_annotations {
        let type_idx = read_u16(bytes, &mut cur)? as usize;
        let is_kotlin_metadata = get_utf8(cp, type_idx) == Some("Lkotlin/Metadata;");
        let num_pairs = read_u16(bytes, &mut cur)? as usize;

        for _ in 0..num_pairs {
            let name_idx = read_u16(bytes, &mut cur)? as usize;
            let is_k_field = is_kotlin_metadata && get_utf8(cp, name_idx) == Some("k");

            if is_k_field {
                let tag = read_u8(bytes, &mut cur)?;
                if tag == b'I' {
                    let const_idx = read_u16(bytes, &mut cur)? as usize;
                    if let Some(val) = get_integer(cp, const_idx) {
                        return Some(val as u32);
                    }
                }
                return None;
            }
            skip_element_value(bytes, &mut cur)?;
        }
    }
    None
}

pub(crate) fn read_class_attributes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    cp: &'a [CpEntry],
    has_kotlin_metadata: bool,
) -> ClassAttributes<'a> {
    let Some(attr_count) = read_u16(bytes, cursor) else {
        return ClassAttributes {
            signature: None,
            metadata_k: None,
        };
    };
    let mut signature = None;
    let mut metadata_k = None;

    for _ in 0..attr_count {
        let Some(name_idx) = read_u16(bytes, cursor) else {
            break;
        };
        let Some(length) = read_u32(bytes, cursor) else {
            break;
        };
        let length = length as usize;
        if *cursor + length > bytes.len() {
            break;
        }
        match get_utf8(cp, name_idx as usize) {
            Some("Signature") if length == 2 => {
                let mut sig_cur = *cursor;
                if let Some(sig_idx) = read_u16(bytes, &mut sig_cur)
                    && let Some(s) = get_utf8(cp, sig_idx as usize)
                {
                    signature = Some(s);
                }
            }
            Some("RuntimeVisibleAnnotations") if has_kotlin_metadata => {
                let attr_bytes = &bytes[*cursor..*cursor + length];
                if let Some(k) = parse_kotlin_metadata_kind(attr_bytes, cp) {
                    metadata_k = Some(k);
                    if is_skippable_class_file(metadata_k) {
                        break;
                    }
                }
            }
            _ => {}
        }
        *cursor += length;
    }

    ClassAttributes {
        signature,
        metadata_k,
    }
}
