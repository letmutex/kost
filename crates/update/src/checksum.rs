//! SHA-256 checksum computation and verification using native system tools.

use common::concat_str;
use std::path::Path;
use std::process::Command;

/// Computes the SHA-256 hash of a file using system utilities (sha256sum -> shasum -> certutil -> powershell).
pub fn compute_sha256(file_path: &Path) -> Result<String, String> {
    if cfg!(unix) {
        if let Ok(hash) = run_unix_hasher("sha256sum", &[], file_path) {
            return Ok(hash);
        }
        if let Ok(hash) = run_unix_hasher("shasum", &["-a", "256"], file_path) {
            return Ok(hash);
        }
    }

    if cfg!(windows) {
        if let Ok(hash) = run_windows_certutil(file_path) {
            return Ok(hash);
        }
        if let Ok(hash) = run_windows_powershell(file_path) {
            return Ok(hash);
        }
    }

    Err(concat_str!(
        "Failed to compute SHA-256 for '",
        &file_path.to_string_lossy(),
        "'. Ensure 'sha256sum', 'shasum', or 'certutil' is installed."
    ))
}

fn run_unix_hasher(binary: &str, extra_args: &[&str], file_path: &Path) -> Result<String, ()> {
    let mut command = Command::new(binary);
    command.args(extra_args).arg(file_path);

    let output = command.output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    match stdout_str.split_whitespace().next() {
        Some(token) if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) => {
            Ok(token.to_ascii_lowercase())
        }
        _ => Err(()),
    }
}

fn run_windows_certutil(file_path: &Path) -> Result<String, ()> {
    let output = Command::new("certutil")
        .args(["-hashfile"])
        .arg(file_path)
        .arg("SHA256")
        .output()
        .map_err(|_| ())?;

    if !output.status.success() {
        return Err(());
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    for line in stdout_str.lines().skip(1) {
        let clean: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if clean.len() == 64 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(clean.to_ascii_lowercase());
        }
    }

    Err(())
}

fn run_windows_powershell(file_path: &Path) -> Result<String, ()> {
    let ps_cmd = "(Get-FileHash -LiteralPath $env:KOST_FILE -Algorithm SHA256).Hash";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_cmd])
        .env("KOST_FILE", file_path)
        .output()
        .map_err(|_| ())?;

    if !output.status.success() {
        return Err(());
    }

    let hash_str = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();

    if hash_str.len() == 64 && hash_str.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hash_str)
    } else {
        Err(())
    }
}

/// Parses the expected 64-character hex hash from a `.sha256` checksum file.
pub fn parse_expected_sha256(checksum_content: &str) -> Result<String, String> {
    let first_token = checksum_content
        .split_whitespace()
        .next()
        .ok_or_else(|| "Checksum file is empty.".to_string())?;

    if first_token.len() == 64 && first_token.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(first_token.to_ascii_lowercase())
    } else {
        Err(concat_str!(
            "Invalid SHA-256 hash in checksum file: '",
            first_token,
            "'."
        ))
    }
}

/// Verifies that the file at `file_path` matches the expected hash.
pub fn verify_file_checksum(file_path: &Path, expected_hash: &str) -> Result<(), String> {
    let actual_hash = compute_sha256(file_path)?;
    if actual_hash.eq_ignore_ascii_case(expected_hash) {
        Ok(())
    } else {
        Err(concat_str!(
            "Integrity check failed: checksum mismatch (expected ",
            expected_hash,
            ", computed ",
            &actual_hash,
            "). The download may be incomplete or corrupted."
        ))
    }
}
