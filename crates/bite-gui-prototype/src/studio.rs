//! Functional editor state shared by the native GUI and its tests.
use bite_core::{execution::RunOptions, Registry};
use bite_schema::{
    BuiltinNodeKind, Graph, GraphEdge, GraphNode, NodeData, NodeKind, ParamValue, Position,
    ProcessingNodeKind, Viewport, Workflow,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub struct Studio {
    pub registry: Registry,
    pub workflow: Workflow,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub status: String,
    next_id: u64,
}

impl Studio {
    pub fn load_registry(root: &Path) -> Result<Registry, String> {
        Registry::load(
            &root.join("node-definitions-v2"),
            &root.join("format-definitions-v2"),
        )
    }

    pub fn blank(registry: Registry) -> Self {
        Self {
            registry,
            workflow: Workflow {
                schema_version: 2,
                created_with: env!("CARGO_PKG_VERSION").into(),
                graph: Graph {
                    nodes: Vec::new(),
                    edges: Vec::new(),
                    viewport: Viewport {
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                    },
                },
            },
            path: None,
            dirty: false,
            status: "New workflow".into(),
            next_id: 1,
        }
    }

    pub fn open(registry: Registry, path: &Path) -> Result<Self, String> {
        let loaded = bite_core::workflow::load(
            &fs::read_to_string(path).map_err(|error| error.to_string())?,
            &registry,
        )?;
        Ok(Self {
            registry,
            workflow: loaded.workflow,
            path: Some(path.to_owned()),
            dirty: loaded.migrated,
            status: if loaded.migrated {
                "Opened and migrated; save to write schema v2".into()
            } else {
                format!("Opened {}", path.display())
            },
            next_id: 1,
        })
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.workflow
            .validate()
            .map_err(|errors| errors.join("\n"))?;
        bite_core::graph::validate(&self.workflow.graph, &self.registry)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(&self.workflow).map_err(|error| error.to_string())? + "\n",
        )
        .map_err(|error| error.to_string())?;
        self.path = Some(path.to_owned());
        self.dirty = false;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    fn id(&mut self, prefix: &str) -> String {
        loop {
            let id = format!("{prefix}-native-{}", self.next_id);
            self.next_id += 1;
            if self.workflow.graph.nodes.iter().all(|node| node.id != id) {
                return id;
            }
        }
    }

    pub fn add_processing(
        &mut self,
        definition: &str,
        position: Position,
    ) -> Result<String, String> {
        let compiled = self
            .registry
            .nodes
            .get(definition)
            .ok_or_else(|| format!("Unknown node definition {definition}"))?;
        let label = compiled.definition.label.clone();
        let params = compiled
            .definition
            .params
            .iter()
            .filter_map(|param| {
                param
                    .default
                    .clone()
                    .map(|value| (param.name.clone(), value))
            })
            .collect();
        let id = self.id(definition);
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Processing(ProcessingNodeKind::Process),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label,
                definition_id: definition.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.dirty = true;
        Ok(id)
    }

    pub fn add_input(&mut self, position: Position) -> String {
        let id = self.id("input");
        let number = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .count();
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Builtin(BuiltinNodeKind::Input),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: "Input".into(),
                definition_id: String::new(),
                params: BTreeMap::from([
                    ("thumbnailSize".into(), ParamValue::Int(256)),
                    (
                        "cliName".into(),
                        ParamValue::String(if number == 0 {
                            "in".into()
                        } else {
                            format!("in{}", number + 1)
                        }),
                    ),
                ]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.dirty = true;
        id
    }

    pub fn add_output(&mut self, position: Position) -> String {
        let id = self.id("output");
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: "Image Output".into(),
                definition_id: String::new(),
                params: BTreeMap::from([
                    ("cliName".into(), ParamValue::String("out".into())),
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
        self.dirty = true;
        id
    }

    pub fn connect(
        &mut self,
        source: &str,
        source_handle: &str,
        target: &str,
        target_handle: &str,
    ) -> Result<String, String> {
        let mut graph = self.workflow.graph.clone();
        graph
            .edges
            .retain(|edge| !(edge.target == target && edge.target_handle == target_handle));
        let id = format!("edge-native-{}", self.next_id);
        self.next_id += 1;
        graph.edges.push(GraphEdge {
            id: id.clone(),
            source: source.into(),
            source_handle: source_handle.into(),
            target: target.into(),
            target_handle: target_handle.into(),
        });
        bite_core::graph::validate(&graph, &self.registry)?;
        self.workflow.graph = graph;
        self.dirty = true;
        Ok(id)
    }

    pub fn run_options(&self, input: &Path, output: &Path) -> RunOptions {
        let mut options = RunOptions {
            overwrite: true,
            ..Default::default()
        };
        for node in &self.workflow.graph.nodes {
            let flag = node
                .data
                .params
                .get("cliName")
                .and_then(|value| match value {
                    ParamValue::String(value) => Some(value.clone()),
                    _ => None,
                });
            if let Some(flag) = flag {
                match node.kind {
                    NodeKind::Builtin(BuiltinNodeKind::Input) => {
                        options.named_paths.insert(flag, input.to_owned());
                    }
                    NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                        options.named_paths.insert(flag, output.to_owned());
                    }
                    _ => {}
                }
            }
        }
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_saves_and_reopens_primary_workflow() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let input = studio.add_input(Position { x: 0.0, y: 0.0 });
        let resize = studio
            .add_processing("resize", Position { x: 200.0, y: 0.0 })
            .unwrap();
        let sharpen = studio
            .add_processing("sharpen", Position { x: 400.0, y: 0.0 })
            .unwrap();
        let format = studio
            .add_processing("format_convert", Position { x: 600.0, y: 0.0 })
            .unwrap();
        let output = studio.add_output(Position { x: 800.0, y: 0.0 });
        for (source, target) in [
            (&input, &resize),
            (&resize, &sharpen),
            (&sharpen, &format),
            (&format, &output),
        ] {
            studio
                .connect(source, "out:output", target, "in:input")
                .unwrap();
        }
        let dir = std::env::temp_dir().join(format!("bite-native-studio-{}", std::process::id()));
        let path = dir.join("primary.bite");
        studio.save(&path).unwrap();
        let reopened = Studio::open(studio.registry.clone(), &path).unwrap();
        assert_eq!(reopened.workflow.graph.nodes.len(), 5);
        assert_eq!(reopened.workflow.graph.edges.len(), 4);
        fs::remove_dir_all(dir).unwrap();
    }
}
