//! Shell-script export shared by native frontends.
use bite_schema::{BuiltinNodeKind, Graph, NodeKind, ParamValue};

#[derive(Clone, Copy)]
pub enum Shell {
    PowerShell,
    Bash,
    Cmd,
}

struct ParamSpec {
    flag: String,
    variable: String,
    ps_name: String,
    description: &'static str,
    default_value: String,
    required: bool,
}

fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.trim().to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            separator = false;
            result.push(character);
        } else {
            separator = true;
        }
    }
    result
}

fn string_param(node: &bite_schema::GraphNode, name: &str) -> String {
    match node.data.params.get(name) {
        Some(ParamValue::String(value)) => value.clone(),
        _ => String::new(),
    }
}

fn ps_name(flag: &str) -> String {
    flag.split('-')
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn specs(graph: &Graph) -> Vec<ParamSpec> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for node in &graph.nodes {
        let flag = slug(&string_param(node, "cliName"));
        if flag.is_empty() {
            continue;
        }
        let variable = flag.to_ascii_uppercase().replace('-', "_");
        let ps_name = ps_name(&flag);
        match node.kind {
            NodeKind::Builtin(BuiltinNodeKind::Input) => inputs.push(ParamSpec {
                flag,
                variable,
                ps_name,
                description: "Source image folder",
                default_value: ".".into(),
                required: true,
            }),
            NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
                let default_value = if string_param(node, "outputPath") == "custom" {
                    let custom = string_param(node, "customPath");
                    if custom.is_empty() {
                        "./output".into()
                    } else {
                        custom
                    }
                } else {
                    "./output".into()
                };
                outputs.push(ParamSpec {
                    flag,
                    variable,
                    ps_name,
                    description: "Output folder",
                    default_value,
                    required: false,
                });
            }
            NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
                let value = string_param(node, "outputPath");
                outputs.push(ParamSpec {
                    flag,
                    variable,
                    ps_name,
                    description: "Output file path",
                    default_value: if value.is_empty() {
                        "./output.txt".into()
                    } else {
                        value
                    },
                    required: false,
                });
            }
            NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
                let value = string_param(node, "flipbookOutputPath");
                outputs.push(ParamSpec {
                    flag,
                    variable,
                    ps_name,
                    description: "Output file path",
                    default_value: if value.is_empty() {
                        "./flipbook.png".into()
                    } else {
                        value
                    },
                    required: false,
                });
            }
            _ => {}
        }
    }
    inputs.extend(outputs);
    inputs
}

fn escape_cmd(value: &str) -> String {
    value.replace('%', "%%")
}

fn escape_ps(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_bash(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if matches!(character, '\\' | '$' | '`' | '"') {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

pub fn generate(shell: Shell, workflow_file: &str, generated: &str, graph: &Graph) -> String {
    let params = specs(graph);
    match shell {
        Shell::Cmd => {
            let mut lines = vec![
                "@echo off".into(),
                ":: Bite - Generated Batch Script".into(),
                format!(":: Generated: {generated}"),
                "::".into(),
                if params.is_empty() {
                    ":: Usage: script.bat".into()
                } else {
                    ":: Usage: script.bat [flags]".into()
                },
            ];
            if !params.is_empty() {
                lines.extend(["::".into(), ":: Flags (positional order):".into()]);
                for (index, parameter) in params.iter().enumerate() {
                    let requirement = if parameter.required {
                        "(required)".into()
                    } else {
                        format!("(default: {})", parameter.default_value)
                    };
                    lines.push(format!(
                        "::   %{}  --{}   {} {}",
                        index + 1,
                        parameter.flag,
                        parameter.description,
                        requirement
                    ));
                }
            }
            lines.extend([
                "::".into(),
                ":: Requires Bite to be installed. bite is added to PATH automatically.".into(),
                String::new(),
            ]);
            for (index, parameter) in params.iter().enumerate() {
                lines.push(format!("set \"{}=%~{}\"", parameter.variable, index + 1));
                lines.push(format!(
                    "if \"%{}%\"==\"\" set \"{}={}\"",
                    parameter.variable,
                    parameter.variable,
                    escape_cmd(&parameter.default_value)
                ));
            }
            if !params.is_empty() {
                lines.push(String::new());
            }
            let arguments = params
                .iter()
                .map(|parameter| format!("--{} \"%{}%\"", parameter.flag, parameter.variable))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!(
                "bite run \"%~dp0{}\"{}{}",
                escape_cmd(workflow_file),
                if arguments.is_empty() { "" } else { " " },
                arguments
            ));
            lines.join("\r\n") + "\r\n"
        }
        Shell::PowerShell => {
            let mut lines = vec![
                "# Bite - Generated PowerShell Script".into(),
                format!("# Generated: {generated}"),
                "#".into(),
                if params.is_empty() {
                    "# Usage: .\\script.ps1".into()
                } else {
                    "# Usage: .\\script.ps1 [-FlagName \"value\"] ...".into()
                },
            ];
            if !params.is_empty() {
                lines.push("#".into());
                for parameter in &params {
                    let requirement = if parameter.required {
                        "required".into()
                    } else {
                        format!("default: {}", parameter.default_value)
                    };
                    lines.push(format!(
                        "#   -{:<20} {} ({})",
                        parameter.ps_name, parameter.description, requirement
                    ));
                }
            }
            lines.extend([
                "#".into(),
                "# Requires Bite to be installed. bite is added to PATH automatically.".into(),
                String::new(),
            ]);
            if !params.is_empty() {
                lines.push("param (".into());
                for (index, parameter) in params.iter().enumerate() {
                    lines.push(format!(
                        "  [string]${} = '{}'{}",
                        parameter.ps_name,
                        escape_ps(&parameter.default_value),
                        if index + 1 == params.len() { "" } else { "," }
                    ));
                }
                lines.extend([")".into(), String::new()]);
            }
            lines.push(format!(
                "$WorkflowFile = Join-Path $PSScriptRoot '{}'",
                escape_ps(workflow_file)
            ));
            let arguments = params
                .iter()
                .map(|parameter| format!("--{} ${}", parameter.flag, parameter.ps_name))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!(
                "bite run $WorkflowFile{}{}",
                if arguments.is_empty() { "" } else { " " },
                arguments
            ));
            lines.join("\n") + "\n"
        }
        Shell::Bash => {
            let mut lines = vec![
                "#!/usr/bin/env bash".into(),
                "# Bite - Generated Shell Script".into(),
                format!("# Generated: {generated}"),
                "#".into(),
                if params.is_empty() {
                    "# Usage: bash script.sh".into()
                } else {
                    "# Usage: bash script.sh [positional values]".into()
                },
            ];
            if !params.is_empty() {
                lines.push("#".into());
                for (index, parameter) in params.iter().enumerate() {
                    let requirement = if parameter.required {
                        "required".into()
                    } else {
                        format!("default: {}", parameter.default_value)
                    };
                    lines.push(format!(
                        "#   ${}  --{}   {} ({})",
                        index + 1,
                        parameter.flag,
                        parameter.description,
                        requirement
                    ));
                }
            }
            lines.extend([
                "#".into(),
                "# Requires Bite to be installed. bite must be available in PATH.".into(),
                String::new(),
                "SCRIPT_DIR=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")\" && pwd)\"".into(),
            ]);
            for (index, parameter) in params.iter().enumerate() {
                lines.push(format!(
                    "{}=\"${{{}:-{}}}\"",
                    parameter.variable,
                    index + 1,
                    escape_bash(&parameter.default_value)
                ));
            }
            if !params.is_empty() {
                lines.push(String::new());
            }
            let arguments = params
                .iter()
                .map(|parameter| format!("--{} \"${{{}}}\"", parameter.flag, parameter.variable))
                .collect::<Vec<_>>()
                .join(" \\\n  ");
            if arguments.is_empty() {
                lines.push(format!(
                    "bite run \"${{SCRIPT_DIR}}/{}\"",
                    escape_bash(workflow_file)
                ));
            } else {
                lines.push(format!(
                    "bite run \"${{SCRIPT_DIR}}/{}\" \\",
                    escape_bash(workflow_file)
                ));
                lines.push(format!("  {arguments}"));
            }
            lines.join("\n") + "\n"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{GraphNode, NodeData, Position};
    use std::collections::BTreeMap;

    fn graph() -> Graph {
        let node = |id: &str, kind, params| GraphNode {
            id: id.into(),
            kind,
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: id.into(),
                definition_id: String::new(),
                params,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        };
        Graph {
            nodes: vec![
                node(
                    "input",
                    NodeKind::Builtin(BuiltinNodeKind::Input),
                    BTreeMap::from([("cliName".into(), ParamValue::String("Input 1".into()))]),
                ),
                node(
                    "output",
                    NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
                    BTreeMap::from([
                        ("cliName".into(), ParamValue::String("Output Image".into())),
                        ("outputPath".into(), ParamValue::String("custom".into())),
                        (
                            "customPath".into(),
                            ParamValue::String("C:\\A%20\\it's".into()),
                        ),
                    ]),
                ),
            ],
            edges: Vec::new(),
            viewport: bite_schema::Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }

    #[test]
    fn exports_all_shells_with_safe_names_and_values() {
        let graph = graph();
        let ps = generate(Shell::PowerShell, "flow's.bite", "today", &graph);
        assert!(ps.contains("-Input1"));
        assert!(ps.contains("C:\\A%20\\it''s"));
        assert!(ps.contains("flow''s.bite"));
        let cmd = generate(Shell::Cmd, "flow.bite", "today", &graph);
        assert!(cmd.contains("--output-image \"%OUTPUT_IMAGE%\""));
        assert!(cmd.contains("C:\\A%%20\\it's"));
        let bash = generate(Shell::Bash, "flow.bite", "today", &graph);
        assert!(bash.contains("--input-1 \"${INPUT_1}\""));
        assert!(bash.contains("${SCRIPT_DIR}/flow.bite"));
    }
}
