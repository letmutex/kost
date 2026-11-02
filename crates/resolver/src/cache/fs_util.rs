use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn is_safe_component(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && !component.contains('/')
        && !component.contains('\\')
        && !component.contains(':')
        && !component.contains('*')
        && !component.contains('?')
        && !component.contains('"')
        && !component.contains('<')
        && !component.contains('>')
        && !component.contains('|')
        && !component.contains("..")
        && !component.contains('\0')
}

pub(super) fn write_cache_atomic(
    target_dir: &Path,
    final_path: &Path,
    file_name: &str,
    data: &[u8],
) -> Result<(), String> {
    if final_path.is_file() {
        let _ = fs::remove_file(final_path);
    }

    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = common::concat_str!(
        ".tmp_",
        &std::process::id().to_string(),
        "_",
        &seq.to_string(),
        "_",
        file_name
    );
    let temp_path = target_dir.join(&temp_name);

    fs::write(&temp_path, data)
        .map_err(|err| common::concat_str!("Failed to write temporary file: ", &err.to_string()))?;

    if let Err(err) = fs::rename(&temp_path, final_path) {
        let _ = fs::remove_file(&temp_path);
        let valid = final_path.is_file()
            && fs::metadata(final_path).map(|m| m.len()).unwrap_or(0) == data.len() as u64;
        if !valid {
            return Err(common::concat_str!(
                "Failed to finalize cache artifact: ",
                &err.to_string()
            ));
        }
    }

    Ok(())
}

pub fn hex_digest(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX_CHARS[(byte >> 4) as usize] as char);
        output.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub fn dirs_home() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(user_profile));
        }
        if let Ok(home_path) = std::env::var("HOME") {
            return Some(PathBuf::from(home_path));
        }
    } else {
        if let Ok(home_path) = std::env::var("HOME") {
            return Some(PathBuf::from(home_path));
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(user_profile));
        }
    }
    None
}
