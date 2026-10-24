use update::archive::{extract_archive, find_binary_in_dir};
use update::checksum::{compute_sha256, parse_expected_sha256, verify_file_checksum};
use update::system::TempDir;

#[test]
fn test_temp_dir_cleans_up_on_drop() {
    let temp_dir_path;
    {
        let temp_dir = TempDir::new("test_cleanup").expect("Failed to create temp dir");
        temp_dir_path = temp_dir.path().to_path_buf();
        assert!(temp_dir_path.exists());

        let test_file = temp_dir_path.join("test.txt");
        std::fs::write(&test_file, b"content").expect("Failed to write test file");
        assert!(test_file.exists());
    }

    assert!(!temp_dir_path.exists());
}

#[test]
fn test_tar_gz_decompression_and_binary_extraction() {
    let temp_dir = TempDir::new("test_tar").expect("Failed to create temp dir");
    let staging_dir = temp_dir.path().join("staging");
    let output_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&staging_dir).expect("Failed to create staging dir");
    std::fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let dummy_binary = staging_dir.join("kost");
    std::fs::write(&dummy_binary, b"binary contents").expect("Failed to write dummy binary");

    let archive_path = temp_dir.path().join("kost-archive.tar.gz");
    let tar_creation = std::process::Command::new("tar")
        .args(["-czf"])
        .arg(&archive_path)
        .args(["-C"])
        .arg(&staging_dir)
        .arg("kost")
        .status()
        .expect("tar command must be present and executable");

    assert!(tar_creation.success(), "tar creation failed");

    let extract_result = extract_archive(&archive_path, &output_dir);
    assert!(extract_result.is_ok(), "failed to extract tar archive");

    let found_binary = find_binary_in_dir(&output_dir, "kost").expect("binary not found");
    let content = std::fs::read(found_binary).expect("Failed to read extracted file");
    assert_eq!(content, b"binary contents");
}

#[test]
fn test_find_binary_resolves_nested_executable() {
    let temp_dir = TempDir::new("test_find").expect("Failed to create temp dir");
    let nested_subdir = temp_dir.path().join("nested_folder");
    std::fs::create_dir_all(&nested_subdir).expect("Failed to create nested folder");
    let nested_bin = nested_subdir.join("kost");
    std::fs::write(&nested_bin, b"nested").expect("Failed to write nested");
    assert_eq!(
        find_binary_in_dir(temp_dir.path(), "kost").unwrap(),
        nested_bin
    );
}

#[test]
fn test_compute_sha256_verifies_file_content() {
    let temp_dir = TempDir::new("test_hash").expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("sample.txt");
    std::fs::write(&test_file, b"kost test\n").expect("Failed to write sample file");

    let computed_hash = compute_sha256(&test_file).expect("Failed to compute SHA-256");
    assert_eq!(
        computed_hash,
        "e9a4ca419a779254107a1a1237924db7e3f85de38e6194f0d65340b768d0513d"
    );
}

#[test]
fn test_parse_expected_sha256_standard_format() {
    let sample_checksum_content = "3476eccb6c5a7477f05581902fcc07e56938a7715531cd735c43d7eed02df3a7  kost-v0.1.0-macos-arm64.tar.gz\n";
    let parsed_hash = parse_expected_sha256(sample_checksum_content).expect("Failed to parse hash");
    assert_eq!(
        parsed_hash,
        "3476eccb6c5a7477f05581902fcc07e56938a7715531cd735c43d7eed02df3a7"
    );

    let temp_dir = TempDir::new("test_verify").expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("file.bin");
    std::fs::write(&test_file, b"kost test\n").expect("Failed to write file");
    let verify_result = verify_file_checksum(
        &test_file,
        "e9a4ca419a779254107a1a1237924db7e3f85de38e6194f0d65340b768d0513d",
    );
    assert!(verify_result.is_ok());
}
