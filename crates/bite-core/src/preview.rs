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
    pub png: Vec<u8>,
    pub resolved_values: BTreeMap<String, Context>,
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
    let metadata = host.metadata(image, true)?;
    let mut resolved = ResolvedParams::new();
    let mut values = BTreeMap::new();
    for id in crate::graph::topo_sort(graph)? {
        let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
        let context = resolve::node_params(node, graph, &resolved, registry, &metadata)?;
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
    let staged = input.join(
        image
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("preview.png")),
    );
    fs::hard_link(image, &staged)
        .or_else(|_| fs::copy(image, &staged).map(|_| ()))
        .map_err(|error| error.to_string())?;
    let mut options = RunOptions {
        overwrite: true,
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
    let rendered: PathBuf = result
        .outputs
        .into_iter()
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .ok_or("Preview produced no PNG output")?;
    Ok(PreviewResult {
        png: fs::read(rendered).map_err(|error| error.to_string())?,
        resolved_values: values,
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
            &root.join("node-definitions-v2"),
            &root.join("format-definitions-v2"),
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
}
