use lsp::gradle::version::ArtifactVersion;

#[test]
fn test_semver_numerical_ordering() {
    let v1 = ArtifactVersion::parse("1.2.0");
    let v2 = ArtifactVersion::parse("1.10.0");
    let v3 = ArtifactVersion::parse("1.15.0");
    let v4 = ArtifactVersion::parse("2.0.0");

    assert!(v2 > v1);
    assert!(v3 > v2);
    assert!(v4 > v3);
}

#[test]
fn test_gradle_qualifiers_ordering() {
    let v_dev = ArtifactVersion::parse("1.0.0-dev");
    let v_alpha = ArtifactVersion::parse("1.0.0-alpha1");
    let v_beta = ArtifactVersion::parse("1.0.0-beta2");
    let v_rc = ArtifactVersion::parse("1.0.0-rc1");
    let v_final = ArtifactVersion::parse("1.0.0");
    let v_patch = ArtifactVersion::parse("1.0.1");

    assert!(v_alpha > v_dev);
    assert!(v_beta > v_alpha);
    assert!(v_rc > v_beta);
    assert!(v_final > v_rc);
    assert!(v_patch > v_final);
}

#[test]
fn test_android_sdk_platform_versions() {
    let v33 = ArtifactVersion::parse("android-33");
    let v34 = ArtifactVersion::parse("android-34");
    let v35 = ArtifactVersion::parse("android-35");

    assert!(v34 > v33);
    assert!(v35 > v34);
}
