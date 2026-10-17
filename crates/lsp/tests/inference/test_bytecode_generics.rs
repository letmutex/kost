use lsp::gradle::class_file::parse_class_file_symbols;
use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

struct MockClassBuilder {
    cp_entries: Vec<Vec<u8>>,
    this_class: u16,
    super_class: u16,
    fields: Vec<Vec<u8>>,
    methods: Vec<Vec<u8>>,
    attributes: Vec<Vec<u8>>,
}

impl MockClassBuilder {
    fn new(class_name: &str, super_name: &str) -> Self {
        let mut builder = Self {
            cp_entries: Vec::new(),
            this_class: 0,
            super_class: 0,
            fields: Vec::new(),
            methods: Vec::new(),
            attributes: Vec::new(),
        };
        builder.this_class = builder.add_class(class_name);
        builder.super_class = builder.add_class(super_name);
        builder
    }

    fn add_utf8(&mut self, s: &str) -> u16 {
        let mut entry = Vec::new();
        entry.push(1); // CONSTANT_Utf8
        let bytes = s.as_bytes();
        let len = bytes.len() as u16;
        entry.extend_from_slice(&len.to_be_bytes());
        entry.extend_from_slice(bytes);
        self.cp_entries.push(entry);
        self.cp_entries.len() as u16
    }

    fn add_class(&mut self, name: &str) -> u16 {
        let name_idx = self.add_utf8(name);
        let mut entry = Vec::new();
        entry.push(7); // CONSTANT_Class
        entry.extend_from_slice(&name_idx.to_be_bytes());
        self.cp_entries.push(entry);
        self.cp_entries.len() as u16
    }

    fn add_field(&mut self, name: &str, desc: &str, sig: Option<&str>) {
        let name_idx = self.add_utf8(name);
        let desc_idx = self.add_utf8(desc);
        let mut field = Vec::new();
        field.extend_from_slice(&0x0001u16.to_be_bytes()); // ACC_PUBLIC
        field.extend_from_slice(&name_idx.to_be_bytes());
        field.extend_from_slice(&desc_idx.to_be_bytes());
        if let Some(sig_str) = sig {
            field.extend_from_slice(&1u16.to_be_bytes()); // attr_count = 1
            let sig_attr_name = self.add_utf8("Signature");
            let sig_idx = self.add_utf8(sig_str);
            field.extend_from_slice(&sig_attr_name.to_be_bytes());
            field.extend_from_slice(&2u32.to_be_bytes()); // length = 2
            field.extend_from_slice(&sig_idx.to_be_bytes());
        } else {
            field.extend_from_slice(&0u16.to_be_bytes()); // attr_count = 0
        }
        self.fields.push(field);
    }

    fn add_method(&mut self, name: &str, desc: &str, sig: Option<&str>, is_static: bool) {
        let name_idx = self.add_utf8(name);
        let desc_idx = self.add_utf8(desc);
        let flags: u16 = if is_static { 0x0009 } else { 0x0001 }; // ACC_PUBLIC | ACC_STATIC
        let mut method = Vec::new();
        method.extend_from_slice(&flags.to_be_bytes());
        method.extend_from_slice(&name_idx.to_be_bytes());
        method.extend_from_slice(&desc_idx.to_be_bytes());
        if let Some(sig_str) = sig {
            method.extend_from_slice(&1u16.to_be_bytes()); // attr_count = 1
            let sig_attr_name = self.add_utf8("Signature");
            let sig_idx = self.add_utf8(sig_str);
            method.extend_from_slice(&sig_attr_name.to_be_bytes());
            method.extend_from_slice(&2u32.to_be_bytes()); // length = 2
            method.extend_from_slice(&sig_idx.to_be_bytes());
        } else {
            method.extend_from_slice(&0u16.to_be_bytes()); // attr_count = 0
        }
        self.methods.push(method);
    }

    fn add_class_signature(&mut self, sig: &str) {
        let sig_attr_name = self.add_utf8("Signature");
        let sig_idx = self.add_utf8(sig);
        let mut attr = Vec::new();
        attr.extend_from_slice(&sig_attr_name.to_be_bytes());
        attr.extend_from_slice(&2u32.to_be_bytes());
        attr.extend_from_slice(&sig_idx.to_be_bytes());
        self.attributes.push(attr);
    }

    fn build(self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]); // Magic
        bytes.extend_from_slice(&0u16.to_be_bytes()); // Minor
        bytes.extend_from_slice(&52u16.to_be_bytes()); // Major

        let cp_count = (self.cp_entries.len() + 1) as u16;
        bytes.extend_from_slice(&cp_count.to_be_bytes());
        for entry in self.cp_entries {
            bytes.extend_from_slice(&entry);
        }

        bytes.extend_from_slice(&0x0021u16.to_be_bytes()); // ACC_PUBLIC | ACC_SUPER
        bytes.extend_from_slice(&self.this_class.to_be_bytes());
        bytes.extend_from_slice(&self.super_class.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes()); // interfaces_count = 0

        bytes.extend_from_slice(&(self.fields.len() as u16).to_be_bytes());
        for f in self.fields {
            bytes.extend_from_slice(&f);
        }

        bytes.extend_from_slice(&(self.methods.len() as u16).to_be_bytes());
        for m in self.methods {
            bytes.extend_from_slice(&m);
        }

        bytes.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());
        for a in self.attributes {
            bytes.extend_from_slice(&a);
        }

        bytes
    }
}

#[test]
fn test_bytecode_generic_field_preserves_type_argument() {
    let mut builder = MockClassBuilder::new("com/example/Container", "java/lang/Object");
    // Field `items` has erased descriptor `Ljava/util/List;`, but generic signature `Ljava/util/List<Ljava/lang/String;>;`
    builder.add_field(
        "items",
        "Ljava/util/List;",
        Some("Ljava/util/List<Ljava/lang/String;>;"),
    );

    let class_bytes = builder.build();
    let symbols = parse_class_file_symbols(&class_bytes).expect("Should parse class file");

    let field_decl = symbols
        .declarations
        .iter()
        .find(|d| d.name == "items")
        .expect("Property items should exist");
    assert_eq!(
        field_decl.signature.as_deref(),
        Some("val items: List<String>")
    );

    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("file:///cache/Container.class").unwrap();
    index.insert_file_symbols(class_url, symbols);

    let src_url = Url::parse("file:///project/Main.kt").unwrap();
    let src = r#"package com.example

fun test(c: Container) {
    val x = c.items
}
"#;
    index.index_source(src_url.clone(), src);

    let pos = Position::new(3, 8);
    let inferred = infer_variable_type(&index, &src_url, "x", pos, src, None);
    assert_eq!(inferred, KotlinType::parse("List<String>"));
}

#[test]
fn test_bytecode_generic_method_signature_inference() {
    let mut builder = MockClassBuilder::new("com/example/Collections", "java/lang/Object");
    // Method `singleton`: <T:Ljava/lang/Object;>(TT;)Ljava/util/List<TT;>;
    builder.add_method(
        "singleton",
        "(Ljava/lang/Object;)Ljava/util/List;",
        Some("<T:Ljava/lang/Object;>(TT;)Ljava/util/List<TT;>;"),
        false,
    );

    let class_bytes = builder.build();
    let symbols = parse_class_file_symbols(&class_bytes).expect("Should parse class file");

    let method_decl = symbols
        .declarations
        .iter()
        .find(|d| d.name == "singleton")
        .expect("Method singleton should exist");
    assert_eq!(
        method_decl.signature.as_deref(),
        Some("fun <T> singleton(arg1: T): List<T>")
    );

    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("file:///cache/Collections.class").unwrap();
    index.insert_file_symbols(class_url, symbols);

    let src_url = Url::parse("file:///project/Main.kt").unwrap();
    let src = r#"package com.example

fun test(c: Collections) {
    val list = c.singleton("hello")
}
"#;
    index.index_source(src_url.clone(), src);

    let pos = Position::new(3, 8);
    let inferred = infer_variable_type(&index, &src_url, "list", pos, src, None);
    assert_eq!(inferred, KotlinType::parse("List<String>"));
}

#[test]
fn test_bytecode_class_supertypes_generic_preservation() {
    let mut builder = MockClassBuilder::new("com/example/StringHolder", "java/lang/Object");
    builder
        .add_class_signature("<T:Ljava/lang/Object;>Ljava/lang/Object;Ljava/lang/Comparable<TT;>;");

    let class_bytes = builder.build();
    let symbols = parse_class_file_symbols(&class_bytes).expect("Should parse class file");

    let supers = symbols
        .class_supertypes
        .get("StringHolder")
        .expect("StringHolder supertypes should be recorded");
    assert!(
        supers.contains(&"Comparable<T>".to_string()),
        "Supertypes should preserve generic signature: {:?}",
        supers
    );
}
