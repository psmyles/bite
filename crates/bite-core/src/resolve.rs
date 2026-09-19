use crate::Registry;
use bite_expr::{
    definition::{default_context, to_value, CompiledImplementation},
    Context, Value,
};
use bite_schema::{Graph, GraphNode};
use std::collections::BTreeMap;

pub type ResolvedParams = BTreeMap<String, Context>;
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
                    context.insert(target.into(), value.clone());
                } else if let Some(target) = e.target_handle.strip_prefix("txo:") {
                    context.insert(format!("_txo_{target}"), value.clone());
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
