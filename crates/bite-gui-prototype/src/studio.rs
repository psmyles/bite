//! Functional editor state shared by the native GUI and its tests.
use bite_core::{execution::RunOptions, Registry};
use bite_schema::{
    BuiltinNodeKind, Extent, Graph, GraphEdge, GraphNode, NodeData, NodeKind, ParamValue,
    PortDefinition, PortType, Position, ProcessingNodeKind, StructuredParam, Viewport, Workflow,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_HISTORY: usize = 100;

fn normalize_text_slots(graph: &mut Graph) {
    for node in graph
        .nodes
        .iter_mut()
        .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput))
    {
        let connected: BTreeSet<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.target == node.id)
            .filter_map(|edge| edge.target_handle.strip_prefix("txo:").map(str::to_owned))
            .collect();
        let slots = match node.data.params.get("portIds") {
            Some(ParamValue::Structured(StructuredParam::TextSlots { slots })) => slots.clone(),
            _ => vec!["0".into()],
        };
        let mut next = node
            .data
            .params
            .get("nextPortIndex")
            .and_then(|value| match value {
                ParamValue::Int(value) => u64::try_from(*value).ok(),
                _ => None,
            })
            .unwrap_or(0)
            .max(
                slots
                    .iter()
                    .filter_map(|slot| slot.parse::<u64>().ok())
                    .max()
                    .unwrap_or(0)
                    + 1,
            );
        let mut normalized: Vec<_> = slots
            .iter()
            .filter(|slot| connected.contains(*slot))
            .cloned()
            .collect();
        let ghost = slots
            .iter()
            .rev()
            .find(|slot| !connected.contains(*slot))
            .cloned()
            .unwrap_or_else(|| {
                let value = next.to_string();
                next += 1;
                value
            });
        normalized.push(ghost);
        node.data.params.insert(
            "portIds".into(),
            ParamValue::Structured(StructuredParam::TextSlots { slots: normalized }),
        );
        node.data
            .params
            .insert("nextPortIndex".into(), ParamValue::Int(next as i64));
    }
}

#[derive(Clone)]
struct EditSnapshot {
    graph: Graph,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Clipboard {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

pub struct Studio {
    pub registry: Registry,
    pub workflow: Workflow,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub status: String,
    next_id: u64,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    clipboard: Clipboard,
    clean_graph: String,
    transaction_open: bool,
}

impl Studio {
    pub fn load_registry(root: &Path) -> Result<Registry, String> {
        Registry::load(
            &root.join("node-definitions-v2"),
            &root.join("format-definitions-v2"),
        )
    }

    pub fn reload_registry(&mut self, root: &Path) -> Result<(), String> {
        let registry = Self::load_registry(root)?;
        bite_core::graph::validate(&self.workflow.graph, &registry)?;
        let nodes = registry.nodes.len();
        let formats = registry.formats.len();
        self.registry = registry;
        self.status = format!("Reloaded {nodes} node and {formats} format definitions");
        Ok(())
    }

    pub fn blank(registry: Registry) -> Self {
        let mut studio = Self {
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
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: Clipboard::default(),
            clean_graph: String::new(),
            transaction_open: false,
        };
        studio.clean_graph = studio.graph_key();
        studio
    }

    pub fn open(registry: Registry, path: &Path) -> Result<Self, String> {
        let loaded = bite_core::workflow::load(
            &fs::read_to_string(path).map_err(|error| error.to_string())?,
            &registry,
        )?;
        let mut studio = Self {
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
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: Clipboard::default(),
            clean_graph: String::new(),
            transaction_open: false,
        };
        studio.clean_graph = studio.graph_key();
        if loaded.migrated {
            studio.clean_graph.clear();
        }
        Ok(studio)
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
        self.clean_graph = self.graph_key();
        self.dirty = false;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    pub fn export_cli(
        &mut self,
        script_path: &Path,
        shell: bite_core::cli_export::Shell,
    ) -> Result<(), String> {
        self.workflow
            .validate()
            .map_err(|errors| errors.join("\n"))?;
        bite_core::graph::validate(&self.workflow.graph, &self.registry)?;
        let companion = script_path.with_extension("bite");
        if let Some(parent) = script_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(
            &companion,
            serde_json::to_string_pretty(&self.workflow).map_err(|error| error.to_string())? + "\n",
        )
        .map_err(|error| error.to_string())?;
        let workflow_file = companion
            .file_name()
            .ok_or("Export path has no file name")?
            .to_string_lossy();
        let generated = format!(
            "Unix timestamp {}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_secs()
        );
        fs::write(
            script_path,
            bite_core::cli_export::generate(
                shell,
                &workflow_file,
                &generated,
                &self.workflow.graph,
            ),
        )
        .map_err(|error| error.to_string())?;
        self.status = format!(
            "Exported {} and {}",
            script_path.display(),
            companion.display()
        );
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

    fn graph_key(&self) -> String {
        serde_json::to_string(&self.workflow.graph).expect("graph serialization cannot fail")
    }

    fn checkpoint(&mut self) {
        if self.transaction_open {
            return;
        }
        let key = self.graph_key();
        if self.undo.last().is_some_and(|snapshot| {
            serde_json::to_string(&snapshot.graph).ok().as_deref() == Some(&key)
        }) {
            return;
        }
        self.undo.push(EditSnapshot {
            graph: self.workflow.graph.clone(),
        });
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn finish_edit(&mut self, status: impl Into<String>) {
        self.dirty = self.graph_key() != self.clean_graph;
        self.status = status.into();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn mark_clean(&mut self) {
        self.clean_graph = self.graph_key();
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.transaction_open = false;
    }

    pub fn begin_transaction(&mut self) {
        if self.transaction_open {
            return;
        }
        self.checkpoint();
        self.transaction_open = true;
    }

    pub fn finish_transaction(&mut self, status: impl Into<String>) {
        if !self.transaction_open {
            return;
        }
        self.transaction_open = false;
        self.finish_edit(status);
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(EditSnapshot {
            graph: self.workflow.graph.clone(),
        });
        self.workflow.graph = snapshot.graph;
        self.finish_edit("Undo");
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(EditSnapshot {
            graph: self.workflow.graph.clone(),
        });
        self.workflow.graph = snapshot.graph;
        self.finish_edit("Redo");
        true
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
        self.checkpoint();
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Processing(ProcessingNodeKind::Process),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: label.clone(),
                definition_id: definition.into(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.finish_edit(format!("Added {label}"));
        Ok(id)
    }

    pub fn add_input(&mut self, position: Position) -> String {
        let cli_name = self.unique_cli_name("input");
        self.checkpoint();
        let id = self.id("input");
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
                    ("cliName".into(), ParamValue::String(cli_name)),
                ]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.finish_edit("Added Input");
        id
    }

    pub fn add_output(&mut self, position: Position) -> String {
        let cli_name = self.unique_cli_name("output-image");
        self.checkpoint();
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
                    ("cliName".into(), ParamValue::String(cli_name)),
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
        self.finish_edit("Added Image Output");
        id
    }

    fn unique_cli_name(&self, prefix: &str) -> String {
        let used: BTreeSet<_> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter_map(|node| match node.data.params.get("cliName") {
                Some(ParamValue::String(name)) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        (1..)
            .map(|index| format!("{prefix}-{index}"))
            .find(|name| !used.contains(name.as_str()))
            .expect("CLI name space is unbounded")
    }

    pub fn add_text_output(&mut self, position: Position) -> String {
        self.checkpoint();
        let id = self.id("text-output");
        let cli_name = self.unique_cli_name("output-text");
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Builtin(BuiltinNodeKind::TextOutput),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: "Text Output".into(),
                definition_id: String::new(),
                params: BTreeMap::from([
                    ("outputPath".into(), ParamValue::String(String::new())),
                    ("overwrite".into(), ParamValue::String("skip".into())),
                    (
                        "portIds".into(),
                        ParamValue::Structured(StructuredParam::TextSlots {
                            slots: vec!["0".into()],
                        }),
                    ),
                    ("nextPortIndex".into(), ParamValue::Int(1)),
                    ("separatorType".into(), ParamValue::String("comma".into())),
                    ("customSeparator".into(), ParamValue::String(String::new())),
                    ("generateLog".into(), ParamValue::Bool(false)),
                    ("usePreviewForProcessing".into(), ParamValue::Bool(false)),
                    ("cliName".into(), ParamValue::String(cli_name)),
                ]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.finish_edit("Added Text Output");
        id
    }

    pub fn add_flipbook_output(&mut self, position: Position) -> String {
        self.checkpoint();
        let id = self.id("flipbook-output");
        let cli_name = self.unique_cli_name("output-flipbook");
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput),
            position,
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: "Flipbook Output".into(),
                definition_id: String::new(),
                params: BTreeMap::from([
                    (
                        "flipbookOutputPath".into(),
                        ParamValue::String(String::new()),
                    ),
                    ("overwrite".into(), ParamValue::String("skip".into())),
                    ("cols".into(), ParamValue::Int(4)),
                    ("rows".into(), ParamValue::Int(4)),
                    ("cellWidth".into(), ParamValue::Int(128)),
                    ("cellHeight".into(), ParamValue::Int(128)),
                    ("sortBy".into(), ParamValue::String("import_order".into())),
                    (
                        "bgColor".into(),
                        ParamValue::Vector(vec![0.0, 0.0, 0.0, 0.0]),
                    ),
                    ("generateLog".into(), ParamValue::Bool(false)),
                    ("cliName".into(), ParamValue::String(cli_name)),
                ]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.finish_edit("Added Flipbook Output");
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
        normalize_text_slots(&mut graph);
        bite_core::graph::validate(&graph, &self.registry)?;
        self.checkpoint();
        self.workflow.graph = graph;
        self.finish_edit("Connected nodes");
        Ok(id)
    }

    pub fn set_param(&mut self, node_id: &str, name: String, value: ParamValue) -> bool {
        let Some(index) = self
            .workflow
            .graph
            .nodes
            .iter()
            .position(|node| node.id == node_id)
        else {
            return false;
        };
        if self.workflow.graph.nodes[index].data.params.get(&name) == Some(&value) {
            return false;
        }
        self.checkpoint();
        self.workflow.graph.nodes[index]
            .data
            .params
            .insert(name.clone(), value.clone());
        if self.workflow.graph.nodes[index].data.definition_id == "process_as_set"
            && name == "suffixes"
        {
            let suffixes = match value {
                ParamValue::Structured(StructuredParam::SetSuffixes { suffixes }) => suffixes,
                _ => Vec::new(),
            };
            self.workflow.graph.nodes[index].data.outputs = suffixes
                .iter()
                .enumerate()
                .map(|(index, suffix)| PortDefinition {
                    name: format!("suffix_{index}"),
                    kind: PortType::Image,
                    label: suffix.clone(),
                })
                .collect();
            let valid: BTreeSet<_> = (0..suffixes.len())
                .map(|index| format!("out:suffix_{index}"))
                .collect();
            let valid_params: BTreeSet<_> = (0..suffixes.len())
                .map(|index| format!("param:suffix_{index}"))
                .collect();
            self.workflow.graph.edges.retain(|edge| {
                (edge.source != node_id || valid.contains(&edge.source_handle))
                    && (edge.target != node_id
                        || edge.target_handle == "in:input"
                        || edge.target_handle == "param:prefix"
                        || valid_params.contains(&edge.target_handle))
            });
        }
        self.finish_edit("Changed parameter");
        true
    }

    pub fn delete_selection(&mut self, selected: &[String]) -> bool {
        let selected: BTreeSet<_> = selected.iter().cloned().collect();
        let selected_groups: BTreeMap<_, _> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| {
                selected.contains(&node.id)
                    && node.kind == NodeKind::Builtin(BuiltinNodeKind::Group)
            })
            .map(|node| (node.id.clone(), node.position.clone()))
            .collect();
        let input_count = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
            .count();
        let output_count = self
            .workflow
            .graph
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
            .count();
        let mut removed_inputs = 0usize;
        let mut removed_outputs = 0usize;
        let removable: BTreeSet<_> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| selected.contains(&node.id))
            .filter(|node| match node.kind {
                NodeKind::Builtin(BuiltinNodeKind::Group) => false,
                NodeKind::Builtin(BuiltinNodeKind::Input) => {
                    if input_count.saturating_sub(removed_inputs) <= 1 {
                        false
                    } else {
                        removed_inputs += 1;
                        true
                    }
                }
                NodeKind::Builtin(
                    BuiltinNodeKind::ImageOutput
                    | BuiltinNodeKind::TextOutput
                    | BuiltinNodeKind::FlipbookOutput,
                ) => {
                    if output_count.saturating_sub(removed_outputs) <= 1 {
                        false
                    } else {
                        removed_outputs += 1;
                        true
                    }
                }
                _ => true,
            })
            .map(|node| node.id.clone())
            .collect();
        if removable.is_empty() && selected_groups.is_empty() {
            return false;
        }
        self.checkpoint();
        self.workflow.graph.nodes.retain(|node| {
            !removable.contains(&node.id) && !selected_groups.contains_key(&node.id)
        });
        for node in &mut self.workflow.graph.nodes {
            if let Some(parent) = node
                .parent_id
                .as_ref()
                .and_then(|id| selected_groups.get(id))
            {
                node.position.x += parent.x;
                node.position.y += parent.y;
                node.parent_id = None;
                node.extent = None;
            }
        }
        self.workflow
            .graph
            .edges
            .retain(|edge| !removable.contains(&edge.source) && !removable.contains(&edge.target));
        self.finish_edit(format!(
            "Deleted {} node(s), ungrouped {} group(s)",
            removable.len(),
            selected_groups.len()
        ));
        true
    }

    pub fn copy_selection(&mut self, selected: &[String]) -> bool {
        let mut selected: BTreeSet<_> = selected.iter().cloned().collect();
        let groups: BTreeSet<_> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| {
                selected.contains(&node.id)
                    && node.kind == NodeKind::Builtin(BuiltinNodeKind::Group)
            })
            .map(|node| node.id.clone())
            .collect();
        selected.extend(self.workflow.graph.nodes.iter().filter_map(|node| {
            node.parent_id
                .as_ref()
                .filter(|parent| groups.contains(*parent))
                .map(|_| node.id.clone())
        }));
        let nodes: Vec<_> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| {
                selected.contains(&node.id)
                    && !matches!(
                        node.kind,
                        NodeKind::Builtin(
                            BuiltinNodeKind::Input
                                | BuiltinNodeKind::ImageOutput
                                | BuiltinNodeKind::TextOutput
                                | BuiltinNodeKind::FlipbookOutput
                        )
                    )
            })
            .cloned()
            .collect();
        if nodes.is_empty() {
            return false;
        }
        let ids: BTreeSet<_> = nodes.iter().map(|node| node.id.clone()).collect();
        let edges = self
            .workflow
            .graph
            .edges
            .iter()
            .filter(|edge| ids.contains(&edge.source) && ids.contains(&edge.target))
            .cloned()
            .collect();
        self.clipboard = Clipboard { nodes, edges };
        self.status = format!("Copied {} node(s)", self.clipboard.nodes.len());
        true
    }

    pub fn clipboard_json(&self) -> Option<String> {
        (!self.clipboard.nodes.is_empty()).then(|| {
            "BITE_GRAPH_FRAGMENT_V1\n".to_owned()
                + &serde_json::to_string(&self.clipboard)
                    .expect("clipboard serialization cannot fail")
        })
    }

    pub fn import_clipboard_json(&mut self, text: &str) -> bool {
        let Some(json) = text.strip_prefix("BITE_GRAPH_FRAGMENT_V1\n") else {
            return false;
        };
        let Ok(clipboard) = serde_json::from_str::<Clipboard>(json) else {
            return false;
        };
        if clipboard.nodes.is_empty() {
            return false;
        }
        self.clipboard = clipboard;
        true
    }

    pub fn paste(&mut self) -> Vec<String> {
        if self.clipboard.nodes.is_empty() {
            return Vec::new();
        }
        self.checkpoint();
        let clipboard = self.clipboard.clone();
        let id_map: BTreeMap<_, _> = clipboard
            .nodes
            .iter()
            .map(|node| (node.id.clone(), self.id("paste")))
            .collect();
        let mut pasted = Vec::new();
        for mut node in clipboard.nodes {
            node.id = id_map[&node.id].clone();
            node.position.x += 20.0;
            node.position.y += 20.0;
            if let Some(parent) = node.parent_id.as_mut() {
                if let Some(replacement) = id_map.get(parent) {
                    *parent = replacement.clone();
                }
            }
            pasted.push(node.id.clone());
            self.workflow.graph.nodes.push(node);
        }
        for mut edge in clipboard.edges {
            edge.id = self.id("edge");
            edge.source = id_map[&edge.source].clone();
            edge.target = id_map[&edge.target].clone();
            self.workflow.graph.edges.push(edge);
        }
        self.finish_edit(format!("Pasted {} node(s)", pasted.len()));
        pasted
    }

    pub fn duplicate_selection(&mut self, selected: &[String]) -> Vec<String> {
        if !self.copy_selection(selected) {
            return Vec::new();
        }
        self.paste()
    }

    pub fn delete_edge(&mut self, edge_id: &str) -> bool {
        if self
            .workflow
            .graph
            .edges
            .iter()
            .all(|edge| edge.id != edge_id)
        {
            return false;
        }
        self.checkpoint();
        self.workflow.graph.edges.retain(|edge| edge.id != edge_id);
        normalize_text_slots(&mut self.workflow.graph);
        self.finish_edit("Deleted connection");
        true
    }

    pub fn add_comment(&mut self, position: Position) -> String {
        self.checkpoint();
        let id = self.id("comment");
        self.workflow.graph.nodes.push(GraphNode {
            id: id.clone(),
            kind: NodeKind::Builtin(BuiltinNodeKind::Comment),
            position,
            parent_id: None,
            extent: None,
            width: Some(280.0),
            height: Some(150.0),
            data: NodeData {
                label: "Comment".into(),
                definition_id: String::new(),
                params: BTreeMap::from([
                    ("heading".into(), ParamValue::String("Comment".into())),
                    ("body".into(), ParamValue::String(String::new())),
                ]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        self.finish_edit("Added Comment");
        id
    }

    pub fn group_selection(&mut self, selected: &[String]) -> Option<String> {
        let selected: BTreeSet<_> = selected.iter().cloned().collect();
        let targets: Vec<_> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| selected.contains(&node.id))
            .filter(|node| {
                !matches!(
                    node.kind,
                    NodeKind::Builtin(
                        BuiltinNodeKind::Input
                            | BuiltinNodeKind::ImageOutput
                            | BuiltinNodeKind::TextOutput
                            | BuiltinNodeKind::FlipbookOutput
                            | BuiltinNodeKind::Group
                    )
                )
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            return None;
        }
        let min_x = targets
            .iter()
            .map(|node| node.position.x)
            .fold(f64::INFINITY, f64::min);
        let min_y = targets
            .iter()
            .map(|node| node.position.y)
            .fold(f64::INFINITY, f64::min);
        let max_x = targets
            .iter()
            .map(|node| node.position.x + node.width.unwrap_or(160.0))
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = targets
            .iter()
            .map(|node| node.position.y + node.height.unwrap_or(80.0))
            .fold(f64::NEG_INFINITY, f64::max);
        let (group_x, group_y) = (min_x - 40.0, min_y - 40.0);
        self.checkpoint();
        let id = self.id("group");
        for node in &mut self.workflow.graph.nodes {
            if selected.contains(&node.id) && targets.iter().any(|target| target.id == node.id) {
                node.position.x -= group_x;
                node.position.y -= group_y;
                node.parent_id = Some(id.clone());
                node.extent = Some(Extent::Parent);
            }
        }
        self.workflow.graph.nodes.insert(
            0,
            GraphNode {
                id: id.clone(),
                kind: NodeKind::Builtin(BuiltinNodeKind::Group),
                position: Position {
                    x: group_x,
                    y: group_y,
                },
                parent_id: None,
                extent: None,
                width: Some(max_x - min_x + 80.0),
                height: Some(max_y - min_y + 80.0),
                data: NodeData {
                    label: "Group".into(),
                    definition_id: String::new(),
                    params: BTreeMap::from([("name".into(), ParamValue::String("Group".into()))]),
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                },
            },
        );
        self.finish_edit(format!("Grouped {} node(s)", targets.len()));
        Some(id)
    }

    pub fn ungroup_selection(&mut self, selected: &[String]) -> bool {
        let groups: BTreeMap<_, _> = self
            .workflow
            .graph
            .nodes
            .iter()
            .filter(|node| {
                selected.contains(&node.id)
                    && node.kind == NodeKind::Builtin(BuiltinNodeKind::Group)
            })
            .map(|node| (node.id.clone(), node.position.clone()))
            .collect();
        if groups.is_empty() {
            return false;
        }
        self.checkpoint();
        self.workflow
            .graph
            .nodes
            .retain(|node| !groups.contains_key(&node.id));
        for node in &mut self.workflow.graph.nodes {
            if let Some(parent) = node.parent_id.as_ref().and_then(|id| groups.get(id)) {
                node.position.x += parent.x;
                node.position.y += parent.y;
                node.parent_id = None;
                node.extent = None;
            }
        }
        self.finish_edit(format!("Ungrouped {} group(s)", groups.len()));
        true
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

    #[test]
    fn history_restores_graph_and_saved_dirty_state() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let input = studio.add_input(Position { x: 0.0, y: 0.0 });
        let output = studio.add_output(Position { x: 400.0, y: 0.0 });
        studio
            .connect(&input, "out:output", &output, "in:input")
            .unwrap();
        let dir = std::env::temp_dir().join(format!("bite-native-history-{}", std::process::id()));
        let path = dir.join("saved.bite");
        studio.save(&path).unwrap();
        assert!(!studio.dirty);

        let resize = studio
            .add_processing("resize", Position { x: 200.0, y: 0.0 })
            .unwrap();
        assert!(studio.dirty && studio.can_undo());
        assert!(studio.undo());
        assert!(!studio.dirty);
        assert!(studio
            .workflow
            .graph
            .nodes
            .iter()
            .all(|node| node.id != resize));
        assert!(studio.redo());
        assert!(studio.dirty);
        assert!(studio
            .workflow
            .graph
            .nodes
            .iter()
            .any(|node| node.id == resize));
        studio.begin_transaction();
        studio
            .workflow
            .graph
            .nodes
            .iter_mut()
            .find(|node| node.id == resize)
            .unwrap()
            .position
            .x = 900.0;
        studio.finish_transaction("Moved node");
        assert!(studio.undo());
        assert_eq!(
            studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == resize)
                .unwrap()
                .position
                .x,
            200.0
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn duplicate_preserves_internal_edges_and_delete_guards_workflow_endpoints() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let input = studio.add_input(Position { x: 0.0, y: 0.0 });
        let a = studio
            .add_processing("resize", Position { x: 200.0, y: 0.0 })
            .unwrap();
        let b = studio
            .add_processing("sharpen", Position { x: 400.0, y: 0.0 })
            .unwrap();
        let output = studio.add_output(Position { x: 600.0, y: 0.0 });
        studio.connect(&a, "out:output", &b, "in:input").unwrap();

        let pasted = studio.duplicate_selection(&[a, b]);
        assert_eq!(pasted.len(), 2);
        assert_eq!(studio.workflow.graph.edges.len(), 2);
        assert!(studio
            .workflow
            .graph
            .edges
            .iter()
            .any(|edge| { pasted.contains(&edge.source) && pasted.contains(&edge.target) }));
        assert!(!studio.delete_selection(&[input, output]));
    }

    #[test]
    fn groups_copy_with_children_and_delete_back_to_absolute_positions() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let a = studio
            .add_processing("resize", Position { x: 200.0, y: 100.0 })
            .unwrap();
        let b = studio
            .add_processing("sharpen", Position { x: 400.0, y: 120.0 })
            .unwrap();
        let group = studio.group_selection(&[a.clone(), b.clone()]).unwrap();
        assert!(studio
            .workflow
            .graph
            .nodes
            .iter()
            .any(|node| { node.id == a && node.parent_id.as_deref() == Some(group.as_str()) }));
        let pasted = studio.duplicate_selection(std::slice::from_ref(&group));
        assert_eq!(pasted.len(), 3);

        assert!(studio.delete_selection(std::slice::from_ref(&group)));
        let a = studio
            .workflow
            .graph
            .nodes
            .iter()
            .find(|node| node.id == a)
            .unwrap();
        assert!(a.parent_id.is_none());
        assert_eq!(a.position.x, 200.0);
        assert_eq!(a.position.y, 100.0);
    }

    #[test]
    fn creates_valid_text_and_flipbook_outputs_with_unique_cli_names() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        studio.add_input(Position { x: 0.0, y: 0.0 });
        studio.add_output(Position { x: 600.0, y: 0.0 });
        let text_a = studio.add_text_output(Position { x: 600.0, y: 150.0 });
        let text_b = studio.add_text_output(Position { x: 600.0, y: 300.0 });
        let flipbook = studio.add_flipbook_output(Position { x: 600.0, y: 450.0 });
        studio.workflow.validate().unwrap();
        bite_core::graph::validate(&studio.workflow.graph, &studio.registry).unwrap();
        let cli_name = |id: &str| {
            studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == id)
                .and_then(|node| node.data.params.get("cliName"))
                .and_then(|value| match value {
                    ParamValue::String(value) => Some(value.clone()),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(cli_name(&text_a), "output-text-1");
        assert_eq!(cli_name(&text_b), "output-text-2");
        assert_eq!(cli_name(&flipbook), "output-flipbook-1");
    }

    #[test]
    fn exports_cli_script_with_companion_workflow() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        studio.add_input(Position { x: 0.0, y: 0.0 });
        studio.add_output(Position { x: 400.0, y: 0.0 });
        let dir = std::env::temp_dir().join(format!("bite-native-export-{}", std::process::id()));
        let script = dir.join("batch.ps1");
        studio
            .export_cli(&script, bite_core::cli_export::Shell::PowerShell)
            .unwrap();
        assert!(fs::read_to_string(&script)
            .unwrap()
            .contains("bite run $WorkflowFile"));
        assert!(dir.join("batch.bite").is_file());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn process_as_set_suffixes_create_ports_and_remove_stale_edges() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let set = studio
            .add_processing("process_as_set", Position { x: 0.0, y: 0.0 })
            .unwrap();
        let output = studio.add_output(Position { x: 400.0, y: 0.0 });
        assert!(studio.set_param(
            &set,
            "suffixes".into(),
            ParamValue::Structured(StructuredParam::SetSuffixes {
                suffixes: vec!["_color".into(), "_normal".into()],
            }),
        ));
        assert_eq!(
            studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == set)
                .unwrap()
                .data
                .outputs
                .len(),
            2
        );
        studio
            .connect(&set, "out:suffix_1", &output, "in:input")
            .unwrap();
        studio.set_param(
            &set,
            "suffixes".into(),
            ParamValue::Structured(StructuredParam::SetSuffixes {
                suffixes: vec!["_color".into()],
            }),
        );
        assert!(studio.workflow.graph.edges.is_empty());
    }

    #[test]
    fn text_output_keeps_one_empty_dynamic_port() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut studio = Studio::blank(registry);
        let input = studio.add_input(Position { x: 0.0, y: 0.0 });
        let text = studio.add_text_output(Position { x: 400.0, y: 0.0 });
        let edge = studio
            .connect(&input, "out:output", &text, "txo:0")
            .unwrap();
        fn slots(studio: &Studio, text: &str) -> Vec<String> {
            match studio
                .workflow
                .graph
                .nodes
                .iter()
                .find(|node| node.id == text)
                .unwrap()
                .data
                .params
                .get("portIds")
                .unwrap()
            {
                ParamValue::Structured(StructuredParam::TextSlots { slots }) => slots.clone(),
                _ => panic!("expected text slots"),
            }
        }
        assert_eq!(slots(&studio, &text), vec!["0", "1"]);
        studio.delete_edge(&edge);
        assert_eq!(slots(&studio, &text), vec!["1"]);
    }

    #[test]
    fn clipboard_fragment_round_trips_between_studios() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Studio::load_registry(&root).unwrap();
        let mut source = Studio::blank(registry.clone());
        let resize = source
            .add_processing("resize", Position { x: 100.0, y: 100.0 })
            .unwrap();
        let sharpen = source
            .add_processing("sharpen", Position { x: 300.0, y: 100.0 })
            .unwrap();
        source
            .connect(&resize, "out:output", &sharpen, "in:input")
            .unwrap();
        source.copy_selection(&[resize, sharpen]);
        let fragment = source.clipboard_json().unwrap();

        let mut target = Studio::blank(registry);
        assert!(target.import_clipboard_json(&fragment));
        assert_eq!(target.paste().len(), 2);
        assert_eq!(target.workflow.graph.edges.len(), 1);
        assert!(!target.import_clipboard_json("ordinary clipboard text"));
    }
}
