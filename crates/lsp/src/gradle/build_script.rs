use ahash::AHashMap;
use regex::Regex;
use std::sync::OnceLock;

use crate::gradle::catalog::VersionCatalog;
use crate::gradle::dependency::DependencyCoordinate;

/// Information extracted from a `.gradle` or `.gradle.kts` build script.
#[derive(Debug, Clone, Default)]
pub struct BuildScriptInfo {
    pub dependencies: Vec<DependencyCoordinate>,
    pub catalog_accessors: Vec<String>,
    pub kotlin_versions: Vec<String>,
    pub compile_sdk: Option<u32>,
    pub has_android_block: bool,
    pub plugins: Vec<(String, Option<String>)>,
    pub variables: AHashMap<String, String>,
}

impl BuildScriptInfo {
    /// Parse a Gradle script content (.gradle or .gradle.kts)
    pub fn parse(content: &str, catalog: Option<&VersionCatalog>) -> Self {
        Self::parse_with_vars(content, catalog, &AHashMap::default())
    }

    /// Parse a Gradle script with inherited variables from parent project or gradle.properties
    pub fn parse_with_vars(
        content: &str,
        catalog: Option<&VersionCatalog>,
        initial_vars: &AHashMap<String, String>,
    ) -> Self {
        let mut variables = initial_vars.clone();
        extract_variables(content, &mut variables);

        let mut info = BuildScriptInfo::default();
        extract_android_configuration(content, &variables, &mut info);
        extract_plugins(content, &variables, &mut info);
        extract_dependencies(content, catalog, &variables, &mut info);
        info.variables = variables;

        info
    }
}

pub fn parse_gradle_properties(content: &str, vars: &mut AHashMap<String, String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with('!') || trimmed.is_empty() {
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim().to_string();
            vars.insert(key, val);
        }
    }
}

fn extract_variables(content: &str, vars: &mut AHashMap<String, String>) {
    // def / val / var / ext. declarations: def roomVersion = "2.6.1" or ext.lifecycle_version = "2.8.0"
    static VAR_RE: OnceLock<Regex> = OnceLock::new();
    let re = VAR_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:(?:def|val|var)\s+|ext\.)([a-zA-Z0-9_]+)\s*(?::\s*[a-zA-Z0-9_<>]+)?\s*=\s*["']([^"']+)["']
        "#).unwrap()
    });
    for cap in re.captures_iter(content) {
        if let (Some(k), Some(v)) = (cap.get(1), cap.get(2)) {
            vars.insert(k.as_str().to_string(), v.as_str().to_string());
        }
    }

    // Extra syntax: val roomVersion by extra("2.6.1") or extra["roomVersion"] = "2.6.1"
    static EXTRA_RE: OnceLock<Regex> = OnceLock::new();
    let extra_re = EXTRA_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:val\s+([a-zA-Z0-9_]+)\s+by\s+extra\s*\(\s*["']([^"']+)["']\s*\)|extra\s*\[\s*["']([^"']+)["']\s*\]\s*=\s*["']([^"']+)["'])
        "#).unwrap()
    });
    for cap in extra_re.captures_iter(content) {
        let k = cap.get(1).or_else(|| cap.get(3));
        let v = cap.get(2).or_else(|| cap.get(4));
        if let (Some(k), Some(v)) = (k, v) {
            vars.insert(k.as_str().to_string(), v.as_str().to_string());
        }
    }

    // Top-level assignments: roomVersion = "2.6.1" or room_version = '2.6.1'
    static ASSIGN_RE: OnceLock<Regex> = OnceLock::new();
    let assign_re = ASSIGN_RE.get_or_init(|| {
        Regex::new(r#"(?m)^\s*([a-zA-Z0-9_]+(?:Version|version|_version))\s*=\s*["']([^"']+)["']"#)
            .unwrap()
    });
    for cap in assign_re.captures_iter(content) {
        if let (Some(k), Some(v)) = (cap.get(1), cap.get(2)) {
            vars.insert(k.as_str().to_string(), v.as_str().to_string());
        }
    }
}

pub fn resolve_interpolated_string(input: &str, vars: &AHashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (k, v) in vars {
        let braced = common::concat_str!("${", k, "}");
        if result.contains(&braced) {
            result = result.replace(&braced, v);
        }
        let unbraced = common::concat_str!("$", k);
        if result.contains(&unbraced) {
            result = result.replace(&unbraced, v);
        }
    }
    result
}

fn extract_android_configuration(
    content: &str,
    vars: &AHashMap<String, String>,
    info: &mut BuildScriptInfo,
) {
    static ANDROID_BLOCK_RE: OnceLock<Regex> = OnceLock::new();
    let block_re = ANDROID_BLOCK_RE.get_or_init(|| Regex::new(r#"(?m)\bandroid\s*\{"#).unwrap());

    static SDK_RE: OnceLock<Regex> = OnceLock::new();
    let sdk_re = SDK_RE.get_or_init(|| {
        Regex::new(r#"(?i)compileSdk(?:Version)?(?:\s*=\s*|\s+|\s*\(\s*)(?:(\d+)|([a-zA-Z0-9_]+))"#)
            .unwrap()
    });

    for mat in block_re.find_iter(content) {
        info.has_android_block = true;
        let start_idx = mat.end();
        let rest = &content[start_idx..];
        let block_content = extract_matching_brace_block(rest);
        for cap in sdk_re.captures_iter(block_content) {
            if let Some(digits) = cap.get(1) {
                if let Ok(sdk) = digits.as_str().parse::<u32>() {
                    info.compile_sdk = Some(sdk);
                    return;
                }
            } else if let Some(var_name) = cap.get(2)
                && let Some(val) = vars.get(var_name.as_str())
                && let Ok(sdk) = val.parse::<u32>()
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
    }
}

fn extract_matching_brace_block(rest: &str) -> &str {
    let mut depth = 1usize;
    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[..i];
                }
            }
            _ => {}
        }
    }
    rest
}

fn extract_plugins(content: &str, vars: &AHashMap<String, String>, info: &mut BuildScriptInfo) {
    static PLUGIN_RE: OnceLock<Regex> = OnceLock::new();
    let re = PLUGIN_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:id\s*\(\s*["']([^"']+)["']\s*\)|id\s+["']([^"']+)["']|kotlin\s*\(\s*["']([^"']+)["']\s*\))
            (?:\s*version\s*["']([^"']+)["']|\s*version\s*\(\s*["']([^"']+)["']\s*\)|\s*version\s+([a-zA-Z0-9_]+))?
        "#).unwrap()
    });

    for cap in re.captures_iter(content) {
        let plugin_id = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().to_string())
            .or_else(|| {
                cap.get(3)
                    .map(|m| common::concat_str!("org.jetbrains.kotlin.", m.as_str()))
            });

        let version = cap
            .get(4)
            .or_else(|| cap.get(5))
            .map(|m| m.as_str().to_string())
            .or_else(|| cap.get(6).and_then(|m| vars.get(m.as_str()).cloned()));

        if let Some(id) = plugin_id {
            if id.contains("kotlin")
                && let Some(ref v) = version
            {
                info.kotlin_versions.push(v.clone());
            }
            info.plugins.push((id, version));
        }
    }
}

fn extract_dependencies(
    content: &str,
    catalog: Option<&VersionCatalog>,
    vars: &AHashMap<String, String>,
    info: &mut BuildScriptInfo,
) {
    extract_string_dependencies(content, vars, info);
    extract_concatenated_dependencies(content, vars, info);
    extract_map_and_multi_arg_dependencies(content, vars, info);
    extract_catalog_accessor_dependencies(content, catalog, info);
}

fn extract_string_dependencies(
    content: &str,
    vars: &AHashMap<String, String>,
    info: &mut BuildScriptInfo,
) {
    static STRING_DEP_RE: OnceLock<Regex> = OnceLock::new();
    let str_re = STRING_DEP_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:implementation|api|compileOnly|runtimeOnly|kapt|ksp|classpath|testImplementation|androidTestImplementation)
            (?:\s*\(\s*|\s+)
            ["']([a-zA-Z0-9_.-]+:[a-zA-Z0-9_.-]+(?::[a-zA-Z0-9_$.{}~+-]+)?(?:@[a-zA-Z0-9_.-]+)?)["']
        "#).unwrap()
    });

    for cap in str_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            let resolved = resolve_interpolated_string(m.as_str(), vars);
            if let Some(dep) = DependencyCoordinate::parse(&resolved) {
                info.dependencies.push(dep);
            }
        }
    }
}

fn extract_concatenated_dependencies(
    content: &str,
    vars: &AHashMap<String, String>,
    info: &mut BuildScriptInfo,
) {
    static CONCAT_DEP_RE: OnceLock<Regex> = OnceLock::new();
    let concat_re = CONCAT_DEP_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:implementation|api|compileOnly|runtimeOnly|kapt|ksp|classpath|testImplementation|androidTestImplementation)
            (?:\s*\(\s*|\s+)
            ["']([a-zA-Z0-9_.-]+:[a-zA-Z0-9_.-]+:)["']\s*\+\s*([a-zA-Z0-9_]+)
        "#).unwrap()
    });

    for cap in concat_re.captures_iter(content) {
        if let (Some(prefix), Some(var_name)) = (cap.get(1), cap.get(2))
            && let Some(ver) = vars.get(var_name.as_str())
        {
            let full = common::concat_str!(prefix.as_str(), ver);
            if let Some(dep) = DependencyCoordinate::parse(&full) {
                info.dependencies.push(dep);
            }
        }
    }
}

fn extract_map_and_multi_arg_dependencies(
    content: &str,
    vars: &AHashMap<String, String>,
    info: &mut BuildScriptInfo,
) {
    let mut parse_and_add = |cap: &regex::Captures| {
        let group = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let ver = cap.get(3).map(|m| m.as_str()).or_else(|| {
            cap.get(4)
                .and_then(|m| vars.get(m.as_str()).map(|s| s.as_str()))
        });
        if !group.is_empty() && !name.is_empty() {
            info.dependencies
                .push(DependencyCoordinate::new(group, name, ver));
        }
    };

    static MAP_DEP_RE: OnceLock<Regex> = OnceLock::new();
    let map_re = MAP_DEP_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            group\s*(?::\s*|=\s*)\s*["']([^"']+)["']\s*,\s*name\s*(?::\s*|=\s*)\s*["']([^"']+)["'](?:\s*,\s*version\s*(?::\s*|=\s*)\s*(?:["']([^"']+)["']|([a-zA-Z0-9_]+)))?
        "#).unwrap()
    });

    for cap in map_re.captures_iter(content) {
        parse_and_add(&cap);
    }

    static THREE_ARG_RE: OnceLock<Regex> = OnceLock::new();
    let three_re = THREE_ARG_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:implementation|api|compileOnly|runtimeOnly|kapt|ksp|classpath)
            \s*\(\s*["']([^"']+)["']\s*,\s*["']([^"']+)["']\s*,\s*(?:["']([^"']+)["']|([a-zA-Z0-9_]+))\s*\)
        "#).unwrap()
    });

    for cap in three_re.captures_iter(content) {
        parse_and_add(&cap);
    }
}

fn extract_catalog_accessor_dependencies(
    content: &str,
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    static CATALOG_REF_RE: OnceLock<Regex> = OnceLock::new();
    let cat_re = CATALOG_REF_RE.get_or_init(|| {
        Regex::new(r#"(?x)
            (?:implementation|api|compileOnly|runtimeOnly|kapt|ksp|classpath|testImplementation|androidTestImplementation)
            (?:\s*\(\s*|\s+)
            (libs\.[a-zA-Z0-9_.]+)
        "#).unwrap()
    });

    for cap in cat_re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            let accessor = m.as_str();
            info.catalog_accessors.push(accessor.to_string());
            if let Some(cat) = catalog {
                let resolved = cat.resolve_accessor(accessor);
                info.dependencies.extend(resolved);
            }
        }
    }
}
