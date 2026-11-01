use quick_xml::events::Event;
use quick_xml::reader::Reader;
use smol_str::SmolStr;

use super::{PomDependency, PomModel, PomParent};

#[derive(Default)]
struct PomParseState {
    dep: PomDependency,
    parent: PomParent,
    excl_group: SmolStr,
    excl_name: SmolStr,
}

pub(crate) fn parse_pom(xml_content: &str) -> Result<PomModel, String> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut model = PomModel {
        packaging: SmolStr::new("jar"),
        ..Default::default()
    };

    let mut path: Vec<SmolStr> = Vec::new();
    let mut current_text = String::new();
    let mut state = PomParseState::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = SmolStr::new(String::from_utf8_lossy(e.name().as_ref()).as_ref());
                path.push(tag_name);
                current_text.clear();
            }
            Ok(Event::Text(ref e)) => {
                if let Ok(txt) = e.unescape() {
                    current_text.push_str(&txt);
                }
            }
            Ok(Event::CData(ref e)) => {
                if let Ok(txt) = std::str::from_utf8(e.as_ref()) {
                    current_text.push_str(txt);
                }
            }
            Ok(Event::End(ref e)) => {
                process_end_element(&path, &current_text, &mut model, &mut state);
                if path
                    .last()
                    .is_some_and(|s| s.as_bytes() == e.name().as_ref())
                {
                    path.pop();
                }
                current_text.clear();
            }
            Ok(Event::Empty(ref e)) => {
                let tag_name = SmolStr::new(String::from_utf8_lossy(e.name().as_ref()).as_ref());
                path.push(tag_name);
                process_end_element(&path, "", &mut model, &mut state);
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(common::concat_str!("XML parse error: ", &err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(model)
}

fn is_in_project_or_mgmt_dependency(path: &[SmolStr]) -> bool {
    let is_proj_dep = path.len() >= 3
        && path[0] == "project"
        && path[1] == "dependencies"
        && path[2] == "dependency";
    let is_mgmt_dep = path.len() >= 4
        && path[0] == "project"
        && path[1] == "dependencyManagement"
        && path[2] == "dependencies"
        && path[3] == "dependency";
    is_proj_dep || is_mgmt_dep
}

fn process_end_element(
    path: &[SmolStr],
    text: &str,
    model: &mut PomModel,
    state: &mut PomParseState,
) {
    let len = path.len();
    if len < 2 || path[0] != "project" {
        return;
    }

    let trimmed = text.trim();
    if len == 2 {
        match path[1].as_str() {
            "groupId" => model.group = Some(SmolStr::new(trimmed)),
            "artifactId" => model.name = Some(SmolStr::new(trimmed)),
            "version" => model.version = Some(trimmed.to_string()),
            "packaging" => model.packaging = SmolStr::new(trimmed),
            "parent" => model.parent = Some(state.parent.clone()),
            _ => {}
        }
        return;
    } else if len == 3 && path[1] == "parent" {
        match path[2].as_str() {
            "groupId" => state.parent.group = SmolStr::new(trimmed),
            "artifactId" => state.parent.name = SmolStr::new(trimmed),
            "version" => state.parent.version = trimmed.to_string(),
            _ => {}
        }
        return;
    } else if len == 3 && path[1] == "properties" {
        model
            .properties
            .insert(path[2].to_string(), trimmed.to_string());
        return;
    } else if len == 3 && path[1] == "dependencies" && path[2] == "dependency" {
        model.dependencies.push(std::mem::take(&mut state.dep));
        return;
    } else if len == 4
        && path[1] == "dependencyManagement"
        && path[2] == "dependencies"
        && path[3] == "dependency"
    {
        model
            .dependency_management
            .push(std::mem::take(&mut state.dep));
        return;
    }

    if !is_in_project_or_mgmt_dependency(path) {
        return;
    }

    if path[len - 2] == "dependency" {
        match path[len - 1].as_str() {
            "groupId" => state.dep.group = SmolStr::new(trimmed),
            "artifactId" => state.dep.name = SmolStr::new(trimmed),
            "version" => state.dep.version = Some(trimmed.to_string()),
            "scope" => state.dep.scope = Some(trimmed.to_string()),
            "optional" => state.dep.optional = trimmed == "true",
            _ => {}
        }
    } else if path[len - 2] == "exclusion" {
        match path[len - 1].as_str() {
            "groupId" => state.excl_group = SmolStr::new(trimmed),
            "artifactId" => state.excl_name = SmolStr::new(trimmed),
            _ => {}
        }
    } else if path[len - 1] == "exclusion" {
        state.dep.exclusions.push((
            std::mem::take(&mut state.excl_group),
            std::mem::take(&mut state.excl_name),
        ));
    }
}
