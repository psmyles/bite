use crate::Registry;
use bite_schema::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn topo_sort(graph: &Graph) -> Result<Vec<String>, String> {
    let mut degrees: BTreeMap<_, usize> = graph.nodes.iter().map(|n| (n.id.clone(), 0)).collect();
    let mut next: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &graph.edges {
        if !degrees.contains_key(&e.source) || !degrees.contains_key(&e.target) {
            return Err(format!("edge {}: unknown endpoint", e.id));
        }
        *degrees.get_mut(&e.target).unwrap() += 1;
        next.entry(e.source.clone())
            .or_default()
            .push(e.target.clone());
    }
    let mut queue: VecDeque<_> = graph
        .nodes
        .iter()
        .filter(|n| degrees[&n.id] == 0)
        .map(|n| n.id.clone())
        .collect();
    let mut result = Vec::new();
    while let Some(id) = queue.pop_front() {
        result.push(id.clone());
        for target in next.get(&id).into_iter().flatten() {
            let degree = degrees.get_mut(target).unwrap();
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(target.clone());
            }
        }
    }
    if result.len() != graph.nodes.len() {
        return Err("graph contains a cycle".into());
    }
    Ok(result)
}
pub fn trace(graph: &Graph, starts: &[String], backwards: bool) -> Vec<String> {
    let mut queue: VecDeque<_> = starts.iter().cloned().collect();
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        result.push(id.clone());
        for e in &graph.edges {
            if backwards && e.target == id {
                queue.push_back(e.source.clone());
            } else if !backwards && e.source == id {
                queue.push_back(e.target.clone());
            }
        }
    }
    result
}
pub fn trace_input(graph: &Graph, output: &str) -> Option<String> {
    let mut queue = VecDeque::from([output.to_owned()]);
    let mut seen = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if graph
            .nodes
            .iter()
            .any(|n| n.id == id && n.kind == NodeKind::Builtin(BuiltinNodeKind::Input))
        {
            return Some(id);
        }
        for e in &graph.edges {
            if e.target == id
                && !e.source_handle.starts_with("param:")
                && !e.target_handle.starts_with("param:")
            {
                queue.push_back(e.source.clone());
            }
        }
    }
    None
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType {
    Image,
    Mask,
    Number,
    Bool,
    Numeric,
    String,
    Path,
    Vector2,
    Vector3,
    Vector4,
    Color,
    Value,
}
pub fn compatible(a: WireType, b: WireType) -> bool {
    use WireType::*;
    a == b
        || a == Value
        || b == Value
        || (matches!(a, Number | Bool | Numeric) && matches!(b, Number | Bool | Numeric))
        || ((a == Numeric || b == Numeric)
            && matches!(a, Number | Numeric | Vector2 | Vector3 | Vector4 | Color)
            && matches!(b, Number | Numeric | Vector2 | Vector3 | Vector4 | Color))
}
fn param_wire(p: &ParamType) -> Result<WireType, String> {
    Ok(match p {
        ParamType::Int | ParamType::Float | ParamType::Enum => WireType::Number,
        ParamType::Bool => WireType::Bool,
        ParamType::String => WireType::String,
        ParamType::Numeric => WireType::Numeric,
        ParamType::Vector2 => WireType::Vector2,
        ParamType::Vector3 => WireType::Vector3,
        ParamType::Vector4 => WireType::Vector4,
        ParamType::Color => WireType::Color,
        ParamType::Value => WireType::Value,
        _ => return Err("structured parameters cannot be wired as expressions".into()),
    })
}
pub fn handle_type(
    node: &GraphNode,
    handle: &str,
    source: bool,
    registry: &Registry,
) -> Result<WireType, String> {
    let (kind, name) = handle.split_once(':').ok_or("invalid handle")?;
    if kind == "param" {
        if name == "_enabled" && !source {
            return Ok(WireType::Bool);
        }
        if name == "bgColor" && node.kind == NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) {
            return Ok(WireType::Color);
        }
        if (name == "prefix" || name.starts_with("suffix_"))
            && node.data.definition_id == "process_as_set"
        {
            return Ok(WireType::String);
        }
        let def = registry
            .nodes
            .get(&node.data.definition_id)
            .ok_or("unknown definition")?;
        let p = def
            .definition
            .params
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| format!("unknown parameter {name}"))?;
        if !source && (p.readonly || p.no_port) {
            return Err(format!("parameter {name} has no writable port"));
        }
        return param_wire(&p.kind);
    }
    if kind == "txo" && !source && node.kind == NodeKind::Builtin(BuiltinNodeKind::TextOutput) {
        return Ok(if name == "condition" {
            WireType::Bool
        } else {
            WireType::Value
        });
    }
    if (source && kind != "out") || (!source && kind != "in") {
        return Err("wrong handle direction".into());
    }
    if name == "folder" && !source && node.kind == NodeKind::Builtin(BuiltinNodeKind::ImageOutput) {
        return Ok(WireType::Path);
    }
    let ports = if source {
        &node.data.outputs
    } else {
        &node.data.inputs
    };
    let ports: &[PortDefinition] = if !ports.is_empty() {
        ports.as_slice()
    } else if let Some(d) = registry.nodes.get(&node.data.definition_id) {
        if source {
            &d.definition.outputs
        } else {
            &d.definition.inputs
        }
    } else {
        &[]
    };
    if let Some(p) = ports.iter().find(|p| p.name == name) {
        return Ok(match p.kind {
            PortType::Image => WireType::Image,
            PortType::Mask => WireType::Mask,
            PortType::Number => WireType::Number,
            PortType::Path => WireType::Path,
        });
    }
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) if source && name == "output" => {
            Ok(WireType::Image)
        }
        NodeKind::Builtin(
            BuiltinNodeKind::ImageOutput
            | BuiltinNodeKind::TextOutput
            | BuiltinNodeKind::FlipbookOutput,
        ) if !source && name == "input" => Ok(WireType::Image),
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) if source && name == "output" => {
            Ok(WireType::Path)
        }
        _ => Err(format!("node {}: unknown port {handle}", node.id)),
    }
}
pub fn validate(graph: &Graph, registry: &Registry) -> Result<(), String> {
    topo_sort(graph)?;
    let nodes: BTreeMap<_, _> = graph.nodes.iter().map(|n| (&n.id, n)).collect();
    let mut targets = BTreeSet::new();
    for n in &graph.nodes {
        if matches!(n.kind, NodeKind::Processing(_))
            && !registry.nodes.contains_key(&n.data.definition_id)
        {
            return Err(format!(
                "node {}: unknown definition {}",
                n.id, n.data.definition_id
            ));
        }
        if let Some(def) = registry.nodes.get(&n.data.definition_id) {
            for (name, value) in &n.data.params {
                if name == "_enabled" && matches!(value, ParamValue::Bool(_)) {
                    continue;
                }
                let p = def
                    .definition
                    .params
                    .iter()
                    .find(|p| p.name == *name)
                    .or_else(|| {
                        (n.data.definition_id == "format_convert")
                            .then(|| {
                                registry
                                    .formats
                                    .values()
                                    .flat_map(|(f, _)| &f.params)
                                    .find(|p| p.name == *name)
                            })
                            .flatten()
                    });
                let Some(p) = p else {
                    return Err(format!("node {}: unknown parameter {name}", n.id));
                };
                let computed = matches!(&def.implementation, bite_expr::definition::CompiledImplementation::Compute(outputs) if outputs.contains_key(name));
                if !computed && !p.accepts(value) {
                    return Err(format!("node {}: invalid value for parameter {name}", n.id));
                }
            }
        }
    }
    for e in &graph.edges {
        if !targets.insert((&e.target, &e.target_handle)) {
            return Err(format!("edge {}: input already connected", e.id));
        }
        let source = nodes[&e.source];
        let target = nodes[&e.target];
        let a = handle_type(source, &e.source_handle, true, registry)?;
        let b = handle_type(target, &e.target_handle, false, registry)?;
        let effective = |node: &GraphNode, raw: WireType| -> Result<WireType, String> {
            if raw != WireType::Value {
                return Ok(raw);
            }
            let Some(d) = registry.nodes.get(&node.data.definition_id) else {
                return Ok(raw);
            };
            for p in d
                .definition
                .params
                .iter()
                .filter(|p| p.kind == ParamType::Value && !p.readonly)
            {
                if let Some(edge) = graph
                    .edges
                    .iter()
                    .find(|e| e.target == node.id && e.target_handle == format!("param:{}", p.name))
                {
                    let t = handle_type(nodes[&edge.source], &edge.source_handle, true, registry)?;
                    if t != WireType::Value {
                        return Ok(t);
                    }
                }
            }
            Ok(raw)
        };
        let resolved_a = effective(source, a)?;
        let resolved_b = effective(target, b)?;
        if !compatible(resolved_a, resolved_b)
            && !(target.data.definition_id == "channel_merge"
                && b == WireType::Image
                && matches!(a, WireType::Number | WireType::Bool | WireType::Numeric))
        {
            return Err(format!("edge {}: incompatible {:?} -> {:?}", e.id, a, b));
        }
        if b == WireType::Value {
            if let Some(d) = registry.nodes.get(&target.data.definition_id) {
                for p in d
                    .definition
                    .params
                    .iter()
                    .filter(|p| p.kind == ParamType::Value)
                {
                    for sibling in &graph.edges {
                        if !p.readonly
                            && sibling.target == target.id
                            && sibling.target_handle == format!("param:{}", p.name)
                        {
                            let t = handle_type(
                                nodes[&sibling.source],
                                &sibling.source_handle,
                                true,
                                registry,
                            )?;
                            if !compatible(a, t) {
                                return Err(format!(
                                    "edge {}: incompatible sibling value inputs",
                                    e.id
                                ));
                            }
                        }
                        if p.readonly
                            && sibling.source == target.id
                            && sibling.source_handle == format!("param:{}", p.name)
                        {
                            let t = handle_type(
                                nodes[&sibling.target],
                                &sibling.target_handle,
                                false,
                                registry,
                            )?;
                            if !compatible(resolved_a, t) {
                                return Err(format!(
                                    "edge {}: incompatible downstream value output",
                                    e.id
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
