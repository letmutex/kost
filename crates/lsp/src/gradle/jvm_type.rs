use crate::inference::model::PrimitiveType;

pub fn descriptor_to_type(desc: &str) -> String {
    match desc {
        "V" => PrimitiveType::Unit.as_str().to_string(),
        "Z" => PrimitiveType::Boolean.as_str().to_string(),
        "B" => PrimitiveType::Byte.as_str().to_string(),
        "C" => PrimitiveType::Char.as_str().to_string(),
        "S" => PrimitiveType::Short.as_str().to_string(),
        "I" => PrimitiveType::Int.as_str().to_string(),
        "J" => PrimitiveType::Long.as_str().to_string(),
        "F" => PrimitiveType::Float.as_str().to_string(),
        "D" => PrimitiveType::Double.as_str().to_string(),
        _ if desc.starts_with('L') && desc.ends_with(';') => {
            let inner = &desc[1..desc.len() - 1];
            inner.rsplit('/').next().unwrap_or(inner).to_string()
        }
        _ if desc.starts_with('[') => {
            common::concat_str!("Array<", descriptor_to_type(&desc[1..]), ">")
        }
        _ => desc.to_string(),
    }
}

pub use super::jvm_signature::{
    JvmClassSignature, JvmMethodSignature, jvm_method_sig_to_kotlin, parse_jvm_class_signature,
    parse_jvm_method_signature, parse_jvm_type_signature,
};

pub fn method_descriptor_to_sig(desc: &str) -> String {
    let (Some(open), Some(close)) = (desc.find('('), desc.find(')')) else {
        return "()".to_string();
    };

    let params_str = &desc[open + 1..close];
    let return_type_str = &desc[close + 1..];

    let mut params = Vec::new();
    let mut idx = 0;
    let mut p_num = 1;
    let bytes = params_str.as_bytes();

    while idx < bytes.len() {
        let start = idx;
        while idx < bytes.len() && bytes[idx] == b'[' {
            idx += 1;
        }
        if idx < bytes.len() {
            if bytes[idx] == b'L' {
                if let Some(semi) = params_str[idx..].find(';') {
                    idx += semi + 1;
                } else {
                    idx += 1;
                }
            } else {
                idx += 1;
            }
        }
        let p_type = descriptor_to_type(&params_str[start..idx]);
        params.push(common::concat_str!("arg", p_num.to_string(), ": ", p_type));
        p_num += 1;
    }

    let ret = descriptor_to_type(return_type_str);
    if ret == PrimitiveType::Unit.as_str() {
        common::concat_str!("(", params.join(", "), ")")
    } else {
        common::concat_str!("(", params.join(", "), "): ", ret)
    }
}

fn primitive_from_desc_tag(tag: u8) -> Option<PrimitiveType> {
    match tag {
        b'Z' => Some(PrimitiveType::Boolean),
        b'B' => Some(PrimitiveType::Byte),
        b'C' => Some(PrimitiveType::Char),
        b'S' => Some(PrimitiveType::Short),
        b'I' => Some(PrimitiveType::Int),
        b'J' => Some(PrimitiveType::Long),
        b'F' => Some(PrimitiveType::Float),
        b'D' => Some(PrimitiveType::Double),
        _ => None,
    }
}

pub fn extract_extension_info_from_desc(desc: &str) -> Option<(String, String)> {
    let open = desc.find('(')?;
    let close = desc.find(')')?;
    let params_str = &desc[open + 1..close];
    let (receiver_qualified, receiver_simple, rem_params_str) = if params_str.starts_with('L') {
        let semi = params_str.find(';')?;
        let receiver_internal = &params_str[1..semi];
        let receiver_qualified = receiver_internal.replace('/', ".");
        let receiver_simple = receiver_internal
            .rsplit('/')
            .next()
            .unwrap_or(receiver_internal)
            .to_string();
        (receiver_qualified, receiver_simple, &params_str[semi + 1..])
    } else if let Some(first_byte) = params_str.as_bytes().first()
        && let Some(primitive) = primitive_from_desc_tag(*first_byte)
    {
        let simple = primitive.as_str().to_string();
        let qualified = common::concat_str!("kotlin.", primitive.as_str());
        (qualified, simple, &params_str[1..])
    } else {
        return None;
    };

    let mut params = Vec::new();
    let mut idx = 0;
    let mut p_num = 1;
    let bytes = rem_params_str.as_bytes();
    while idx < bytes.len() {
        let start = idx;
        while idx < bytes.len() && bytes[idx] == b'[' {
            idx += 1;
        }
        if idx < bytes.len() {
            if bytes[idx] == b'L' {
                if let Some(s) = rem_params_str[idx..].find(';') {
                    idx += s + 1;
                } else {
                    idx += 1;
                }
            } else {
                idx += 1;
            }
        }
        let p_type = descriptor_to_type(&rem_params_str[start..idx]);
        params.push(common::concat_str!("arg", p_num.to_string(), ": ", p_type));
        p_num += 1;
    }
    let ret = descriptor_to_type(&desc[close + 1..]);
    let sig_suffix = if ret == PrimitiveType::Unit.as_str() {
        common::concat_str!("(", params.join(", "), ")")
    } else {
        common::concat_str!("(", params.join(", "), "): ", ret)
    };
    let signature = common::concat_str!(receiver_simple, ".", sig_suffix);
    Some((receiver_qualified, signature))
}

pub fn read_u8(bytes: &[u8], cursor: &mut usize) -> Option<u8> {
    if *cursor < bytes.len() {
        let val = bytes[*cursor];
        *cursor += 1;
        Some(val)
    } else {
        None
    }
}

pub fn read_u16(bytes: &[u8], cursor: &mut usize) -> Option<u16> {
    if *cursor + 1 < bytes.len() {
        let val = u16::from_be_bytes([bytes[*cursor], bytes[*cursor + 1]]);
        *cursor += 2;
        Some(val)
    } else {
        None
    }
}

pub fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    if *cursor + 3 < bytes.len() {
        let val = u32::from_be_bytes([
            bytes[*cursor],
            bytes[*cursor + 1],
            bytes[*cursor + 2],
            bytes[*cursor + 3],
        ]);
        *cursor += 4;
        Some(val)
    } else {
        None
    }
}

pub(crate) use super::constant_pool::{CpEntry, get_utf8};

fn check_local_variable_table(bytes: &[u8], mut cur: usize, cp: &[CpEntry]) -> Option<bool> {
    let lvt_len = read_u16(bytes, &mut cur)? as usize;
    for _ in 0..lvt_len {
        let _start_pc = read_u16(bytes, &mut cur)?;
        let _len = read_u16(bytes, &mut cur)?;
        let name_idx = read_u16(bytes, &mut cur)? as usize;
        let _desc_idx = read_u16(bytes, &mut cur)?;
        let index = read_u16(bytes, &mut cur)?;
        if index == 0
            && let Some(name) = get_utf8(cp, name_idx)
        {
            return Some(name.starts_with("$this"));
        }
    }
    None
}

fn check_code_attr(bytes: &[u8], mut cur: usize, cp: &[CpEntry]) -> Option<bool> {
    let _max_stack = read_u16(bytes, &mut cur)?;
    let _max_locals = read_u16(bytes, &mut cur)?;
    let code_len = read_u32(bytes, &mut cur)? as usize;
    cur += code_len;
    let exception_count = read_u16(bytes, &mut cur)? as usize;
    cur += exception_count * 8;
    let attr_count = read_u16(bytes, &mut cur)? as usize;
    for _ in 0..attr_count {
        let name_idx = read_u16(bytes, &mut cur)? as usize;
        let length = read_u32(bytes, &mut cur)? as usize;
        let attr_start = cur;
        cur += length;
        if get_utf8(cp, name_idx) == Some("LocalVariableTable")
            && let Some(is_ext) = check_local_variable_table(bytes, attr_start, cp)
        {
            return Some(is_ext);
        }
    }
    None
}

pub(crate) fn is_extension_method(bytes: &[u8], mut cursor: usize, cp: &[CpEntry]) -> bool {
    let Some(attr_count) = read_u16(bytes, &mut cursor) else {
        return false;
    };
    let mut code_attr_start = None;
    for _ in 0..attr_count {
        let Some(name_idx) = read_u16(bytes, &mut cursor) else {
            return false;
        };
        let Some(length) = read_u32(bytes, &mut cursor) else {
            return false;
        };
        let length = length as usize;
        if cursor + length > bytes.len() {
            return false;
        }
        let attr_start = cursor;
        cursor += length;

        if let Some(attr_name) = get_utf8(cp, name_idx as usize) {
            if attr_name == "MethodParameters" {
                let mut p_cur = attr_start;
                if let Some(num_params) = read_u8(bytes, &mut p_cur)
                    && num_params > 0
                    && let Some(p_name_idx) = read_u16(bytes, &mut p_cur)
                    && let Some(p_name) = get_utf8(cp, p_name_idx as usize)
                {
                    return p_name.starts_with("$this");
                }
                return false;
            } else if attr_name == "Code" {
                code_attr_start = Some(attr_start);
            }
        }
    }
    if let Some(code_start) = code_attr_start
        && let Some(is_ext) = check_code_attr(bytes, code_start, cp)
    {
        return is_ext;
    }
    false
}
