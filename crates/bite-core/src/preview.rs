//! Direct single-image preview API for native frontends.
use crate::{
    execution::{self, ImageHost, RunOptions},
    resolve::{self, ResolvedParams},
    Registry,
};
use bite_expr::{definition::CompiledImplementation, Context};
use bite_schema::{
    BuiltinNodeKind, Graph, GraphEdge, GraphNode, NodeData, NodeKind, ParamValue, Position,
    ProcessingNodeKind,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub struct PreviewResult {
    /// Nothing when no image reached the previewed node, which a closed Gate is the one
    /// deliberate cause of. The values still stand: the chain ran, it just stopped.
    pub png: Option<Vec<u8>>,
    pub resolved_values: BTreeMap<String, Context>,
    /// The nodes that stopped the stream, for a panel to say what it is waiting on.
    pub stopped: Vec<String>,
}

/// What every node the graph works out for itself resolves to, measured against one
/// image's metadata.
fn computed_values(
    graph: &Graph,
    registry: &Registry,
    metadata: &Context,
) -> Result<BTreeMap<String, Context>, String> {
    let mut resolved = ResolvedParams::new();
    let mut values = BTreeMap::new();
    for id in crate::graph::topo_sort(graph)? {
        let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
        let context = resolve::node_params(node, graph, &resolved, registry, metadata)?;
        if registry
            .nodes
            .get(&node.data.definition_id)
            .is_some_and(|definition| {
                matches!(
                    &definition.implementation,
                    CompiledImplementation::Compute(_)
                )
            })
        {
            values.insert(node.id.clone(), context.clone());
        }
        resolved.insert(node.id.clone(), context);
    }
    Ok(values)
}

/// What every value node resolves to, measured against one image.
///
/// This is the whole of a preview for a workflow with no chain to render: the property
/// and logic nodes still report what they read from the selected file, which is what the
/// Electron preview returned when it had no image node to follow.
pub fn values_from(
    graph: &Graph,
    registry: &Registry,
    host: &mut dyn ImageHost,
    measured: &Path,
) -> Result<BTreeMap<String, Context>, String> {
    // Only a node that asks about the bit depth, the resolution or the EXIF block needs the
    // file opened; everything else comes from its header. The preview asked for all of it
    // on every image, which is two more processes for each switch of the filmstrip.
    let heavy = crate::execution::wants_heavy_metadata(
        graph
            .nodes
            .iter()
            .filter_map(|node| registry.nodes.get(&node.data.definition_id)),
    );
    let mut metadata = host.metadata(measured, heavy)?;
    // A format the header parser does not read leaves the measurements at nothing, and a
    // node that asks for them would resolve against zero, so those are worth the processes.
    let unmeasured = matches!(
        metadata.get("image.width"),
        None | Some(bite_expr::Value::Int(0)) | Some(bite_expr::Value::Float(0.0))
    );
    if !heavy && unmeasured {
        metadata = host.metadata(measured, true)?;
    }
    computed_values(graph, registry, &metadata)
}

/// What the value nodes resolve to with no image to measure against.
///
/// A Compare or an Add works its number out from its own parameters, and the canvas showed
/// it before anything was imported. Nothing here reads a file: what an image would have
/// supplied stands at nothing, exactly as the Electron preview left it.
pub fn values(graph: &Graph, registry: &Registry) -> Result<BTreeMap<String, Context>, String> {
    let metadata = bite_expr::definition::metadata_types()
        .into_iter()
        .map(|(key, kind)| {
            let value = if kind == bite_expr::Type::String {
                bite_expr::Value::String(String::new())
            } else {
                bite_expr::Value::Int(0)
            };
            (key, value)
        })
        .collect();
    computed_values(graph, registry, &metadata)
}

/// Walks back from a preview source to the nearest handle that carries an image.
fn image_source(
    graph: &Graph,
    registry: &Registry,
    node: String,
    handle: String,
) -> Result<(String, String), String> {
    let mut current = (node, handle);
    // Each step moves one edge upstream, so the node count bounds the walk.
    for _ in 0..=graph.nodes.len() {
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == current.0)
            .ok_or_else(|| format!("Preview source {} is not in the workflow", current.0))?;
        if matches!(
            crate::graph::handle_type(node, &current.1, true, registry),
            Ok(crate::graph::WireType::Image | crate::graph::WireType::Mask)
        ) {
            return Ok(current);
        }
        let Some(edge) = graph
            .edges
            .iter()
            .find(|edge| edge.target == node.id && edge.target_handle.starts_with("in:"))
        else {
            // A node with nothing wired into it, such as a value the canvas only reads,
            // leaves the panel showing the image itself.
            return graph
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
                .map(|input| (input.id.clone(), "out:output".to_owned()))
                .ok_or_else(|| format!("Preview node {} has no image to show", node.id));
        };
        current = (edge.source.clone(), edge.source_handle.clone());
    }
    Err("Preview source wires back on itself".into())
}

/// Evaluate a Text Output against a small image set and return the exact lines
/// that a normal workflow run would write, without touching the configured
/// output path.
pub fn render_text(
    graph: &Graph,
    registry: &Registry,
    host: &mut dyn ImageHost,
    images: &[PathBuf],
    node_id: &str,
) -> Result<Vec<String>, String> {
    if images.is_empty() {
        return Ok(Vec::new());
    }
    let mut preview = graph.clone();
    let target = preview
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput))
        .ok_or("Text preview requires a Text Output node")?;
    let target_id = target.id.clone();
    let target_flag = target
        .data
        .params
        .get("cliName")
        .and_then(|value| match value {
            ParamValue::String(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "__text_preview".into());
    let removed: Vec<_> = preview
        .nodes
        .iter()
        .filter(|node| {
            node.id != target_id
                && matches!(
                    node.kind,
                    NodeKind::Builtin(
                        BuiltinNodeKind::ImageOutput
                            | BuiltinNodeKind::TextOutput
                            | BuiltinNodeKind::FlipbookOutput
                    )
                )
        })
        .map(|node| node.id.clone())
        .collect();
    preview.nodes.retain(|node| !removed.contains(&node.id));
    preview
        .edges
        .retain(|edge| !removed.contains(&edge.source) && !removed.contains(&edge.target));

    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let input = temporary.path().join("input");
    let output = temporary.path().join("preview.txt");
    fs::create_dir_all(&input).map_err(|error| error.to_string())?;
    for (index, image) in images.iter().enumerate() {
        let name = image
            .file_name()
            .map(|name| name.to_owned())
            .unwrap_or_else(|| format!("preview-{index}.png").into());
        let mut staged = input.join(&name);
        if staged.exists() {
            staged = input.join(format!("{index}-{}", name.to_string_lossy()));
        }
        fs::hard_link(image, &staged)
            .or_else(|_| fs::copy(image, &staged).map(|_| ()))
            .map_err(|error| error.to_string())?;
    }
    let target = preview
        .nodes
        .iter_mut()
        .find(|node| node.id == target_id)
        .unwrap();
    target.data.params.insert(
        "outputPath".into(),
        ParamValue::String(output.display().to_string()),
    );
    target
        .data
        .params
        .insert("overwrite".into(), ParamValue::String("overwrite".into()));
    target
        .data
        .params
        .insert("cliName".into(), ParamValue::String(target_flag.clone()));
    let mut options = RunOptions {
        overwrite: true,
        ..Default::default()
    };
    for node in &preview.nodes {
        if node.kind == NodeKind::Builtin(BuiltinNodeKind::Input) {
            if let Some(ParamValue::String(flag)) = node.data.params.get("cliName") {
                options.named_paths.insert(flag.clone(), input.clone());
            }
        }
    }
    options.named_paths.insert(target_flag, output.clone());
    execution::run_workflow(&preview, registry, host, &options, &mut |_, _, _| {})?;
    let contents = fs::read_to_string(output).map_err(|error| error.to_string())?;
    Ok(contents.lines().map(str::to_owned).collect())
}

/// Render one source image through the graph without spawning the CLI. When
/// `source` is omitted, preview follows the first Image Output's incoming wire.
pub fn render(
    graph: &Graph,
    registry: &Registry,
    host: &mut dyn ImageHost,
    image: &Path,
    source: Option<(&str, &str)>,
) -> Result<PreviewResult, String> {
    render_from(graph, registry, host, image, image, &[], source)
}

/// The same, run over `pixels` while every parameter resolves against `measured`.
///
/// A preview renders a thumbnail for speed, but a parameter written as a share of the
/// image's width must still mean a share of the real one, so the two files are given
/// separately. Passing the same path for both measures what it renders.
///
/// `companions` are the other images of a Process As Set: the same pair again for each
/// file the set reads beside this one. Each is staged under its own name, which is what
/// the set matches its suffixes against.
pub fn render_from(
    graph: &Graph,
    registry: &Registry,
    host: &mut dyn ImageHost,
    pixels: &Path,
    measured: &Path,
    companions: &[(PathBuf, PathBuf)],
    source: Option<(&str, &str)>,
) -> Result<PreviewResult, String> {
    let image = pixels;
    let mut values = values_from(graph, registry, host, measured)?;

    let (source_node, source_handle) = if let Some((node, handle)) = source {
        (node.to_owned(), handle.to_owned())
    } else {
        let output = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput))
            .ok_or("Preview requires an Image Output or explicit source")?;
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.target == output.id && edge.target_handle == "in:input")
            .ok_or("Preview Image Output is not connected")?;
        (edge.source.clone(), edge.source_handle.clone())
    };

    // The node the preview is pointed at need not make an image of its own: a Mean Value
    // reads one and reports a number. The panel then shows what that node reads, which is
    // the image wired into it, as the Electron preview did.
    let (source_node, source_handle) = image_source(graph, registry, source_node, source_handle)?;

    let mut preview = graph.clone();
    let removed: Vec<_> = preview
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                NodeKind::Builtin(
                    BuiltinNodeKind::ImageOutput
                        | BuiltinNodeKind::TextOutput
                        | BuiltinNodeKind::FlipbookOutput
                )
            )
        })
        .map(|node| node.id.clone())
        .collect();
    preview.nodes.retain(|node| !removed.contains(&node.id));
    preview
        .edges
        .retain(|edge| !removed.contains(&edge.source) && !removed.contains(&edge.target));
    preview.nodes.push(GraphNode {
        id: "__preview_format".into(),
        kind: NodeKind::Processing(ProcessingNodeKind::Process),
        position: Position { x: 0.0, y: 0.0 },
        parent_id: None,
        extent: None,
        width: None,
        height: None,
        data: NodeData {
            label: "Preview PNG".into(),
            definition_id: "format_convert".into(),
            params: BTreeMap::from([
                ("format".into(), ParamValue::String("PNG".into())),
                ("_enabled".into(), ParamValue::Bool(true)),
            ]),
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
    });
    preview.nodes.push(GraphNode {
        id: "__preview_output".into(),
        kind: NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
        position: Position { x: 0.0, y: 0.0 },
        parent_id: None,
        extent: None,
        width: None,
        height: None,
        data: NodeData {
            label: "Preview Output".into(),
            definition_id: String::new(),
            params: BTreeMap::from([
                ("cliName".into(), ParamValue::String("__preview_out".into())),
                ("outputPath".into(), ParamValue::String("custom".into())),
                ("customPath".into(), ParamValue::String(String::new())),
                ("overwrite".into(), ParamValue::String("overwrite".into())),
                ("generateLog".into(), ParamValue::Bool(false)),
                ("setOutputPrefix".into(), ParamValue::String(String::new())),
                ("setOutputSuffix".into(), ParamValue::String(String::new())),
            ]),
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
    });
    preview.edges.extend([
        GraphEdge {
            id: "__preview_source_edge".into(),
            source: source_node,
            source_handle,
            target: "__preview_format".into(),
            target_handle: "in:input".into(),
        },
        GraphEdge {
            id: "__preview_output_edge".into(),
            source: "__preview_format".into(),
            source_handle: "out:output".into(),
            target: "__preview_output".into(),
            target_handle: "in:input".into(),
        },
    ]);

    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let input = temporary.path().join("input");
    let output = temporary.path().join("output");
    fs::create_dir_all(&input).map_err(|error| error.to_string())?;
    // Each file is staged under the name it has on disk, not the thumbnail's: a Process As
    // Set reads its streams from the suffixes of the names it finds beside each other.
    for (pixels, measured) in std::iter::once((PathBuf::from(image), PathBuf::from(measured)))
        .chain(companions.iter().cloned())
    {
        let staged = input.join(
            measured
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("preview.png")),
        );
        fs::hard_link(&pixels, &staged)
            .or_else(|_| fs::copy(&pixels, &staged).map(|_| ()))
            .map_err(|error| error.to_string())?;
    }
    let mut options = RunOptions {
        overwrite: true,
        // The canvas shows what each value node measured, and a mean is only known once
        // the chain has actually run over the image.
        capture_values: true,
        ..Default::default()
    };
    for node in &preview.nodes {
        if let Some(ParamValue::String(flag)) = node.data.params.get("cliName") {
            match node.kind {
                NodeKind::Builtin(BuiltinNodeKind::Input) => {
                    options.named_paths.insert(flag.clone(), input.clone());
                }
                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                    options.named_paths.insert(flag.clone(), output.clone());
                }
                _ => {}
            }
        }
    }
    let result = execution::run_workflow(&preview, registry, host, &options, &mut |_, _, _| {})?;
    // What the run measured replaces what the static pass could work out on its own.
    values.extend(result.values);
    let rendered = result.outputs.into_iter().find(|path| {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    });
    Ok(PreviewResult {
        png: rendered
            .map(|path| fs::read(path).map_err(|error| error.to_string()))
            .transpose()?,
        resolved_values: values,
        stopped: result.stopped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{Graph, StructuredParam, Viewport};

    struct TextHost;
    impl ImageHost for TextHost {
        fn run(&mut self, _args: &[String]) -> Result<(), String> {
            Err("unexpected image process".into())
        }
        fn capture(&mut self, _args: &[String]) -> Result<String, String> {
            Err("unexpected capture".into())
        }
        fn metadata(&mut self, _path: &Path, _heavy: bool) -> Result<Context, String> {
            Ok(Context::new())
        }
    }

    #[test]
    fn text_preview_uses_normal_text_output_resolution_without_configured_path() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let node = |id: &str, kind: NodeKind, definition_id: &str, params| GraphNode {
            id: id.into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: definition_id.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        let graph = Graph {
            nodes: vec![
                node(
                    "input",
                    NodeKind::Builtin(BuiltinNodeKind::Input),
                    "",
                    BTreeMap::from([("cliName".into(), ParamValue::String("input-1".into()))]),
                ),
                node(
                    "value",
                    NodeKind::Processing(ProcessingNodeKind::Process),
                    "value_string",
                    BTreeMap::from([("value".into(), ParamValue::String("preview value".into()))]),
                ),
                node(
                    "text",
                    NodeKind::Builtin(BuiltinNodeKind::TextOutput),
                    "",
                    BTreeMap::from([
                        ("cliName".into(), ParamValue::String("report".into())),
                        ("outputPath".into(), ParamValue::String(String::new())),
                        ("overwrite".into(), ParamValue::String("skip".into())),
                        (
                            "portIds".into(),
                            ParamValue::Structured(StructuredParam::TextSlots {
                                slots: vec!["0".into(), "1".into()],
                            }),
                        ),
                        ("nextPortIndex".into(), ParamValue::Int(2)),
                        ("separatorType".into(), ParamValue::String("comma".into())),
                        ("customSeparator".into(), ParamValue::String(String::new())),
                        ("generateLog".into(), ParamValue::Bool(false)),
                        ("usePreviewForProcessing".into(), ParamValue::Bool(false)),
                    ]),
                ),
            ],
            edges: vec![
                GraphEdge {
                    id: "image".into(),
                    source: "input".into(),
                    source_handle: "out:output".into(),
                    target: "text".into(),
                    target_handle: "in:input".into(),
                },
                GraphEdge {
                    id: "text-value".into(),
                    source: "value".into(),
                    source_handle: "param:value".into(),
                    target: "text".into(),
                    target_handle: "txo:0".into(),
                },
            ],
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let temporary = tempfile::tempdir().unwrap();
        let image = temporary.path().join("sample.png");
        fs::write(&image, b"fixture").unwrap();
        let lines = render_text(&graph, &registry, &mut TextHost, &[image], "text").unwrap();
        assert_eq!(lines, ["preview value"]);
    }

    /// Reads the mean of whichever channel the separation asked for, and writes any
    /// output it is handed, so a preview can run without ImageMagick.
    struct MeanHost;
    impl MeanHost {
        fn channel(args: &[String]) -> Option<&str> {
            args.windows(2).find_map(|pair| {
                (pair[0] == "-channel").then(|| pair[1].as_str())
            })
        }
    }
    impl ImageHost for MeanHost {
        fn run(&mut self, args: &[String]) -> Result<(), String> {
            let output = args.last().ok_or("no output")?;
            let output = output.strip_prefix("PNG:").unwrap_or(output);
            fs::write(output, b"rendered").map_err(|error| error.to_string())
        }
        fn capture(&mut self, args: &[String]) -> Result<String, String> {
            let mean = match MeanHost::channel(args) {
                Some("Red") => 0.61,
                Some("Green") => 0.56,
                Some("Blue") => 0.53,
                _ => return Err(format!("unexpected capture {args:?}")),
            };
            Ok(format!("{mean}"))
        }
        fn metadata(&mut self, _path: &Path, _heavy: bool) -> Result<Context, String> {
            Ok(Context::new())
        }
    }

    /// Records what it was asked to process, and writes whatever output it is handed.
    #[derive(Default)]
    struct RecordingHost {
        args: Vec<Vec<String>>,
    }
    impl ImageHost for RecordingHost {
        fn run(&mut self, args: &[String]) -> Result<(), String> {
            self.args.push(args.to_vec());
            let output = args.last().ok_or("no output")?;
            let output = output.strip_prefix("PNG:").unwrap_or(output);
            fs::write(output, b"rendered").map_err(|error| error.to_string())
        }
        fn capture(&mut self, args: &[String]) -> Result<String, String> {
            Err(format!("unexpected capture {args:?}"))
        }
        fn metadata(&mut self, _path: &Path, _heavy: bool) -> Result<Context, String> {
            Ok(Context::new())
        }
    }

    /// A Process As Set reads one image per suffix. The preview hands each stream the
    /// companion that belongs to it, rather than rendering the selected file three times
    /// or, as it did before, finding no set at all and producing nothing.
    #[test]
    fn a_set_preview_reads_the_companion_of_each_suffix() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let port = |name: &str| bite_schema::PortDefinition {
            name: name.into(),
            kind: bite_schema::PortType::Image,
            label: name.into(),
        };
        let node = |id: &str, kind: NodeKind, definition_id: &str, params, outputs| GraphNode {
            id: id.into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: definition_id.into(),
                params,
                inputs: Vec::new(),
                outputs,
            },
        };
        let edge = |id: &str, source: &str, handle: &str, target: &str| GraphEdge {
            id: id.into(),
            source: source.into(),
            source_handle: handle.into(),
            target: target.into(),
            target_handle: "in:input".into(),
        };
        let graph = Graph {
            nodes: vec![
                node(
                    "input",
                    NodeKind::Builtin(BuiltinNodeKind::Input),
                    "",
                    BTreeMap::from([("cliName".into(), ParamValue::String("input-1".into()))]),
                    Vec::new(),
                ),
                node(
                    "set",
                    NodeKind::Processing(ProcessingNodeKind::Process),
                    "process_as_set",
                    BTreeMap::from([
                        ("prefix".into(), ParamValue::String("set_".into())),
                        (
                            "suffixes".into(),
                            ParamValue::Structured(StructuredParam::SetSuffixes {
                                suffixes: vec!["_diffuse".into(), "_normal".into()],
                            }),
                        ),
                    ]),
                    vec![port("suffix_0"), port("suffix_1")],
                ),
                node(
                    "negate",
                    NodeKind::Processing(ProcessingNodeKind::Process),
                    "negate",
                    BTreeMap::new(),
                    Vec::new(),
                ),
            ],
            edges: vec![
                edge("to-set", "input", "out:output", "set"),
                edge("to-negate", "set", "out:suffix_1", "negate"),
            ],
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let temporary = tempfile::tempdir().unwrap();
        let selected = temporary.path().join("set_alpha_diffuse.png");
        let companion = temporary.path().join("set_alpha_normal.png");
        fs::write(&selected, b"diffuse").unwrap();
        fs::write(&companion, b"normal").unwrap();
        let mut host = RecordingHost::default();
        render_from(
            &graph,
            &registry,
            &mut host,
            &selected,
            &selected,
            &[(companion.clone(), companion.clone())],
            Some(("negate", "out:output")),
        )
        .unwrap();
        // The chain ran over the companion the wired suffix names, not the selected file.
        let processed = host.args.concat().join(" ");
        assert!(processed.contains("set_alpha_normal.png"), "{processed}");
        assert!(!processed.contains("set_alpha_diffuse.png"), "{processed}");
    }

    /// A Gate that is shut is not a broken preview: the chain ran, and the panel is told
    /// which node stopped it so it can say so over the image itself.
    #[test]
    fn a_shut_gate_reports_what_stopped_the_chain_rather_than_failing() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let node = |id: &str, kind: NodeKind, definition_id: &str, params| GraphNode {
            id: id.into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: definition_id.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        let graph = Graph {
            nodes: vec![
                node(
                    "input",
                    NodeKind::Builtin(BuiltinNodeKind::Input),
                    "",
                    BTreeMap::from([("cliName".into(), ParamValue::String("input-1".into()))]),
                ),
                node(
                    "gate",
                    NodeKind::Processing(ProcessingNodeKind::Process),
                    "gate",
                    BTreeMap::from([("condition".into(), ParamValue::Bool(false))]),
                ),
            ],
            edges: vec![GraphEdge {
                id: "to-gate".into(),
                source: "input".into(),
                source_handle: "out:output".into(),
                target: "gate".into(),
                target_handle: "in:input".into(),
            }],
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let temporary = tempfile::tempdir().unwrap();
        let image = temporary.path().join("sample.png");
        fs::write(&image, b"fixture").unwrap();
        let result = render(
            &graph,
            &registry,
            &mut RecordingHost::default(),
            &image,
            Some(("gate", "out:output")),
        )
        .unwrap();
        assert!(result.png.is_none());
        assert_eq!(result.stopped, ["gate"]);
    }

    /// Nothing is imported yet, so there is no image to measure: the nodes that work
    /// their numbers out from their own parameters still report them.
    #[test]
    fn value_nodes_resolve_with_no_image_to_measure() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let node = |id: &str, definition_id: &str, params| GraphNode {
            id: id.into(),
            kind: NodeKind::Processing(ProcessingNodeKind::Process),
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: definition_id.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        let graph = Graph {
            nodes: vec![
                node(
                    "float",
                    "value_float",
                    BTreeMap::from([("value".into(), ParamValue::Number(2.0))]),
                ),
                node(
                    "add",
                    "math_add",
                    BTreeMap::from([("b".into(), ParamValue::Number(3.0))]),
                ),
                // A property node has no file to read, and reports the nothing it measures.
                node("dimensions", "prop_dimensions", BTreeMap::new()),
            ],
            edges: vec![GraphEdge {
                id: "wire".into(),
                source: "float".into(),
                source_handle: "param:value".into(),
                target: "add".into(),
                target_handle: "param:a".into(),
            }],
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let values = values(&graph, &registry).unwrap();
        assert_eq!(values["add"]["result"], bite_expr::Value::Float(5.0));
        assert_eq!(values["dimensions"]["width"], bite_expr::Value::Int(0));
    }

    /// Every Mean Value reports against the previewed image, including the two the
    /// preview target does not read: the canvas shows all three at once.
    #[test]
    fn preview_reports_measured_values_for_every_value_node() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let node = |id: &str, kind: NodeKind, definition_id: &str, params| GraphNode {
            id: id.into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: definition_id.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        let edge = |id: &str, source: &str, handle: &str, target: &str| GraphEdge {
            id: id.into(),
            source: source.into(),
            source_handle: handle.into(),
            target: target.into(),
            target_handle: "in:input".into(),
        };
        let mean = |id: &str| {
            node(
                id,
                NodeKind::Processing(ProcessingNodeKind::Process),
                "mean_value",
                BTreeMap::new(),
            )
        };
        let graph = Graph {
            nodes: vec![
                node(
                    "input",
                    NodeKind::Builtin(BuiltinNodeKind::Input),
                    "",
                    BTreeMap::from([("cliName".into(), ParamValue::String("input-1".into()))]),
                ),
                node(
                    "split",
                    NodeKind::Processing(ProcessingNodeKind::Process),
                    "channel_split",
                    BTreeMap::new(),
                ),
                mean("mean-r"),
                mean("mean-g"),
                mean("mean-b"),
            ],
            edges: vec![
                edge("to-split", "input", "out:output", "split"),
                edge("to-r", "split", "out:r", "mean-r"),
                edge("to-g", "split", "out:g", "mean-g"),
                edge("to-b", "split", "out:b", "mean-b"),
            ],
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let temporary = tempfile::tempdir().unwrap();
        let image = temporary.path().join("sample.png");
        fs::write(&image, b"fixture").unwrap();
        let result = render(
            &graph,
            &registry,
            &mut MeanHost,
            &image,
            Some(("mean-r", "out:output")),
        )
        .unwrap();
        let value = |id: &str| match result.resolved_values[id]["value"] {
            bite_expr::Value::Float(value) => value,
            ref other => panic!("{id} resolved to {other:?}"),
        };
        assert_eq!(value("mean-r"), 0.61);
        assert_eq!(value("mean-g"), 0.56);
        assert_eq!(value("mean-b"), 0.53);
    }
}
