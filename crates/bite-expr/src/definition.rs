use crate::*;
use bite_schema::{Implementation, NodeDefinition, ParamDefinition, ParamType, ParamValue};

pub fn parameter_types(params: &[ParamDefinition]) -> TypeContext {
    params
        .iter()
        .filter_map(|p| {
            Some((
                p.name.clone(),
                match p.kind {
                    ParamType::Int | ParamType::Float => Type::Number,
                    ParamType::String | ParamType::Enum => Type::String,
                    ParamType::Bool => Type::Bool,
                    ParamType::Vector2 | ParamType::Vector3 | ParamType::Vector4 => Type::Vector,
                    ParamType::Color | ParamType::Value => Type::Value,
                    ParamType::Numeric => Type::Numeric,
                    _ => return None,
                },
            ))
        })
        .collect()
}
pub fn metadata_types() -> TypeContext {
    let mut types: TypeContext = ["path", "name", "extension", "format"]
        .into_iter()
        .map(|s| (format!("image.{s}"), Type::String))
        .chain(
            ["size", "width", "height", "bit_depth", "dpi_x", "dpi_y"]
                .into_iter()
                .map(|s| (format!("image.{s}"), Type::Number)),
        )
        .collect();
    for key in [
        "Make",
        "Model",
        "LensModel",
        "LensMake",
        "ExposureTime",
        "ShutterSpeedValue",
        "FNumber",
        "ApertureValue",
        "PhotographicSensitivity",
        "ISOSpeedRatings",
        "FocalLength",
        "DateTimeOriginal",
    ] {
        types.insert(format!("image.exif.{key}"), Type::String);
    }
    types
}
pub fn to_value(value: &ParamValue) -> Option<Value> {
    Some(match value {
        ParamValue::Null => Value::Null,
        ParamValue::Bool(b) => Value::Bool(*b),
        ParamValue::Int(n) => Value::Int(*n),
        ParamValue::Number(n) => Value::Float(*n),
        ParamValue::String(s) => Value::String(s.clone()),
        ParamValue::Vector(v) => Value::Vector(v.clone()),
        ParamValue::Structured(_) => return None,
    })
}
pub fn default_context(params: &[ParamDefinition]) -> Context {
    params
        .iter()
        .filter_map(|p| {
            p.default
                .as_ref()
                .and_then(to_value)
                .or_else(|| (p.kind == ParamType::Value).then_some(Value::Null))
                .map(|v| (p.name.clone(), v))
        })
        .collect()
}

/// Resolve validated v2 values once; structured values remain executor data.
pub fn resolve_context(
    params: &[ParamDefinition],
    supplied: &bite_schema::Params,
) -> Result<Context, Error> {
    let mut context = default_context(params);
    for (name, value) in supplied {
        if name == "_enabled" {
            if !matches!(value, ParamValue::Bool(_)) {
                return Err(err(0, 0, "_enabled must be boolean"));
            }
            continue;
        }
        let p = params
            .iter()
            .find(|p| p.name == *name)
            .ok_or_else(|| err(0, 0, format!("unknown parameter {name:?}")))?;
        if !p.readonly && !p.accepts(value) {
            return Err(err(
                0,
                0,
                format!("param {name:?}: value does not match type/options"),
            ));
        }
        if let Some(v) = to_value(value) {
            context.insert(name.clone(), v);
        }
    }
    for p in params {
        if !p.readonly
            && !context.contains_key(&p.name)
            && !matches!(
                p.kind,
                ParamType::RenameBlocks | ParamType::SetSuffixes | ParamType::TextSlots
            )
        {
            return Err(err(
                0,
                0,
                format!("param {:?}: missing required value", p.name),
            ));
        }
    }
    Ok(context)
}

#[derive(Debug, Clone)]
pub enum CompiledImplementation {
    Imagemagick(Vec<CompiledArg>),
    Compute(BTreeMap<String, Expression>),
    Native(String),
}
#[derive(Debug, Clone)]
pub struct CompiledDefinition {
    pub definition: NodeDefinition,
    pub implementation: CompiledImplementation,
    pub visible: BTreeMap<String, Expression>,
    pub enabled: BTreeMap<String, Expression>,
    pub metadata: BTreeSet<String>,
}
impl CompiledDefinition {
    pub fn compile(definition: NodeDefinition) -> Result<Self, Error> {
        definition.validate().map_err(|e| err(0, 0, e.join("\n")))?;
        let mut types = parameter_types(&definition.params);
        types.extend(metadata_types());
        let implementation = match &definition.implementation {
            Implementation::Imagemagick { args } => {
                CompiledImplementation::Imagemagick(CompiledArg::compile_all(args, &types)?)
            }
            Implementation::Compute { outputs } => CompiledImplementation::Compute(
                outputs
                    .iter()
                    .map(|(name, expr)| Ok((name.clone(), Expression::compile(expr, &types)?)))
                    .collect::<Result<_, Error>>()?,
            ),
            Implementation::Native { executor } => CompiledImplementation::Native(executor.clone()),
        };
        let mut metadata = match &implementation {
            CompiledImplementation::Imagemagick(args) => {
                args.iter().flat_map(CompiledArg::metadata).collect()
            }
            CompiledImplementation::Compute(outputs) => {
                outputs.values().flat_map(|e| e.metadata.clone()).collect()
            }
            CompiledImplementation::Native(_) => BTreeSet::new(),
        };
        let mut visible = BTreeMap::new();
        let mut enabled = BTreeMap::new();
        for p in &definition.params {
            if let Some(s) = &p.visible_when {
                let e = Expression::compile(s, &types)?;
                metadata.extend(e.metadata.clone());
                visible.insert(p.name.clone(), e);
            }
            if let Some(s) = &p.enabled_when {
                let e = Expression::compile(s, &types)?;
                metadata.extend(e.metadata.clone());
                enabled.insert(p.name.clone(), e);
            }
        }
        Ok(Self {
            definition,
            implementation,
            visible,
            enabled,
            metadata,
        })
    }
}
