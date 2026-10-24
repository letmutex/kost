//! Archive decompression and binary resolution using native system tools.

use common::concat_str;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Decompresses an archive (`.tar.gz` or `.zip`) into the destination directory using system tools.
pub fn extract_archive(archive_path: &Path, destination_dir: &Path) -> Result<(), String> {
    let archive_name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let is_zip_format = archive_name.ends_with(".zip");

    if is_zip_format {
        extract_zip_archive(archive_path, destination_dir)
    } else {
        extract_tar_archive(archive_path, destination_dir)
    }
}

/// Extracts a `.tar.gz` archive using system `tar`.
fn extract_tar_archive(archive_path: &Path, destination_dir: &Path) -> Result<(), String> {
    let tar_status = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(destination_dir)
        .status();

    if let Ok(status) = tar_status
        && status.success()
    {
        return Ok(());
    }

    Err(concat_str!(
        "Failed to extract tar archive '",
        &archive_path.to_string_lossy(),
        "'. Ensure 'tar' is installed."
    ))
}

/// Extracts a `.zip` archive using `tar` (built-in on Win10+ & Unix), falling back to PowerShell or unzip.
fn extract_zip_archive(archive_path: &Path, destination_dir: &Path) -> Result<(), String> {
    let tar_status = Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(destination_dir)
        .status();

    if let Ok(status) = tar_status
        && status.success()
    {
        return Ok(());
    }

    if cfg!(windows) {
        let ps_cmd =
            "Expand-Archive -LiteralPath $env:KOST_ARCHIVE -DestinationPath $env:KOST_DEST -Force";
        let ps_status = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_cmd])
            .env("KOST_ARCHIVE", archive_path)
            .env("KOST_DEST", destination_dir)
            .status();

        if let Ok(status) = ps_status
            && status.success()
        {
            return Ok(());
        }
    }

    if cfg!(unix) {
        let unzip_status = Command::new("unzip")
            .arg("-q")
            .arg(archive_path)
            .arg("-d")
            .arg(destination_dir)
            .status();

        if let Ok(status) = unzip_status
            && status.success()
        {
            return Ok(());
        }
    }

    Err(concat_str!(
        "Failed to extract zip archive '",
        &archive_path.to_string_lossy(),
        "'. Ensure 'tar', 'unzip', or 'powershell' is available."
    ))
}

/// Searches for the named binary file inside the directory and its immediate subdirectories.
pub fn find_binary_in_dir(directory_path: &Path, binary_filename: &str) -> Result<PathBuf, String> {
    let direct_binary_path = directory_path.join(binary_filename);
    if direct_binary_path.is_file() {
        return Ok(direct_binary_path);
    }

    if let Ok(dir_entries) = std::fs::read_dir(directory_path) {
        for entry in dir_entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let nested_binary_path = entry_path.join(binary_filename);
                if nested_binary_path.is_file() {
                    return Ok(nested_binary_path);
                }
            }
        }
    }

    Err(concat_str!(
        "Extracted archive did not contain expected binary file '",
        binary_filename,
        "'."
    ))
}
