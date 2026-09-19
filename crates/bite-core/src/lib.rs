//! Shared workflow services. Frontends use these APIs rather than implementing
//! their own graph traversal, parameter resolution, or migration.
pub mod execution;
pub mod graph;
pub mod plan;
pub mod preview;
pub mod resolve;
pub mod workflow;
use bite_expr::{definition::CompiledDefinition, CompiledArg};
use bite_schema::{FormatDefinition, NodeDefinition};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub nodes: BTreeMap<String, CompiledDefinition>,
    pub formats: BTreeMap<String, (FormatDefinition, Vec<CompiledArg>)>,
}
impl Registry {
    /// Build a fresh registry then swap it at a frontend reload boundary. Failed
    /// loads never partially mutate the registry currently used by execution.
    pub fn load(nodes: &Path, formats: &Path) -> Result<Self, String> {
        let mut registry = Self::default();
        for entry in fs::read_dir(nodes).map_err(|e| format!("{}: {e}", nodes.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let def: NodeDefinition =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let compiled =
                CompiledDefinition::compile(def).map_err(|e| format!("{}: {e}", path.display()))?;
            let id = compiled.definition.id.clone();
            if registry.nodes.insert(id.clone(), compiled).is_some() {
                return Err(format!("duplicate node definition {id}"));
            }
        }
        for entry in fs::read_dir(formats).map_err(|e| format!("{}: {e}", formats.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let def: FormatDefinition =
                serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            def.validate().map_err(|e| e.join("\n"))?;
            let args = CompiledArg::compile_all(
                &def.args,
                &bite_expr::definition::parameter_types(&def.params),
            )
            .map_err(|e| e.to_string())?;
            let id = def.id.to_uppercase();
            if registry.formats.insert(id.clone(), (def, args)).is_some() {
                return Err(format!("duplicate format {id}"));
            }
        }
        Ok(registry)
    }
}
