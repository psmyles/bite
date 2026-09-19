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
        outputs.push(OutputPlan {
            node: output.id.clone(),
            input: graph::trace_input(graph, &output.id),
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
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningFacts {
    #[serde(default)]
    pub metadata: BTreeMap<PathBuf, Context>,
    #[serde(default)]
    pub captures: Vec<CaptureFact>,
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureFact {
    pub args: Vec<String>,
    pub value: String,
}
impl PlanningFacts {
    /// Facts are caller-supplied observations, not executable commands. Reject
    /// ambiguous keys and values before any workflow input is inspected.
    pub fn validate(&self) -> Result<(), String> {
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
    pub summary: Option<BatchResult>,
}
type MetadataReader<'a> = dyn FnMut(&Path, bool) -> Result<Context, String> + 'a;
struct RecordingHost<'a> {
    facts: &'a PlanningFacts,
    metadata_reader: &'a mut MetadataReader<'a>,
    events: Vec<PlannedEvent>,
    pending: Option<String>,
}
impl ImageHost for RecordingHost<'_> {
    fn continue_after_image_error(&self) -> bool {
        false
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
        if args.ends_with(&["-format".into(), "%[fx:mean]".into(), "info:".into()]) {
            if let Some(value) = &value {
                if !value
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && (0.0..=1.0).contains(&v))
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
    let topology = build(graph, registry)?;
    let mut host = RecordingHost {
        facts,
        metadata_reader,
        events: Vec::new(),
        pending: None,
    };
    let result = execution::run_workflow(graph, registry, &mut host, options, &mut |_, _, _| {});
    let summary = match result {
        Ok(result) => Some(result),
        Err(error) if host.pending.is_none() => return Err(error),
        Err(_) => None,
    };
    Ok(ExecutionPlan {
        topology,
        complete: host.pending.is_none(),
        requires_analysis: host.pending,
        events: host.events,
        summary,
    })
}
