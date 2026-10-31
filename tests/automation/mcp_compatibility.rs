//! Current-executor evidence and target API scenarios for the MCP refactor.
//! Current call shapes are temporary runner inputs, not compatibility promises.
//! Later issues replace them with direct target calls against the same outcomes.
use super::{Client, text};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn scenarios() -> Value {
    serde_json::from_str(include_str!("../../docs/mcp-compatibility/scenarios.json")).unwrap()
}

fn inventory() -> Value {
    serde_json::from_str(include_str!("../../docs/mcp-compatibility/operations.json")).unwrap()
}

fn substitute(value: &Value, bindings: &BTreeMap<&str, Value>) -> Value {
    match value {
        Value::String(s) if s.starts_with('$') => bindings
            .get(s.as_str())
            .unwrap_or_else(|| panic!("unknown fixture binding {s}"))
            .clone(),
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| substitute(v, bindings)).collect())
        }
        Value::Object(items) => Value::Object(
            items
                .iter()
                .map(|(k, v)| (k.clone(), substitute(v, bindings)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

// Allow additive metadata, while checking all recorded fields and exact array sizes.
fn assert_subset(actual: &Value, expected: &Value) {
    match expected {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(actual.get(key).is_some(), "missing {key}: {actual}");
                assert_subset(&actual[key], value);
            }
        }
        Value::Array(items) => {
            let got = actual.as_array().unwrap();
            assert_eq!(got.len(), items.len(), "{actual}");
            for (a, e) in got.iter().zip(items) {
                assert_subset(a, e);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

// Reference addressing policy only; this never edits a document. Future adapters
// must match it while resolving under the executor's revision lock.
fn layer_id<'a>(info: &'a Value, name: &str) -> Result<&'a str, &'static str> {
    let matches: Vec<_> = info["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|layer| layer["name"] == name)
        .collect();
    match matches.as_slice() {
        [] => Err("target_not_found"),
        [layer] => Ok(layer["id"].as_str().unwrap()),
        _ => Err("ambiguous_target"),
    }
}

fn frame_id(info: &Value, position: u64) -> Result<&str, &'static str> {
    position
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| info["frames"].as_array().unwrap().get(index))
        .and_then(|frame| frame["id"].as_str())
        .ok_or("target_not_found")
}

#[test]
fn workflow_document_safety_oracle() {
    let root = tempfile::tempdir().unwrap();
    let mut client = Client::new(false);
    let mut bindings =
        BTreeMap::from([("$output_path", json!(root.path().join("contract.pixen")))]);
    let fixture = scenarios();
    assert_eq!(fixture["runner"], "current-executor-oracle");
    for step in fixture["steps"].as_array().unwrap() {
        let current = substitute(
            if step["execute_target"] == true {
                &step["target"]
            } else {
                &step["current_call"]
            },
            &bindings,
        );
        let tool = current["name"].as_str().unwrap();
        let args = current["arguments"].clone();
        if let Some(target) = step.get("target") {
            if ["draw", "read_pixels"].contains(&target["name"].as_str().unwrap())
                && target["arguments"].get("layer").is_some()
            {
                let info = text(&client.call(
                    "sprite_info",
                    json!({
                        "project_id":bindings["$project_id"]
                    }),
                ));
                let target = &target["arguments"];
                assert_eq!(
                    layer_id(&info, target["layer"].as_str().unwrap()).unwrap(),
                    args["layer_id"]
                );
                assert_eq!(
                    frame_id(&info, target["frame"].as_u64().unwrap()).unwrap(),
                    args["frame_id"]
                );
            }
        }
        if let Some(message) = step["error_contains"].as_str() {
            // Compare document state AND exact pixel evidence around rejected edits.
            let project = bindings["$project_id"].clone();
            let before = client.call("sprite_info", json!({"project_id":project}));
            let read = json!({"project_id":project,"frame_id":bindings["$frame_id"],
                "layer_id":"base-id","region":{"x":0,"y":0,"width":4,"height":4}});
            let pixels = client.call("get_pixels", read.clone());
            let result = client.call(tool, args);
            assert_eq!(result["isError"], true, "{}: {result}", step["id"]);
            assert!(
                result.to_string().contains(message),
                "{}: {result}",
                step["id"]
            );
            assert_eq!(
                client.call("sprite_info", json!({"project_id":project})),
                before
            );
            assert_eq!(client.call("get_pixels", read), pixels);
        } else {
            let result = client.call(tool, args);
            assert_ne!(result["isError"], true, "{}: {result}", step["id"]);
            if step["execute_target"] == true {
                assert_subset(&result["structuredContent"], &step["target_expected"]);
            }
            let metadata = text(&result);
            if let Some(expected) = step.get("expected") {
                assert_subset(&metadata, expected);
            }
            if tool == "sprite_manage" {
                for (binding, field) in [
                    ("$project_id", "project_id"),
                    ("$frame_id", "frame_id"),
                    ("$layer_id", "layer_id"),
                ] {
                    assert!(metadata[field].is_string(), "{metadata}");
                    bindings.insert(binding, metadata[field].clone());
                }
            }
        }
    }
    let saved = pixen::input::read(&root.path().join("contract.pixen")).unwrap();
    assert_eq!(saved.frames.len(), 2);
    assert_eq!(saved.layers.len(), 3);
    assert_eq!(saved.frames[0].duration_ms, 150.0);
    assert_eq!(saved.frames[1].duration_ms, 100.0);
    assert_eq!(saved.palette[0], saved.palette[1]);
}

#[test]
fn addressing_oracle_rejects_ambiguous_names_and_preserves_literal_paths() {
    let root = tempfile::tempdir().unwrap();
    let mut client = Client::new(false);
    let created = text(&client.call("sprite_manage", json!({"op":"new","width":2,"height":2,"palette":["#00000000"],"output_path":root.path().join("addressing.pixen")}),
    ));
    let project = &created["project_id"];
    let setup = client.call("edit_batch", json!({"project_id":project,"expected_revision":0,"operations":[{"tool":"layer","arguments":{"op":"create","layer_id":"left-arm","name":"arm"}},{"tool":"layer","arguments":{"op":"create","layer_id":"right-arm","name":"arm"}},{"tool":"layer","arguments":{"op":"create","layer_id":"literal-path","name":"left/arm"}},{"tool":"frame","arguments":{"op":"add","frame_id":"pose-b","duration_ms":100,"index":0}}]}),
    );
    assert_ne!(setup["isError"], true, "{setup}");
    let info = text(&client.call("sprite_info", json!({"project_id":project})));
    assert_eq!(layer_id(&info, "arm"), Err("ambiguous_target"));
    assert_eq!(layer_id(&info, "Arm"), Err("target_not_found"));
    assert_eq!(layer_id(&info, "right/arm"), Err("target_not_found"));
    assert_eq!(layer_id(&info, "left/arm"), Ok("literal-path"));
    assert_eq!(frame_id(&info, 0), Err("target_not_found"));
    assert_eq!(frame_id(&info, 1), Ok("pose-b"));
    assert_eq!(
        frame_id(&info, 2),
        Ok(created["frame_id"].as_str().unwrap())
    );
    assert_eq!(frame_id(&info, 3), Err("target_not_found"));
    // Native ID addressing remains valid even when imported names are ambiguous.
    let edit = client.call(
        "draw",
        json!({"project_id":project,"expected_revision":1,
        "frame_id":"pose-b","layer_id":"right-arm","operation":{"type":"clear_cel"}}),
    );
    assert_eq!(text(&edit)["revision"], 2);
}

#[test]
fn discovery_describes_available_tools_without_freezing_the_old_api() {
    let mut client = Client::new(false);
    let listing = client.request("tools/list", json!({}));
    let tools = listing["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    let mut names = BTreeSet::new();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(names.insert(name), "duplicate tool {name}");
        assert!(tool["inputSchema"].is_object(), "missing schema: {tool}");
    }
    // Tool counts, specific legacy fields, and rejection of the target draw shape
    // are deliberately not asserted. Refactoring the API must be allowed.
}

#[test]
fn target_calls_and_inventory_define_one_refactorable_api() {
    let inventory = inventory();
    assert_eq!(inventory["contract"], scenarios()["contract"]);
    assert_eq!(inventory["contract"], "pixen-mcp/v1");
    let policy = &inventory["api_policy"];
    assert_eq!(policy["backward_compatibility_required"], false);
    assert_eq!(policy["invocation"], "direct_tools");
    assert_eq!(policy["discovery_tool"], "preflight");
    assert_eq!(policy["legacy_dispatcher"], false);
    assert_eq!(policy["argument_naming"], "snake_case");
    assert_eq!(scenarios()["backward_compatibility_required"], false);
    assert_eq!(inventory["excluded"], json!(["gallery_submission"]));
    let mut classified = BTreeSet::new();
    for entry in inventory["operations"].as_array().unwrap() {
        let command = entry["command"].as_str().unwrap();
        let op = entry["op"].as_str().unwrap();
        assert_eq!(entry["target_tool"], command);
        assert!(
            entry["target_op"]
                .as_str()
                .unwrap()
                .chars()
                .all(|c| !c.is_uppercase())
        );
        assert!(
            classified.insert((command.to_owned(), op.to_owned())),
            "duplicate: {entry}"
        );
        assert!(
            ["adapter", "new_capability", "unsupported"]
                .contains(&entry["classification"].as_str().unwrap())
        );
        assert!(!entry["reason"].as_str().unwrap().is_empty());
        assert!(!entry["alternatives"].as_array().unwrap().is_empty());
        let source = entry["source"].as_str().unwrap();
        assert!(source.starts_with("playbook/"));
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(source)
                .is_file(),
            "{source}"
        );
    }
    for step in scenarios()["steps"].as_array().unwrap() {
        let target = &step["target"];
        let tool = target["name"].as_str().unwrap();
        assert!(!["execute_guidance", "draw_legacy"].contains(&tool));
        assert!(target.get("command").is_none());
        assert!(target.get("contract").is_none());
        let args = &target["arguments"];
        assert!(args.is_object());
        assert!(
            args.get("arguments").is_none(),
            "no dispatch envelope: {target}"
        );
        assert_snake_case_keys(args);
        if tool == "draw" {
            assert!(
                args.get("operation").is_none(),
                "old draw schema is not the target: {target}"
            );
            let ops = args["ops"].as_array().unwrap();
            assert!(!ops.is_empty());
            assert!(ops.iter().all(|op| op["kind"].is_string()));
        }
        if tool == "edit_batch" {
            for op in args["operations"].as_array().unwrap() {
                assert!(op["tool"].is_string());
                assert!(op["arguments"].is_object());
                assert!(op["arguments"].get("expected_revision").is_none());
                assert!(op["arguments"].get("project_id").is_none());
            }
        }
        if step["current_call"]["arguments"]
            .get("expected_revision")
            .is_some()
        {
            assert!(
                args["expected_revision"].is_u64(),
                "mutation requires revision: {target}"
            );
        }
        if let Some(expected) = step.get("expected") {
            let target_expected = &step["target_expected"];
            assert_eq!(target_expected["contract"], inventory["contract"]);
            assert_eq!(target_expected["tool"], tool);
            if let Some(revision) = expected.get("revision") {
                assert_eq!(&target_expected["revision"], revision);
            }
            for (key, value) in expected.as_object().unwrap() {
                if key != "revision" {
                    assert_eq!(&target_expected["result"][key], value);
                }
            }
        }
        if step.get("error_contains").is_some() {
            assert_eq!(step["target_error"]["tool"], tool);
            assert_eq!(step["target_error"]["state_unchanged"], true);
            assert!(
                ["stale_revision", "out_of_bounds", "palette_mismatch"]
                    .contains(&step["target_error"]["code"].as_str().unwrap())
            );
        }
        if let Some(source) = step.get("source_guidance") {
            let command = source["command"].as_str().unwrap();
            assert_eq!(tool, command);
            let source_args = &source["arguments"];
            let ops: Vec<_> = if let Some(ops) = source_args["ops"].as_array() {
                ops.iter().map(|op| op["kind"].as_str().unwrap()).collect()
            } else if source_args.get("batch").is_some() {
                vec!["batch"]
            } else {
                vec![source_args["op"].as_str().unwrap_or("default")]
            };
            for op in ops {
                assert!(
                    classified.contains(&(command.to_owned(), op.to_owned())),
                    "unclassified {command}.{op}"
                );
            }
        }
    }
}

fn assert_snake_case_keys(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                // Grid legend glyphs are user data, not argument names.
                if key != "legend" {
                    assert!(
                        !key.chars().any(char::is_uppercase),
                        "noncanonical key: {key}"
                    );
                    assert_snake_case_keys(child);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(assert_snake_case_keys),
        _ => {}
    }
}
