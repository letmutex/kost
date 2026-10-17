//! Self-uninstall command implementation for kost.

use cli::{UninstallArgs, render_uninstall_help};
use common::concat_str;
use std::io::Write;
use std::path::Path;

/// Executes the `uninstall` command given CLI arguments.
pub fn run(args: &[String]) -> Result<(), String> {
    let parsed_args = UninstallArgs::parse(args).map_err(|err| err.to_string())?;

    if parsed_args.help {
        print!("{}", render_uninstall_help());
        return Ok(());
    }

    let current_exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let is_symlink = std::fs::symlink_metadata(&current_exe)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);
    let real_exe = current_exe
        .canonicalize()
        .unwrap_or_else(|_| current_exe.clone());

    let bin_dir = real_exe
        .parent()
        .ok_or_else(|| "Failed to resolve executable directory.".to_string())?;

    if !parsed_args.yes && !confirm_uninstallation(&current_exe, &real_exe, is_symlink)? {
        println!("Uninstallation canceled.");
        return Ok(());
    }

    self_replace::self_delete_outside_path(bin_dir)
        .map_err(|err| concat_str!("Failed to delete binary: ", err.to_string()))?;

    if is_symlink && let Err(err) = std::fs::remove_file(&current_exe) {
        eprintln!(
            "{}",
            concat_str!(
                "Warning: Failed to remove symlink at '",
                &current_exe.to_string_lossy(),
                "': ",
                err.to_string(),
                ". Please remove it manually."
            )
        );
    }

    for backup_name in &["kost.exe.old", "kost.old"] {
        let backup_path = bin_dir.join(backup_name);
        if backup_path.exists() {
            let _ = std::fs::remove_file(backup_path);
        }
    }

    let is_dedicated = is_dedicated_kost_dir(bin_dir);
    if is_dedicated {
        cleanup_dedicated_dirs(bin_dir);
    }

    println!(
        "{}",
        concat_str!(
            "Successfully uninstalled kost from ",
            &display_path(&real_exe),
            "."
        )
    );

    if is_dedicated {
        println!(
            "{}",
            concat_str!(
                "\nNotice: Remember to remove '",
                &display_path(bin_dir),
                "' from your PATH if it was added to your shell configuration."
            )
        );
    }

    Ok(())
}

fn confirm_uninstallation(
    current_exe: &Path,
    real_exe: &Path,
    is_symlink: bool,
) -> Result<bool, String> {
    if is_symlink {
        println!(
            "{}",
            concat_str!(
                "Found kost at: ",
                &display_path(current_exe),
                " -> ",
                &display_path(real_exe)
            )
        );
    } else {
        println!(
            "{}",
            concat_str!("Found kost at: ", &display_path(current_exe))
        );
    }

    print!("Are you sure you want to uninstall kost? [y/N]: ");
    std::io::stdout().flush().map_err(|err| err.to_string())?;

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|err| err.to_string())?;

    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

fn display_path(path: &Path) -> std::borrow::Cow<'_, str> {
    let lossy = path.to_string_lossy();
    if let Some(stripped) = lossy.strip_prefix(r"\\?\UNC\") {
        std::borrow::Cow::Owned(concat_str!(r"\\", stripped))
    } else if let Some(stripped) = lossy.strip_prefix(r"\\?\") {
        std::borrow::Cow::Owned(stripped.to_string())
    } else {
        lossy
    }
}

fn is_dedicated_kost_dir(bin_dir: &Path) -> bool {
    let is_bin = bin_dir.file_name().is_some_and(|name| name == "bin");
    let is_kost_parent = bin_dir
        .parent()
        .and_then(|parent_dir| parent_dir.file_name())
        .is_some_and(|name| name == ".kost" || name == "kost");
    is_bin && is_kost_parent
}

fn cleanup_dedicated_dirs(bin_dir: &Path) {
    let Some(kost_dir) = bin_dir.parent() else {
        return;
    };

    if std::fs::remove_dir(bin_dir).is_ok() {
        let _ = std::fs::remove_dir(kost_dir);
    }
}
