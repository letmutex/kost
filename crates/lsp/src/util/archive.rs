use std::fs::File;
use std::io::{BufReader, Read};
use url::Url;
use zip::ZipArchive;

/// Reads file content from a `jar:` URL directly in-memory.
///
/// Format: `jar:file:///path/to/archive.jar!/entry/path.kt`
pub fn read_jar_url_content(url_str: &str) -> Option<String> {
    let after_jar = url_str.strip_prefix("jar:")?;
    let mut parts = after_jar.split("!/");
    let file_url_str = parts.next()?;
    let entry_name = parts.next()?;

    let file_url = Url::parse(file_url_str).ok()?;
    let archive_path = file_url.to_file_path().ok()?;

    let file = File::open(&archive_path).ok()?;
    let mut zip = ZipArchive::new(BufReader::new(file)).ok()?;
    let mut entry = zip.by_name(entry_name).ok()?;
    let mut content = String::new();
    entry.read_to_string(&mut content).ok()?;
    Some(content)
}
