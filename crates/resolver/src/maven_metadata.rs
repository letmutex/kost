use common::concat_str;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use smol_str::SmolStr;

use crate::version::ArtifactVersion;

/// Parsed `maven-metadata.xml` containing available versions for an artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MavenMetadata {
    pub group_id: SmolStr,
    pub artifact_id: SmolStr,
    pub release: Option<ArtifactVersion>,
    pub latest: Option<ArtifactVersion>,
    pub versions: Vec<ArtifactVersion>,
}

impl MavenMetadata {
    /// Select the best version from this metadata:
    /// 1. Highest stable release version from `versions` and `release`.
    /// 2. If no stable release exists, highest pre-release version from `versions` and `latest`.
    pub fn select_best_version(&self) -> Option<ArtifactVersion> {
        let mut all_versions: Vec<&ArtifactVersion> = self.versions.iter().collect();
        if let Some(ref rel) = self.release
            && !all_versions.contains(&rel)
        {
            all_versions.push(rel);
        }
        if let Some(ref lat) = self.latest
            && !all_versions.contains(&lat)
        {
            all_versions.push(lat);
        }

        crate::version::select_best_version(all_versions)
    }
}

/// Parse `maven-metadata.xml` content into `MavenMetadata`.
pub fn parse_maven_metadata(xml_content: &str) -> Result<MavenMetadata, String> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut metadata = MavenMetadata::default();
    let mut path: Vec<SmolStr> = Vec::new();
    let mut current_text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref start_elem)) => {
                let qname = start_elem.name();
                let name_bytes = qname.as_ref();
                let local_bytes = name_bytes
                    .rsplit(|&byte| byte == b':')
                    .next()
                    .unwrap_or(name_bytes);
                let local_name = std::str::from_utf8(local_bytes).unwrap_or("");
                path.push(SmolStr::new(local_name));
                current_text.clear();
            }
            Ok(Event::Text(ref text_elem)) => {
                if let Ok(txt) = text_elem.unescape() {
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::CData(ref cdata_elem)) => {
                if let Ok(txt) = std::str::from_utf8(cdata_elem.as_ref()) {
                    current_text.push_str(txt);
                }
            }
            Ok(Event::End(ref end_elem)) => {
                apply_metadata_element(&path, &current_text, &mut metadata);
                let qname = end_elem.name();
                let name_bytes = qname.as_ref();
                let local_bytes = name_bytes
                    .rsplit(|&byte| byte == b':')
                    .next()
                    .unwrap_or(name_bytes);
                if path.last().is_some_and(|tag| tag.as_bytes() == local_bytes) {
                    path.pop();
                }
                current_text.clear();
            }
            Ok(Event::Empty(ref _empty_elem)) => {}
            Ok(Event::Eof) => break,
            Err(err) => return Err(concat_str!("XML parse error: ", &err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(metadata)
}

fn apply_metadata_element(path: &[SmolStr], text: &str, metadata: &mut MavenMetadata) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    match path {
        [root, group] if root == "metadata" && group == "groupId" => {
            metadata.group_id = SmolStr::new(trimmed);
        }
        [root, artifact] if root == "metadata" && artifact == "artifactId" => {
            metadata.artifact_id = SmolStr::new(trimmed);
        }
        [root, versioning, release]
            if root == "metadata" && versioning == "versioning" && release == "release" =>
        {
            metadata.release = Some(ArtifactVersion::parse(trimmed));
        }
        [root, versioning, latest]
            if root == "metadata" && versioning == "versioning" && latest == "latest" =>
        {
            metadata.latest = Some(ArtifactVersion::parse(trimmed));
        }
        [root, versioning, versions, version]
            if root == "metadata"
                && versioning == "versioning"
                && versions == "versions"
                && version == "version" =>
        {
            metadata.versions.push(ArtifactVersion::parse(trimmed));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_maven_metadata_full() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>org.jetbrains.kotlinx</groupId>
  <artifactId>kotlinx-coroutines-core</artifactId>
  <versioning>
    <latest>1.10.0-RC</latest>
    <release>1.9.0</release>
    <versions>
      <version>1.7.3</version>
      <version>1.8.0</version>
      <version>1.9.0</version>
      <version>1.10.0-RC</version>
    </versions>
    <lastUpdated>20250109153421</lastUpdated>
  </versioning>
</metadata>"#;

        let meta = parse_maven_metadata(xml).expect("parse metadata");
        assert_eq!(meta.group_id, "org.jetbrains.kotlinx");
        assert_eq!(meta.artifact_id, "kotlinx-coroutines-core");
        assert_eq!(meta.release.as_ref().map(|v| v.raw()), Some("1.9.0"));
        assert_eq!(meta.latest.as_ref().map(|v| v.raw()), Some("1.10.0-RC"));
        assert_eq!(meta.versions.len(), 4);

        // Highest stable version should be 1.9.0 (RC is pre-release)
        let best = meta.select_best_version().unwrap();
        assert_eq!(best.raw(), "1.9.0");
    }

    #[test]
    fn select_best_version_prerelease_fallback() {
        let xml = r#"<metadata>
  <groupId>com.example</groupId>
  <artifactId>new-lib</artifactId>
  <versioning>
    <latest>0.2.0-beta01</latest>
    <versions>
      <version>0.1.0-alpha01</version>
      <version>0.2.0-beta01</version>
    </versions>
  </versioning>
</metadata>"#;

        let meta = parse_maven_metadata(xml).expect("parse metadata");
        let best = meta.select_best_version().unwrap();
        assert_eq!(best.raw(), "0.2.0-beta01");
    }

    #[test]
    fn parse_maven_metadata_with_namespaces() {
        let xml = r#"<ns2:metadata xmlns:ns2="http://maven.apache.org/METADATA/1.1.0">
  <ns2:groupId>com.google.guava</ns2:groupId>
  <ns2:artifactId>guava</ns2:artifactId>
  <ns2:versioning>
    <ns2:release>33.0.0-jre</ns2:release>
    <ns2:versions>
      <ns2:version>32.1.3-jre</ns2:version>
      <ns2:version>33.0.0-jre</ns2:version>
    </ns2:versions>
  </ns2:versioning>
</ns2:metadata>"#;

        let meta = parse_maven_metadata(xml).expect("parse metadata");
        assert_eq!(meta.group_id, "com.google.guava");
        assert_eq!(meta.artifact_id, "guava");
        assert_eq!(meta.versions.len(), 2);
    }
}
