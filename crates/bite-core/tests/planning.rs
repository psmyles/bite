use bite_core::{
    execution::RunOptions,
    plan::{self, PlannedEvent, PlanningFacts},
    workflow, Registry,
};
use std::{fs, path::PathBuf};

#[test]
fn concrete_plan_matches_original_commands_without_writing_outputs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    let graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-01-fastpath.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    let golden: Vec<Vec<String>> = serde_json::from_str(
        &fs::read_to_string(root.join("tests/golden/workflows/wf-01-magick.json")).unwrap(),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fixtures/main");
    let output = dir.path().join("wf-01");
    fs::create_dir_all(&input).unwrap();
    for command in &golden {
        fs::write(
            input.join(command[0].rsplit('/').next().unwrap()),
            b"planning only",
        )
        .unwrap();
    }
    let mut options = RunOptions::default();
    for node in &graph.nodes {
        if let Some(bite_schema::ParamValue::String(name)) = node.data.params.get("cliName") {
            options.named_paths.insert(
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
    let facts = PlanningFacts::default();
    let mut metadata = |_: &std::path::Path, _: bool| {
        Ok(bite_expr::Context::from([(
            "image.name".into(),
            bite_expr::Value::String("image.png".into()),
        )]))
    };
    let result = plan::concrete(&graph, &registry, &options, &facts, &mut metadata).unwrap();
    assert!(result.complete);
    assert!(
        !output.exists(),
        "planning must not create even an output directory"
    );
    let prefix = dir.path().to_string_lossy().replace('\\', "/");
    let commands: Vec<Vec<String>> = result
        .events
        .iter()
        .filter_map(|event| match event {
            PlannedEvent::Output {
                process_args: Some(args),
                ..
            } => Some(
                args.iter()
                    .map(|arg| arg.replace('\\', "/").replace(&prefix, "<run>"))
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(commands, golden);
}

#[test]
fn analysis_dependencies_are_explicit_and_can_be_replayed() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    let fixture = fs::read_dir(root.join("test-workflows"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("wf-05")
                && p.extension().is_some_and(|e| e == "bite")
        })
        .unwrap();
    let graph = workflow::load(&fs::read_to_string(fixture).unwrap(), &registry)
        .unwrap()
        .workflow
        .graph;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("image.png"), b"planning only").unwrap();
    let output = dir.path().join("output");
    let mut options = RunOptions::default();
    for node in &graph.nodes {
        if let Some(bite_schema::ParamValue::String(name)) = node.data.params.get("cliName") {
            options.named_paths.insert(
                name.clone(),
                if matches!(
                    node.kind,
                    bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::Input)
                ) {
                    input.clone()
                } else {
                    output.join(name)
                },
            );
        }
    }
    let mut facts = PlanningFacts::default();
    let mut saw_dependency = false;
    for _ in 0..10 {
        let mut metadata = |_: &std::path::Path, _: bool| {
            Ok(bite_expr::Context::from([(
                "image.name".into(),
                bite_expr::Value::String("image.png".into()),
            )]))
        };
        let result = plan::concrete(&graph, &registry, &options, &facts, &mut metadata).unwrap();
        assert!(!output.exists());
        if result.complete {
            assert!(saw_dependency);
            return;
        }
        saw_dependency = true;
        assert!(result.requires_analysis.is_some());
        assert!(result.summary.is_none());
        let args = result
            .events
            .into_iter()
            .find_map(|e| match e {
                PlannedEvent::Capture { args, value: None } => Some(args),
                _ => None,
            })
            .expect("an unresolved analysis command must be recorded");
        facts.captures.push(plan::CaptureFact {
            args,
            value: "0.5".into(),
        });
        for invalid in ["NaN", "inf", "-0.1", "1.1", "not a mean"] {
            facts.captures.last_mut().unwrap().value = invalid.into();
            let mut metadata = |_: &std::path::Path, _: bool| {
                Ok(bite_expr::Context::from([(
                    "image.name".into(),
                    bite_expr::Value::String("image.png".into()),
                )]))
            };
            assert!(
                plan::concrete(&graph, &registry, &options, &facts, &mut metadata)
                    .unwrap_err()
                    .contains("Invalid planning mean")
            );
        }
        facts.captures.last_mut().unwrap().value = "0.5".into();
    }
    panic!("explicit facts did not complete the plan");
}

#[test]
fn format_and_atlas_plans_match_original_command_goldens() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    for id in ["wf-08", "wf-10"] {
        let fixture = fs::read_dir(root.join("test-workflows"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name().unwrap().to_string_lossy().starts_with(id)
                    && p.extension().is_some_and(|e| e == "bite")
            })
            .unwrap();
        let graph = workflow::load(&fs::read_to_string(fixture).unwrap(), &registry)
            .unwrap()
            .workflow
            .graph;
        let mut golden: Vec<Vec<String>> = serde_json::from_str(
            &fs::read_to_string(root.join(format!("tests/golden/workflows/{id}-magick.json")))
                .unwrap(),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join(if id == "wf-10" {
            "fixtures/flip"
        } else {
            "fixtures/main"
        });
        let output = dir.path().join(id);
        fs::create_dir_all(&input).unwrap();
        for arg in golden
            .iter()
            .flatten()
            .filter(|a| a.starts_with("<run>/fixtures/"))
        {
            fs::write(
                input.join(arg.rsplit('/').next().unwrap()),
                b"planning only",
            )
            .unwrap();
        }
        let mut options = RunOptions::default();
        for node in &graph.nodes {
            if let Some(bite_schema::ParamValue::String(name)) = node.data.params.get("cliName") {
                let path = match node.kind {
                    bite_schema::NodeKind::Builtin(bite_schema::BuiltinNodeKind::Input) => {
                        input.clone()
                    }
                    bite_schema::NodeKind::Builtin(
                        bite_schema::BuiltinNodeKind::FlipbookOutput,
                    ) => output.join("atlas.png"),
                    _ => output.join(name.trim_start_matches("out-")),
                };
                options.named_paths.insert(name.clone(), path);
            }
        }
        let facts = PlanningFacts::default();
        let mut metadata = |_: &std::path::Path, _: bool| Ok(bite_expr::Context::new());
        let plan = plan::concrete(&graph, &registry, &options, &facts, &mut metadata).unwrap();
        assert!(plan.complete, "{id}");
        assert!(!output.exists());
        let prefix = dir.path().to_string_lossy().replace('\\', "/");
        let mut actual: Vec<Vec<String>> = plan
            .events
            .into_iter()
            .filter_map(|e| match e {
                PlannedEvent::Output {
                    process_args: Some(args),
                    ..
                } => Some(
                    args.into_iter()
                        .map(|a| a.replace('\\', "/").replace(&prefix, "<run>"))
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        // Explicit migration deviation: preserve stored PNG bit depth and avoid
        // ImageMagick's grayscale encoder corruption; never modify the golden.
        for args in &mut golden {
            if args.last().is_some_and(|a| a.starts_with("PNG:")) {
                let index = args.iter().position(|a| a == "-depth").unwrap();
                let depth = args[index + 1].clone();
                args.splice(
                    index..index + 2,
                    [
                        "-define".into(),
                        format!("png:bit-depth={depth}"),
                        "-define".into(),
                        "png:color-type=6".into(),
                    ],
                );
            }
        }
        actual.sort();
        golden.sort();
        assert_eq!(actual, golden, "{id}");
    }
}

#[test]
fn rejects_ambiguous_and_mistyped_planning_facts() {
    use bite_expr::Value;
    let mut facts = PlanningFacts::default();
    let path = std::env::current_dir().unwrap().join("image.png");
    facts.metadata.insert(
        path.clone(),
        [("image.width".into(), Value::String("1024".into()))].into(),
    );
    assert!(facts.validate().unwrap_err().contains("image.width"));
    facts.metadata.insert(
        path.clone(),
        [("image.width".into(), Value::Float(f64::NAN))].into(),
    );
    assert!(facts.validate().is_err());
    facts
        .metadata
        .insert(path, [("image.width".into(), Value::Int(1024))].into());
    facts.validate().unwrap();
    for _ in 0..2 {
        facts.captures.push(plan::CaptureFact {
            args: vec!["image.png".into()],
            value: "0.5".into(),
        });
    }
    assert!(facts.validate().unwrap_err().contains("duplicate"));
}

#[test]
fn native_copy_gate_rename_and_report_plans_preserve_fixture_behavior() {
    use bite_core::execution::OutputOperation;
    use bite_schema::{BuiltinNodeKind as B, NodeKind, ParamValue};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    let baseline: Vec<Vec<String>> = serde_json::from_str(
        &fs::read_to_string(root.join("tests/golden/workflows/wf-01-magick.json")).unwrap(),
    )
    .unwrap();
    let names: Vec<_> = baseline
        .iter()
        .map(|a| a[0].rsplit('/').next().unwrap().to_string())
        .collect();
    for id in ["wf-07", "wf-09", "wf-11"] {
        let fixture = fs::read_dir(root.join("test-workflows"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name().unwrap().to_string_lossy().starts_with(id)
                    && p.extension().is_some_and(|e| e == "bite")
            })
            .unwrap();
        let graph = workflow::load(&fs::read_to_string(fixture).unwrap(), &registry)
            .unwrap()
            .workflow
            .graph;
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("fixtures/main");
        let output = dir.path().join(id);
        fs::create_dir_all(&input).unwrap();
        for name in &names {
            fs::write(input.join(name), b"source sentinel").unwrap();
        }
        let mut options = RunOptions::default();
        for node in &graph.nodes {
            if let Some(ParamValue::String(name)) = node.data.params.get("cliName") {
                let path = match node.kind {
                    NodeKind::Builtin(B::Input) => input.clone(),
                    NodeKind::Builtin(B::TextOutput) => output.join("report.txt"),
                    _ if id == "wf-11" => output.join("images"),
                    _ => output.clone(),
                };
                options.named_paths.insert(name.clone(), path);
            }
        }
        let facts = PlanningFacts::default();
        let mut metadata = |path: &std::path::Path, _: bool| {
            Ok(bite_expr::Context::from([(
                "image.name".into(),
                bite_expr::Value::String(path.file_name().unwrap().to_string_lossy().into()),
            )]))
        };
        let result = plan::concrete(&graph, &registry, &options, &facts, &mut metadata).unwrap();
        assert!(result.complete);
        assert!(!output.exists());
        let mut copies = Vec::new();
        let mut commands = Vec::new();
        let mut reports = Vec::new();
        let prefix = dir.path().to_string_lossy().replace('\\', "/");
        for event in result.events {
            if let PlannedEvent::Output {
                operation,
                process_args,
            } = event
            {
                match operation {
                    OutputOperation::Copy { source, output } => {
                        assert_eq!(fs::read(&source).unwrap(), b"source sentinel");
                        assert!(process_args.is_none());
                        copies.push((
                            source.file_name().unwrap().to_string_lossy().into_owned(),
                            output.file_name().unwrap().to_string_lossy().into_owned(),
                        ));
                    }
                    OutputOperation::Text {
                        contents,
                        output: target,
                    } => {
                        assert_eq!(target, output.join("report.txt"));
                        reports.push(contents);
                    }
                    OutputOperation::Image { .. } => commands.push(
                        process_args
                            .unwrap()
                            .into_iter()
                            .map(|a| a.replace('\\', "/").replace(&prefix, "<run>"))
                            .collect::<Vec<_>>(),
                    ),
                }
            }
        }
        let mut golden: Vec<Vec<String>> = serde_json::from_str(
            &fs::read_to_string(root.join(format!("tests/golden/workflows/{id}-magick.json")))
                .unwrap(),
        )
        .unwrap();
        commands.sort();
        golden.sort();
        assert_eq!(commands, golden, "{id}");
        match id {
            "wf-07" => assert_eq!(copies, vec![("red_256.png".into(), "red_256.png".into())]),
            "wf-09" => assert_eq!(
                copies,
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.clone(), format!("test_{:03}_{n}", i + 1)))
                    .collect::<Vec<_>>()
            ),
            "wf-11" => {
                assert!(copies.is_empty());
                assert_eq!(reports.len(), 1);
                assert_eq!(
                    reports[0].lines().collect::<Vec<_>>(),
                    vec!["5,5,0.6,1024,OK"; 6]
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn solid_image_plan_records_the_documented_native_fill() {
    use bite_schema::ParamValue;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    let mut graph = workflow::load(
        &fs::read_to_string(root.join("test-workflows/wf-04-channels.bite")).unwrap(),
        &registry,
    )
    .unwrap()
    .workflow
    .graph;
    let node = graph
        .nodes
        .iter_mut()
        .find(|n| n.data.definition_id == "value_float")
        .unwrap();
    node.data.definition_id = "solid_image".into();
    node.data.params = [("value".into(), ParamValue::Number(0.25))].into();
    let solid_id = node.id.clone();
    for edge in graph.edges.iter_mut().filter(|e| e.source == solid_id) {
        edge.source_handle = "out:output".into();
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    fs::create_dir(&input).unwrap();
    let source = input.join("sample.png");
    fs::write(&source, b"planning only").unwrap();
    let options = RunOptions {
        named_paths: [("in".into(), input), ("out".into(), output.clone())].into(),
        ..RunOptions::default()
    };
    let facts = PlanningFacts::default();
    for enabled in [true, false] {
        graph
            .nodes
            .iter_mut()
            .find(|n| n.data.definition_id == "solid_image")
            .unwrap()
            .data
            .params
            .insert("_enabled".into(), ParamValue::Bool(enabled));
        let mut metadata = |_: &std::path::Path, _: bool| Ok(bite_expr::Context::new());
        let result = plan::concrete(&graph, &registry, &options, &facts, &mut metadata).unwrap();
        assert!(result.complete);
        assert!(!output.exists());
        let operations: Vec<_> = result
            .events
            .into_iter()
            .filter_map(|e| match e {
                PlannedEvent::Output {
                    operation,
                    process_args,
                } => Some((operation, process_args)),
                _ => None,
            })
            .collect();
        assert_eq!(operations.len(), 1);
        let args = operations[0].1.as_ref().unwrap();
        let expected = if enabled {
            vec!["-evaluate", "set", "25%"]
        } else {
            vec![]
        };
        assert_eq!(
            args.iter()
                .filter(|s| ["-evaluate", "set", "25%"].contains(&s.as_str()))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            args.iter()
                .filter(|a| **a == source.to_string_lossy())
                .count(),
            3
        );
    }
}

#[test]
fn channel_and_set_plans_record_fused_native_structure() {
    use bite_core::execution::OutputOperation;
    use bite_schema::{BuiltinNodeKind as B, NodeKind, ParamValue};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = Registry::load(
        &root.join("node-definitions-v2"),
        &root.join("format-definitions-v2"),
    )
    .unwrap();
    for (id, names) in [
        ("wf-04", vec!["sample.png"]),
        (
            "wf-06",
            vec![
                "set_alpha_diffuse.png",
                "set_alpha_normal.png",
                "set_alpha_rough.png",
                "set_beta_diffuse.png",
                "set_beta_normal.png",
                "set_beta_rough.png",
            ],
        ),
    ] {
        let fixture = fs::read_dir(root.join("test-workflows"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name().unwrap().to_string_lossy().starts_with(id)
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "bite")
            })
            .unwrap();
        let graph = workflow::load(&fs::read_to_string(fixture).unwrap(), &registry)
            .unwrap()
            .workflow
            .graph;
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input");
        let output = dir.path().join("output");
        fs::create_dir(&input).unwrap();
        for name in names {
            fs::write(input.join(name), b"planning only").unwrap();
        }
        let mut options = RunOptions::default();
        for node in &graph.nodes {
            if let Some(ParamValue::String(name)) = node.data.params.get("cliName") {
                options.named_paths.insert(
                    name.clone(),
                    if node.kind == NodeKind::Builtin(B::Input) {
                        input.clone()
                    } else {
                        output.clone()
                    },
                );
            }
        }
        let mut metadata = |_: &std::path::Path, _: bool| Ok(bite_expr::Context::new());
        let result = plan::concrete(
            &graph,
            &registry,
            &options,
            &PlanningFacts::default(),
            &mut metadata,
        )
        .unwrap();
        assert!(result.complete);
        assert!(!output.exists());
        let operations: Vec<_> = result
            .events
            .into_iter()
            .filter_map(|event| match event {
                PlannedEvent::Output {
                    operation: OutputOperation::Image { args, output, .. },
                    ..
                } => Some((args, output)),
                _ => None,
            })
            .collect();
        if id == "wf-04" {
            let source = input.join("sample.png").to_string_lossy().into_owned();
            assert_eq!(operations.len(), 1);
            assert_eq!(
                operations[0].0,
                [
                    "(",
                    &source,
                    "-channel",
                    "Red",
                    "-separate",
                    "+channel",
                    ")",
                    "(",
                    &source,
                    "-channel",
                    "Green",
                    "-separate",
                    "+channel",
                    "-negate",
                    ")",
                    "(",
                    &source,
                    "-evaluate",
                    "set",
                    "50%",
                    "-colorspace",
                    "Gray",
                    ")",
                    "-set",
                    "colorspace",
                    "sRGB",
                    "-combine",
                ]
                .map(str::to_owned)
            );
        } else {
            assert_eq!(operations.len(), 2);
            for (set, (args, target)) in ["alpha", "beta"].into_iter().zip(operations) {
                assert_eq!(
                    target.file_name().unwrap().to_string_lossy(),
                    format!("packed_{set}.png")
                );
                let path = |suffix: &str| {
                    input
                        .join(format!("set_{set}_{suffix}.png"))
                        .to_string_lossy()
                        .into_owned()
                };
                assert_eq!(
                    args,
                    [
                        "(".into(),
                        path("diffuse"),
                        ")".into(),
                        "(".into(),
                        path("normal"),
                        ")".into(),
                        "(".into(),
                        path("rough"),
                        "-negate".into(),
                        ")".into(),
                        "-set".into(),
                        "colorspace".into(),
                        "sRGB".into(),
                        "-combine".into(),
                    ]
                );
            }
        }
    }
}
