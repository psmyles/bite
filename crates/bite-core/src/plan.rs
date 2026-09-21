use crate::{graph, Registry};
use bite_schema::{BuiltinNodeKind, Graph, GraphEdge, NodeKind, Params};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Serialize)]
pub struct Plan {
    pub execution_order: Vec<String>,
    pub metadata: BTreeSet<String>,
    pub outputs: Vec<OutputPlan>,
}
#[derive(Debug, Serialize)]
pub struct OutputPlan {
    pub node: String,
    pub input: Option<String>,
    pub contributors: Vec<String>,
    pub operations: Vec<Operation>,
    pub params: Params,
}
#[derive(Debug, Serialize)]
pub struct Operation {
    pub node: String,
    pub definition: String,
    pub implementation: serde_json::Value,
    pub params: Params,
    pub bindings: Vec<GraphEdge>,
}
pub fn build(graph: &Graph, registry: &Registry) -> Result<Plan, String> {
    graph::validate(graph, registry)?;
    let order = graph::topo_sort(graph)?;
    let mut metadata = BTreeSet::new();
    let mut outputs = Vec::new();
    for output in graph.nodes.iter().filter(|n| {
        matches!(
            n.kind,
            NodeKind::Builtin(
                BuiltinNodeKind::ImageOutput
                    | BuiltinNodeKind::TextOutput
                    | BuiltinNodeKind::FlipbookOutput
            )
        )
    }) {
        let contributors = graph::trace(graph, std::slice::from_ref(&output.id), true);
        let set: BTreeSet<_> = contributors.iter().collect();
        let mut operations = Vec::new();
        for id in &order {
            if !set.contains(id) {
                continue;
            }
            let node = graph.nodes.iter().find(|n| &n.id == id).unwrap();
            if let Some(d) = registry.nodes.get(&node.data.definition_id) {
                metadata.extend(d.metadata.clone());
                operations.push(Operation {
                    node: id.clone(),
                    definition: node.data.definition_id.clone(),
                    implementation: serde_json::to_value(&d.definition.implementation)
                        .map_err(|e| e.to_string())?,
                    params: d
                        .definition
                        .params
                        .iter()
                        .filter_map(|p| p.default.clone().map(|v| (p.name.clone(), v)))
                        .chain(node.data.params.clone())
                        .collect(),
                    bindings: graph
                        .edges
                        .iter()
                        .filter(|e| e.target == node.id)
                        .cloned()
                        .collect(),
                });
            }
        }
        let mut input = graph::trace_input(graph, &output.id);
        if input.is_none()
            && contributors.iter().any(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *id)
                    .is_some_and(|node| node.data.definition_id == "solid_image")
            })
        {
            let inputs: Vec<_> = graph
                .nodes
                .iter()
                .filter(|node| node.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
                .map(|node| node.id.clone())
                .collect();
            input = match inputs.as_slice() {
                [only] => Some(only.clone()),
                [] => {
                    return Err(format!(
                        "output {} uses Solid Image but has no workflow input for its dimensions",
                        output.id
                    ))
                }
                _ => {
                    return Err(format!(
                        "output {} uses Solid Image without an image connection and has multiple workflow inputs",
                        output.id
                    ))
                }
            };
        }
        outputs.push(OutputPlan {
            node: output.id.clone(),
            input,
            contributors,
            operations,
            params: output.data.params.clone(),
        });
    }
    Ok(Plan {
        execution_order: order,
        metadata,
        outputs,
    })
}

use crate::execution::{self, BatchResult, ImageHost, OutputOperation, RunOptions};
use bite_expr::Context;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Optional observations let a plan resolve data-dependent gates and expressions
/// without running an image process. They are explicit inputs, never guessed.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningFacts {
    #[serde(default = "facts_schema_version")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<FactProvenance>,
    #[serde(default)]
    pub metadata: BTreeMap<PathBuf, Context>,
    #[serde(default)]
    pub captures: Vec<CaptureFact>,
}
impl Default for PlanningFacts {
    fn default() -> Self {
        Self {
            schema_version: 1,
            provenance: None,
            metadata: BTreeMap::new(),
            captures: Vec::new(),
        }
    }
}
const fn facts_schema_version() -> u32 {
    1
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactProvenance {
    pub plan_digest: String,
    pub analysis_identity: String,
    pub files: BTreeMap<PathBuf, FileFingerprint>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileFingerprint {
    pub size: u64,
    pub modified_ns: u128,
    pub content_digest: String,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureFact {
    pub args: Vec<String>,
    pub value: String,
}
impl PlanningFacts {
    /// Facts are caller-supplied observations, not executable commands. Reject
    /// ambiguous keys and values before any workflow input is inspected.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported planning facts schema_version {}",
                self.schema_version
            ));
        }
        let types = bite_expr::definition::metadata_types();
        for (path, values) in &self.metadata {
            if !path.is_absolute() {
                return Err(format!(
                    "Planning metadata path must be absolute: {}",
                    path.display()
                ));
            }
            for (key, value) in values {
                if types.get(key) != Some(&value.kind()) {
                    return Err(format!(
                        "Invalid planning metadata {key:?} for {}",
                        path.display()
                    ));
                }
                match value {
                    bite_expr::Value::Float(n) if !n.is_finite() || *n < 0.0 => {
                        return Err(format!("Invalid planning metadata number: {key}"))
                    }
                    bite_expr::Value::Int(n) if *n < 0 => {
                        return Err(format!("Invalid planning metadata number: {key}"))
                    }
                    bite_expr::Value::String(s) if s.len() > bite_expr::MAX_STRING => {
                        return Err(format!("Planning metadata exceeds string limit: {key}"))
                    }
                    _ => {}
                }
            }
        }
        let mut commands = BTreeSet::new();
        for fact in &self.captures {
            if fact.args.is_empty() || !commands.insert(&fact.args) {
                return Err("Empty or duplicate planning capture arguments".into());
            }
            if fact.value.len() > bite_expr::MAX_STRING {
                return Err("Planning capture exceeds string limit".into());
            }
        }
        Ok(())
    }
    pub fn seal(
        &mut self,
        graph: &Graph,
        registry: &Registry,
        analysis_identity: impl Into<String>,
    ) -> Result<(), String> {
        let mut paths: BTreeSet<PathBuf> = self.metadata.keys().cloned().collect();
        for capture in &self.captures {
            paths.extend(
                capture
                    .args
                    .iter()
                    .map(PathBuf::from)
                    .filter(|path| path.is_file()),
            );
        }
        let files = paths
            .into_iter()
            .map(|path| file_fingerprint(&path).map(|fingerprint| (path, fingerprint)))
            .collect::<Result<_, _>>()?;
        self.schema_version = 1;
        self.provenance = Some(FactProvenance {
            plan_digest: plan_digest(graph, registry)?,
            analysis_identity: analysis_identity.into(),
            files,
        });
        Ok(())
    }
    fn validate_provenance(
        &self,
        graph: &Graph,
        registry: &Registry,
        analysis_identity: &str,
    ) -> Result<(), String> {
        if self.metadata.is_empty() && self.captures.is_empty() {
            return Ok(());
        }
        let provenance = self
            .provenance
            .as_ref()
            .ok_or("Planning facts are missing provenance; regenerate them with `bite observe`")?;
        if provenance.plan_digest != plan_digest(graph, registry)? {
            return Err("Planning facts do not match this workflow or its definitions".into());
        }
        if provenance.analysis_identity != analysis_identity {
            return Err("Planning facts were produced by a different ImageMagick binary".into());
        }
        for (path, expected) in &provenance.files {
            if &file_fingerprint(path)? != expected {
                return Err(format!(
                    "Planning facts are stale because {} changed",
                    path.display()
                ));
            }
        }
        for path in self.metadata.keys() {
            if !provenance.files.contains_key(path) {
                return Err(format!(
                    "Planning metadata has no file fingerprint: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }
}
fn hash_bytes(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut a = 0xcbf29ce484222325u64;
    let mut b = 0x84222325cbf29ce4u64;
    for part in parts {
        for byte in part.as_ref() {
            a = (a ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            b = (b ^ u64::from(*byte)).wrapping_mul(0x100000001b3 ^ 0x9e37);
        }
        a = (a ^ 0xff).wrapping_mul(0x100000001b3);
        b = (b ^ 0xff).wrapping_mul(0x100000001b3 ^ 0x9e37);
    }
    format!("{a:016x}{b:016x}")
}
fn plan_digest(graph: &Graph, registry: &Registry) -> Result<String, String> {
    let graph = serde_json::to_vec(graph).map_err(|error| error.to_string())?;
    let mut definitions = Vec::new();
    for node in &registry.nodes {
        definitions.extend_from_slice(node.0.as_bytes());
        definitions.extend_from_slice(node.1.definition.version.as_bytes());
        definitions.push(0);
    }
    for format in &registry.formats {
        definitions.extend_from_slice(format.0.as_bytes());
        definitions.extend_from_slice(format.1 .0.version.as_bytes());
        definitions.push(0);
    }
    Ok(hash_bytes([graph, definitions]))
}
fn file_fingerprint(path: &Path) -> Result<FileFingerprint, String> {
    use std::time::UNIX_EPOCH;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("Cannot fingerprint {}: {error}", path.display()))?;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Cannot fingerprint {}: {error}", path.display()))?;
    let modified_ns = metadata
        .modified()
        .map_err(|error| format!("Cannot fingerprint {}: {error}", path.display()))?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| format!("Invalid modification time for {}", path.display()))?
        .as_nanos();
    Ok(FileFingerprint {
        size: metadata.len(),
        modified_ns,
        content_digest: hash_bytes([bytes]),
    })
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannedEvent {
    Output {
        operation: OutputOperation,
        process_args: Option<Vec<String>>,
    },
    Capture {
        args: Vec<String>,
        value: Option<String>,
    },
    Metadata {
        path: PathBuf,
        heavy: bool,
        supplied: bool,
        values: Option<Context>,
    },
}
#[derive(Debug, Serialize)]
pub struct ExecutionPlan {
    pub topology: Plan,
    pub events: Vec<PlannedEvent>,
    pub complete: bool,
    pub requires_analysis: Option<String>,
    pub dependencies: Vec<AnalysisDependency>,
    pub deferred_outputs: Vec<DeferredOutput>,
    pub summary: Option<BatchResult>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnalysisDependency {
    Capture {
        id: String,
        args: Vec<String>,
    },
    Metadata {
        id: String,
        path: PathBuf,
        heavy: bool,
    },
}
impl AnalysisDependency {
    fn id(&self) -> &str {
        match self {
            Self::Capture { id, .. } | Self::Metadata { id, .. } => id,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct DeferredOutput {
    pub input: PathBuf,
    pub output: String,
    pub depends_on: Vec<String>,
    pub operations: Vec<String>,
}
fn dependency_id(kind: &str, parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |byte| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    };
    for byte in kind.bytes() {
        add(byte);
    }
    for part in parts {
        for byte in part.as_ref().bytes() {
            add(byte);
        }
        add(0);
    }
    format!("{kind}:{hash:016x}")
}
type MetadataReader<'a> = dyn FnMut(&Path, bool) -> Result<Context, String> + 'a;
struct RecordingHost<'a> {
    facts: &'a PlanningFacts,
    metadata_reader: &'a mut MetadataReader<'a>,
    events: Vec<PlannedEvent>,
    pending: Option<String>,
    dependencies: Vec<AnalysisDependency>,
    deferred_outputs: Vec<DeferredOutput>,
    current_dependencies: Vec<String>,
    fatal: Option<String>,
}
impl ImageHost for RecordingHost<'_> {
    fn continue_after_image_error(&self) -> bool {
        self.fatal.is_none()
    }
    fn image_error(&mut self, input: &Path, output: &str, error: &str) {
        if !self.current_dependencies.is_empty() {
            self.deferred_outputs.push(DeferredOutput {
                input: input.into(),
                output: output.into(),
                depends_on: std::mem::take(&mut self.current_dependencies),
                operations: Vec::new(),
            });
        } else {
            self.fatal = Some(error.into());
        }
    }
    fn run(&mut self, _: &[String]) -> Result<(), String> {
        Err("planner received an unclassified process operation".into())
    }
    fn emit(&mut self, operation: OutputOperation, _: &RunOptions) -> Result<(), String> {
        let process_args = operation.process_args()?;
        self.events.push(PlannedEvent::Output {
            operation,
            process_args,
        });
        Ok(())
    }
    fn capture(&mut self, args: &[String]) -> Result<String, String> {
        let value = self
            .facts
            .captures
            .iter()
            .find(|f| f.args == args)
            .map(|f| f.value.clone());
        // A mean is measured either one channel at a time or every channel of an image at
        // once, and the second reports one number per channel. Both end by formatting
        // `%[fx:mean]`, and every number either reports has to be a real mean.
        let measures_mean = matches!(args, [.., flag, format, info]
            if flag == "-format" && format.trim() == "%[fx:mean]" && info == "info:");
        if measures_mean {
            if let Some(value) = &value {
                let mut means = value.split_whitespace().peekable();
                if means.peek().is_none()
                    || !means.all(|mean| {
                        mean.parse::<f64>()
                            .is_ok_and(|mean| mean.is_finite() && (0.0..=1.0).contains(&mean))
                    })
                {
                    return Err(
                        "Invalid planning mean capture: expected a finite number in [0, 1]".into(),
                    );
                }
            }
        }
        self.events.push(PlannedEvent::Capture {
            args: args.to_vec(),
            value: value.clone(),
        });
        value.ok_or_else(|| {
            let id = dependency_id("capture", args);
            if !self.dependencies.iter().any(|dependency| dependency.id() == id) {
                self.dependencies.push(AnalysisDependency::Capture { id: id.clone(), args: args.to_vec() });
            }
            self.current_dependencies.push(id);
            let message = "Image analysis result required; supply the recorded capture in --facts to continue".to_owned();
            self.pending = Some(message.clone()); message
        })
    }
    fn metadata(&mut self, path: &Path, heavy: bool) -> Result<Context, String> {
        let values = self
            .facts
            .metadata
            .get(path)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| (self.metadata_reader)(path, heavy));
        self.events.push(PlannedEvent::Metadata {
            path: path.into(),
            heavy,
            supplied: self.facts.metadata.contains_key(path),
            values: values.as_ref().ok().cloned(),
        });
        values.inspect_err(|message| {
            let path_text = path.to_string_lossy();
            let id = dependency_id(
                "metadata",
                [path_text.as_ref(), if heavy { "heavy" } else { "light" }],
            );
            if !self
                .dependencies
                .iter()
                .any(|dependency| dependency.id() == id)
            {
                self.dependencies.push(AnalysisDependency::Metadata {
                    id: id.clone(),
                    path: path.into(),
                    heavy,
                });
            }
            self.current_dependencies.push(id);
            self.pending = Some(message.clone());
        })
    }
}

/// Uses precisely the same topology, parameter resolution, native executors and
/// output naming as execution, while replacing every write/process with data.
/// A missing analysis fact suspends planning; the unresolved topology remains
/// available and `complete` is false. No fabricated analysis values are used.
pub fn concrete<'a>(
    graph: &Graph,
    registry: &Registry,
    options: &RunOptions,
    facts: &'a PlanningFacts,
    metadata_reader: &'a mut MetadataReader<'a>,
) -> Result<ExecutionPlan, String> {
    facts.validate()?;
    facts.validate_provenance(graph, registry, &options.analysis_identity)?;
    let topology = build(graph, registry)?;
    let mut host = RecordingHost {
        facts,
        metadata_reader,
        events: Vec::new(),
        pending: None,
        dependencies: Vec::new(),
        deferred_outputs: Vec::new(),
        current_dependencies: Vec::new(),
        fatal: None,
    };
    let result = execution::run_workflow(graph, registry, &mut host, options, &mut |_, _, _| {});
    let summary = match result {
        Ok(result) if host.pending.is_none() => Some(result),
        Ok(_) => None,
        Err(error) if host.pending.is_none() => return Err(error),
        Err(_) => None,
    };
    for deferred in &mut host.deferred_outputs {
        if let Some(output) = topology
            .outputs
            .iter()
            .find(|output| output.node == deferred.output)
        {
            deferred.operations = output
                .operations
                .iter()
                .map(|operation| operation.node.clone())
                .collect();
            deferred.operations.push(output.node.clone());
        }
    }
    Ok(ExecutionPlan {
        topology,
        complete: host.pending.is_none(),
        requires_analysis: host.pending,
        events: host.events,
        dependencies: host.dependencies,
        deferred_outputs: host.deferred_outputs,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_expr::Value;

    #[test]
    fn fact_provenance_rejects_changed_files_tools_and_plans() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(
            &root.join("node-definitions"),
            &root.join("format-definitions"),
        )
        .unwrap();
        let mut graph = crate::workflow::load(
            &std::fs::read_to_string(root.join("test-workflows/wf-01-fastpath.bite")).unwrap(),
            &registry,
        )
        .unwrap()
        .workflow
        .graph;
        let directory = tempfile::tempdir().unwrap();
        let image = directory.path().join("image.png");
        std::fs::write(&image, b"first image").unwrap();
        let mut facts = PlanningFacts::default();
        facts.metadata.insert(
            image.clone(),
            [("image.width".into(), Value::Int(1))].into(),
        );
        facts.seal(&graph, &registry, "magick:a").unwrap();
        facts
            .validate_provenance(&graph, &registry, "magick:a")
            .unwrap();

        std::fs::write(&image, b"changed image").unwrap();
        assert!(facts
            .validate_provenance(&graph, &registry, "magick:a")
            .unwrap_err()
            .contains("changed"));
        facts.seal(&graph, &registry, "magick:a").unwrap();
        assert!(facts
            .validate_provenance(&graph, &registry, "magick:b")
            .unwrap_err()
            .contains("different ImageMagick"));
        graph.nodes[0].data.label.push_str(" changed");
        assert!(facts
            .validate_provenance(&graph, &registry, "magick:a")
            .unwrap_err()
            .contains("workflow or its definitions"));
    }
}
