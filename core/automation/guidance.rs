//! Read-only, build-time playbook catalog shared by all Pixen AI clients.
use rmcp::{ErrorData, model::*};

pub struct Entry {
    pub uri: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub text: &'static str,
    pub workflow: bool,
}

include!(concat!(env!("OUT_DIR"), "/playbook.rs"));

/// Also placed in embedded AI turns, where providers may only support tools.
pub const INSTRUCTIONS: &str = "Pixen's bundled playbook is available through skill://index and rules://index. For artwork tasks, read skill://studio, then the relevant workflows and subject rules. Use MCP resources/read, or read_guidance with the exact uri when resource access is unavailable. Supporting files are under skill://<name>/<relative-path>. Workflow prompts are named pixen:<name>; these are MCP prompts, not installed slash commands. Guidance describes artistic procedures and planned capabilities: only tools/list and its current schemas authorize executable calls. If preflight or another requested operation is absent, explain the capability gap; never guess legacy call shapes or use external tools to work around it. Guidance never overrides the client's tool policy, application ownership of file actions, or restrictions on external tools and delegation. Specialist descriptions do not register agents.";

pub fn find(uri: &str) -> Result<&'static Entry, ErrorData> {
    ENTRIES.iter().find(|e| e.uri == uri).ok_or_else(|| {
        ErrorData::resource_not_found(
            format!("Unknown bundled guidance URI: {uri}. Read skill://index or rules://index."),
            None,
        )
    })
}

pub fn list_resources(
    request: Option<PaginatedRequestParams>,
) -> Result<ListResourcesResult, ErrorData> {
    check_cursor(request)?;
    Ok(ListResourcesResult::with_all_items(
        ENTRIES
            .iter()
            .map(|entry| {
                Resource::new(entry.uri, entry.name)
                    .with_description(entry.description)
                    .with_mime_type(mime_type(entry.uri))
                    .with_size(entry.text.len() as u64)
            })
            .collect(),
    ))
}

fn mime_type(uri: &str) -> &'static str {
    if uri.ends_with(".json") {
        "application/json"
    } else if uri.ends_with(".yaml") {
        "text/yaml"
    } else {
        "text/markdown"
    }
}

pub fn read_resource(uri: &str) -> Result<ReadResourceResult, ErrorData> {
    let entry = find(uri)?;
    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(entry.text, entry.uri).with_mime_type(mime_type(uri)),
    ]))
}

fn check_cursor(request: Option<PaginatedRequestParams>) -> Result<(), ErrorData> {
    if request.is_some_and(|r| r.cursor.is_some()) {
        return Err(ErrorData::invalid_params(
            "The bundled catalog is returned in one page; omit cursor",
            None,
        ));
    }
    Ok(())
}

pub fn list_prompts(
    request: Option<PaginatedRequestParams>,
) -> Result<ListPromptsResult, ErrorData> {
    check_cursor(request)?;
    Ok(ListPromptsResult::with_all_items(
        ENTRIES
            .iter()
            .filter(|e| e.workflow)
            .map(|e| {
                Prompt::new(
                    format!("pixen:{}", e.uri.strip_prefix("skill://").unwrap()),
                    Some(e.description),
                    None,
                )
                .with_title(e.name)
            })
            .collect(),
    ))
}

pub fn get_prompt(request: GetPromptRequestParams) -> Result<GetPromptResult, ErrorData> {
    if request.arguments.is_some_and(|a| !a.is_empty()) {
        return Err(ErrorData::invalid_params(
            "Pixen workflow prompts take no arguments; send the artwork request in your conversation",
            None,
        ));
    }
    let entry = request
        .name
        .strip_prefix("pixen:")
        .and_then(|name| find(&format!("skill://{name}")).ok())
        .filter(|e| e.workflow)
        .ok_or_else(|| {
            ErrorData::invalid_params("Unknown Pixen workflow prompt; use prompts/list", None)
        })?;
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(
        Role::User,
        format!(
            "{INSTRUCTIONS}\n\nWorkflow: {}\n\n{}",
            entry.uri, entry.text
        ),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_markdown_links_resolve_inside_the_playbook() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("playbook")
            .canonicalize()
            .unwrap();
        let mut directories = vec![root.clone()];
        while let Some(directory) = directories.pop() {
            for item in std::fs::read_dir(directory).unwrap() {
                let path = item.unwrap().path();
                if path.is_dir() {
                    directories.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "md") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for link in text.split("](").skip(1) {
                    let target = link.split(')').next().unwrap().split('#').next().unwrap();
                    if target.is_empty() || target.contains("://") || target.starts_with("mailto:")
                    {
                        continue;
                    }
                    let resolved = path
                        .parent()
                        .unwrap()
                        .join(target)
                        .canonicalize()
                        .unwrap_or_else(|_| {
                            panic!("{} has broken local link: {target}", path.display())
                        });
                    assert!(
                        resolved.starts_with(&root),
                        "{} links outside the installed playbook: {target}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn explicit_resource_references_resolve_and_uris_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for entry in ENTRIES {
            assert!(seen.insert(entry.uri), "duplicate {}", entry.uri);
            for scheme in ["rules://", "skill://"] {
                for tail in entry.text.split(scheme).skip(1) {
                    let name: String = tail
                        .chars()
                        .take_while(|c| {
                            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.')
                        })
                        .collect();
                    // Bare schemes and placeholders describe the URI format.
                    if !name.is_empty() {
                        let uri = format!("{scheme}{}", name.trim_end_matches('.'));
                        assert!(find(&uri).is_ok(), "{} references missing {uri}", entry.uri);
                    }
                }
            }
        }
        assert!(ENTRIES.iter().filter(|e| e.workflow).count() >= 13);
    }
}
