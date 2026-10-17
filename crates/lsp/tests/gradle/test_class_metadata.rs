use lsp::gradle::class_metadata::{
    KOTLIN_KIND_CLASS, KOTLIN_KIND_FILE_FACADE, KOTLIN_KIND_MULTIFILE_FACADE,
    KOTLIN_KIND_MULTIFILE_PART, KOTLIN_KIND_SYNTHETIC_CLASS, is_kotlin_file_facade,
    is_skippable_class_file,
};
use lsp::gradle::constant_pool::{CpEntry, get_integer, parse_constant_pool};

#[test]
fn test_class_metadata_skippable_kinds() {
    assert!(!is_skippable_class_file(Some(KOTLIN_KIND_CLASS)));
    assert!(!is_skippable_class_file(Some(KOTLIN_KIND_FILE_FACADE)));
    assert!(is_skippable_class_file(Some(KOTLIN_KIND_SYNTHETIC_CLASS)));
    assert!(!is_skippable_class_file(Some(KOTLIN_KIND_MULTIFILE_FACADE)));
    assert!(is_skippable_class_file(Some(KOTLIN_KIND_MULTIFILE_PART)));
    assert!(!is_skippable_class_file(None));

    assert!(is_kotlin_file_facade(Some(KOTLIN_KIND_FILE_FACADE)));
    assert!(is_kotlin_file_facade(Some(KOTLIN_KIND_MULTIFILE_FACADE)));
    assert!(!is_kotlin_file_facade(Some(KOTLIN_KIND_CLASS)));
    assert!(!is_kotlin_file_facade(Some(KOTLIN_KIND_MULTIFILE_PART)));
}

#[test]
fn test_constant_pool_get_integer_boundaries() {
    let pool = vec![
        CpEntry::Other,
        CpEntry::Integer(42),
        CpEntry::Integer(-100),
        CpEntry::String,
    ];

    assert_eq!(get_integer(&pool, 1), Some(42));
    assert_eq!(get_integer(&pool, 2), Some(-100));
    assert_eq!(get_integer(&pool, 3), None);
    assert_eq!(get_integer(&pool, 0), None);
    assert_eq!(get_integer(&pool, 99), None);
}

#[test]
fn test_constant_pool_flags_distinguish_kotlin_metadata_and_receivers() {
    let parse_pool = |bytes: &[u8]| {
        let mut cursor = 8;
        let cp_count = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
        cursor += 2;
        parse_constant_pool(bytes, &mut cursor, cp_count).expect("valid pool")
    };

    let remember_pool = parse_pool(include_bytes!("resources/RememberKt.class"));
    assert!(remember_pool.has_kotlin_metadata);

    let service_pool = parse_pool(include_bytes!("resources/MyService.class"));
    assert!(service_pool.has_kotlin_metadata);
    assert!(!service_pool.has_this_receiver);

    let stripped_metadata_pool =
        parse_pool(include_bytes!("resources/AndroidExternalSurfaceKt.class"));
    assert!(!stripped_metadata_pool.has_kotlin_metadata);
    assert!(stripped_metadata_pool.has_this_receiver);

    let calculator_pool = parse_pool(include_bytes!("resources/Calculator.class"));
    assert!(!calculator_pool.has_kotlin_metadata);
    assert!(!calculator_pool.has_this_receiver);
}
