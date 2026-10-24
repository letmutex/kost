//! Native system tool execution for HTTP fetching and asset downloads.

use common::concat_str;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Lightweight RAII guard for managing a temporary directory.
pub struct TempDir {
    directory_path: PathBuf,
}

impl TempDir {
    /// Creates a uniquely named temporary directory under `std::env::temp_dir()`.
    pub fn new(prefix: &str) -> Result<Self, String> {
        let base_temp = std::env::temp_dir();
        let pid = std::process::id();
        let timestamp_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);

        for attempt in 0..10 {
            let folder_name = concat_str!(
                prefix,
                "_",
                &pid.to_string(),
                "_",
                &timestamp_nanos.to_string(),
                "_",
                &attempt.to_string()
            );
            let candidate_path = base_temp.join(folder_name);

            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = std::fs::DirBuilder::new();
                builder.mode(0o700);
                if builder.create(&candidate_path).is_ok() {
                    return Ok(Self {
                        directory_path: candidate_path,
                    });
                }
            }

            #[cfg(not(unix))]
            {
                if std::fs::create_dir(&candidate_path).is_ok() {
                    return Ok(Self {
                        directory_path: candidate_path,
                    });
                }
            }
        }

        Err("Failed to create a secure temporary directory after multiple attempts.".to_string())
    }

    /// Returns a reference to the temporary directory path.
    pub fn path(&self) -> &Path {
        &self.directory_path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory_path);
    }
}

/// Fetches the latest release tag for the given repository using system tools.
pub(crate) fn fetch_latest_tag(repo_owner: &str, repo_name: &str) -> Result<String, String> {
    let redirect_url = concat_str!(
        "https://github.com/",
        repo_owner,
        "/",
        repo_name,
        "/releases/latest"
    );

    let dev_null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let redirect_result = Command::new("curl")
        .args([
            "-sIL",
            "--connect-timeout",
            "10",
            "--max-time",
            "15",
            "-o",
            dev_null,
            "-w",
            "%{url_effective}",
            &redirect_url,
        ])
        .output();

    if let Ok(output) = redirect_result
        && output.status.success()
    {
        let final_url = String::from_utf8_lossy(&output.stdout);
        if let Some(index) = final_url.rfind("/releases/tag/") {
            let raw_tag = final_url[index + "/releases/tag/".len()..].trim();
            let clean_tag = raw_tag
                .trim_matches('/')
                .split(['?', '#'])
                .next()
                .unwrap_or(raw_tag);
            if !clean_tag.is_empty() {
                return Ok(clean_tag.to_string());
            }
        }
    }

    let api_url = concat_str!(
        "https://api.github.com/repos/",
        repo_owner,
        "/",
        repo_name,
        "/releases/latest"
    );
    let payload = fetch_url_content(&api_url)?;
    extract_tag_name_from_json(&payload)
}

/// Fetches text/JSON content from a URL using system tools (curl -> wget -> powershell).
pub(crate) fn fetch_url_content(url_string: &str) -> Result<String, String> {
    let curl_result = Command::new("curl")
        .args([
            "-sSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github.v3+json",
            "-H",
            "User-Agent: kost",
            url_string,
        ])
        .output();

    if let Ok(output) = curl_result
        && output.status.success()
    {
        return String::from_utf8(output.stdout)
            .map_err(|err| concat_str!("Invalid UTF-8 in response: ", err.to_string()));
    }

    if cfg!(unix) {
        let wget_result = Command::new("wget")
            .args([
                "-qO-",
                "--timeout=15",
                "--header=User-Agent: kost",
                url_string,
            ])
            .output();

        if let Ok(output) = wget_result
            && output.status.success()
        {
            return String::from_utf8(output.stdout)
                .map_err(|err| concat_str!("Invalid UTF-8 in response: ", err.to_string()));
        }
    }

    if cfg!(windows) {
        let ps_cmd = "(Invoke-WebRequest -UseBasicParsing -TimeoutSec 15 -Uri $env:KOST_URL -Headers @{'User-Agent'='kost'}).Content";
        let ps_result = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_cmd])
            .env("KOST_URL", url_string)
            .output();

        if let Ok(output) = ps_result
            && output.status.success()
        {
            return String::from_utf8(output.stdout)
                .map_err(|err| concat_str!("Invalid UTF-8 in response: ", err.to_string()));
        }
    }

    Err(concat_str!(
        "Failed to retrieve release information from '",
        url_string,
        "'. Please check your network connection or verify 'curl' is installed."
    ))
}

/// Parses the release tag name from a GitHub release JSON response.
fn extract_tag_name_from_json(json_content: &str) -> Result<String, String> {
    let parsed_json: serde_json::Value = serde_json::from_str(json_content)
        .map_err(|err| concat_str!("Failed to parse release JSON payload: ", err.to_string()))?;

    if let Some(error_message) = parsed_json.get("message").and_then(|v| v.as_str()) {
        return Err(concat_str!("GitHub API error: ", error_message));
    }

    parsed_json
        .get("tag_name")
        .and_then(|tag_value| tag_value.as_str())
        .map(|tag_string| tag_string.to_string())
        .ok_or_else(|| "Release payload did not contain a valid 'tag_name' field.".to_string())
}

/// Downloads a remote file to a local destination using system tools.
pub(crate) fn download_asset(source_url: &str, destination_path: &Path) -> Result<(), String> {
    let curl_status = Command::new("curl")
        .args(["-fL", "--connect-timeout", "15", "--progress-bar", "-o"])
        .arg(destination_path)
        .arg(source_url)
        .status();

    if let Ok(status) = curl_status
        && status.success()
    {
        return Ok(());
    }

    if cfg!(unix) {
        let wget_status = Command::new("wget")
            .args(["-q", "--timeout=30", "--show-progress", "-O"])
            .arg(destination_path)
            .arg(source_url)
            .status();

        if let Ok(status) = wget_status
            && status.success()
        {
            return Ok(());
        }
    }

    if cfg!(windows) {
        let ps_cmd = "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri $env:KOST_URL -OutFile $env:KOST_DEST";
        let ps_status = Command::new("powershell")
            .args(["-NoProfile", "-Command", ps_cmd])
            .env("KOST_URL", source_url)
            .env("KOST_DEST", destination_path)
            .status();

        if let Ok(status) = ps_status
            && status.success()
        {
            return Ok(());
        }
    }

    Err(concat_str!(
        "Failed to download '",
        source_url,
        "'. Ensure 'curl', 'wget', or 'powershell' is available."
    ))
}
