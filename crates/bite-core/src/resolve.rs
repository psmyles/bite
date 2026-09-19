use crate::Registry;
use bite_expr::{
    definition::{default_context, to_value, CompiledImplementation},
    Context, Value,
};
use bite_schema::{Graph, GraphNode, ParamType};
use std::collections::BTreeMap;

pub type ResolvedParams = BTreeMap<String, Context>;
fn wired_value(node: &GraphNode, target: &str, value: &Value, registry: &Registry) -> Value {
    let kind = if target == "_enabled" {
        Some(ParamType::Bool)
    } else {
        registry
            .nodes
            .get(&node.data.definition_id)
            .and_then(|definition| {
                definition
                    .definition
                    .params
                    .iter()
                    .find(|param| param.name == target)
                    .map(|param| param.kind.clone())
            })
    };
    match (kind, value) {
        (Some(ParamType::Int | ParamType::Float | ParamType::Numeric), Value::Bool(value)) => {
            Value::Int(i64::from(*value))
        }
        (Some(ParamType::Bool), value) => Value::Bool(value.truthy()),
        (Some(ParamType::String | ParamType::Enum), value) => Value::String(value.text()),
        _ => value.clone(),
    }
}
pub fn node_params(
    node: &GraphNode,
    graph: &Graph,
    resolved: &ResolvedParams,
    registry: &Registry,
    metadata: &Context,
) -> Result<Context, String> {
    let definition = registry.nodes.get(&node.data.definition_id);
    let mut context = definition
        .map(|d| default_context(&d.definition.params))
        .unwrap_or_default();
    for (key, value) in &node.data.params {
        if let Some(value) = to_value(value) {
            context.insert(key.clone(), value);
        }
    }
    for e in graph.edges.iter().filter(|e| e.target == node.id) {
        if let Some(source) = e.source_handle.strip_prefix("param:") {
            if let Some(value) = resolved.get(&e.source).and_then(|p| p.get(source)) {
                if let Some(target) = e.target_handle.strip_prefix("param:") {
                    context.insert(target.into(), wired_value(node, target, value, registry));
                } else if let Some(target) = e.target_handle.strip_prefix("txo:") {
                    let value = if target == "condition" {
                        Value::Bool(value.truthy())
                    } else {
                        value.clone()
                    };
                    context.insert(format!("_txo_{target}"), value);
                }
            }
        }
    }
    context.extend(metadata.clone());
    if let Some(d) = definition {
        if let CompiledImplementation::Compute(outputs) = &d.implementation {
            let values: Context = outputs
                .iter()
                .map(|(name, e)| {
                    e.evaluate(&context)
                        .map(|v| (name.clone(), v))
                        .map_err(|e| format!("node {}: {e}", node.id))
                })
                .collect::<Result<_, _>>()?;
            context.extend(values);
        }
    }
    Ok(context)
}
pub fn text_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Float(n) => {
            if n.fract() == 0.0 {
                n.to_string()
            } else {
                format!("{n:.4}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .into()
            }
        }
        Value::Vector(v) => v
            .iter()
            .map(|n| text_value(&Value::Float(*n)))
            .collect::<Vec<_>>()
            .join(", "),
        v => v.text(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{GraphEdge, ParamValue};
    use std::path::PathBuf;

    fn registry() -> Registry {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        Registry::load(
            &root.join("node-definitions-v2"),
            &root.join("format-definitions-v2"),
        )
        .unwrap()
    }

    #[test]
    fn parameter_wires_apply_defaults_and_legacy_boolean_number_coercion() {
        let registry = registry();
        let mut source: GraphNode = serde_json::from_value(serde_json::json!({
            "id":"source","type":"process","position":{"x":0,"y":0},
            "data":{"label":"Boolean","definitionId":"value_boolean","params":{"value":true}}
        }))
        .unwrap();
        let target: GraphNode = serde_json::from_value(serde_json::json!({
            "id":"target","type":"process","position":{"x":0,"y":0},
            "data":{"label":"Add","definitionId":"math_add","params":{"b":2}}
        }))
        .unwrap();
        let edge = GraphEdge {
            id: "wire".into(),
            source: source.id.clone(),
            source_handle: "param:value".into(),
            target: target.id.clone(),
            target_handle: "param:a".into(),
        };
        let mut graph = Graph {
            nodes: vec![source.clone(), target.clone()],
            edges: vec![edge],
            viewport: bite_schema::Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        crate::graph::validate(&graph, &registry).unwrap();
        let mut resolved = ResolvedParams::new();
        let source_values =
            node_params(&source, &graph, &resolved, &registry, &Context::new()).unwrap();
        resolved.insert(source.id.clone(), source_values);
        let target_values =
            node_params(&target, &graph, &resolved, &registry, &Context::new()).unwrap();
        assert_eq!(target_values["a"], Value::Int(1));
        assert_eq!(target_values["b"], Value::Int(2));
        assert_eq!(target_values["result"], Value::Float(3.0));

        source.data.definition_id = "value_float".into();
        source.data.params = [("value".into(), ParamValue::Number(0.0))].into();
        let bypass_target = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "target")
            .unwrap();
        bypass_target.data.definition_id = "negate".into();
        bypass_target.data.params.clear();
        graph.nodes[0] = source.clone();
        graph.edges[0].target_handle = "param:_enabled".into();
        graph.edges[0].source_handle = "param:value".into();
        crate::graph::validate(&graph, &registry).unwrap();
        resolved.clear();
        resolved.insert(
            source.id.clone(),
            node_params(&source, &graph, &resolved, &registry, &Context::new()).unwrap(),
        );
        let target = graph.nodes.iter().find(|node| node.id == "target").unwrap();
        let target_values =
            node_params(target, &graph, &resolved, &registry, &Context::new()).unwrap();
        assert_eq!(target_values["_enabled"], Value::Bool(false));
    }
}
