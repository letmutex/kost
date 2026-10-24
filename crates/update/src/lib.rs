//! Self-update command implementation for kost.

use cli::{UpdateArgs, render_update_help};
use common::concat_str;
use std::path::{Path, PathBuf};

pub mod archive;
pub mod checksum;
pub mod system;
pub mod uninstall;

pub use uninstall::run as uninstall;

const REPO_OWNER: &str = "letmutex";
const REPO_NAME: &str = "kost";

/// Executes the `update` command given CLI arguments and the current binary version.
pub fn run(args: &[String], current_version: &str) -> Result<(), String> {
    let parsed_args = UpdateArgs::parse(args).map_err(|err| err.to_string())?;

    if parsed_args.help {
        print!("{}", render_update_help());
        return Ok(());
    }

    if parsed_args.check && (parsed_args.version.is_some() || parsed_args.force) {
        return Err("Option '--check' cannot be used with '--version' or '--force'.".to_string());
    }

    if let Some(ref version_tag) = parsed_args.version
        && !is_valid_release_tag(version_tag)
    {
        return Err(concat_str!(
            "Invalid version tag '",
            version_tag,
            "'. Version tags must only contain alphanumeric characters, dots, hyphens, and underscores."
        ));
    }

    if parsed_args.check {
        return run_check(current_version);
    }

    let target = resolve_target()?;

    run_update(
        current_version,
        target,
        parsed_args.version.as_deref(),
        parsed_args.force,
    )
}

/// Validates that a release tag contains only safe identifier characters.
fn is_valid_release_tag(tag: &str) -> bool {
    let clean = tag.strip_prefix('v').unwrap_or(tag);
    clean.starts_with(|ch: char| ch.is_ascii_alphanumeric())
        && clean.ends_with(|ch: char| ch.is_ascii_alphanumeric())
        && clean
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
}

/// Parses standard SemVer major, minor, patch numbers from a version string.
fn parse_semver_core(version_str: &str) -> Option<(u64, u64, u64)> {
    let clean = version_str.trim_start_matches('v');
    let core_part = clean.split(['-', '+']).next()?;
    let mut segments = core_part.split('.');
    let major = segments.next()?.parse::<u64>().ok()?;
    let minor = segments.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = segments.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Checks whether `latest_str` represents a newer SemVer version than `current_str`.
fn is_newer_version(latest_str: &str, current_str: &str) -> bool {
    let latest_nums = parse_semver_core(latest_str);
    let current_nums = parse_semver_core(current_str);

    match (latest_nums, current_nums) {
        (Some(latest), Some(current)) if latest > current => true,
        (Some(latest), Some(current)) if latest < current => false,
        (Some(latest), Some(current)) if latest == current => {
            let latest_is_prerelease = latest_str.contains('-');
            let current_is_prerelease = current_str.contains('-');
            !latest_is_prerelease && current_is_prerelease
        }
        _ => false,
    }
}

/// Resolves the release asset target identifier based on host OS and architecture.
fn resolve_target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("macos", "x86_64") => Ok("macos-x64"),
        ("macos", "aarch64") => Ok("macos-arm64"),
        ("windows", "x86_64") => Ok("windows-x64"),
        ("windows", "aarch64") => Ok("windows-arm64"),
        (os, arch) => Err(concat_str!(
            "Unsupported platform: ",
            os,
            " (",
            arch,
            "). Please build kost from source or install manually."
        )),
    }
}

/// Resolves the archive format extension for the target platform.
fn resolve_archive_ext(target: &str) -> &'static str {
    if target.starts_with("windows") {
        "zip"
    } else {
        "tar.gz"
    }
}

/// Resolves the executable binary name within the archive for the target platform.
fn resolve_binary_name(target: &str) -> &'static str {
    if target.starts_with("windows") {
        "kost.exe"
    } else {
        "kost"
    }
}

/// Normalizes a release tag by ensuring it has the standard 'v' prefix.
fn normalize_release_tag(tag: &str) -> String {
    if tag.starts_with('v') {
        tag.to_string()
    } else {
        concat_str!("v", tag)
    }
}

/// Builds the release asset filename for a given tag and target triple.
fn build_asset_filename(tag: &str, target: &str) -> String {
    let extension = resolve_archive_ext(target);
    concat_str!("kost-", tag, "-", target, ".", extension)
}

/// Builds the direct download URL for a release asset.
fn build_asset_download_url(tag: &str, asset_filename: &str) -> String {
    concat_str!(
        "https://github.com/",
        REPO_OWNER,
        "/",
        REPO_NAME,
        "/releases/download/",
        tag,
        "/",
        asset_filename
    )
}

fn run_check(current_version: &str) -> Result<(), String> {
    let normalized_current = current_version.trim_start_matches('v');
    println!(
        "{}",
        concat_str!(
            "Checking for updates (current: v",
            normalized_current,
            ")..."
        )
    );

    let latest_tag = system::fetch_latest_tag(REPO_OWNER, REPO_NAME)?;
    let normalized_latest = latest_tag.trim_start_matches('v');

    if is_newer_version(&latest_tag, current_version) {
        println!(
            "{}",
            concat_str!(
                "A new version of kost is available: v",
                normalized_current,
                " -> v",
                normalized_latest
            )
        );
        println!("Run 'kost update' to upgrade.");
    } else {
        println!(
            "{}",
            concat_str!("kost is already up to date (v", normalized_current, ").")
        );
    }

    Ok(())
}

fn download_and_verify_archive(
    target_tag: &str,
    target: &str,
    workspace: &Path,
) -> Result<PathBuf, String> {
    let asset_filename = build_asset_filename(target_tag, target);
    let download_url = build_asset_download_url(target_tag, &asset_filename);

    let checksum_filename = concat_str!(&asset_filename, ".sha256");
    let checksum_url = build_asset_download_url(target_tag, &checksum_filename);

    let checksum_content = system::fetch_url_content(&checksum_url)?;
    let expected_hash = checksum::parse_expected_sha256(&checksum_content)?;

    let archive_path = workspace.join(&asset_filename);
    system::download_asset(&download_url, &archive_path)?;

    checksum::verify_file_checksum(&archive_path, &expected_hash)?;
    Ok(archive_path)
}

fn install_extracted_binary(
    archive_path: &Path,
    target: &str,
    workspace: &Path,
) -> Result<(), String> {
    archive::extract_archive(archive_path, workspace)?;

    let binary_filename = resolve_binary_name(target);
    let extracted_binary = archive::find_binary_in_dir(workspace, binary_filename)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&extracted_binary) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            let _ = std::fs::set_permissions(&extracted_binary, permissions);
        }
    }

    self_replace::self_replace(&extracted_binary)
        .map_err(|err| concat_str!("Failed to replace binary: ", err.to_string()))?;

    Ok(())
}

fn run_update(
    current_version: &str,
    target: &str,
    target_version: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let normalized_current = current_version.trim_start_matches('v');

    if !force && let Some(tag) = target_version {
        let normalized_target = tag.trim_start_matches('v');
        if normalized_target == normalized_current {
            println!(
                "{}",
                concat_str!(
                    "kost is already at version v",
                    normalized_current,
                    " (use -f/--force to reinstall)."
                )
            );
            return Ok(());
        }
    }

    println!(
        "{}",
        concat_str!(
            "Checking for updates from ",
            REPO_OWNER,
            "/",
            REPO_NAME,
            "..."
        )
    );

    let target_tag = match target_version {
        Some(version_string) => normalize_release_tag(version_string),
        None => system::fetch_latest_tag(REPO_OWNER, REPO_NAME)?,
    };

    let normalized_target = target_tag.trim_start_matches('v');
    if !force && target_version.is_none() && !is_newer_version(&target_tag, current_version) {
        println!(
            "{}",
            concat_str!("kost is already up to date (v", normalized_current, ").")
        );
        return Ok(());
    }

    let temp_workspace = system::TempDir::new("kost_update")?;
    let archive_path = download_and_verify_archive(&target_tag, target, temp_workspace.path())?;
    install_extracted_binary(&archive_path, target, temp_workspace.path())?;

    println!(
        "{}",
        concat_str!("Successfully updated kost to v", normalized_target, ".")
    );
    Ok(())
}
