use super::jvm_type::{read_u8, read_u16, read_u32};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpEntry {
    Empty,
    Utf8(String),
    Class(usize),
    String,
    Integer(i32),
    Other,
}

pub(crate) fn get_utf8(cp: &[CpEntry], idx: usize) -> Option<&str> {
    match cp.get(idx) {
        Some(CpEntry::Utf8(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub fn get_integer(cp: &[CpEntry], idx: usize) -> Option<i32> {
    match cp.get(idx) {
        Some(CpEntry::Integer(val)) => Some(*val),
        _ => None,
    }
}

pub(crate) fn get_class_name(cp: &[CpEntry], idx: usize) -> Option<String> {
    match cp.get(idx) {
        Some(CpEntry::Class(name_idx)) => get_utf8(cp, *name_idx).map(|s| s.to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ParsedConstantPool {
    pub entries: Vec<CpEntry>,
    pub has_kotlin_metadata: bool,
    pub has_this_receiver: bool,
}

pub fn parse_constant_pool(
    bytes: &[u8],
    cursor: &mut usize,
    cp_count: usize,
) -> Option<ParsedConstantPool> {
    let mut pool = Vec::with_capacity(cp_count);
    pool.push(CpEntry::Empty);
    let mut idx = 1;
    let mut has_kotlin_metadata = false;
    let mut has_this_receiver = false;

    while idx < cp_count {
        let tag = read_u8(bytes, cursor)?;
        match tag {
            1 => {
                let len = read_u16(bytes, cursor)? as usize;
                if *cursor + len > bytes.len() {
                    return None;
                }
                let s = std::str::from_utf8(&bytes[*cursor..*cursor + len]).unwrap_or("");
                *cursor += len;
                if s == "Lkotlin/Metadata;" {
                    has_kotlin_metadata = true;
                } else if s.starts_with("$this") {
                    has_this_receiver = true;
                }
                pool.push(CpEntry::Utf8(s.to_string()));
                idx += 1;
            }
            7 => {
                let name_idx = read_u16(bytes, cursor)? as usize;
                pool.push(CpEntry::Class(name_idx));
                idx += 1;
            }
            8 => {
                let _ = read_u16(bytes, cursor)? as usize;
                pool.push(CpEntry::String);
                idx += 1;
            }
            3 => {
                let val = read_u32(bytes, cursor)? as i32;
                pool.push(CpEntry::Integer(val));
                idx += 1;
            }
            4 | 9 | 10 | 11 | 12 | 17 | 18 => {
                *cursor += 4;
                pool.push(CpEntry::Other);
                idx += 1;
            }
            5 | 6 => {
                *cursor += 8;
                pool.push(CpEntry::Other);
                pool.push(CpEntry::Empty);
                idx += 2;
            }
            15 => {
                *cursor += 3;
                pool.push(CpEntry::Other);
                idx += 1;
            }
            16 | 19 | 20 => {
                *cursor += 2;
                pool.push(CpEntry::Other);
                idx += 1;
            }
            _ => return None,
        }
    }
    Some(ParsedConstantPool {
        entries: pool,
        has_kotlin_metadata,
        has_this_receiver,
    })
}
