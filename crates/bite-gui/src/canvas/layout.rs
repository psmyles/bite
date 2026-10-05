//! Node card measurement, transcribed from `src/renderer/nodeEditor/ProcessNode.svelte`.
//!
//! The Electron node is laid out by the browser from the `--node-layout-*` tokens. The
//! native canvas has to compute the same rectangle, so the arithmetic here follows that
//! component line for line: a header, an optional port section, an optional body parameter
//! section, an optional output slot section, and an optional footer.
use crate::theme;
use bite_core::{graph::WireType, Registry};
use bite_imgui::{Face, Weight};
use bite_schema::{
    BuiltinNodeKind, Graph, GraphNode, NodeKind, ParamDefinition, ParamType, ParamValue,
    PortDefinition, PortType, ProcessingNodeKind, StructuredParam,
};

/// `.op-badge` on the Compare header: the mono family at the base size, in bold.
pub const OPERATOR_FACE: Face = Face::mono_weight(theme::FONT_SIZE_BASE, Weight::Bold);
/// The gap `CompareNode.svelte` leaves between the title and its operator badge.
pub const OPERATOR_GAP: f32 = 6.0;
/// `.footer-label` stops at 165 pixels and ellipsizes the rest, so a long output path
/// never widens the card.
pub const FOOTER_MAX_WIDTH: f32 = 165.0;
/// What Folder Path shows, dimmed, before a folder is chosen.
pub const FOLDER_UNSET: &str = "not set";
/// Every row of `SetInputNode.svelte` is 26 pixels tall, border included.
const SET_ROW_H: f32 = 26.0;
/// `.node-empty` pads eight pixels above and below its one line.
const SET_EMPTY_PAD_Y: f32 = 8.0;

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
        /// The row's height, which the labels centre in: twenty pixels in `.node-ports`,
        /// twenty-six in the Process As Set card.
        height: f32,
    },
    /// Folder Path's single body row: the `path` tag with the folder name beside it.
    Folder {
        label: String,
        /// The folder's last path component, or none for the dimmed "not set".
        name: Option<String>,
        top: f32,
    },
    /// A muted line standing in for rows that are absent, such as `.node-empty`.
    Note { text: String, top: f32, height: f32 },
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
    /// A symbol drawn after the header title, which is how the Compare card shows its
    /// operator.
    pub badge: Option<String>,
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

/// `parseFloat(n.toFixed(places)).toString()`: rounded, with trailing zeros dropped.
///
/// A value that rounds to zero prints as `0` even when it was negative, since `parseFloat`
/// of `"-0.00"` is negative zero and JavaScript prints that as `0`.
fn trim_fixed(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    let trimmed = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text.as_str()
    };
    if trimmed.is_empty() || trimmed == "-" || trimmed == "-0" {
        "0".into()
    } else {
        trimmed.into()
    }
}

fn trim_float(value: f64) -> String {
    trim_fixed(value, 2)
}

/// A value as a JavaScript `Number()` would read it, or none where that gives `NaN`.
fn as_number(value: &ParamValue) -> Option<f64> {
    match value {
        ParamValue::Int(value) => Some(*value as f64),
        ParamValue::Number(value) => (!value.is_nan()).then_some(*value),
        ParamValue::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        ParamValue::String(text) if text.trim().is_empty() => Some(0.0),
        ParamValue::String(text) => text.trim().parse().ok(),
        // `Number([x])` is `x`; any other array is `NaN`.
        ParamValue::Vector(values) if values.len() == 1 => Some(values[0]),
        _ => None,
    }
}

/// Formats a card value by its parameter type, as `ProcessNode.svelte`'s
/// `formatParamValue(type, value)` does.
///
/// The type decides, not the value: a vector read as a float is `NaN` and shows nothing,
/// and a scalar in a `value` parameter falls through every case and shows nothing either.
pub fn format_card_value(kind: &ParamType, value: &ParamValue) -> Option<String> {
    if matches!(value, ParamValue::Null) {
        return None;
    }
    match kind {
        ParamType::Int => as_number(value).map(|number| (number.round() as i64).to_string()),
        ParamType::Float | ParamType::Numeric => as_number(value).map(trim_float),
        ParamType::String => match value {
            ParamValue::Int(value) => Some(value.to_string()),
            ParamValue::Number(value) => Some(value.to_string()),
            other => format_param_value(other),
        },
        ParamType::Bool => Some(
            match value {
                ParamValue::Bool(value) => *value,
                ParamValue::Int(value) => *value != 0,
                ParamValue::Number(value) => *value != 0.0 && !value.is_nan(),
                ParamValue::String(text) => !text.is_empty(),
                _ => true,
            }
            .to_string(),
        ),
        _ => match value {
            ParamValue::Vector(_) => format_param_value(value),
            _ => None,
        },
    }
}

/// `CompareNode.svelte`'s `formatVal`: three decimals for a number, ten characters and an
/// ellipsis for a string past twelve, and at most four components of an array.
pub fn format_compare_value(value: &ParamValue) -> Option<String> {
    match value {
        ParamValue::Null | ParamValue::Structured(_) => None,
        ParamValue::Bool(value) => Some(value.to_string()),
        ParamValue::Int(value) => Some(value.to_string()),
        ParamValue::Number(value) if value.is_nan() => Some("NaN".into()),
        ParamValue::Number(value) => Some(trim_fixed(*value, 3)),
        ParamValue::String(text) => Some(if text.chars().count() > 12 {
            let truncated: String = text.chars().take(10).collect();
            format!("{truncated}...")
        } else {
            text.clone()
        }),
        ParamValue::Vector(values) => Some(
            values
                .iter()
                .take(4)
                .map(|value| trim_float(*value))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }
}

/// Formats an inline value exactly as `formatParamValue` does.
pub fn format_param_value(value: &ParamValue) -> Option<String> {
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

/// True for an output slot that carries several components at once, Electron's
/// `COMBINED_SLICE` keys. Such a slot shows only its label, never a value.
fn is_combined_slot(name: &str) -> bool {
    matches!(name, "xy" | "xyz" | "rgb" | "xyzw" | "rgba")
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

/// Inputs to card measurement that come from outside the node.
pub struct CardContext<'a> {
    pub registry: &'a Registry,
    /// The whole graph, for the parts of a card that depend on its wires: a wired Compare
    /// input hides its value, a `value` port takes the colour of what feeds it, and the
    /// Text Output slots are named after their sources.
    pub graph: &'a Graph,
    /// Live values from the preview run. Only computed rows and output slots read these;
    /// a writable row shows what the user stored.
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

impl CardContext<'_> {
    /// The edge driving `handle` on `node`, if one is connected.
    fn incoming(&self, node: &GraphNode, handle: &str) -> Option<&bite_schema::GraphEdge> {
        self.graph
            .edges
            .iter()
            .find(|edge| edge.target == node.id && edge.target_handle == handle)
    }
}

/// True for the Process As Set node, which has a card of its own.
fn is_set(node: &GraphNode) -> bool {
    node.data.definition_id == "process_as_set"
        || node.kind == NodeKind::Processing(ProcessingNodeKind::SetInput)
}

/// The parameters a card draws, in the order it draws them.
///
/// An enum parameter is edited in the inspector only. `nodeEditorHelpers.ts` filters
/// `type !== 'enum'` when it builds `paramDefs`, so an enum has neither a row nor a port.
/// Nor does a structured one - Rename's blocks, a set's suffixes - which has no value to
/// show on a row; in Electron those nodes declared no parameters at all.
///
/// Resize is the one definition whose rows follow its mode. `buildResizeParamDefs` put
/// Preserve Aspect first and then only the dimensions the mode, the preserve flag and the
/// anchor leave in play, and gave Resolution no row at all.
fn card_params<'a>(
    node: &GraphNode,
    params: &[&'a ParamDefinition],
    context: &CardContext,
) -> Vec<&'a ParamDefinition> {
    let carded = |param: &&ParamDefinition| {
        !matches!(
            param.kind,
            ParamType::Enum
                | ParamType::RenameBlocks
                | ParamType::SetSuffixes
                | ParamType::TextSlots
        ) && (context.visible)(&param.name)
    };
    let mut carded: Vec<&ParamDefinition> = params.iter().copied().filter(carded).collect();
    if node.data.definition_id == "resize" {
        carded.retain(|param| param.name != "density");
        if let Some(index) = carded
            .iter()
            .position(|param| param.name == "preserve_aspect")
        {
            let preserve = carded.remove(index);
            carded.insert(0, preserve);
        }
    }
    carded
}

/// The wire type arriving at `handle`, read from whatever feeds it - Electron's
/// `getConnectedWireType`. A `value` source says nothing about its type, so it gives none.
fn connected_wire(node: &GraphNode, handle: &str, context: &CardContext) -> Option<WireType> {
    let edge = context.incoming(node, handle)?;
    let source = context
        .graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == edge.source)?;
    if let Some(name) = edge.source_handle.strip_prefix("param:") {
        let param = context
            .registry
            .nodes
            .get(&source.data.definition_id)?
            .definition
            .params
            .iter()
            .find(|param| param.name == name)?;
        return (param.kind != ParamType::Value).then(|| param_wire(param));
    }
    let (_, outputs) = section_ports(source, context);
    outputs
        .into_iter()
        .find(|(handle, _, _)| *handle == edge.source_handle)
        .map(|(_, _, wire)| wire)
}

/// The colour a parameter's label and handle are drawn in, as `paramPortColor` decides it.
///
/// A `value` parameter has no colour of its own, so it borrows the type of the wire feeding
/// it. A computed one, such as Branch's result, borrows from the first wired `value` input.
fn param_port_wire(
    node: &GraphNode,
    param: &ParamDefinition,
    params: &[&ParamDefinition],
    context: &CardContext,
) -> WireType {
    if param.kind != ParamType::Value {
        return param_wire(param);
    }
    let borrowed = if param.readonly {
        params
            .iter()
            .filter(|input| input.kind == ParamType::Value && !input.readonly)
            .find_map(|input| connected_wire(node, &format!("param:{}", input.name), context))
    } else {
        connected_wire(node, &format!("param:{}", param.name), context)
    };
    borrowed.unwrap_or(WireType::Value)
}

/// A colour parameter's swatch, clamped the way `colorToCss` clamps it.
fn swatch_color(value: Option<&ParamValue>) -> Option<[f32; 4]> {
    match value {
        Some(ParamValue::Vector(values)) if values.len() >= 3 => {
            let channel = |index: usize, fallback: f64| {
                values
                    .get(index)
                    .copied()
                    .unwrap_or(fallback)
                    .clamp(0.0, 1.0) as f32
            };
            Some([
                channel(0, 0.0),
                channel(1, 0.0),
                channel(2, 0.0),
                channel(3, 1.0),
            ])
        }
        _ => None,
    }
}

/// `CompareNode.svelte`'s `OPERATOR_SYMBOLS`, with `?` for anything it does not name.
pub fn operator_symbol(operator: Option<&str>) -> &'static str {
    match operator {
        Some("equal") => "=",
        Some("not equal") => "!=",
        Some("greater than") => ">",
        Some("less than") => "<",
        Some("greater or equal") => ">=",
        Some("less or equal") => "<=",
        _ => "?",
    }
}

/// The Compare card's header badge, from the stored operator or the definition default.
fn compare_badge(node: &GraphNode, params: &[&ParamDefinition]) -> String {
    let stored = node.data.params.get("operator").or_else(|| {
        params
            .iter()
            .find(|param| param.name == "operator")
            .and_then(|param| param.default.as_ref())
    });
    let operator = match stored {
        Some(ParamValue::String(text)) => Some(text.as_str()),
        _ => None,
    };
    operator_symbol(operator).to_string()
}

/// Measures a card for `node`.
pub fn measure(node: &GraphNode, context: &CardContext) -> Card {
    if is_set(node) {
        return measure_set(node, context);
    }
    if node.kind == NodeKind::Builtin(BuiltinNodeKind::FolderPath) {
        return measure_folder(node, context);
    }

    let mut ports = Vec::new();
    let mut rows = Vec::new();
    let mut separators = Vec::new();

    let (input_ports, output_ports) = section_ports(node, context);
    let has_image_ports = !input_ports.is_empty() || !output_ports.is_empty();

    let definition = context.registry.nodes.get(&node.data.definition_id);
    let params: Vec<&ParamDefinition> = definition
        .map(|entry| entry.definition.params.iter().collect())
        .unwrap_or_default();
    // `paramDefs` in Electron: every parameter but the enums, which `paramPortColor` reads
    // when a computed `value` row looks for a wired input to borrow its colour from.
    let port_params: Vec<&ParamDefinition> = params
        .iter()
        .copied()
        .filter(|param| param.kind != ParamType::Enum)
        .collect();
    let carded = card_params(node, &params, context);
    let body_params: Vec<&ParamDefinition> = carded
        .iter()
        .copied()
        .filter(|param| !param.port_only)
        .collect();
    let slot_params: Vec<&ParamDefinition> = carded
        .iter()
        .copied()
        .filter(|param| param.port_only)
        .collect();
    let compare = node.kind == NodeKind::Processing(ProcessingNodeKind::Compare)
        || node.data.definition_id == "logic_comparison";

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
                height: port_row,
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
            let stored = node.data.params.get(&param.name);
            let live = context
                .resolved
                .and_then(|values| values.get(&param.name))
                .or(stored);
            let (value, wire) = if compare {
                // `CompareNode.svelte` paints A and B in the `any` colour whatever feeds
                // them, and shows their live values only while nothing is wired in.
                let wired = context
                    .incoming(node, &format!("param:{}", param.name))
                    .is_some();
                let value = if !param.readonly && wired {
                    None
                } else {
                    live.and_then(format_compare_value)
                };
                let wire = if param.kind == ParamType::Value {
                    WireType::Value
                } else {
                    param_wire(param)
                };
                (value, wire)
            } else {
                // A colour shows its swatch in place of any text. Otherwise a computed row
                // shows its live value, and a writable one shows what the user stored, once
                // that differs from the default.
                let value = if param.kind == ParamType::Color {
                    None
                } else if param.readonly {
                    live.and_then(|value| format_card_value(&param.kind, value))
                } else if is_changed(param, stored) {
                    stored.and_then(|value| format_card_value(&param.kind, value))
                } else {
                    None
                };
                (value, param_port_wire(node, param, &port_params, context))
            };
            let swatch = if param.kind == ParamType::Color && !param.readonly {
                swatch_color(stored)
            } else {
                None
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
            let wire = param_port_wire(node, param, &port_params, context);
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

    // Only `ProcessNode` is actionable: the set, Compare and the built-in endpoints have no
    // bypass at all, so they carry neither the tick nor its port.
    let is_actionable = !input_ports.is_empty()
        && !output_ports.is_empty()
        && matches!(node.kind, NodeKind::Processing(ProcessingNodeKind::Process));
    let has_bypass_toggle = is_actionable && !context.enabled_wired;
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

    let badge = compare.then(|| compare_badge(node, &params));
    let width = content_width(node, &rows, context, has_bypass_toggle, badge.as_deref());

    Card {
        width,
        height,
        header_height: header,
        ports,
        rows,
        footer: context.footer.clone(),
        badge,
        has_bypass_toggle,
        separators,
    }
}

/// Measures the Process As Set card, which `SetInputNode.svelte` lays out by hand.
///
/// An Image row and a Prefix row, each ruled off beneath, then one row per suffix. A suffix
/// row is both the string input that can drive that suffix and the image output carrying
/// its stream, so both handles sit on it and its label is drawn once, on the right, in the
/// image colour. With no suffixes a muted line says so.
fn measure_set(node: &GraphNode, context: &CardContext) -> Card {
    let header = theme::NODE_LAYOUT_HEADER_H;
    let mut ports = Vec::new();
    let mut rows = Vec::new();
    let mut separators = Vec::new();

    let fixed = [
        ("in:input", "Image", WireType::Image),
        ("param:prefix", "Prefix", WireType::String),
    ];
    let mut top = header;
    for (handle, label, wire) in fixed {
        ports.push(Port {
            handle: handle.into(),
            label: label.into(),
            wire,
            output: false,
            offset_y: top + SET_ROW_H / 2.0,
        });
        rows.push(Row::PortLabel {
            left: Some(label.into()),
            left_wire: Some(wire),
            right: None,
            right_wire: None,
            top,
            height: SET_ROW_H,
        });
        top += SET_ROW_H;
        separators.push(top);
    }

    let suffixes = match node.data.params.get("suffixes") {
        Some(ParamValue::Structured(StructuredParam::SetSuffixes { suffixes })) => {
            suffixes.as_slice()
        }
        _ => &[],
    };
    if suffixes.is_empty() {
        let height = SET_EMPTY_PAD_Y * 2.0 + f32::from(theme::FONT_SIZE_SM) * theme::UI_LINE_HEIGHT;
        rows.push(Row::Note {
            text: "No suffixes configured".into(),
            top,
            height,
        });
        top += height;
    }
    for (index, suffix) in suffixes.iter().enumerate() {
        let label = label_or_placeholder(suffix, index);
        let offset_y = top + SET_ROW_H / 2.0;
        ports.push(Port {
            handle: format!("param:suffix_{index}"),
            label: label.clone(),
            wire: WireType::String,
            output: false,
            offset_y,
        });
        ports.push(Port {
            handle: format!("out:suffix_{index}"),
            label: label.clone(),
            wire: WireType::Image,
            output: true,
            offset_y,
        });
        rows.push(Row::PortLabel {
            left: None,
            left_wire: None,
            right: Some(label),
            right_wire: Some(WireType::Image),
            top,
            height: SET_ROW_H,
        });
        top += SET_ROW_H;
    }

    let mut height = top;
    if context.footer.is_some() {
        separators.push(top);
        height += theme::NODE_FOOTER_H;
    }
    let width = content_width(node, &rows, context, false, None);
    Card {
        width,
        height,
        header_height: header,
        ports,
        rows,
        footer: context.footer.clone(),
        badge: None,
        has_bypass_toggle: false,
        separators,
    }
}

/// Measures the Folder Path card: one body row holding the `path` tag and the folder name,
/// as `FolderPathNode.svelte` lays it out, with no footer.
fn measure_folder(node: &GraphNode, context: &CardContext) -> Card {
    let header = theme::NODE_LAYOUT_HEADER_H;
    let top = header + theme::NODE_LAYOUT_PORT_PAD;
    // `folder.split(/[/\\]/).pop()`: a path ending in a separator names nothing, which the
    // card shows as unset just as Electron did.
    let name = match node.data.params.get("folderPath") {
        Some(ParamValue::String(path)) => path
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        _ => None,
    };
    let rows = vec![Row::Folder {
        label: "path".into(),
        name,
        top,
    }];
    let ports = vec![Port {
        handle: "out:output".into(),
        label: "Path".into(),
        wire: WireType::Path,
        output: true,
        offset_y: top + theme::NODE_LAYOUT_PORT_ROW_H / 2.0,
    }];
    let width = content_width(node, &rows, context, false, None);
    Card {
        width,
        height: header + theme::NODE_LAYOUT_PORT_PAD * 2.0 + theme::NODE_LAYOUT_PORT_ROW_H,
        header_height: header,
        ports,
        rows,
        footer: None,
        badge: None,
        has_bypass_toggle: false,
        separators: Vec::new(),
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
    badge: Option<&str>,
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
    let title = measure(theme::face::NODE_HEAD, &label);
    width = width.max(match badge {
        // Compare's header pads ten pixels a side and spaces the badge six from the title.
        Some(badge) => {
            title + OPERATOR_GAP + measure(OPERATOR_FACE, badge) + theme::NODE_ROW_INSET * 2.0
        }
        None => title + header_inset * 2.0,
    });

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
            Row::Folder { label, name, .. } => {
                let name = name.as_deref().unwrap_or(FOLDER_UNSET);
                measure(theme::face::PORT_TAG, label)
                    + theme::NODE_VALUE_GAP
                    + measure(theme::face::SMALL_MONO, name)
            }
            // `.node-empty` pads twelve pixels a side, two more than a port row.
            Row::Note { text, .. } => measure(theme::face::LABEL, text) + 4.0,
        };
        width = width.max(row_width + inset);
    }

    // The footer never widens the card: `.footer-label` is capped and ellipsized instead.
    width.min(theme::NODE_MAX_WIDTH).ceil()
}

/// Derives an output slot's value from the first array-valued body parameter.
fn slot_value(
    node: &GraphNode,
    param: &ParamDefinition,
    body_params: &[&ParamDefinition],
    context: &CardContext,
) -> Option<String> {
    // Combined slots show only their label, never a value, however it was resolved.
    if is_combined_slot(&param.name) {
        return None;
    }
    let lookup = |name: &str| {
        context
            .resolved
            .and_then(|values| values.get(name))
            .or_else(|| node.data.params.get(name))
    };
    if let Some(stored) = lookup(&param.name) {
        if !matches!(stored, ParamValue::Null) {
            return format_card_value(&param.kind, stored);
        }
    }
    let source = body_params
        .iter()
        .find_map(|body| match lookup(&body.name) {
            Some(ParamValue::Vector(values)) => Some(values),
            _ => None,
        })?;
    let index = component_index(&param.name)?;
    format_card_value(
        &param.kind,
        &ParamValue::Number(source.get(index).copied().unwrap_or(0.0)),
    )
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

/// The Text Output slot rows, as `TextOutputNode.svelte` lists them.
///
/// The rows run in slot order, but the ghost - the free slot a new wire lands on - is the
/// last one stored, whatever its number. It reads "New Input"; every other slot is named
/// after the node wired into it, or `-` when nothing is.
fn text_output_slots(node: &GraphNode, context: &CardContext) -> Vec<SectionPort> {
    let slots = match node.data.params.get("portIds") {
        Some(ParamValue::Structured(StructuredParam::TextSlots { slots })) if !slots.is_empty() => {
            slots.clone()
        }
        _ => vec!["0".to_string()],
    };
    let ghost = slots.last().cloned();
    let mut ordered = slots;
    ordered.sort_by_key(|slot| slot.parse::<u64>().unwrap_or(0));
    ordered
        .into_iter()
        .map(|slot| {
            let handle = format!("txo:{slot}");
            let label = if Some(&slot) == ghost.as_ref() {
                "New Input".to_string()
            } else {
                context
                    .incoming(node, &handle)
                    .and_then(|edge| {
                        context
                            .graph
                            .nodes
                            .iter()
                            .find(|candidate| candidate.id == edge.source)
                    })
                    .map(|source| super::view::card_label(source, context.registry))
                    .unwrap_or_else(|| "-".to_string())
            };
            (handle, label, WireType::Value)
        })
        .collect()
}

/// The ports drawn in the upper section, before parameters.
pub fn section_ports(
    node: &GraphNode,
    context: &CardContext,
) -> (Vec<SectionPort>, Vec<SectionPort>) {
    let registry = context.registry;
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => (
            Vec::new(),
            vec![("out:output".into(), "Image".into(), WireType::Image)],
        ),
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            // `ImageOutputNode.svelte` says when a Folder Path drives the folder.
            let folder = if context.incoming(node, "in:folder").is_some() {
                "Folder (wired)"
            } else {
                "Folder"
            };
            (
                vec![
                    ("in:input".into(), "Image".into(), WireType::Image),
                    ("in:folder".into(), folder.into(), WireType::Path),
                ],
                Vec::new(),
            )
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            let mut inputs = vec![("in:input".into(), "Image".into(), WireType::Image)];
            inputs.extend(text_output_slots(node, context));
            (inputs, Vec::new())
        }
        // `FlipbookOutputNode.svelte`'s six inputs, in its order: the grid and cell sizes
        // can each be driven by a number wire.
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => (
            vec![
                ("in:input".into(), "Image".into(), WireType::Image),
                ("param:cols".into(), "Columns".into(), WireType::Number),
                ("param:rows".into(), "Rows".into(), WireType::Number),
                (
                    "param:cellWidth".into(),
                    "Cell Width".into(),
                    WireType::Number,
                ),
                (
                    "param:cellHeight".into(),
                    "Cell Height".into(),
                    WireType::Number,
                ),
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
        // The set's card is laid out by `measure_set`; this lists its outputs only, for
        // whatever reads the type of a wire leaving one.
        _ if is_set(node) => {
            let suffixes = match node.data.params.get("suffixes") {
                Some(ParamValue::Structured(StructuredParam::SetSuffixes { suffixes })) => {
                    suffixes.as_slice()
                }
                _ => &[],
            };
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
            (Vec::new(), outputs)
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

    fn empty_graph() -> Graph {
        graph_of(Vec::new(), Vec::new())
    }

    fn graph_of(nodes: Vec<GraphNode>, edges: Vec<bite_schema::GraphEdge>) -> Graph {
        Graph {
            nodes,
            edges,
            viewport: bite_schema::Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }

    fn edge(
        source: &str,
        source_handle: &str,
        target: &str,
        target_handle: &str,
    ) -> bite_schema::GraphEdge {
        bite_schema::GraphEdge {
            id: format!("{source}-{target_handle}"),
            source: source.into(),
            source_handle: source_handle.into(),
            target: target.into(),
            target_handle: target_handle.into(),
        }
    }

    /// A card context whose font makes every character two units wide.
    fn width_context<'a>(
        registry: &'a Registry,
        graph: &'a Graph,
        visible: &'a dyn Fn(&str) -> bool,
        measure: &'a dyn Fn(bite_imgui::Face, &str) -> f32,
        footer: Option<String>,
    ) -> CardContext<'a> {
        CardContext {
            registry,
            graph,
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
        let graph = empty_graph();
        let context = width_context(&registry, &graph, &visible, &measure, None);
        let width = content_width(&node, &[], &context, false, None);
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
        let graph = empty_graph();
        let context = width_context(&registry, &graph, &visible, &measure, None);
        assert_eq!(
            content_width(&node, &[], &context, false, None),
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
        let graph = empty_graph();
        let context = width_context(&registry, &graph, &visible, &measure, None);
        let plain = content_width(&node, &[], &context, false, None);
        let ticked = content_width(&node, &[], &context, true, None);
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
        let graph = empty_graph();
        let context = width_context(&registry, &graph, &visible, &measure, None);
        assert_eq!(
            content_width(&node, &[], &context, false, None),
            theme::NODE_MAX_WIDTH
        );
    }

    fn registry() -> Registry {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        crate::studio::Studio::load_registry(&root).expect("the shipped definitions load")
    }

    fn typed(kind: NodeKind, definition_id: &str, params: Vec<(&str, ParamValue)>) -> GraphNode {
        let mut node = node_with(definition_id, params);
        node.kind = kind;
        node
    }

    fn process(definition_id: &str, params: Vec<(&str, ParamValue)>) -> GraphNode {
        typed(
            NodeKind::Processing(ProcessingNodeKind::Process),
            definition_id,
            params,
        )
    }

    /// Measures `node` the way the canvas does, with the inspector's visibility rules.
    fn measure_in(
        node: &GraphNode,
        graph: &Graph,
        registry: &Registry,
        resolved: Option<&std::collections::BTreeMap<String, ParamValue>>,
        footer: Option<String>,
    ) -> Card {
        let measure_text = |_: bite_imgui::Face, text: &str| text.chars().count() as f32 * 2.0;
        let visible = |name: &str| {
            registry
                .nodes
                .get(&node.data.definition_id)
                .and_then(|entry| entry.definition.params.iter().find(|p| p.name == name))
                .is_none_or(|param| {
                    crate::panels::inspector::generic::is_visible(param, node, registry)
                })
        };
        measure(
            node,
            &CardContext {
                registry,
                graph,
                resolved,
                visible: &visible,
                footer,
                enabled_wired: false,
                measure_text: &measure_text,
            },
        )
    }

    fn row_names(card: &Card) -> Vec<&str> {
        card.rows
            .iter()
            .filter_map(|row| match row {
                Row::Param { name, .. } | Row::ReadonlyParam { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    fn handles(card: &Card) -> Vec<&str> {
        card.ports.iter().map(|port| port.handle.as_str()).collect()
    }

    #[test]
    fn small_negatives_print_as_zero_as_javascript_does() {
        assert_eq!(
            format_param_value(&ParamValue::Number(-0.001)),
            Some("0".into())
        );
        assert_eq!(
            format_param_value(&ParamValue::Vector(vec![-0.004, 1.0])),
            Some("0, 1".into())
        );
        assert_eq!(
            format_card_value(&ParamType::Int, &ParamValue::Number(-0.4)),
            Some("0".into())
        );
        assert_eq!(
            format_param_value(&ParamValue::Number(-1.5)),
            Some("-1.5".into())
        );
    }

    #[test]
    fn the_parameter_type_decides_what_a_card_value_shows() {
        // A `value` parameter holding a scalar falls through `formatParamValue`.
        assert_eq!(
            format_card_value(&ParamType::Value, &ParamValue::Number(3.0)),
            None
        );
        // A vector read as a float is `NaN`.
        assert_eq!(
            format_card_value(&ParamType::Numeric, &ParamValue::Vector(vec![1.0, 2.0])),
            None
        );
        assert_eq!(
            format_card_value(&ParamType::Int, &ParamValue::Number(2.6)),
            Some("3".into())
        );
        assert_eq!(
            format_card_value(
                &ParamType::Vector3,
                &ParamValue::Vector(vec![1.0, 0.5, 0.0])
            ),
            Some("1, 0.5, 0".into())
        );
        assert_eq!(
            format_card_value(&ParamType::Value, &ParamValue::Vector(vec![1.0, 2.0])),
            Some("1, 2".into())
        );
    }

    #[test]
    fn compare_values_keep_three_decimals_and_cut_strings_at_ten() {
        assert_eq!(
            format_compare_value(&ParamValue::Number(1.23456)),
            Some("1.235".into())
        );
        assert_eq!(
            format_compare_value(&ParamValue::String("abcdefghijklm".into())),
            Some("abcdefghij...".into())
        );
    }

    #[test]
    fn resize_shows_preserve_aspect_and_only_the_dimension_in_play() {
        let registry = registry();
        let graph = empty_graph();
        let node = process("resize", Vec::new());
        let card = measure_in(&node, &graph, &registry, None, None);
        // Absolute, preserving, anchored on width: Electron's `buildResizeParamDefs`.
        assert_eq!(row_names(&card), vec!["preserve_aspect", "width"]);

        let free = process(
            "resize",
            vec![
                ("preserve_aspect", ParamValue::Bool(false)),
                ("mode", ParamValue::String("relative".into())),
            ],
        );
        let card = measure_in(&free, &graph, &registry, None, None);
        assert_eq!(
            row_names(&card),
            vec!["preserve_aspect", "scale_width", "scale_height"]
        );
        assert!(card.port("param:density").is_none());
    }

    #[test]
    fn the_set_card_pairs_each_suffix_input_with_its_output() {
        let registry = registry();
        let graph = empty_graph();
        let node = typed(
            NodeKind::Processing(ProcessingNodeKind::SetInput),
            "process_as_set",
            vec![(
                "suffixes",
                ParamValue::Structured(StructuredParam::SetSuffixes {
                    suffixes: vec!["_n".into(), String::new()],
                }),
            )],
        );
        let card = measure_in(&node, &graph, &registry, None, Some("1 set matched".into()));
        let offset = |handle: &str| card.port(handle).map(|port| port.offset_y);
        // `SetInputNode.svelte`: 41, 67, then 93 plus 26 a suffix.
        assert_eq!(offset("in:input"), Some(41.0));
        assert_eq!(offset("param:prefix"), Some(67.0));
        assert_eq!(offset("param:suffix_0"), Some(93.0));
        assert_eq!(offset("out:suffix_0"), Some(93.0));
        assert_eq!(offset("param:suffix_1"), Some(119.0));
        assert_eq!(offset("out:suffix_1"), Some(119.0));
        assert!(card.port("param:_enabled").is_none(), "a set has no bypass");
        assert!(!card.has_bypass_toggle);
        // Each suffix label is drawn once, on the right.
        let labels: Vec<_> = card
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::PortLabel {
                    left: None,
                    right: Some(label),
                    right_wire: Some(WireType::Image),
                    ..
                } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, vec!["_n", "suffix2"]);
        assert_eq!(card.height, 28.0 + 26.0 * 4.0 + theme::NODE_FOOTER_H);
    }

    #[test]
    fn a_set_without_suffixes_says_so() {
        let registry = registry();
        let graph = empty_graph();
        let node = typed(
            NodeKind::Processing(ProcessingNodeKind::SetInput),
            "process_as_set",
            Vec::new(),
        );
        let card = measure_in(&node, &graph, &registry, None, None);
        assert!(card
            .rows
            .iter()
            .any(|row| matches!(row, Row::Note { text, .. } if text == "No suffixes configured")));
        assert_eq!(handles(&card), vec!["in:input", "param:prefix"]);
    }

    #[test]
    fn only_a_colour_parameter_gets_a_swatch_and_it_shows_no_text() {
        let registry = registry();
        let graph = empty_graph();
        let color = process(
            "value_color",
            vec![("color", ParamValue::Vector(vec![1.0, 0.0, 0.0, 1.0]))],
        );
        let card = measure_in(&color, &graph, &registry, None, None);
        let Some(Row::Param { swatch, value, .. }) = card
            .rows
            .iter()
            .find(|row| matches!(row, Row::Param { name, .. } if name == "color"))
        else {
            panic!("the colour row is drawn");
        };
        assert_eq!(*swatch, Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(*value, None);

        let vector = process(
            "value_vector3",
            vec![("vec", ParamValue::Vector(vec![1.0, 2.0, 3.0]))],
        );
        let card = measure_in(&vector, &graph, &registry, None, None);
        let Some(Row::Param { swatch, .. }) = card
            .rows
            .iter()
            .find(|row| matches!(row, Row::Param { name, .. } if name == "vec"))
        else {
            panic!("the vector row is drawn");
        };
        assert_eq!(*swatch, None);
    }

    #[test]
    fn combined_slots_never_show_a_value() {
        let registry = registry();
        let graph = empty_graph();
        let node = process(
            "value_color",
            vec![("color", ParamValue::Vector(vec![0.5, 0.25, 0.0, 1.0]))],
        );
        let resolved: std::collections::BTreeMap<String, ParamValue> = [(
            "rgba".to_string(),
            ParamValue::Vector(vec![0.5, 0.25, 0.0, 1.0]),
        )]
        .into();
        let card = measure_in(&node, &graph, &registry, Some(&resolved), None);
        let slot = |wanted: &str| {
            card.rows.iter().find_map(|row| match row {
                Row::Slot { name, value, .. } if name == wanted => Some(value.clone()),
                _ => None,
            })
        };
        assert_eq!(slot("rgba"), Some(None));
        assert_eq!(slot("rgb"), Some(None));
        assert_eq!(slot("g"), Some(Some("0.25".into())));
    }

    #[test]
    fn a_writable_row_shows_the_stored_value_over_a_stale_resolved_one() {
        let registry = registry();
        let graph = empty_graph();
        let node = process("blur", vec![("sigma", ParamValue::Number(7.0))]);
        let resolved: std::collections::BTreeMap<String, ParamValue> =
            [("sigma".to_string(), ParamValue::Number(1.0))].into();
        let card = measure_in(&node, &graph, &registry, Some(&resolved), None);
        let value = card.rows.iter().find_map(|row| match row {
            Row::Param { name, value, .. } if name == "sigma" => Some(value.clone()),
            _ => None,
        });
        assert_eq!(value, Some(Some("7".into())));
    }

    #[test]
    fn text_output_slots_are_named_after_their_sources() {
        let registry = registry();
        let source = process("value_string", Vec::new());
        let mut output = typed(
            NodeKind::Builtin(BuiltinNodeKind::TextOutput),
            "textoutput",
            vec![(
                "portIds",
                ParamValue::Structured(StructuredParam::TextSlots {
                    slots: vec!["2".into(), "0".into(), "1".into()],
                }),
            )],
        );
        output.id = "out".into();
        let graph = graph_of(
            vec![source.clone(), output.clone()],
            vec![edge("n", "param:value", "out", "txo:2")],
        );
        let card = measure_in(&output, &graph, &registry, None, None);
        let labels: Vec<_> = card
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::PortLabel {
                    left: Some(label), ..
                } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        // Slot order, with the last stored slot as the ghost.
        assert_eq!(labels, vec!["Image", "-", "New Input", "String"]);
    }

    #[test]
    fn flipbook_output_offers_its_six_inputs() {
        let registry = registry();
        let graph = empty_graph();
        let node = typed(
            NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput),
            "flipbookoutput",
            Vec::new(),
        );
        let card = measure_in(&node, &graph, &registry, None, Some("4 x 4 grid".into()));
        assert_eq!(
            handles(&card),
            vec![
                "in:input",
                "param:cols",
                "param:rows",
                "param:cellWidth",
                "param:cellHeight",
                "param:bgColor"
            ]
        );
    }

    #[test]
    fn a_wired_folder_says_so_on_the_image_output() {
        let registry = registry();
        let folder = typed(
            NodeKind::Builtin(BuiltinNodeKind::FolderPath),
            "folderpath",
            Vec::new(),
        );
        let mut output = typed(
            NodeKind::Builtin(BuiltinNodeKind::ImageOutput),
            "imageoutput",
            Vec::new(),
        );
        output.id = "out".into();
        let unwired = graph_of(vec![folder.clone(), output.clone()], Vec::new());
        let wired = graph_of(
            vec![folder, output.clone()],
            vec![edge("n", "out:output", "out", "in:folder")],
        );
        let label = |graph: &Graph| {
            measure_in(&output, graph, &registry, None, None)
                .port("in:folder")
                .map(|port| port.label.clone())
        };
        assert_eq!(label(&unwired), Some("Folder".into()));
        assert_eq!(label(&wired), Some("Folder (wired)".into()));
    }

    #[test]
    fn compare_shows_its_operator_and_hides_a_wired_value() {
        let registry = registry();
        let source = process("value_float", Vec::new());
        let mut compare = typed(
            NodeKind::Processing(ProcessingNodeKind::Compare),
            "logic_comparison",
            vec![
                ("operator", ParamValue::String("greater or equal".into())),
                ("a", ParamValue::Number(1.0)),
                ("b", ParamValue::Number(2.0)),
            ],
        );
        compare.id = "cmp".into();
        let graph = graph_of(
            vec![source, compare.clone()],
            vec![edge("n", "param:value", "cmp", "param:a")],
        );
        let card = measure_in(&compare, &graph, &registry, None, None);
        assert_eq!(card.badge.as_deref(), Some(">="));
        let value = |wanted: &str| {
            card.rows.iter().find_map(|row| match row {
                Row::Param { name, value, .. } if name == wanted => Some(value.clone()),
                _ => None,
            })
        };
        assert_eq!(value("a"), Some(None));
        assert_eq!(value("b"), Some(Some("2".into())));
        assert_eq!(operator_symbol(Some("sideways")), "?");
    }

    #[test]
    fn a_long_footer_leaves_the_card_at_its_width() {
        let registry = registry();
        let graph = empty_graph();
        let node = typed(
            NodeKind::Builtin(BuiltinNodeKind::TextOutput),
            "textoutput",
            Vec::new(),
        );
        let card = measure_in(&node, &graph, &registry, None, Some("x".repeat(400)));
        assert_eq!(card.width, theme::NODE_WORKFLOW_WIDTH);
    }

    #[test]
    fn folder_path_puts_the_folder_in_its_one_row() {
        let registry = registry();
        let graph = empty_graph();
        let node = typed(
            NodeKind::Builtin(BuiltinNodeKind::FolderPath),
            "folderpath",
            vec![("folderPath", ParamValue::String("C:\\work\\renders".into()))],
        );
        let card = measure_in(&node, &graph, &registry, None, None);
        assert!(card.footer.is_none());
        assert!(matches!(
            card.rows.as_slice(),
            [Row::Folder { label, name: Some(name), .. }] if label == "path" && name == "renders"
        ));
        assert_eq!(handles(&card), vec!["out:output"]);
    }

    #[test]
    fn a_value_port_takes_the_colour_of_the_wire_feeding_it() {
        let registry = registry();
        let source = process("value_string", Vec::new());
        let mut branch = process("logic_branch", Vec::new());
        branch.id = "branch".into();
        let graph = graph_of(
            vec![source, branch.clone()],
            vec![edge("n", "param:value", "branch", "param:value_true")],
        );
        let card = measure_in(&branch, &graph, &registry, None, None);
        let wire = |handle: &str| card.port(handle).map(|port| port.wire);
        assert_eq!(wire("param:value_true"), Some(WireType::String));
        assert_eq!(wire("param:value_false"), Some(WireType::Value));
        // The computed result borrows from the first wired input.
        assert_eq!(wire("param:result"), Some(WireType::String));
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
