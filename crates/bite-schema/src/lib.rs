//! Versioned data only. No filesystem access, evaluation, or pipeline execution.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type Params = BTreeMap<String, ParamValue>;
pub type ValidationResult = Result<(), Vec<String>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ParamValue {
    Null,
    Bool(bool),
    Int(i64),
    Number(f64),
    String(String),
    Vector(Vec<f64>),
    Structured(StructuredParam),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StructuredParam {
    RenameBlocks { blocks: Vec<RenameBlock> },
    SetSuffixes { suffixes: Vec<String> },
    TextSlots { slots: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum RenameBlock {
    Text { value: String },
    Number { start: f64, pad: f64 },
    Oldname { find: String, replace_with: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Int,
    Float,
    String,
    Enum,
    Bool,
    Vector2,
    Vector3,
    Vector4,
    Color,
    Numeric,
    Value,
    RenameBlocks,
    SetSuffixes,
    TextSlots,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetType {
    Slider,
    Number,
    Dropdown,
    Text,
    Checkbox,
    ColorPicker,
    Vector,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParamDefinition {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: ParamType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<WidgetType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default, rename = "portOnly")]
    pub port_only: bool,
    #[serde(default, rename = "noPort")]
    pub no_port: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_when: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    Image,
    Mask,
    Number,
    Path,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: PortType,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum ArgSpec {
    Literal(String),
    Expression {
        expr: String,
    },
    Conditional {
        when: String,
        args: Vec<ArgSpec>,
    },
    Switch {
        switch: String,
        cases: BTreeMap<String, Vec<ArgSpec>>,
        #[serde(default)]
        default: Vec<ArgSpec>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Implementation {
    Imagemagick { args: Vec<ArgSpec> },
    Compute { outputs: BTreeMap<String, String> },
    Native { executor: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeDefinition {
    #[schemars(range(min = 2, max = 2))]
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub icon: String,
    pub inputs: Vec<PortDefinition>,
    pub outputs: Vec<PortDefinition>,
    pub params: Vec<ParamDefinition>,
    pub implementation: Implementation,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FormatDefinition {
    #[schemars(range(min = 2, max = 2))]
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub extension: String,
    pub params: Vec<ParamDefinition>,
    pub args: Vec<ArgSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    #[schemars(range(min = 2, max = 2))]
    pub schema_version: u32,
    pub created_with: String,
    pub graph: Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub viewport: Viewport,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: NodeKind,
    pub position: Position,
    #[serde(default, rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    pub data: NodeData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Extent {
    Parent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum BuiltinNodeKind {
    #[serde(rename = "inputNode")]
    Input,
    #[serde(rename = "imageOutputNode")]
    ImageOutput,
    #[serde(rename = "textOutputNode")]
    TextOutput,
    #[serde(rename = "flipbookOutputNode")]
    FlipbookOutput,
    #[serde(rename = "folderPathNode")]
    FolderPath,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "commentNode")]
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum NodeKind {
    Builtin(BuiltinNodeKind),
    Processing(ProcessingNodeKind),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProcessingNodeKind {
    #[serde(rename = "process")]
    Process,
    #[serde(rename = "compareNode")]
    Compare,
    #[serde(rename = "setInputNode")]
    SetInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeData {
    pub label: String,
    #[serde(rename = "definitionId")]
    pub definition_id: String,
    pub params: Params,
    /// Instance-defined ports for Text Output and Process As Set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<PortDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PortDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    #[serde(rename = "sourceHandle")]
    pub source_handle: String,
    pub target: String,
    #[serde(rename = "targetHandle")]
    pub target_handle: String,
}

// Builtin parameters have a typed contract too. These structs validate the map
// selected by NodeKind, without allowing executable objects in workflow data.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct InputParams {
    pub cli_name: String,
    pub thumbnail_size: Option<u32>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageOutputParams {
    pub cli_name: String,
    pub output_path: OutputPathMode,
    pub custom_path: String,
    pub overwrite: OverwriteMode,
    pub generate_log: bool,
    pub set_output_prefix: String,
    pub set_output_suffix: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputPathMode {
    #[default]
    Source,
    Custom,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OverwriteMode {
    #[default]
    Skip,
    Overwrite,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct TextOutputParams {
    pub next_port_index: Option<u32>,
    pub cli_name: String,
    pub output_path: String,
    pub generate_log: bool,
    pub use_preview_for_processing: bool,
    pub separator_type: SeparatorType,
    pub custom_separator: String,
    pub port_ids: Option<StructuredParam>,
    pub overwrite: OverwriteMode,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SeparatorType {
    #[default]
    Comma,
    Tab,
    Space,
    Custom,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct FlipbookParams {
    pub overwrite: OverwriteMode,
    pub cli_name: String,
    pub flipbook_output_path: String,
    pub generate_log: bool,
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub cell_width: Option<u32>,
    pub cell_height: Option<u32>,
    pub sort_by: SortBy,
    pub bg_color: Option<[f64; 4]>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    ImportOrder,
    Name,
    NameDesc,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct FolderPathParams {
    pub folder_path: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CommentParams {
    pub heading: String,
    pub body: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GroupParams {
    pub color: Option<ParamValue>,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with("__")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
fn unique<'a>(names: impl Iterator<Item = &'a str>, what: &str, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for name in names {
        if !valid_name(name) {
            errors.push(format!("{what} {name:?}: invalid name"));
        }
        if !seen.insert(name) {
            errors.push(format!("{what} {name:?}: duplicate name"));
        }
    }
}
fn version(v: u32, errors: &mut Vec<String>) {
    if v != 2 {
        errors.push(format!("unsupported schema_version {v}; expected 2"));
    }
}
fn finish(errors: Vec<String>) -> ValidationResult {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
fn params_validate(params: &[ParamDefinition], errors: &mut Vec<String>) {
    unique(params.iter().map(|p| p.name.as_str()), "param", errors);
    for p in params {
        let prefix = format!("param {:?}", p.name);
        if p.widget == Some(WidgetType::Slider) && (p.min.is_none() || p.max.is_none()) {
            errors.push(format!("{prefix}: slider requires min and max"));
        }
        if p.min.zip(p.max).is_some_and(|(a, b)| a > b) {
            errors.push(format!("{prefix}: min exceeds max"));
        }
        if p.step.is_some_and(|v| v <= 0.0) {
            errors.push(format!("{prefix}: step must be positive"));
        }
        if p.kind == ParamType::Enum && p.options.is_empty() {
            errors.push(format!("{prefix}: enum requires options"));
        }
        if let Some(value) = &p.default {
            if !p.accepts(value) {
                errors.push(format!("{prefix}: default does not match type/options"));
            }
        }
    }
}
impl ParamDefinition {
    pub fn accepts(&self, value: &ParamValue) -> bool {
        if let ParamValue::Int(n) = value {
            return self.accepts(&ParamValue::Number(*n as f64));
        }
        use ParamType as T;
        use ParamValue as V;
        match (&self.kind, value) {
            (T::Int, V::Number(n)) => n.fract() == 0.0,
            (T::Float | T::Numeric, V::Number(_))
            | (T::Bool, V::Bool(_))
            | (T::String, V::String(_)) => true,
            (T::Enum, V::String(s)) => self.options.contains(s),
            (T::Vector2, V::Vector(v)) => v.len() == 2,
            (T::Vector3, V::Vector(v)) => v.len() == 3,
            (T::Vector4 | T::Color, V::Vector(v)) => v.len() == 4,
            (T::Color, V::String(_)) => true,
            (T::Numeric, V::Vector(v)) => (1..=4).contains(&v.len()),
            (T::Value, V::Structured(_)) => false,
            (T::Value, _) => true,
            (T::RenameBlocks, V::Structured(StructuredParam::RenameBlocks { .. }))
            | (T::SetSuffixes, V::Structured(StructuredParam::SetSuffixes { .. }))
            | (T::TextSlots, V::Structured(StructuredParam::TextSlots { .. })) => true,
            _ => false,
        }
    }
}
fn args_validate(args: &[ArgSpec], depth: usize, errors: &mut Vec<String>) {
    if depth > 32 {
        errors.push("argument nesting exceeds 32".into());
        return;
    }
    for a in args {
        match a {
            ArgSpec::Literal(_) => {}
            ArgSpec::Expression { expr } => expression_validate(expr, errors),
            ArgSpec::Conditional { when, args } => {
                expression_validate(when, errors);
                args_validate(args, depth + 1, errors);
            }
            ArgSpec::Switch {
                switch,
                cases,
                default,
            } => {
                expression_validate(switch, errors);
                for args in cases.values().chain(std::iter::once(default)) {
                    args_validate(args, depth + 1, errors);
                }
            }
        }
    }
}
fn expression_validate(expr: &str, errors: &mut Vec<String>) {
    if expr.trim().is_empty() || expr.len() > 4096 {
        errors.push("expression must contain 1..4096 bytes".into());
    }
}
impl NodeDefinition {
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();
        version(self.schema_version, &mut errors);
        if !valid_name(&self.id) || self.version.is_empty() {
            errors.push("definition requires valid id and nonempty version".into());
        }
        unique(
            self.inputs.iter().map(|p| p.name.as_str()),
            "input port",
            &mut errors,
        );
        unique(
            self.outputs.iter().map(|p| p.name.as_str()),
            "output port",
            &mut errors,
        );
        params_validate(&self.params, &mut errors);
        for p in &self.params {
            for e in [&p.visible_when, &p.enabled_when].into_iter().flatten() {
                expression_validate(e, &mut errors);
            }
        }
        match &self.implementation {
            Implementation::Imagemagick { args } => args_validate(args, 0, &mut errors),
            Implementation::Compute { outputs } => {
                for (name, expr) in outputs {
                    if !self.params.iter().any(|p| p.name == *name && p.readonly) {
                        errors.push(format!(
                            "compute output {name:?}: requires a readonly parameter"
                        ));
                    }
                    expression_validate(expr, &mut errors);
                }
            }
            Implementation::Native { executor } => {
                if !valid_name(executor) {
                    errors.push("invalid native executor name".into());
                }
            }
        }
        finish(errors)
    }
}
impl FormatDefinition {
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();
        version(self.schema_version, &mut errors);
        if !valid_name(&self.id) || self.version.is_empty() {
            errors.push("format requires valid id and nonempty version".into());
        }
        if !self.extension.starts_with('.')
            || self.extension.len() < 2
            || !self.extension[1..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
        {
            errors.push("extension must be a dot followed by alphanumerics".into());
        }
        params_validate(&self.params, &mut errors);
        args_validate(&self.args, 0, &mut errors);
        finish(errors)
    }
}
fn check_builtin<T: serde::de::DeserializeOwned>(params: &Params) -> Result<(), String> {
    let value = serde_json::to_value(params).map_err(|e| e.to_string())?;
    serde_json::from_value::<T>(value)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
impl Workflow {
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();
        version(self.schema_version, &mut errors);
        if self.created_with.is_empty() {
            errors.push("created_with must not be empty".into());
        }
        if self.graph.viewport.zoom <= 0.0 {
            errors.push("viewport zoom must be positive".into());
        }
        unique(
            self.graph.nodes.iter().map(|n| n.id.as_str()),
            "node",
            &mut errors,
        );
        unique(
            self.graph.edges.iter().map(|e| e.id.as_str()),
            "edge",
            &mut errors,
        );
        let ids: BTreeSet<_> = self.graph.nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &self.graph.nodes {
            for key in node.data.params.keys() {
                if !valid_name(key) {
                    errors.push(format!("node {:?}: invalid parameter {key:?}", node.id));
                }
            }
            if node.width.is_some_and(|n| n <= 0.0) || node.height.is_some_and(|n| n <= 0.0) {
                errors.push(format!("node {:?}: dimensions must be positive", node.id));
            }
            if let Some(parent) = &node.parent_id {
                let mut ancestors = BTreeSet::from([node.id.as_str()]);
                let mut next = Some(parent.as_str());
                while let Some(id) = next {
                    if !ancestors.insert(id) {
                        errors.push(format!("node {:?}: group containment cycle", node.id));
                        break;
                    }
                    next = self
                        .graph
                        .nodes
                        .iter()
                        .find(|n| n.id == id)
                        .and_then(|n| n.parent_id.as_deref());
                }
                if parent == &node.id
                    || !self.graph.nodes.iter().any(|n| {
                        &n.id == parent && n.kind == NodeKind::Builtin(BuiltinNodeKind::Group)
                    })
                {
                    errors.push(format!(
                        "node {:?}: parent must reference another group",
                        node.id
                    ));
                }
            }
            if node.extent.is_some() && node.parent_id.is_none() {
                errors.push(format!("node {:?}: extent requires parentId", node.id));
            }
            let builtin = match &node.kind {
                NodeKind::Builtin(BuiltinNodeKind::Input) => {
                    check_builtin::<InputParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                    check_builtin::<ImageOutputParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                    check_builtin::<TextOutputParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
                    check_builtin::<FlipbookParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::FolderPath) => {
                    check_builtin::<FolderPathParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::Group) => {
                    check_builtin::<GroupParams>(&node.data.params)
                }
                NodeKind::Builtin(BuiltinNodeKind::Comment) => {
                    check_builtin::<CommentParams>(&node.data.params)
                }
                NodeKind::Processing(_) => {
                    if node.data.definition_id.is_empty() {
                        Err("processing node requires definitionId".into())
                    } else {
                        Ok(())
                    }
                }
            };
            if let Err(e) = builtin {
                errors.push(format!("node {:?}: {e}", node.id));
            }
            unique(
                node.data.inputs.iter().map(|p| p.name.as_str()),
                "instance input",
                &mut errors,
            );
            unique(
                node.data.outputs.iter().map(|p| p.name.as_str()),
                "instance output",
                &mut errors,
            );
        }
        for edge in &self.graph.edges {
            if !ids.contains(edge.source.as_str()) || !ids.contains(edge.target.as_str()) {
                errors.push(format!("edge {:?}: unknown endpoint", edge.id));
            }
            for (h, source) in [(&edge.source_handle, true), (&edge.target_handle, false)] {
                let valid = h.split_once(':').is_some_and(|(kind, name)| {
                    valid_name(name)
                        && (kind == "param"
                            || (source && kind == "out")
                            || (!source && (kind == "in" || kind == "txo")))
                });
                if !valid {
                    errors.push(format!("edge {:?}: invalid handle {h:?}", edge.id));
                }
            }
        }
        finish(errors)
    }
}

/// The formal schemas are derived from the same serde contracts as the CLI.
pub fn json_schemas() -> [(String, Value); 3] {
    let mut workflow = serde_json::to_value(schemars::schema_for!(Workflow)).unwrap();
    let builtins = [
        ("inputNode", schemars::schema_for!(InputParams)),
        ("imageOutputNode", schemars::schema_for!(ImageOutputParams)),
        ("textOutputNode", schemars::schema_for!(TextOutputParams)),
        ("flipbookOutputNode", schemars::schema_for!(FlipbookParams)),
        ("folderPathNode", schemars::schema_for!(FolderPathParams)),
        ("group", schemars::schema_for!(GroupParams)),
        ("commentNode", schemars::schema_for!(CommentParams)),
    ];
    let mut conditions = Vec::new();
    for (kind, schema) in builtins {
        let mut schema = serde_json::to_value(schema).unwrap();
        if let Some(Value::Object(defs)) = schema.as_object_mut().unwrap().remove("$defs") {
            workflow["$defs"].as_object_mut().unwrap().extend(defs);
        }
        schema.as_object_mut().unwrap().remove("$schema");
        conditions.push(serde_json::json!({"if":{"properties":{"type":{"const":kind}},"required":["type"]},"then":{"properties":{"data":{"properties":{"params":schema}}}}}));
    }
    workflow["$defs"]["GraphNode"]["allOf"] = Value::Array(conditions);
    [
        (
            "node-definition-v2.schema.json".into(),
            serde_json::to_value(schemars::schema_for!(NodeDefinition)).unwrap(),
        ),
        (
            "format-definition-v2.schema.json".into(),
            serde_json::to_value(schemars::schema_for!(FormatDefinition)).unwrap(),
        ),
        ("workflow-v2.schema.json".into(), workflow),
    ]
}
