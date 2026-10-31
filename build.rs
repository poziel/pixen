fn main() {
    embed_playbook();
    println!("cargo:rerun-if-changed=assets/pixen.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/pixen.ico")
            .set("ProductName", "Pixen")
            .compile()
            .expect("compile Pixen Windows icon resource");
    }
}

/// Keep installed guidance independent of the executable's working directory.
fn embed_playbook() {
    use std::{fs, path::Path};
    fn files(root: &Path, paths: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(root).expect("read playbook directory") {
            let entry = entry.expect("read playbook entry");
            let kind = entry.file_type().expect("playbook file type");
            assert!(!kind.is_symlink(), "playbook must not contain symlinks");
            if kind.is_dir() {
                files(&entry.path(), paths);
            } else if kind.is_file() {
                paths.push(entry.path());
            }
        }
    }
    fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        text.strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---"))
            .and_then(|(header, _)| header.lines().find_map(|l| l.strip_prefix(key)))
            .map(str::trim)
    }
    let root =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("playbook");
    println!("cargo:rerun-if-changed=playbook");
    let mut paths = Vec::new();
    files(&root.join("skills"), &mut paths);
    files(&root.join("rules"), &mut paths);
    paths.sort();
    let mut generated = String::from("pub static ENTRIES: &[Entry] = &[\n");
    let mut skills = String::from(
        "# Pixen skills\n\nStart with skill://studio. Read rules://index for shared craft guidance.\n\n",
    );
    let mut rules = String::from("# Pixen rules\n\n");
    for path in paths {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let parts: Vec<_> = relative.split('/').collect();
        let entry_point = parts[0] == "skills" && parts.len() == 3 && parts[2] == "SKILL.md";
        let rule = parts[0] == "rules"
            && path.extension().is_some_and(|e| e == "md")
            && parts[1].starts_with(|c: char| c.is_ascii_digit());
        if !entry_point && !rule && !(parts[0] == "skills" && parts.len() > 2) {
            continue;
        }
        // Only text resources are exposed. Role registration is a separate feature.
        if !matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("md" | "json" | "yaml")
        ) {
            continue;
        }
        let text = fs::read_to_string(&path)
            .expect("UTF-8 playbook resource")
            .replace("\r\n", "\n");
        let uri = if entry_point {
            format!("skill://{}", parts[1])
        } else if rule {
            format!("rules://{}", path.file_stem().unwrap().to_str().unwrap())
        } else {
            format!("skill://{}", parts[1..].join("/"))
        };
        let name = field(&text, "title:")
            .or_else(|| text.lines().find_map(|l| l.strip_prefix("# ")))
            .unwrap_or(&uri);
        let description = field(&text, "description:").unwrap_or(name);
        let workflow = entry_point && !matches!(parts[1], "pixen" | "pixel-art");
        assert!(
            !matches!(parts[1], "submit" | "gallery"),
            "removed workflow in playbook"
        );
        if entry_point {
            skills.push_str(&format!("- `{uri}` — {description}\n"));
        }
        if rule {
            rules.push_str(&format!("- `{uri}` — {name}\n"));
        }
        generated.push_str(&format!("Entry {{ uri: {uri:?}, name: {name:?}, description: {description:?}, text: {text:?}, workflow: {workflow} }},\n"));
    }
    for (uri, name, text) in [
        ("skill://index", "Pixen skills", skills),
        ("rules://index", "Pixen rules", rules),
    ] {
        generated.push_str(&format!("Entry {{ uri: {uri:?}, name: {name:?}, description: {name:?}, text: {text:?}, workflow: false }},\n"));
    }
    generated.push_str("];\n");
    fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("playbook.rs"),
        generated,
    )
    .expect("write embedded playbook");
}
