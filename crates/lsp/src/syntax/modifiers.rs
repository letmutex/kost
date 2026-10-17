use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassModifier {
    Data,
    Enum,
    Sealed,
    Annotation,
    Inner,
    Value,
    FunInterface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ClassModifiers {
    pub is_data: bool,
    pub is_enum: bool,
    pub is_sealed: bool,
    pub is_annotation: bool,
    pub is_inner: bool,
    pub is_value: bool,
    pub is_fun_interface: bool,
}

impl ClassModifiers {
    pub fn is_empty(&self) -> bool {
        !self.is_data
            && !self.is_enum
            && !self.is_sealed
            && !self.is_annotation
            && !self.is_inner
            && !self.is_value
            && !self.is_fun_interface
    }
}

/// Parses class modifiers from an AST node and its source text.
pub fn parse_class_modifiers(node: &Node, source: &str) -> ClassModifiers {
    let mut mods = ClassModifiers::default();
    if node.kind() != "class_declaration" {
        return mods;
    }

    let mut has_fun = false;
    let mut is_interface = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "enum_class_body" => mods.is_enum = true,
            "enum" => mods.is_enum = true,
            "interface" => is_interface = true,
            "fun" => has_fun = true,
            "modifiers" => {
                parse_modifiers_node(&child, source, &mut mods, &mut has_fun);
            }
            _ => {}
        }
    }

    if is_interface && has_fun {
        mods.is_fun_interface = true;
    }

    mods
}

fn parse_modifiers_node(
    modifiers_node: &Node,
    source: &str,
    mods: &mut ClassModifiers,
    has_fun: &mut bool,
) {
    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        match child.kind() {
            "class_modifier" => {
                if let Some(text) = source.get(child.start_byte()..child.end_byte()) {
                    match text.trim() {
                        "data" => mods.is_data = true,
                        "enum" => mods.is_enum = true,
                        "sealed" => mods.is_sealed = true,
                        "annotation" => mods.is_annotation = true,
                        "inner" => mods.is_inner = true,
                        "value" => mods.is_value = true,
                        _ => {}
                    }
                }
            }
            "fun" => *has_fun = true,
            _ => {
                if let Some(text) = source.get(child.start_byte()..child.end_byte())
                    && text.trim() == "fun"
                {
                    *has_fun = true;
                }
            }
        }
    }
}
