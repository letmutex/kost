use tree_sitter::Node;

use crate::syntax::modifiers::parse_class_modifiers;
use crate::syntax::symbol_item::{FileSymbols, ImportItem, find_identifier_node};
use crate::syntax::synthetic::synthesize_class_members;

/// Extracts supertype names from a class declaration AST node.
pub fn extract_class_supertypes(node: &Node, source: &str) -> Vec<String> {
    let mut supertypes = Vec::new();
    let class_name = find_identifier_node(node)
        .and_then(|id| source.get(id.start_byte()..id.end_byte()))
        .map(str::trim);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        if k == "delegation_specifiers" {
            let mut inner = child.walk();
            for spec in child.children(&mut inner) {
                if let Some(t) = extract_type_from_specifier(&spec, source) {
                    let simple = t.split(['<', '(', ' ', '\t']).next().unwrap_or(&t).trim();
                    let simple = simple.rsplit('.').next().unwrap_or(simple).trim();
                    if class_name != Some(simple) {
                        supertypes.push(t);
                    }
                }
            }
        } else if k == "delegation_specifier"
            && let Some(t) = extract_type_from_specifier(&child, source)
        {
            let simple = t.split(['<', '(', ' ', '\t']).next().unwrap_or(&t).trim();
            let simple = simple.rsplit('.').next().unwrap_or(simple).trim();
            if class_name != Some(simple) {
                supertypes.push(t);
            }
        }
    }

    supertypes
}

fn format_supertype_name(text: &str, simple: &str) -> String {
    if let (Some(a), Some(c)) = (text.find('<'), text.rfind('>'))
        && c > a
    {
        return crate::concat_str!(simple, &text[a..=c]);
    }
    simple.to_string()
}

fn extract_type_from_specifier(spec: &Node, source: &str) -> Option<String> {
    let mut cursor = spec.walk();
    for child in spec.children(&mut cursor) {
        let k = child.kind();
        if k == "constructor_invocation" || k == "user_type" {
            let user_node = if k == "constructor_invocation" {
                (0..child.child_count())
                    .find_map(|i| child.child(i).filter(|c| c.kind() == "user_type"))
                    .unwrap_or(child)
            } else {
                child
            };
            let text = source.get(user_node.start_byte()..user_node.end_byte())?;
            let clean = text.split(['(', '<']).next()?.trim();
            let simple = clean.rsplit('.').next().unwrap_or(clean).trim();
            if !simple.is_empty() && simple.chars().next().is_some_and(|c| c.is_uppercase()) {
                return Some(format_supertype_name(text, simple));
            }
        }
    }
    let text = source.get(spec.start_byte()..spec.end_byte())?;
    let clean = text.split(['(', '<', ' ', '\t']).next()?.trim();
    let simple = clean.rsplit('.').next().unwrap_or(clean).trim();
    if !simple.is_empty() && simple.chars().next().is_some_and(|c| c.is_uppercase()) {
        Some(format_supertype_name(text, simple))
    } else {
        None
    }
}

/// Extracts the target type from a typealias declaration AST node.
pub fn extract_type_alias_target(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let mut past_equals = false;
    let mut start_byte = None;
    let mut end_byte = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "=" {
            past_equals = true;
        } else if past_equals
            && child.is_named()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            if start_byte.is_none() {
                start_byte = Some(child.start_byte());
            }
            end_byte = Some(child.end_byte());
        }
    }
    let (start, end) = (start_byte?, end_byte?);
    let target = source.get(start..end)?.trim();
    if !target.is_empty() {
        Some(target.to_string())
    } else {
        None
    }
}

/// Handles declaration extensions such as synthetics, supertypes, and type aliases.
pub fn handle_declaration_extensions(
    decl_node: &Node,
    source: &str,
    name: &str,
    file_symbols: &mut FileSymbols,
) {
    let dk = decl_node.kind();
    if dk == "class_declaration" {
        let modifiers = parse_class_modifiers(decl_node, source);
        file_symbols
            .declarations
            .extend(synthesize_class_members(decl_node, source, name, modifiers));
        let supers = extract_class_supertypes(decl_node, source);
        if !supers.is_empty() {
            file_symbols
                .class_supertypes
                .insert(name.to_string(), supers.clone());
        }
        if let Some(pkg) = file_symbols
            .package_name
            .as_deref()
            .filter(|p| !p.is_empty())
        {
            let has_star_import = file_symbols.imports.iter().any(|i| i.is_star);
            let q_supers = supers
                .iter()
                .map(|s| {
                    let clean = s.split(['<', '?']).next().unwrap_or(s).trim();
                    let declared_locally = file_symbols
                        .declarations
                        .iter()
                        .any(|d| d.name == clean && d.container_name.is_none());
                    let pkg_opt = if !has_star_import || declared_locally {
                        Some(pkg)
                    } else {
                        None
                    };
                    resolve_extension_container(s, &file_symbols.imports, pkg_opt)
                })
                .collect();
            file_symbols
                .class_supertypes
                .insert(crate::concat_str!(pkg, ".", name), q_supers);
        }
    } else if dk == "type_alias"
        && let Some(target) = extract_type_alias_target(decl_node, source)
    {
        file_symbols.type_aliases.insert(name.to_string(), target);
    }
}

/// Resolves an extension receiver type to its fully-qualified container if found in imports or package.
pub fn resolve_extension_container(
    ext_rec: &str,
    imports: &[ImportItem],
    pkg: Option<&str>,
) -> String {
    let clean = ext_rec.split(['<', '?']).next().unwrap_or(ext_rec).trim();
    if clean.contains('.') {
        return clean.to_string();
    }
    imports
        .iter()
        .find_map(|imp| {
            let is_alias = imp.alias.as_deref() == Some(clean);
            let is_explicit = !imp.is_star
                && (imp.path.ends_with(clean)
                    && (imp.path.len() == clean.len()
                        || imp.path.as_bytes()[imp.path.len() - clean.len() - 1] == b'.'));
            if is_alias || is_explicit {
                Some(imp.path.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            pkg.filter(|p| !p.is_empty())
                .map(|p| crate::concat_str!(p, ".", clean))
        })
        .unwrap_or_else(|| clean.to_string())
}

/// Compares two container names (classes or interfaces) for a match.
///
/// Handles exact matches, package qualification, and nested class paths.
pub fn containers_match(container_a: &str, container_b: &str) -> bool {
    let clean_a = container_a
        .split(['<', '?'])
        .next()
        .unwrap_or(container_a)
        .trim();
    let clean_b = container_b
        .split(['<', '?'])
        .next()
        .unwrap_or(container_b)
        .trim();
    if clean_a == clean_b {
        return true;
    }

    let is_class_suffix = |full: &str, suffix: &str| -> bool {
        if full.len() > suffix.len()
            && full.ends_with(suffix)
            && full.as_bytes().get(full.len() - suffix.len() - 1) == Some(&b'.')
        {
            return true;
        }
        false
    };

    is_class_suffix(clean_a, clean_b) || is_class_suffix(clean_b, clean_a)
}
