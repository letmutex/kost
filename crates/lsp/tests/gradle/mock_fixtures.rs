use std::path::PathBuf;

pub struct TestFixtureEnvironment {
    pub root_dir: PathBuf,
    pub sdk_dir: PathBuf,
    pub gradle_home: PathBuf,
    pub project_dir: PathBuf,
}

impl TestFixtureEnvironment {
    pub fn setup() -> Self {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("gradle")
            .join("fixtures");
        Self {
            root_dir: fixtures_dir.clone(),
            sdk_dir: fixtures_dir.join("android_sdk"),
            gradle_home: fixtures_dir.join("gradle_cache"),
            project_dir: fixtures_dir.join("project"),
        }
    }
}
