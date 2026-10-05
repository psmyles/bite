use bite_core::{graph, workflow, Registry};
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn registry() -> Registry {
    Registry::load(
        &root().join("node-definitions"),
        &root().join("format-definitions"),
    )
    .unwrap()
}
#[test]
fn migrates_every_fixture_without_mutation_and_matches_traversal_goldens() {
    let registry = registry();
    let mut count = 0;
    for f in fs::read_dir(root().join("test-workflows")).unwrap() {
        let f = f.unwrap();
        if f.path().extension().and_then(|s| s.to_str()) != Some("bite") {
            continue;
        }
        let text = fs::read_to_string(f.path()).unwrap();
        let loaded = workflow::load(&text, &registry)
            .unwrap_or_else(|e| panic!("{}: {e}", f.path().display()));
        assert!(loaded.migrated);
        let golden: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root()
                    .join("tests/golden/workflows")
                    .join(f.path().file_stem().unwrap())
                    .with_extension("json"),
            )
            .unwrap(),
        )
        .unwrap();
        let g = &loaded.workflow.graph;
        assert_eq!(g.nodes.len(), golden["nodes"].as_u64().unwrap() as usize);
        assert_eq!(g.edges.len(), golden["edges"].as_u64().unwrap() as usize);
        assert_eq!(
            serde_json::json!(graph::topo_sort(g).unwrap()),
            golden["execution_order"]
        );
        for o in golden["outputs"].as_array().unwrap() {
            let id = o["id"].as_str().unwrap();
            assert_eq!(serde_json::json!(graph::trace_input(g, id)), o["input"]);
            assert_eq!(
                serde_json::json!(graph::trace(g, &[id.into()], true)),
                o["contributors"]
            );
        }
        for d in golden["descendants"].as_array().unwrap() {
            assert_eq!(
                serde_json::json!(graph::trace(g, &[d["id"].as_str().unwrap().into()], false)),
                d["nodes"]
            );
        }
        let saved = serde_json::to_string(&loaded.workflow).unwrap();
        assert!(!workflow::load(&saved, &registry).unwrap().migrated);
        assert_eq!(text, fs::read_to_string(f.path()).unwrap());
        count += 1;
    }
    assert_eq!(count, 11);
}
#[test]
fn rejects_cycles_incompatible_connections_and_unsupported_versions() {
    let r = registry();
    let text = fs::read_to_string(root().join("test-workflows/wf-01-fastpath.bite")).unwrap();
    let mut w = workflow::load(&text, &r).unwrap().workflow;
    let mut e = w.graph.edges[0].clone();
    e.id = "cycle".into();
    std::mem::swap(&mut e.source, &mut e.target);
    w.graph.edges.push(e);
    assert!(graph::topo_sort(&w.graph).unwrap_err().contains("cycle"));
    assert!(!workflow::compatible_v1("0.3.1"));
    assert!(workflow::compatible_v1("0.4.3"));
    assert!(!workflow::compatible_v1("garbage"));
    for (a, b, expected) in [
        (graph::WireType::Bool, graph::WireType::Number, true),
        (graph::WireType::Image, graph::WireType::String, false),
        (graph::WireType::Vector3, graph::WireType::Numeric, true),
    ] {
        assert_eq!(graph::compatible(a, b), expected);
    }
}

#[test]
fn value_wires_constrain_siblings_and_downstream_consumers() {
    use serde_json::json;
    let r = registry();
    let node = |id: &str, kind: &str, def: &str| json!({"id":id,"type":kind,"position":{"x":0,"y":0},"data":{"label":id,"definitionId":def,"params":{}}});
    let edge = |id: &str, source: &str, sh: &str, target: &str, th: &str| json!({"id":id,"source":source,"sourceHandle":sh,"target":target,"targetHandle":th});
    let mut g: bite_schema::Graph = serde_json::from_value(json!({
        "viewport":{"x":0,"y":0,"zoom":1},
        "nodes":[node("number","process","value_float"),node("branch","process","logic_branch"),node("image","inputNode",""),node("output","imageOutputNode","")],
        "edges":[edge("a","number","param:value","branch","param:value_true")]
    })).unwrap();
    graph::validate(&g, &r).unwrap();
    g.edges.push(
        serde_json::from_value(edge(
            "b",
            "image",
            "out:output",
            "branch",
            "param:value_false",
        ))
        .unwrap(),
    );
    assert!(
        graph::validate(&g, &r).is_err(),
        "image and number siblings must not mix"
    );
    g.edges.pop();
    g.edges.push(
        serde_json::from_value(edge("b", "branch", "param:result", "output", "in:input")).unwrap(),
    );
    assert!(
        graph::validate(&g, &r).is_err(),
        "number-constrained value cannot drive image input"
    );
    g.edges.pop();
    g.nodes[0].data.params.insert(
        "value".into(),
        bite_schema::ParamValue::String("not a number".into()),
    );
    assert!(graph::validate(&g, &r)
        .unwrap_err()
        .contains("invalid value"));
}

#[test]
fn rejects_group_containment_cycles() {
    let r = registry();
    let mut w = workflow::load(
        &fs::read_to_string(root().join("test-workflows/wf-01-fastpath.bite")).unwrap(),
        &r,
    )
    .unwrap()
    .workflow;
    let mut a = w.graph.nodes[0].clone();
    a.id = "group-a".into();
    a.kind = bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::Group);
    a.data.params.clear();
    a.parent_id = Some("group-b".into());
    let mut b = a.clone();
    b.id = "group-b".into();
    b.parent_id = Some("group-a".into());
    w.graph.nodes.extend([a, b]);
    assert!(w
        .validate()
        .unwrap_err()
        .iter()
        .any(|e| e.contains("containment cycle")));
}

#[test]
fn legacy_migration_covers_structures_quality_enums_and_special_handles() {
    use bite_schema::{BuiltinNodeKind as B, NodeKind, ParamValue, StructuredParam};
    use serde_json::json;
    let r = registry();

    let mut set_doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root().join("test-workflows/wf-06-setmode.bite")).unwrap(),
    )
    .unwrap();
    let nodes = set_doc["graph"]["nodes"].as_array_mut().unwrap();
    nodes.iter_mut().find(|n| n["id"] == "input-1").unwrap()["type"] = json!("workflow-input");
    nodes
        .iter_mut()
        .find(|n| n["id"] == "imageOutput-1")
        .unwrap()["type"] = json!("workflow-output");
    nodes.iter_mut().find(|n| n["id"] == "negate-1").unwrap()["type"] = json!("processNode");
    let set = nodes.iter_mut().find(|n| n["id"] == "set-1").unwrap();
    set["data"]["params"]["__legacy_code"] = json!("must be removed");
    set["legacyUiState"] = json!({"ignored":true});
    nodes.iter_mut().find(|n| n["id"] == "merge-1").unwrap()["data"]["params"]["channels"] =
        json!(3);
    nodes.extend([
        json!({"id":"string-1","type":"processNode","position":{"x":0,"y":0},"data":{"label":"String","definitionId":"value_string","params":{"value":"wired"}}}),
        json!({"id":"folder-1","type":"folderPathNode","position":{"x":0,"y":0},"data":{"label":"Folder","definitionId":"folderpath","params":{"folderPath":"C:\\output"}}}),
    ]);
    set_doc["graph"]["edges"].as_array_mut().unwrap().extend([
        json!({"id":"prefix","source":"string-1","sourceHandle":"param-out-value","target":"set-1","targetHandle":"prefix-in"}),
        json!({"id":"suffix","source":"string-1","sourceHandle":"param-out-value","target":"set-1","targetHandle":"suf-in-0"}),
        json!({"id":"folder","source":"folder-1","sourceHandle":"out-0","target":"imageOutput-1","targetHandle":"folder-in"}),
        json!({"id":"executable","source":"string-1","sourceHandle":"param-out-value","target":"set-1","targetHandle":"param-in-__legacy"}),
    ]);
    let loaded = workflow::load(&set_doc.to_string(), &r).unwrap();
    assert!(loaded
        .warnings
        .iter()
        .any(|warning| warning.contains("__legacy_code")));
    let graph = &loaded.workflow.graph;
    assert!(matches!(
        graph.nodes.iter().find(|n| n.id == "input-1").unwrap().kind,
        NodeKind::Builtin(B::Input)
    ));
    assert!(matches!(
        graph
            .nodes
            .iter()
            .find(|n| n.id == "imageOutput-1")
            .unwrap()
            .kind,
        NodeKind::Builtin(B::ImageOutput)
    ));
    let set = graph.nodes.iter().find(|n| n.id == "set-1").unwrap();
    assert!(matches!(
        set.data.params["suffixes"],
        ParamValue::Structured(StructuredParam::SetSuffixes { .. })
    ));
    assert!(!set.data.params.contains_key("__legacy_code"));
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|n| n.id == "merge-1")
            .unwrap()
            .data
            .params["channels"],
        ParamValue::String("3".into())
    );
    let handles: Vec<_> = graph
        .edges
        .iter()
        .map(|edge| {
            (
                edge.id.as_str(),
                edge.source_handle.as_str(),
                edge.target_handle.as_str(),
            )
        })
        .collect();
    assert!(handles.contains(&("prefix", "param:value", "param:prefix")));
    assert!(handles.contains(&("suffix", "param:value", "param:suffix_0")));
    assert!(handles.contains(&("folder", "out:output", "in:folder")));
    assert!(!handles.iter().any(|(id, _, _)| *id == "executable"));

    let mut formats: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root().join("test-workflows/wf-08-formats.bite")).unwrap(),
    )
    .unwrap();
    let jpeg = formats["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["data"]["params"]["format"] == "JPEG")
        .unwrap();
    jpeg["data"]["params"]
        .as_object_mut()
        .unwrap()
        .remove("jpeg_quality");
    jpeg["data"]["params"]["quality"] = json!(71);
    let loaded = workflow::load(&formats.to_string(), &r).unwrap();
    let jpeg = loaded
        .workflow
        .graph
        .nodes
        .iter()
        .find(|node| node.data.params.get("format") == Some(&ParamValue::String("JPEG".into())))
        .unwrap();
    assert_eq!(jpeg.data.params["jpeg_quality"], ParamValue::Int(71));
    assert!(!jpeg.data.params.contains_key("quality"));

    let text = workflow::load(
        &fs::read_to_string(root().join("test-workflows/wf-02-props-light.bite")).unwrap(),
        &r,
    )
    .unwrap();
    let output = text
        .workflow
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Builtin(B::TextOutput))
        .unwrap();
    assert!(matches!(
        output.data.params["portIds"],
        ParamValue::Structured(StructuredParam::TextSlots { .. })
    ));
    assert!(text
        .workflow
        .graph
        .edges
        .iter()
        .any(|edge| edge.target_handle == "txo:0" && edge.source_handle == "param:value"));

    let mut flipbook: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root().join("test-workflows/wf-10-flipbook.bite")).unwrap(),
    )
    .unwrap();
    let output = flipbook["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "flipbookOutputNode")
        .unwrap();
    output["data"]["params"]["bgColor"] = json!("legacy-css-color");
    let loaded = workflow::load(&flipbook.to_string(), &r).unwrap();
    let output = loaded
        .workflow
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Builtin(B::FlipbookOutput))
        .unwrap();
    assert_eq!(
        output.data.params["bgColor"],
        ParamValue::Vector(vec![0.0; 4])
    );
}

/// A hand-edited v1 graph whose node has lost its `type` is a load error, not a panic: the
/// loader used to unwrap the field after only conditionally putting it back.
#[test]
fn a_v1_node_without_a_type_is_rejected_rather_than_panicking() {
    let r = registry();
    let mut doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root().join("test-workflows/wf-01-fastpath.bite")).unwrap(),
    )
    .unwrap();
    // A node the migration does not rename by id, so nothing puts a type back for it.
    let node = doc["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "blur-1783250589996")
        .unwrap();
    node.as_object_mut().unwrap().remove("type");
    let error = workflow::load(&doc.to_string(), &r).unwrap_err();
    assert!(error.contains("node type"), "{error}");
}
