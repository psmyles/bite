use crate::{graph, Registry};
use bite_schema::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub struct LoadedWorkflow {
    pub workflow: Workflow,
    pub migrated: bool,
    /// The graph was changed on load to keep its behaviour (see [`migrate_set_naming`]), so
    /// it differs from the file even when the schema did not.
    pub rewritten: bool,
    pub warnings: Vec<String>,
}
fn semver(s: &str) -> Option<Vec<u64>> {
    let v: Vec<_> = s
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<_, _>>()
        .ok()?;
    (v.len() == 3).then_some(v)
}
pub fn compatible_v1(file: &str) -> bool {
    let Some(v) = semver(file) else {
        return false;
    };
    let ranges: Value = serde_json::from_str(include_str!("../compat-ranges.json")).unwrap();
    // The active legacy app is 0.5.0: preserve its configured range, not a guessed
    // major-version rule. A future policy change must update this fixture too.
    let app = vec![0, 5, 0];
    let range = |v: &Vec<u64>| {
        ranges.as_array().unwrap().iter().position(|r| {
            let from = semver(r["from"].as_str().unwrap()).unwrap();
            v >= &from && (r["to"].is_null() || v <= &semver(r["to"].as_str().unwrap()).unwrap())
        })
    };
    range(&v).is_some() && range(&v) == range(&app)
}
pub fn load(text: &str, registry: &Registry) -> Result<LoadedWorkflow, String> {
    let mut doc: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let version = doc.get("schema_version").and_then(Value::as_u64);
    if version.is_some_and(|v| v != 1 && v != 2) {
        return Err(format!(
            "unsupported workflow schema_version {}",
            version.unwrap()
        ));
    }
    let migrated = version != Some(2);
    let mut warnings = Vec::new();
    if migrated {
        if let Some(app) = doc["appVersion"].as_str() {
            if !compatible_v1(app) {
                return Err(format!("incompatible v1 application version {app}"));
            }
        } else {
            warnings
                .push("v1 workflow has no appVersion; accepting legacy unversioned graph".into());
        }
        let created = doc["appVersion"].as_str().unwrap_or("legacy").to_owned();
        let graph = doc.get("graph").cloned().unwrap_or(doc);
        doc = json!({"schema_version":2,"created_with":created,"graph":graph});
        migrate_graph(&mut doc["graph"], registry, &mut warnings)?;
    }
    let mut workflow: Workflow =
        serde_json::from_value(doc).map_err(|e| format!("workflow: {e}"))?;
    let rewritten = migrate_set_naming(&mut workflow.graph, &mut warnings);
    workflow.validate().map_err(|e| e.join("\n"))?;
    graph::validate(&workflow.graph, registry)?;
    Ok(LoadedWorkflow {
        workflow,
        migrated,
        rewritten,
        warnings,
    })
}

/// Moves an Image Output's set naming fields into a Rename node in front of it.
///
/// Set outputs used to be named `setOutputPrefix + middle + setOutputSuffix` by the output
/// itself, ignoring any Rename node. A Rename node now names them, working on the set's
/// full name (the input prefix plus the middle), so an output whose name would change gets
/// a Rename that produces the old one: the output prefix, the set's name with the input
/// prefix taken out, then the output suffix. The fields are cleared either way.
///
/// It also warns about Rename nodes that are not on an output's image path. Those used to
/// rename every output and now rename none.
fn migrate_set_naming(graph: &mut Graph, warnings: &mut Vec<String>) -> bool {
    let text = |params: &Params, name: &str| match params.get(name) {
        Some(ParamValue::String(value)) => value.clone(),
        _ => String::new(),
    };
    let mut changed = false;
    let outputs: Vec<String> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput))
        .map(|n| n.id.clone())
        .collect();
    for output_id in &outputs {
        let index = graph.nodes.iter().position(|n| n.id == *output_id).unwrap();
        let params = &graph.nodes[index].data.params;
        let out_prefix = text(params, "setOutputPrefix");
        let out_suffix = text(params, "setOutputSuffix");
        // The fields mark a file saved under the old naming; one saved since has neither,
        // and its outputs are already named by their Rename nodes.
        let legacy =
            params.contains_key("setOutputPrefix") || params.contains_key("setOutputSuffix");
        // Empty fields are what every output used to be saved with; dropping them changes
        // nothing a run does, so it does not count as a change to the document.
        let had_fields = !out_prefix.is_empty() || !out_suffix.is_empty();
        let set_prefix = graph::image_upstream(graph, output_id)
            .iter()
            .filter_map(|id| graph.nodes.iter().find(|n| n.id == *id))
            .find(|n| n.data.definition_id == "process_as_set")
            .map(|set| text(&set.data.params, "prefix"));
        let params = &mut graph.nodes[index].data.params;
        params.remove("setOutputPrefix");
        params.remove("setOutputSuffix");
        changed |= had_fields;
        let Some(set_prefix) = set_prefix.filter(|_| legacy) else {
            continue;
        };
        if out_prefix.is_empty() && out_suffix.is_empty() && set_prefix.is_empty() {
            continue;
        }
        let Some(edge) = graph
            .edges
            .iter()
            .position(|e| e.target == *output_id && e.target_handle == "in:input")
        else {
            continue;
        };
        let mut blocks = Vec::new();
        if !out_prefix.is_empty() {
            blocks.push(RenameBlock::Text { value: out_prefix });
        }
        blocks.push(RenameBlock::Oldname {
            find: set_prefix,
            replace_with: String::new(),
        });
        if !out_suffix.is_empty() {
            blocks.push(RenameBlock::Text { value: out_suffix });
        }
        let id = unique_id(graph, "rename-set");
        let output = &graph.nodes[index];
        let node = GraphNode {
            id: id.clone(),
            kind: NodeKind::Processing(ProcessingNodeKind::Process),
            // Under the output, where it does not sit on the wire it now interrupts.
            position: Position {
                x: output.position.x - 40.0,
                y: output.position.y + 150.0,
            },
            parent_id: output.parent_id.clone(),
            extent: output.extent.clone(),
            width: None,
            height: None,
            data: NodeData {
                label: "Rename".into(),
                definition_id: "rename".into(),
                params: Params::from([(
                    "blocks".into(),
                    ParamValue::Structured(StructuredParam::RenameBlocks { blocks }),
                )]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        graph.nodes.push(node);
        graph.edges[edge].target = id.clone();
        let edge_id = unique_id(graph, "e-rename-set");
        graph.edges.push(GraphEdge {
            id: edge_id,
            source: id.clone(),
            source_handle: "out:output".into(),
            target: output_id.clone(),
            target_handle: "in:input".into(),
        });
        warnings.push(format!(
            "Image Output {output_id}: its set naming fields now live in Rename node {id}"
        ));
        changed = true;
    }
    if !outputs.is_empty() {
        let on_a_path: BTreeSet<String> = outputs
            .iter()
            .flat_map(|output| graph::image_upstream(graph, output))
            .collect();
        for node in &graph.nodes {
            if node.data.definition_id == "rename" && !on_a_path.contains(&node.id) {
                warnings.push(format!(
                    "Rename node {} is not wired into an Image Output, so it renames nothing",
                    node.id
                ));
            }
        }
    }
    changed
}

/// An id made from `stem` that no node or edge in the graph uses yet.
fn unique_id(graph: &Graph, stem: &str) -> String {
    (1..)
        .map(|n| format!("{stem}-{n}"))
        .find(|id| {
            !graph.nodes.iter().any(|n| n.id == *id) && !graph.edges.iter().any(|e| e.id == *id)
        })
        .unwrap()
}
fn migrate_graph(
    graph: &mut Value,
    registry: &Registry,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let nodes = graph["nodes"]
        .as_array_mut()
        .ok_or("workflow nodes must be an array")?;
    for n in nodes.iter_mut() {
        let kind = n["type"].as_str().unwrap_or("").to_owned();
        if kind == "workflow-input" || n["id"] == "workflow-input" {
            n["type"] = json!("inputNode");
        }
        if kind == "workflow-output" || n["id"] == "workflow-output" {
            n["type"] = json!("imageOutputNode");
        }
        if kind == "processNode" {
            n["type"] = json!("process");
        }
        n.as_object_mut()
            .ok_or("node must be an object")?
            .retain(|k, _| {
                [
                    "id", "type", "position", "parentId", "extent", "width", "height", "data",
                ]
                .contains(&k.as_str())
            });
        n["data"]
            .as_object_mut()
            .ok_or("node data must be an object")?
            .retain(|k, _| ["label", "definitionId", "params"].contains(&k.as_str()));
        if n["data"].get("definitionId").is_none() {
            n["data"]["definitionId"] = json!("");
        }
        if n["data"].get("params").is_none() {
            n["data"]["params"] = json!({});
        }
        let id = n["data"]["definitionId"].as_str().unwrap_or("").to_owned();
        let kind = n["type"]
            .as_str()
            .ok_or("node type must be a string")?
            .to_owned();
        let p = n["data"]["params"]
            .as_object_mut()
            .ok_or("params must be an object")?;
        let forbidden: Vec<_> = p.keys().filter(|k| k.starts_with("__")).cloned().collect();
        for k in forbidden {
            p.remove(&k);
            warnings.push(format!("removed legacy executable parameter {k}"));
        }
        if id == "format_convert" {
            if let Some(quality) = p.get("quality").cloned() {
                let key = match p.get("format").and_then(Value::as_str).unwrap_or("PNG") {
                    "JPEG" => Some("jpeg_quality"),
                    "WEBP" => Some("webp_quality"),
                    "AVIF" => Some("avif_quality"),
                    _ => None,
                };
                if let Some(key) = key {
                    p.entry(key).or_insert(quality);
                }
                p.remove("quality");
            }
        }
        if id == "rename" {
            let blocks = p.remove("blocks").unwrap_or(json!([]));
            p.insert(
                "blocks".into(),
                json!({"type":"rename_blocks","blocks":blocks}),
            );
        }
        if id == "process_as_set" {
            let suffixes = p.remove("suffixes").unwrap_or(json!([]));
            p.insert(
                "suffixes".into(),
                json!({"type":"set_suffixes","suffixes":suffixes}),
            );
        }
        if kind == "textOutputNode" {
            let slots = p.remove("portIds").unwrap_or(json!(["txo-0"]));
            let slots: Vec<_> = slots
                .as_array()
                .ok_or("text portIds must be an array")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .unwrap_or("")
                        .trim_start_matches("txo-")
                        .to_owned()
                })
                .collect();
            p.insert("portIds".into(), json!({"type":"text_slots","slots":slots}));
        }
        if kind == "flipbookOutputNode" && p.get("bgColor").is_some_and(Value::is_string) {
            p.insert("bgColor".into(), json!([0, 0, 0, 0]));
        }
        if let Some(d) = registry.nodes.get(&id) {
            for param in &d.definition.params {
                if param.kind == ParamType::Enum {
                    if let Some(value) = p.get_mut(&param.name) {
                        if value.is_number() {
                            *value = json!(value.to_string());
                        }
                    }
                }
            }
        }
        if id == "process_as_set" {
            let suffixes = n["data"]["params"]["suffixes"]["suffixes"]
                .as_array()
                .ok_or("suffixes must be an array")?;
            let outputs:Vec<_>=suffixes.iter().enumerate().map(|(i,s)|json!({"name":format!("suffix_{i}"),"type":"image","label":s.as_str().unwrap_or("")})).collect();
            n["data"]["outputs"] = json!(outputs);
        }
    }
    let nodes: BTreeMap<String, Value> = nodes
        .iter()
        .map(|n| (n["id"].as_str().unwrap_or("").to_owned(), n.clone()))
        .collect();
    let edges = graph["edges"]
        .as_array_mut()
        .ok_or("workflow edges must be an array")?;
    edges.retain(|e| {
        !e["targetHandle"]
            .as_str()
            .unwrap_or("")
            .starts_with("param-in-__")
            && !e["sourceHandle"]
                .as_str()
                .unwrap_or("")
                .starts_with("param-out-__")
    });
    for e in edges {
        e.as_object_mut()
            .ok_or("edge must be an object")?
            .retain(|k, _| {
                ["id", "source", "sourceHandle", "target", "targetHandle"].contains(&k.as_str())
            });
        for (endpoint, handle, source) in [
            ("source", "sourceHandle", true),
            ("target", "targetHandle", false),
        ] {
            let n = nodes
                .get(
                    e[endpoint]
                        .as_str()
                        .ok_or("edge endpoint must be a string")?,
                )
                .ok_or("edge references unknown node")?;
            let old = e[handle]
                .as_str()
                .unwrap_or(if source { "out-0" } else { "in-0" });
            e[handle] = json!(migrate_handle(n, old, source, registry)?);
        }
    }
    if graph.get("viewport").is_none() {
        graph["viewport"] = json!({"x":0,"y":0,"zoom":1});
    }
    graph
        .as_object_mut()
        .unwrap()
        .retain(|k, _| ["nodes", "edges", "viewport"].contains(&k.as_str()));
    Ok(())
}
fn migrate_handle(
    node: &Value,
    old: &str,
    source: bool,
    registry: &Registry,
) -> Result<String, String> {
    if let Some(name) = old
        .strip_prefix("param-in-")
        .or_else(|| old.strip_prefix("param-out-"))
    {
        return Ok(format!("param:{name}"));
    }
    if let Some(slot) = old.strip_prefix("txo-") {
        return Ok(format!("txo:{slot}"));
    }
    if old == "folder-in" {
        return Ok("in:folder".into());
    }
    if old == "prefix-in" {
        return Ok("param:prefix".into());
    }
    if let Some(i) = old.strip_prefix("suf-in-") {
        return Ok(format!("param:suffix_{i}"));
    }
    let dir = if source { "out" } else { "in" };
    let index = old
        .strip_prefix(&format!("{dir}-"))
        .ok_or_else(|| format!("unknown v1 handle {old}"))?
        .parse::<usize>()
        .map_err(|_| format!("invalid port index {old}"))?;
    let id = node["data"]["definitionId"].as_str().unwrap_or("");
    if source && id == "process_as_set" {
        let ports = node["data"]["outputs"].as_array().unwrap();
        let port = ports.get(index).ok_or("dynamic set port out of range")?;
        return Ok(format!("out:{}", port["name"].as_str().unwrap()));
    }
    if let Some(def) = registry.nodes.get(id) {
        let ports = if source {
            &def.definition.outputs
        } else {
            &def.definition.inputs
        };
        if let Some(p) = ports.get(index) {
            return Ok(format!("{dir}:{}", p.name));
        }
    }
    if index == 0
        && [
            "inputNode",
            "imageOutputNode",
            "textOutputNode",
            "flipbookOutputNode",
            "folderPathNode",
        ]
        .contains(&node["type"].as_str().unwrap_or(""))
    {
        return Ok(format!("{dir}:{}", if source { "output" } else { "input" }));
    }
    Err(format!("node {}: cannot migrate handle {old}", node["id"]))
}
