use super::mock_fixtures::TestFixtureEnvironment;
use lsp::gradle::android_sdk::AndroidSdkResolver;
use lsp::gradle::config::GradleConfig;

#[test]
fn test_android_sdk_resolution_highest_platform() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new().with_android_sdk_root(&env.sdk_dir);
    let sdk_root = AndroidSdkResolver::find_sdk_root(None, &config)
        .expect("Should resolve sdk root from config");

    // Resolve without preference -> should pick highest platform (android-35)
    let platform = AndroidSdkResolver::resolve_platform(&sdk_root, None)
        .expect("Should resolve highest Android SDK platform");

    assert_eq!(platform.api_level, 35);
    assert_eq!(platform.platform_name, "android-35");
    assert!(platform.android_jar.is_file());
}

#[test]
fn test_android_sdk_resolution_with_compile_sdk_preference() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new().with_android_sdk_root(&env.sdk_dir);
    let sdk_root = AndroidSdkResolver::find_sdk_root(None, &config)
        .expect("Should resolve sdk root from config");

    // Resolve with compileSdk = 34 -> should pick exact match (android-34)
    let platform = AndroidSdkResolver::resolve_platform(&sdk_root, Some(34))
        .expect("Should resolve Android SDK platform 34");

    assert_eq!(platform.api_level, 34);
    assert_eq!(platform.platform_name, "android-34");
}

#[test]
fn test_find_sdk_root_from_local_properties() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new();

    let root = AndroidSdkResolver::find_sdk_root(Some(&env.project_dir), &config)
        .expect("Should find SDK root from local.properties");

    assert_eq!(
        root.canonicalize().unwrap(),
        env.sdk_dir.canonicalize().unwrap()
    );
}
