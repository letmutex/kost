//! Self-update command implementation for kost.

use cli::{UpdateArgs, render_update_help};
use common::concat_str;
use self_update::VersionStatus;
use self_update::backends::github::Update;

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

    let target = resolve_target()?;

    if parsed_args.check {
        return run_check(current_version, target);
    }

    run_update(
        current_version,
        target,
        parsed_args.version.as_deref(),
        parsed_args.force,
    )
}

/// Resolves the release asset target identifier based on host OS and architecture.
pub fn resolve_target() -> Result<&'static str, String> {
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

fn run_check(current_version: &str, target: &str) -> Result<(), String> {
    println!(
        "{}",
        concat_str!("Checking for updates (current: v", current_version, ")...")
    );

    let mut builder = Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("kost")
        .target(target)
        .show_output(false)
        .current_version(current_version);

    let updater = builder.build().map_err(|err| err.to_string())?;
    match updater
        .is_update_available()
        .map_err(|err| err.to_string())?
    {
        Some(release) => {
            println!(
                "{}",
                concat_str!(
                    "A new version of kost is available: v",
                    current_version,
                    " -> v",
                    release.version()
                )
            );
            println!("Run 'kost update' to upgrade.");
        }
        None => {
            println!(
                "{}",
                concat_str!("kost is already up to date (v", current_version, ").")
            );
        }
    }

    Ok(())
}

fn normalize_release_tag(tag: &str) -> String {
    if tag.starts_with('v') {
        tag.to_string()
    } else {
        concat_str!("v", tag)
    }
}

fn build_updater(
    current_version: &str,
    target: &str,
    target_version: Option<&str>,
    force: bool,
) -> Result<Update, String> {
    let mut builder = Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("kost")
        .target(target)
        .show_output(false)
        .show_download_progress(true)
        .no_confirm(true);

    if force && target_version.is_none() {
        builder.current_version("0.0.0");
    } else {
        builder.current_version(current_version);
    }

    let normalized_tag;
    if let Some(tag) = target_version {
        normalized_tag = normalize_release_tag(tag);
        builder.release_tag(&normalized_tag);
    }

    builder.build().map_err(|err| err.to_string())
}

fn report_update_status(status: VersionStatus) {
    match status {
        VersionStatus::Updated(version) => {
            println!(
                "{}",
                concat_str!(
                    "Successfully updated kost to v",
                    version.trim_start_matches('v'),
                    "."
                )
            );
        }
        VersionStatus::UpToDate(version) => {
            println!(
                "{}",
                concat_str!(
                    "kost is already up to date (v",
                    version.trim_start_matches('v'),
                    ")."
                )
            );
        }
        _ => {}
    }
}

fn run_update(
    current_version: &str,
    target: &str,
    target_version: Option<&str>,
    force: bool,
) -> Result<(), String> {
    if !force && let Some(tag) = target_version {
        let normalized_target = tag.trim_start_matches('v');
        let normalized_current = current_version.trim_start_matches('v');
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

    let updater = build_updater(current_version, target, target_version, force)?;
    let status = updater.update().map_err(|err| err.to_string())?;
    report_update_status(status);
    Ok(())
}
