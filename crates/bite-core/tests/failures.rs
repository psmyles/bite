use bite_core::{
    execution::{self, ImageHost, RunOptions},
    workflow, Registry,
};
use bite_expr::Context;
use bite_schema::{GraphEdge, ParamValue, StructuredParam};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

struct FailingHost {
    cancelled: Arc<AtomicBool>,
    cancel_on_failure: bool,
    calls: usize,
}
impl ImageHost for FailingHost {
    fn metadata(&mut self, _: &Path, _: bool) -> Result<Context, String> {
        Ok(Context::new())
    }
    fn capture(&mut self, _: &[String]) -> Result<String, String> {
        Err("unexpected capture".into())
    }
    fn run(&mut self, args: &[String]) -> Result<(), String> {
        self.calls += 1;
        fs::write(args.last().unwrap(), b"partial or successful image").unwrap();
        if self.calls == 1 {
            if self.cancel_on_failure {
                self.cancelled.store(true, Ordering::Relaxed);
            }
            Err("injected image failure".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_images_preserve_existing_outputs_and_cancellation_still_stops_batch() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions"),
        &root.join("format-definitions"),
    )
    .unwrap();
    let graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-01-fastpath.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    for cancel in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("in");
        let output = temp.path().join("out");
        fs::create_dir(&input).unwrap();
        fs::create_dir(&output).unwrap();
        for name in ["a.png", "b.png"] {
            fs::write(input.join(name), b"input").unwrap();
        }
        fs::write(output.join("a.png"), b"original").unwrap();
        let options = RunOptions {
            named_paths: [("in".into(), input), ("out".into(), output.clone())].into(),
            overwrite: true,
            ..RunOptions::default()
        };
        let mut host = FailingHost {
            cancelled: options.cancelled.clone(),
            cancel_on_failure: cancel,
            calls: 0,
        };
        let result =
            execution::run_workflow(&graph, &registry, &mut host, &options, &mut |_, _, _| {});
        assert_eq!(fs::read(output.join("a.png")).unwrap(), b"original");
        if cancel {
            assert!(result.is_err());
            assert_eq!(host.calls, 1);
            assert!(!output.join("b.png").exists());
        } else {
            let result = result.unwrap();
            assert_eq!((result.processed, result.skipped, result.failed), (1, 0, 1));
            assert_eq!(result.errors.len(), 1);
            assert!(result.errors[0].contains("a.png"));
            assert_eq!(result.outputs, vec![output.join("b.png")]);
            assert_eq!(host.calls, 2);
        }
        assert_eq!(
            fs::read_dir(&output).unwrap().count(),
            if cancel { 1 } else { 2 },
            "temporary output leaked"
        );
    }
}

#[test]
fn text_output_matches_legacy_empty_and_fully_filtered_behavior() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions"),
        &root.join("format-definitions"),
    )
    .unwrap();
    let mut graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-11-compute.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    let input_id = graph
        .nodes
        .iter()
        .find(|node| {
            matches!(
                node.kind,
                bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::Input)
            )
        })
        .unwrap()
        .id
        .clone();
    let output_id = graph
        .nodes
        .iter()
        .find(|node| {
            matches!(
                node.kind,
                bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::TextOutput)
            )
        })
        .unwrap()
        .id
        .clone();
    let input_node = graph
        .nodes
        .iter()
        .find(|node| node.id == input_id)
        .unwrap()
        .clone();
    let mut output_node = graph
        .nodes
        .iter()
        .find(|node| node.id == output_id)
        .unwrap()
        .clone();
    output_node.data.params.insert(
        "portIds".into(),
        ParamValue::Structured(StructuredParam::TextSlots {
            slots: vec!["0".into(), "new".into()],
        }),
    );
    let empty_node = serde_json::from_value(serde_json::json!({
        "id":"empty","type":"process","position":{"x":0,"y":0},
        "data":{"label":"Empty","definitionId":"value_string","params":{"value":""}}
    }))
    .unwrap();
    graph.nodes = vec![input_node, empty_node, output_node];
    graph.edges = vec![
        GraphEdge {
            id: "input-output".into(),
            source: input_id.clone(),
            source_handle: "out:output".into(),
            target: output_id.clone(),
            target_handle: "in:input".into(),
        },
        GraphEdge {
            id: "empty-slot".into(),
            source: "empty".into(),
            source_handle: "param:value".into(),
            target: output_id.clone(),
            target_handle: "txo:0".into(),
        },
    ];

    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("in");
    let report = temp.path().join("report.txt");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("a.png"), b"input").unwrap();
    let options = RunOptions {
        named_paths: [("in".into(), input), ("report".into(), report.clone())].into(),
        overwrite: true,
        ..RunOptions::default()
    };
    let mut host = FailingHost {
        cancelled: options.cancelled.clone(),
        cancel_on_failure: false,
        calls: 0,
    };
    let error = execution::run_workflow(&graph, &registry, &mut host, &options, &mut |_, _, _| {})
        .unwrap_err();
    assert!(error.contains("All values resolved to empty"));
    assert!(!report.exists());

    let false_node = serde_json::from_value(serde_json::json!({
        "id":"false","type":"process","position":{"x":0,"y":0},
        "data":{"label":"False","definitionId":"value_boolean","params":{"value":false}}
    }))
    .unwrap();
    graph.nodes.push(false_node);
    graph.edges.push(GraphEdge {
        id: "condition".into(),
        source: "false".into(),
        source_handle: "param:value".into(),
        target: output_id,
        target_handle: "txo:condition".into(),
    });
    let result =
        execution::run_workflow(&graph, &registry, &mut host, &options, &mut |_, _, _| {}).unwrap();
    assert_eq!((result.processed, result.skipped, result.failed), (0, 1, 0));
    assert!(result.outputs.is_empty());
    assert!(!report.exists());
}

#[test]
fn failed_atlas_process_is_fatal_and_preserves_existing_output() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions"),
        &root.join("format-definitions"),
    )
    .unwrap();
    let graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-10-flipbook.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("in");
    let output = temp.path().join("atlas.png");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("frame.png"), b"input").unwrap();
    fs::write(&output, b"original atlas").unwrap();
    let mut named_paths = std::collections::BTreeMap::new();
    for node in &graph.nodes {
        if let Some(ParamValue::String(name)) = node.data.params.get("cliName") {
            named_paths.insert(
                name.clone(),
                if matches!(
                    node.kind,
                    bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::Input)
                ) {
                    input.clone()
                } else {
                    output.clone()
                },
            );
        }
    }
    let options = RunOptions {
        named_paths,
        overwrite: true,
        ..RunOptions::default()
    };
    let mut host = FailingHost {
        cancelled: options.cancelled.clone(),
        cancel_on_failure: false,
        calls: 0,
    };
    let error = execution::run_workflow(&graph, &registry, &mut host, &options, &mut |_, _, _| {})
        .unwrap_err();
    assert!(error.contains("injected image failure"));
    assert_eq!(host.calls, 1);
    assert_eq!(fs::read(&output).unwrap(), b"original atlas");
    assert_eq!(
        fs::read_dir(temp.path()).unwrap().count(),
        2,
        "temporary atlas leaked"
    );
}
