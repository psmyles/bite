//! Node card measurement, transcribed from `src/renderer/nodeEditor/ProcessNode.svelte`.
//!
//! The Electron node is laid out by the browser from the `--node-layout-*` tokens. The
//! native canvas has to compute the same rectangle, so the arithmetic here follows that
//! component line for line: a header, an optional port section, an optional body parameter
//! section, an optional output slot section, and an optional footer.
use crate::theme;
use bite_core::{graph::WireType, Registry};
use bite_schema::{
    BuiltinNodeKind, GraphNode, NodeKind, ParamDefinition, ParamType, ParamValue, PortDefinition,
    PortType, ProcessingNodeKind, StructuredParam,
};

/// One connection point on a card, with the position its wire attaches to.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// The handle name used by the graph, such as `in:input` or `param:width`.
    pub handle: String,
    pub label: String,
    pub wire: WireType,
    /// True for a port on the right edge.
    pub output: bool,
    /// Distance from the top of the card to the centre of the handle.
    pub offset_y: f32,
}

/// A row drawn inside the card body.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A port label in the section that mirrors `.node-ports`.
    PortLabel {
        left: Option<String>,
        left_wire: Option<WireType>,
        right: Option<String>,
        right_wire: Option<WireType>,
        top: f32,
    },
    /// A writable parameter, mirroring `.param-port-row`.
    Param {
        name: String,
        label: String,
        value: Option<String>,
        wire: WireType,
        swatch: Option<[f32; 4]>,
        top: f32,
    },
    /// A computed parameter, whose value is drawn on the left and label on the right.
    ReadonlyParam {
        name: String,
        label: String,
        value: Option<String>,
        wire: WireType,
        top: f32,
    },
    /// A `portOnly` parameter, mirroring `.output-slot-row`.
    Slot {
        name: String,
        label: String,
        value: Option<String>,
        wire: WireType,
        top: f32,
    },
}

/// The measured card: its size, where each port attaches and what each row contains.
#[derive(Clone, Debug)]
pub struct Card {
    pub width: f32,
    pub height: f32,
    pub header_height: f32,
    pub ports: Vec<Port>,
    pub rows: Vec<Row>,
    pub footer: Option<String>,
    /// True when the card shows the bypass tick, which also insets the header text.
    pub has_bypass_toggle: bool,
    /// Separator lines, as distances from the top of the card.
    pub separators: Vec<f32>,
}

impl Card {
    /// The port whose handle matches, if the card has one.
    pub fn port(&self, handle: &str) -> Option<&Port> {
        self.ports.iter().find(|port| port.handle == handle)
    }
}

/// Formats an inline value exactly as `formatParamValue` does.
pub fn format_param_value(value: &ParamValue) -> Option<String> {
    fn trim_float(value: f64) -> String {
        let text = format!("{value:.2}");
        let trimmed = text.trim_end_matches('0').trim_end_matches('.');
        if trimmed.is_empty() || trimmed == "-" {
            "0".into()
        } else {
            trimmed.into()
        }
    }
    match value {
        ParamValue::Null => None,
        ParamValue::Int(value) => Some(value.to_string()),
        ParamValue::Number(value) => {
            if value.is_nan() {
                None
            } else {
                Some(trim_float(*value))
            }
        }
        ParamValue::String(text) if text.is_empty() => None,
        ParamValue::String(text) => Some(if text.chars().count() > 14 {
            let truncated: String = text.chars().take(12).collect();
            format!("{truncated}...")
        } else {
            text.clone()
        }),
        ParamValue::Bool(value) => Some(if *value {
            "true".into()
        } else {
            "false".into()
        }),
        ParamValue::Vector(values) => Some(
            values
                .iter()
                .map(|value| trim_float(*value))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        ParamValue::Structured(_) => None,
    }
}

/// True when the stored value differs from the definition default, which is when the card
/// shows an inline value.
pub fn is_changed(definition: &ParamDefinition, value: Option<&ParamValue>) -> bool {
    if definition.readonly {
        return false;
    }
    match (&definition.default, value) {
        (None, current) => !matches!(current, None | Some(ParamValue::Null)),
        (Some(default), Some(current)) => default != current,
        (Some(_), None) => true,
    }
}

/// The wire color for a parameter type, matching `paramTypeToWireType`.
pub fn param_wire(definition: &ParamDefinition) -> WireType {
    use bite_schema::ParamType;
    match definition.kind {
        ParamType::Int | ParamType::Float => WireType::Number,
        ParamType::Enum | ParamType::String => WireType::String,
        ParamType::Bool => WireType::Bool,
        ParamType::Numeric => WireType::Numeric,
        ParamType::Vector2 => WireType::Vector2,
        ParamType::Vector3 => WireType::Vector3,
        ParamType::Vector4 => WireType::Vector4,
        ParamType::Color => WireType::Color,
        ParamType::Value => WireType::Value,
        // Structured parameters cannot be wired; they are drawn in the number color.
        _ => WireType::Number,
    }
}

fn port_wire(kind: &PortType) -> WireType {
    match kind {
        PortType::Image => WireType::Image,
        PortType::Mask => WireType::Mask,
        PortType::Number => WireType::Number,
        PortType::Path => WireType::Path,
    }
}

/// Which components a combined output slot slices from its source array.
fn combined_slice(name: &str) -> Option<usize> {
    match name {
        "xy" => Some(2),
        "xyz" | "rgb" => Some(3),
        "xyzw" | "rgba" => Some(4),
        _ => None,
    }
}

fn component_index(name: &str) -> Option<usize> {
    match name {
        "x" | "r" => Some(0),
        "y" | "g" => Some(1),
        "z" | "b" => Some(2),
        "w" | "a" => Some(3),
        _ => None,
    }
}

/// Inputs to card measurement that come from outside the graph.
pub struct CardContext<'a> {
    pub registry: &'a Registry,
    /// Live values from the preview run, which take priority over stored parameters.
    pub resolved: Option<&'a std::collections::BTreeMap<String, ParamValue>>,
    /// Parameter visibility, already evaluated for this node.
    pub visible: &'a dyn Fn(&str) -> bool,
    /// The footer text, which each built-in card computes for itself.
    pub footer: Option<String>,
    /// True when an incoming wire drives the bypass port, which hides the manual tick.
    pub enabled_wired: bool,
    /// Measures a string in a face, so the card can grow to fit its own content the way
    /// an absolutely positioned element in the browser shrink-wraps to it.
    pub measure_text: &'a dyn Fn(bite_imgui::Face, &str) -> f32,
}

/// Measures a card for `node`.
pub fn measure(node: &GraphNode, context: &CardContext) -> Card {
    let mut ports = Vec::new();
    let mut rows = Vec::new();
    let mut separators = Vec::new();

    let (input_ports, output_ports) = section_ports(node, context.registry);
    let has_image_ports = !input_ports.is_empty() || !output_ports.is_empty();

    let definition = context.registry.nodes.get(&node.data.definition_id);
    let params: Vec<&ParamDefinition> = definition
        .map(|entry| entry.definition.params.iter().collect())
        .unwrap_or_default();
    // An enum parameter is edited in the inspector only. `nodeEditorHelpers.ts` filters
    // `type !== 'enum'` when it builds `paramDefs`, so an enum has neither a row nor a port.
    let carded =
        |param: &&ParamDefinition| param.kind != ParamType::Enum && (context.visible)(&param.name);
    let body_params: Vec<&&ParamDefinition> = params
        .iter()
        .filter(|param| !param.port_only && carded(param))
        .collect();
    let slot_params: Vec<&&ParamDefinition> = params
        .iter()
        .filter(|param| param.port_only && carded(param))
        .collect();

    let header = theme::NODE_LAYOUT_HEADER_H;
    let port_pad = theme::NODE_LAYOUT_PORT_PAD;
    let port_row = theme::NODE_LAYOUT_PORT_ROW_H;
    let param_pad = theme::NODE_LAYOUT_PARAM_PAD;
    let param_row = theme::NODE_LAYOUT_PARAM_ROW_H;
    let sep = theme::NODE_LAYOUT_SEP_H;

    let image_section = if has_image_ports {
        port_pad * 2.0 + (input_ports.len().max(output_ports.len()).max(1)) as f32 * port_row
    } else {
        0.0
    };
    let body_section = if body_params.is_empty() {
        0.0
    } else {
        (if has_image_ports { sep } else { 0.0 })
            + param_pad * 2.0
            + body_params.len() as f32 * param_row
    };

    // The port section: paired labels on the left and right of each row.
    if has_image_ports {
        let count = input_ports.len().max(output_ports.len()).max(1);
        for index in 0..count {
            let top = header + port_pad + index as f32 * port_row;
            let left = input_ports.get(index);
            let right = output_ports.get(index);
            if let Some((handle, label, wire)) = left {
                ports.push(Port {
                    handle: handle.clone(),
                    label: label.clone(),
                    wire: *wire,
                    output: false,
                    offset_y: top + port_row / 2.0,
                });
            }
            if let Some((handle, label, wire)) = right {
                ports.push(Port {
                    handle: handle.clone(),
                    label: label.clone(),
                    wire: *wire,
                    output: true,
                    offset_y: top + port_row / 2.0,
                });
            }
            rows.push(Row::PortLabel {
                left: left.map(|(_, label, _)| label.clone()),
                left_wire: left.map(|(_, _, wire)| *wire),
                right: right.map(|(_, label, _)| label.clone()),
                right_wire: right.map(|(_, _, wire)| *wire),
                top,
            });
        }
    }

    // Body parameters, each with a handle on the matching edge.
    if !body_params.is_empty() {
        let sep_offset = if has_image_ports { sep } else { 0.0 };
        if has_image_ports {
            separators.push(header + image_section);
        }
        for (index, param) in body_params.iter().enumerate() {
            let top = header + image_section + sep_offset + param_pad + index as f32 * param_row;
            let wire = param_wire(param);
            let stored = context
                .resolved
                .and_then(|values| values.get(&param.name))
                .or_else(|| node.data.params.get(&param.name));
            // A computed value always shows; a writable one shows once it differs
            // from the definition default.
            let value = if param.readonly || is_changed(param, stored) {
                stored.and_then(format_param_value)
            } else {
                None
            };
            let swatch = match stored {
                Some(ParamValue::Vector(values)) if values.len() >= 3 => Some([
                    values[0] as f32,
                    values[1] as f32,
                    values[2] as f32,
                    values.get(3).copied().unwrap_or(1.0) as f32,
                ]),
                _ => None,
            };
            let offset_y = top + param_row / 2.0;
            if param.readonly {
                ports.push(Port {
                    handle: format!("param:{}", param.name),
                    label: param.label.clone(),
                    wire,
                    output: true,
                    offset_y,
                });
                rows.push(Row::ReadonlyParam {
                    name: param.name.clone(),
                    label: param.label.clone(),
                    value,
                    wire,
                    top,
                });
            } else {
                if !param.no_port {
                    ports.push(Port {
                        handle: format!("param:{}", param.name),
                        label: param.label.clone(),
                        wire,
                        output: false,
                        offset_y,
                    });
                }
                rows.push(Row::Param {
                    name: param.name.clone(),
                    label: param.label.clone(),
                    value,
                    wire,
                    swatch,
                    top,
                });
            }
        }
    }

    // Output slots, drawn right-aligned below everything else.
    if !slot_params.is_empty() {
        let sep_offset = if has_image_ports || !body_params.is_empty() {
            separators.push(header + image_section + body_section);
            sep
        } else {
            0.0
        };
        for (index, param) in slot_params.iter().enumerate() {
            let top = header
                + image_section
                + body_section
                + sep_offset
                + port_pad
                + index as f32 * port_row;
            let wire = param_wire(param);
            ports.push(Port {
                handle: format!("param:{}", param.name),
                label: param.label.clone(),
                wire,
                output: true,
                offset_y: top + port_row / 2.0,
            });
            rows.push(Row::Slot {
                name: param.name.clone(),
                label: param.label.clone(),
                value: slot_value(node, param, &body_params, context),
                wire,
                top,
            });
        }
    }

    let slot_section = if slot_params.is_empty() {
        0.0
    } else {
        (if has_image_ports || !body_params.is_empty() {
            sep
        } else {
            0.0
        }) + port_pad * 2.0
            + slot_params.len() as f32 * port_row
    };

    let footer_height = if context.footer.is_some() {
        separators.push(header + image_section + body_section + slot_section);
        theme::NODE_FOOTER_H
    } else {
        0.0
    };

    // A card with nothing but a header still shows one empty port row, as the browser does.
    let body = image_section + body_section + slot_section + footer_height;
    let height = header + if body > 0.0 { body } else { 0.0 };

    let is_actionable = !input_ports.is_empty() && !output_ports.is_empty();
    let has_bypass_toggle = is_actionable
        && !context.enabled_wired
        && matches!(node.kind, NodeKind::Processing(ProcessingNodeKind::Process));
    // Any node that can be bypassed carries the port; only an unwired one also draws
    // the manual tick.
    if is_actionable {
        ports.push(Port {
            handle: "param:_enabled".into(),
            label: "Enabled".into(),
            wire: WireType::Bool,
            output: false,
            offset_y: header / 2.0,
        });
    }

    let width = content_width(node, &rows, context, has_bypass_toggle);

    Card {
        width,
        height,
        header_height: header,
        ports,
        rows,
        footer: context.footer.clone(),
        has_bypass_toggle,
        separators,
    }
}

/// The card's drawn width.
///
/// In the browser a node is an absolutely positioned element, so it shrink-wraps to its
/// content and `--node-min-width` is only a floor. A fixed width instead truncates every
/// label that does not happen to fit.
fn content_width(
    node: &GraphNode,
    rows: &[Row],
    context: &CardContext,
    has_bypass_toggle: bool,
) -> f32 {
    let measure = context.measure_text;
    let mut width: f32 = card_width(node);
    if matches!(
        node.kind,
        NodeKind::Builtin(BuiltinNodeKind::Comment | BuiltinNodeKind::Group)
    ) {
        return width;
    }

    // `.node-head` pads twelve pixels a side, or thirty-four when it carries the tick.
    let header_inset = if has_bypass_toggle {
        theme::NODE_HEAD_TOGGLE_INSET
    } else {
        theme::NODE_HEAD_INSET
    };
    let label = super::view::card_label(node, context.registry);
    width = width.max(measure(theme::face::NODE_HEAD, &label) + header_inset * 2.0);

    let inset = theme::NODE_ROW_INSET * 2.0;
    for row in rows {
        let row_width = match row {
            Row::PortLabel { left, right, .. } => {
                let left = left
                    .as_deref()
                    .map(|text| measure(theme::face::PORT_TAG, text))
                    .unwrap_or(0.0);
                let right = right
                    .as_deref()
                    .map(|text| measure(theme::face::PORT_TAG, text))
                    .unwrap_or(0.0);
                left + right + theme::NODE_ROW_MIN_GAP
            }
            Row::Param {
                label,
                value,
                swatch,
                ..
            } => {
                let name = measure(theme::face::PORT_TAG, label);
                let value = value
                    .as_deref()
                    .map(|text| measure(theme::face::PORT_TAG, text) + theme::NODE_VALUE_GAP)
                    .unwrap_or(0.0);
                let swatch = if swatch.is_some() {
                    theme::NODE_SWATCH_SIZE + theme::NODE_VALUE_GAP
                } else {
                    0.0
                };
                swatch + name + value + theme::NODE_ROW_MIN_GAP
            }
            Row::ReadonlyParam { label, value, .. } => {
                // A computed row puts its value on the left and its label on the right.
                let label = measure(theme::face::PORT_TAG, label);
                let value = value
                    .as_deref()
                    .map(|text| measure(theme::face::PORT_TAG, text))
                    .unwrap_or(0.0);
                label + value + theme::NODE_ROW_MIN_GAP
            }
            Row::Slot { label, value, .. } => {
                let label = measure(theme::face::PORT_TAG, label);
                let value = value
                    .as_deref()
                    .map(|text| measure(theme::face::PORT_TAG, text) + theme::NODE_VALUE_GAP)
                    .unwrap_or(0.0);
                label + value
            }
        };
        width = width.max(row_width + inset);
    }

    if let Some(footer) = &context.footer {
        width = width.max(measure(theme::face::SMALL_MONO, footer) + inset);
    }
    width.min(theme::NODE_MAX_WIDTH).ceil()
}

/// Derives an output slot's value from the first array-valued body parameter.
fn slot_value(
    node: &GraphNode,
    param: &ParamDefinition,
    body_params: &[&&ParamDefinition],
    context: &CardContext,
) -> Option<String> {
    let lookup = |name: &str| {
        context
            .resolved
            .and_then(|values| values.get(name))
            .or_else(|| node.data.params.get(name))
    };
    if let Some(stored) = lookup(&param.name) {
        if !matches!(stored, ParamValue::Null) {
            return format_param_value(stored);
        }
    }
    let source = body_params
        .iter()
        .find_map(|body| match lookup(&body.name) {
            Some(ParamValue::Vector(values)) => Some(values.clone()),
            _ => None,
        })?;
    if let Some(length) = combined_slice(&param.name) {
        // Combined slots show only their label, never a value.
        let _ = length;
        return None;
    }
    let index = component_index(&param.name)?;
    format_param_value(&ParamValue::Number(
        source.get(index).copied().unwrap_or(0.0),
    ))
}

/// The card width for a node kind. Process and compare cards are narrower.
pub fn card_width(node: &GraphNode) -> f32 {
    match node.kind {
        NodeKind::Processing(ProcessingNodeKind::Process | ProcessingNodeKind::Compare) => {
            theme::NODE_MIN_WIDTH
        }
        NodeKind::Builtin(BuiltinNodeKind::Comment) => theme::NODE_COMMENT_MIN_WIDTH,
        _ => theme::NODE_WORKFLOW_WIDTH,
    }
}

/// How many image inputs a `channels` parameter leaves visible, if the definition has one.
///
/// `nodeEditorHelpers.ts` slices the input list to this count, falling back to the
/// definition's default when the node has not overridden it.
fn channel_limit(node: &GraphNode, definition: &bite_schema::NodeDefinition) -> Option<usize> {
    let parameter = definition
        .params
        .iter()
        .find(|param| param.name == "channels")?;
    let count = match node.data.params.get("channels") {
        Some(ParamValue::Int(value)) => Some(*value as f64),
        Some(ParamValue::Number(value)) => Some(*value),
        Some(ParamValue::String(text)) => text.parse().ok(),
        _ => None,
    }
    .or_else(|| match parameter.default.as_ref()? {
        ParamValue::Int(value) => Some(*value as f64),
        ParamValue::Number(value) => Some(*value),
        ParamValue::String(text) => text.parse().ok(),
        _ => None,
    })?;
    (count.is_finite() && count >= 0.0).then_some(count as usize)
}

type SectionPort = (String, String, WireType);

/// The ports drawn in the upper section, before parameters.
pub fn section_ports(
    node: &GraphNode,
    registry: &Registry,
) -> (Vec<SectionPort>, Vec<SectionPort>) {
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => (
            Vec::new(),
            vec![("out:output".into(), "Image".into(), WireType::Image)],
        ),
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => (
            vec![
                ("in:input".into(), "Image".into(), WireType::Image),
                ("in:folder".into(), "Folder".into(), WireType::Path),
            ],
            Vec::new(),
        ),
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            let mut inputs = vec![("in:input".into(), "Image".into(), WireType::Image)];
            if let Some(ParamValue::Structured(StructuredParam::TextSlots { slots })) =
                node.data.params.get("portIds")
            {
                inputs.extend(slots.iter().enumerate().map(|(index, slot)| {
                    (
                        format!("txo:{slot}"),
                        format!("Text {}", index + 1),
                        WireType::Value,
                    )
                }));
            }
            (inputs, Vec::new())
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => (
            vec![
                ("in:input".into(), "Image".into(), WireType::Image),
                ("param:bgColor".into(), "BG Color".into(), WireType::Color),
            ],
            Vec::new(),
        ),
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => (
            Vec::new(),
            vec![("out:output".into(), "Path".into(), WireType::Path)],
        ),
        NodeKind::Builtin(BuiltinNodeKind::Group | BuiltinNodeKind::Comment) => {
            (Vec::new(), Vec::new())
        }
        _ if node.data.definition_id == "process_as_set" => {
            let suffixes = match node.data.params.get("suffixes") {
                Some(ParamValue::Structured(StructuredParam::SetSuffixes { suffixes })) => {
                    suffixes.clone()
                }
                _ => Vec::new(),
            };
            let mut inputs = vec![
                ("in:input".into(), "Image".into(), WireType::Image),
                ("param:prefix".into(), "Prefix".into(), WireType::String),
            ];
            inputs.extend(suffixes.iter().enumerate().map(|(index, suffix)| {
                (
                    format!("param:suffix_{index}"),
                    label_or_placeholder(suffix, index),
                    WireType::String,
                )
            }));
            let outputs = suffixes
                .iter()
                .enumerate()
                .map(|(index, suffix)| {
                    (
                        format!("out:suffix_{index}"),
                        label_or_placeholder(suffix, index),
                        WireType::Image,
                    )
                })
                .collect();
            (inputs, outputs)
        }
        _ => registry
            .nodes
            .get(&node.data.definition_id)
            .map(|entry| {
                let inputs = if node.data.inputs.is_empty() {
                    &entry.definition.inputs
                } else {
                    &node.data.inputs
                };
                // A definition with a `channels` parameter shows only as many image inputs
                // as it names, so Merge Channels at 3 has no alpha port.
                let inputs = match channel_limit(node, &entry.definition) {
                    Some(limit) if limit < inputs.len() => &inputs[..limit],
                    _ => inputs.as_slice(),
                };
                let outputs = if node.data.outputs.is_empty() {
                    &entry.definition.outputs
                } else {
                    &node.data.outputs
                };
                let map = |ports: &[PortDefinition], prefix: &str| -> Vec<SectionPort> {
                    ports
                        .iter()
                        .map(|port| {
                            (
                                format!("{prefix}:{}", port.name),
                                port.label.clone(),
                                port_wire(&port.kind),
                            )
                        })
                        .collect()
                };
                (map(inputs, "in"), map(outputs, "out"))
            })
            .unwrap_or_default(),
    }
}

/// A suffix row's label, falling back to the placeholder the Svelte card shows.
fn label_or_placeholder(suffix: &str, index: usize) -> String {
    if suffix.is_empty() {
        format!("suffix{}", index + 1)
    } else {
        suffix.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_values_follow_the_card_formatting_rules() {
        assert_eq!(
            format_param_value(&ParamValue::Int(7)),
            Some("7".to_string())
        );
        assert_eq!(
            format_param_value(&ParamValue::Number(1.5)),
            Some("1.5".to_string())
        );
        assert_eq!(
            format_param_value(&ParamValue::Number(2.0)),
            Some("2".to_string())
        );
        assert_eq!(
            format_param_value(&ParamValue::Bool(false)),
            Some("false".to_string())
        );
        assert_eq!(format_param_value(&ParamValue::String(String::new())), None);
        assert_eq!(
            format_param_value(&ParamValue::Vector(vec![1.0, 2.25])),
            Some("1, 2.25".to_string())
        );
    }

    #[test]
    fn long_strings_are_cut_to_twelve_characters_plus_an_ellipsis() {
        let value = ParamValue::String("abcdefghijklmnopqrstuvwxyz".into());
        assert_eq!(format_param_value(&value), Some("abcdefghijkl...".into()));
        let exact = ParamValue::String("abcdefghijklmn".into());
        assert_eq!(format_param_value(&exact), Some("abcdefghijklmn".into()));
    }

    #[test]
    fn changed_detection_matches_the_card_rule() {
        let definition = ParamDefinition {
            name: "width".into(),
            label: "Width".into(),
            kind: bite_schema::ParamType::Int,
            widget: None,
            default: Some(ParamValue::Int(100)),
            min: None,
            max: None,
            step: None,
            options: Vec::new(),
            labels: Vec::new(),
            readonly: false,
            port_only: false,
            no_port: false,
            visible_when: None,
            enabled_when: None,
        };
        assert!(!is_changed(&definition, Some(&ParamValue::Int(100))));
        assert!(is_changed(&definition, Some(&ParamValue::Int(200))));
        assert!(is_changed(&definition, None));
    }

    /// A node carrying one parameter, for the card rules that depend on it.
    fn node_with(definition_id: &str, params: Vec<(&str, ParamValue)>) -> GraphNode {
        GraphNode {
            id: "n".into(),
            kind: NodeKind::Processing(ProcessingNodeKind::Process),
            position: bite_schema::Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: bite_schema::NodeData {
                label: String::new(),
                definition_id: definition_id.into(),
                params: params
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value))
                    .collect(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        }
    }

    fn bare_definition() -> bite_schema::NodeDefinition {
        bite_schema::NodeDefinition {
            schema_version: 2,
            id: "test".into(),
            version: "1.0.0".into(),
            label: "Test".into(),
            category: "Test".into(),
            description: String::new(),
            aliases: Vec::new(),
            icon: String::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Vec::new(),
            implementation: bite_schema::Implementation::Native {
                executor: "noop".into(),
            },
        }
    }

    fn channels_definition(default: &str) -> bite_schema::NodeDefinition {
        let mut definition = bare_definition();
        definition.params.push(ParamDefinition {
            name: "channels".into(),
            label: "Channels".into(),
            kind: ParamType::Enum,
            widget: None,
            default: Some(ParamValue::String(default.into())),
            min: None,
            max: None,
            step: None,
            options: vec!["3".into(), "4".into()],
            labels: Vec::new(),
            readonly: false,
            port_only: false,
            no_port: false,
            visible_when: None,
            enabled_when: None,
        });
        definition
    }

    #[test]
    fn a_channel_count_trims_the_image_inputs_to_match() {
        let definition = channels_definition("3");
        // Merge Channels at three shows red, green and blue but no alpha.
        assert_eq!(
            channel_limit(&node_with("channel_merge", Vec::new()), &definition),
            Some(3)
        );
    }

    #[test]
    fn a_channel_count_set_on_the_node_beats_the_definition_default() {
        let definition = channels_definition("3");
        let node = node_with(
            "channel_merge",
            vec![("channels", ParamValue::String("4".into()))],
        );
        assert_eq!(channel_limit(&node, &definition), Some(4));
        let numeric = node_with("channel_merge", vec![("channels", ParamValue::Int(4))]);
        assert_eq!(channel_limit(&numeric, &definition), Some(4));
    }

    /// A card context whose font makes every character two units wide.
    fn width_context<'a>(
        registry: &'a Registry,
        visible: &'a dyn Fn(&str) -> bool,
        measure: &'a dyn Fn(bite_imgui::Face, &str) -> f32,
        footer: Option<String>,
    ) -> CardContext<'a> {
        CardContext {
            registry,
            resolved: None,
            visible,
            footer,
            enabled_wired: false,
            measure_text: measure,
        }
    }

    #[test]
    fn a_card_grows_to_fit_a_header_the_minimum_width_would_cut() {
        let registry = Registry::default();
        let visible = |_: &str| true;
        let measure = |_: bite_imgui::Face, text: &str| text.chars().count() as f32 * 2.0;
        let mut node = node_with("long", Vec::new());
        node.data.label = "a".repeat(120);
        let context = width_context(&registry, &visible, &measure, None);
        let width = content_width(&node, &[], &context, false);
        // 120 characters at two units, plus twelve pixels of padding a side.
        assert_eq!(width, 240.0 + theme::NODE_HEAD_INSET * 2.0);
    }

    #[test]
    fn a_short_header_leaves_the_card_at_its_minimum_width() {
        let registry = Registry::default();
        let visible = |_: &str| true;
        let measure = |_: bite_imgui::Face, text: &str| text.chars().count() as f32 * 2.0;
        let mut node = node_with("short", Vec::new());
        node.data.label = "Blur".into();
        let context = width_context(&registry, &visible, &measure, None);
        assert_eq!(
            content_width(&node, &[], &context, false),
            theme::NODE_MIN_WIDTH
        );
    }

    #[test]
    fn the_bypass_tick_widens_the_header_it_sits_in() {
        let registry = Registry::default();
        let visible = |_: &str| true;
        let measure = |_: bite_imgui::Face, text: &str| text.chars().count() as f32 * 2.0;
        let mut node = node_with("tick", Vec::new());
        node.data.label = "a".repeat(80);
        let context = width_context(&registry, &visible, &measure, None);
        let plain = content_width(&node, &[], &context, false);
        let ticked = content_width(&node, &[], &context, true);
        assert_eq!(
            ticked - plain,
            (theme::NODE_HEAD_TOGGLE_INSET - theme::NODE_HEAD_INSET) * 2.0
        );
    }

    #[test]
    fn a_card_never_grows_past_the_ceiling() {
        let registry = Registry::default();
        let visible = |_: &str| true;
        let measure = |_: bite_imgui::Face, text: &str| text.chars().count() as f32 * 2.0;
        let mut node = node_with("huge", Vec::new());
        node.data.label = "a".repeat(4000);
        let context = width_context(&registry, &visible, &measure, None);
        assert_eq!(
            content_width(&node, &[], &context, false),
            theme::NODE_MAX_WIDTH
        );
    }

    #[test]
    fn a_definition_without_a_channel_count_keeps_every_input() {
        let definition = bare_definition();
        assert_eq!(
            channel_limit(&node_with("blur", Vec::new()), &definition),
            None
        );
    }
}
