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
