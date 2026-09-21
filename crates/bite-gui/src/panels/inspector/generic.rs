//! The generic parameter editor, from `InspectorParamEditor.svelte`.
//!
//! Each row puts its label above a full-width control, with badges for wired and computed
//! parameters and a reset control that keeps its space when the value is already default.
use super::{Edit, InspectorContext};
use crate::{color_picker, controls, theme};
use bite_imgui::{Rounding, StyleColor, Ui};
use bite_schema::{GraphNode, ParamDefinition, ParamType, ParamValue, WidgetType};

/// The state of one row, which decides how the value is presented.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowState {
    Editable,
    /// Driven by an incoming wire, so the source value is shown instead of a control.
    Wired,
    /// Produced by the node itself.
    Computed,
}

/// Decides how a parameter row is presented. `computed` says whether the node's own
/// implementation produces this parameter.
pub fn row_state(
    definition: &ParamDefinition,
    node: &GraphNode,
    graph: &bite_schema::Graph,
    computed: bool,
) -> RowState {
    let handle = format!("param:{}", definition.name);
    let wired = graph
        .edges
        .iter()
        .any(|edge| edge.target == node.id && edge.target_handle == handle);
    if wired {
        RowState::Wired
    } else if definition.readonly && computed {
        RowState::Computed
    } else {
        RowState::Editable
    }
}

/// Whether the implementation works this parameter out for itself.
///
/// Readonly alone does not settle it: a Float, String or Boolean node marks its value
/// readonly to give the card an output port, yet nothing computes the value, so the
/// inspector edits it. The Svelte editor drew the same line by asking whether the
/// definition named an executor at all, which those three did not.
pub fn is_computed(implementation: &bite_schema::Implementation, name: &str) -> bool {
    match implementation {
        bite_schema::Implementation::Native { .. } => true,
        bite_schema::Implementation::Compute { outputs } => outputs.contains_key(name),
        bite_schema::Implementation::Imagemagick { .. } => false,
    }
}

/// The value an incoming wire supplies, read from the source node's parameter.
pub fn wired_value(
    definition: &ParamDefinition,
    node: &GraphNode,
    graph: &bite_schema::Graph,
) -> Option<String> {
    let handle = format!("param:{}", definition.name);
    let edge = graph
        .edges
        .iter()
        .find(|edge| edge.target == node.id && edge.target_handle == handle)?;
    let source = graph
        .nodes
        .iter()
        .find(|candidate| candidate.id == edge.source)?;
    let name = edge.source_handle.strip_prefix("param:")?;
    let value = source.data.params.get(name)?;
    Some(match value {
        // The Svelte editor prints array components at three decimal places.
        ParamValue::Vector(values) => values
            .iter()
            .map(|component| format!("{component:.3}"))
            .collect::<Vec<_>>()
            .join(", "),
        other => crate::canvas::layout::format_param_value(other).unwrap_or_default(),
    })
}

/// True when the row should offer the reset control, matching the Svelte condition.
pub fn shows_reset(definition: &ParamDefinition, state: RowState) -> bool {
    state == RowState::Editable
        && !definition.readonly
        && definition.default.is_some()
        && definition.widget != Some(WidgetType::Checkbox)
}

/// Whether the stored value already equals the definition default.
pub fn at_default(definition: &ParamDefinition, value: Option<&ParamValue>) -> bool {
    match (&definition.default, value) {
        (Some(default), Some(current)) => default == current,
        (Some(_), None) => false,
        (None, _) => false,
    }
}

/// Evaluates a `visible_when` rule against the node's parameters.
pub fn is_visible(
    definition: &ParamDefinition,
    node: &GraphNode,
    registry: &bite_core::Registry,
) -> bool {
    let Some(entry) = registry.nodes.get(&node.data.definition_id) else {
        return true;
    };
    let Some(expression) = entry.visible.get(&definition.name) else {
        return true;
    };
    let mut context = bite_expr::definition::default_context(&entry.definition.params);
    for (name, value) in &node.data.params {
        if let Some(value) = bite_expr::definition::to_value(value) {
            context.insert(name.clone(), value);
        }
    }
    expression
        .evaluate(&context)
        .map(|value| value.truthy())
        .unwrap_or(true)
}

/// Draws every visible parameter of a definition-backed node.
pub fn draw(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    context: &InspectorContext,
    pickers: &mut color_picker::States,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let Some(entry) = context.registry.nodes.get(&node.data.definition_id) else {
        ui.dummy([width, 14.0]);
        indent(ui);
        controls::hint(ui, "No definition found for this node.");
        return edits;
    };
    let visible: Vec<&ParamDefinition> = entry
        .definition
        .params
        .iter()
        .filter(|param| !param.port_only && is_visible(param, node, context.registry))
        .collect();
    if visible.is_empty() {
        ui.dummy([width, 14.0]);
        indent(ui);
        controls::hint(ui, "This node has no parameters.");
        return edits;
    }
    for definition in visible {
        let computed = is_computed(&entry.definition.implementation, &definition.name);
        edits.extend(row(ui, width, node, definition, computed, context, pickers));
    }
    edits
}

fn indent(ui: &mut Ui) {
    let position = ui.cursor_screen_position();
    ui.set_cursor_screen_position([position[0] + 12.0, position[1]]);
}

/// One parameter row.
#[allow(clippy::too_many_arguments)]
pub fn row(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    definition: &ParamDefinition,
    computed: bool,
    context: &InspectorContext,
    pickers: &mut color_picker::States,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let state = row_state(definition, node, context.graph, computed);
    let stored = node.data.params.get(&definition.name);
    let padding = theme::INSPECTOR_PARAM_PADDING;
    let _id = ui.push_id(&definition.name);

    ui.dummy([width, padding[1]]);
    let row_origin = ui.cursor_screen_position();
    ui.set_cursor_screen_position([row_origin[0] + padding[0], row_origin[1]]);
    let content_width = width - padding[0] * 2.0;

    // The header row carries the label, any badge, the reset control and the checkbox.
    ui.group(|ui| {
        let header_origin = ui.cursor_screen_position();
        controls::row_label(ui, &definition.label);
        // `.param-label` sets a six pixel gap between the label and its badge.
        ui.same_line_at(0.0, theme::INSPECTOR_LABEL_GAP);
        match state {
            RowState::Wired => {
                controls::badge(ui, "wired", theme::PORT_COLOR_NUMBER);
            }
            RowState::Computed => {
                controls::badge(ui, "out", theme::TEXT_BRIGHT.with_alpha(0.5));
            }
            RowState::Editable => {}
        }

        if definition.widget == Some(WidgetType::Checkbox) && state == RowState::Editable {
            let mut value = matches!(stored, Some(ParamValue::Bool(true)));
            let box_x = header_origin[0] + content_width - theme::CHECKBOX_SIZE;
            ui.set_cursor_screen_position([box_x, header_origin[1]]);
            if controls::checkbox(ui, &definition.name, &mut value) {
                edits.push(Edit::SetParam {
                    node: node.id.clone(),
                    name: definition.name.clone(),
                    value: ParamValue::Bool(value),
                });
            }
        } else if shows_reset(definition, state) {
            let label_width = 42.0;
            let reset_x = header_origin[0] + content_width - label_width;
            ui.set_cursor_screen_position([reset_x, header_origin[1] - 1.0]);
            // The control keeps its space when the value is default, so rows never shift.
            if at_default(definition, stored) {
                ui.dummy([label_width, 18.0]);
            } else if reset_button(ui, label_width) {
                if let Some(default) = &definition.default {
                    edits.push(Edit::SetParam {
                        node: node.id.clone(),
                        name: definition.name.clone(),
                        value: default.clone(),
                    });
                }
            }
        }
    });

    ui.dummy([width, theme::INSPECTOR_PARAM_GAP]);
    ui.set_cursor_screen_position([row_origin[0] + padding[0], ui.cursor_screen_position()[1]]);

    match state {
        RowState::Wired => {
            let value = wired_value(definition, node, context.graph).unwrap_or_default();
            ui.with_face(theme::face::VALUE, |ui| {
                ui.with_colors(
                    &[(StyleColor::Text, theme::PORT_COLOR_NUMBER.with_alpha(0.8))],
                    |ui| ui.text(&value),
                )
            });
        }
        RowState::Computed => {
            let live = context
                .resolved
                .and_then(|values| values.get(&definition.name))
                .or(stored);
            let text = live
                .and_then(crate::canvas::layout::format_param_value)
                .unwrap_or_default();
            ui.with_face(theme::face::VALUE, |ui| {
                ui.with_colors(
                    &[(StyleColor::Text, theme::TEXT_BRIGHT.with_alpha(0.5))],
                    |ui| ui.text(&text),
                )
            });
        }
        RowState::Editable => {
            if definition.widget != Some(WidgetType::Checkbox) {
                edits.extend(control(ui, content_width, node, definition, stored, pickers));
            }
        }
    }

    ui.dummy([width, padding[1]]);
    controls::row_separator(ui);
    edits
}

/// Draws the control for one parameter and reports any change.
fn control(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    definition: &ParamDefinition,
    stored: Option<&ParamValue>,
    pickers: &mut color_picker::States,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let mut emit = |value: ParamValue| {
        edits.push(Edit::SetParam {
            node: node.id.clone(),
            name: definition.name.clone(),
            value,
        });
    };

    match definition.widget {
        Some(WidgetType::Slider) => {
            let mut value = number_of(stored).unwrap_or_else(|| default_number(definition));
            let min = definition.min.unwrap_or(0.0) as f32;
            let max = definition.max.unwrap_or(100.0) as f32;
            let slider_width = width - theme::SLIDER_VAL_WIDTH - theme::SLIDER_WRAP_GAP;
            // The track centres itself in the number box's height, so the two line up.
            if controls::slider(
                ui,
                &definition.name,
                &mut value,
                min,
                max,
                slider_width,
                theme::INPUT_HEIGHT,
            ) {
                emit(number_value(definition, value));
            }
            ui.same_line_at(0.0, theme::SLIDER_WRAP_GAP);
            let mut text = trim_number(value);
            if controls::text_input(
                ui,
                &format!("{}-value", definition.name),
                &mut text,
                "",
                theme::SLIDER_VAL_WIDTH,
            ) {
                if let Ok(parsed) = text.trim().parse::<f32>() {
                    emit(number_value(definition, parsed.clamp(min, max)));
                }
            }
        }
        Some(WidgetType::Dropdown) => {
            edits.extend(dropdown(ui, width, node, definition, stored));
        }
        // The two colour shapes are not interchangeable: a `color-picker` parameter is
        // stored as a hex string and has no alpha of its own, a `vector` one as four
        // numbers. Reading a `#ff0000` as a vector came back as transparent black, which
        // is how the Tint node's red default was drawn as nothing at all.
        Some(WidgetType::ColorPicker) => {
            let text = match stored {
                Some(ParamValue::String(text)) => text.clone(),
                _ => hex_default(definition),
            };
            let parsed = color_picker::from_hex(&text).unwrap_or([0.0, 0.0, 0.0]);
            let mut rgba = [parsed[0], parsed[1], parsed[2], 1.0];
            if color_picker::draw(
                ui,
                &definition.name,
                &mut rgba,
                width,
                false,
                pickers,
                theme::PANEL_BG,
            ) {
                emit(ParamValue::String(color_picker::to_hex([
                    rgba[0], rgba[1], rgba[2],
                ])));
            }
        }
        Some(WidgetType::Vector) if definition.kind == ParamType::Color => {
            let value = vector_of(stored, 4);
            let mut rgba = [
                value[0] as f32,
                value[1] as f32,
                value[2] as f32,
                value.get(3).copied().unwrap_or(1.0) as f32,
            ];
            if color_picker::draw(
                ui,
                &definition.name,
                &mut rgba,
                width,
                false,
                pickers,
                theme::PANEL_BG,
            ) {
                emit(ParamValue::Vector(
                    rgba.iter().map(|component| f64::from(*component)).collect(),
                ));
            }
        }
        Some(WidgetType::Vector) => {
            edits.extend(vector(ui, width, node, definition, stored));
        }
        _ => match definition.kind {
            ParamType::Enum => edits.extend(dropdown(ui, width, node, definition, stored)),
            ParamType::Bool => {
                let mut value = matches!(stored, Some(ParamValue::Bool(true)));
                if controls::checkbox(ui, &definition.name, &mut value) {
                    emit(ParamValue::Bool(value));
                }
            }
            ParamType::Vector2 | ParamType::Vector3 | ParamType::Vector4 => {
                edits.extend(vector(ui, width, node, definition, stored));
            }
            ParamType::Int | ParamType::Float | ParamType::Numeric => {
                let value = number_of(stored).unwrap_or_else(|| default_number(definition));
                let mut text = trim_number(value);
                if controls::text_input(
                    ui,
                    &format!("{}-number", definition.name),
                    &mut text,
                    "",
                    width,
                ) {
                    if let Ok(parsed) = text.trim().parse::<f32>() {
                        emit(number_value(definition, parsed));
                    }
                }
            }
            _ => {
                let mut text = match stored {
                    Some(ParamValue::String(text)) => text.clone(),
                    _ => String::new(),
                };
                if controls::text_input(ui, &definition.name, &mut text, "", width) {
                    emit(ParamValue::String(text));
                }
            }
        },
    }
    edits
}

fn dropdown(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    definition: &ParamDefinition,
    stored: Option<&ParamValue>,
) -> Vec<Edit> {
    if definition.options.is_empty() {
        return Vec::new();
    }
    let labels: Vec<String> = if definition.labels.len() == definition.options.len() {
        definition.labels.clone()
    } else {
        definition.options.clone()
    };
    let current_value = match stored {
        Some(ParamValue::String(text)) => text.clone(),
        _ => definition
            .options
            .first()
            .cloned()
            .unwrap_or_default(),
    };
    let mut index = definition
        .options
        .iter()
        .position(|option| *option == current_value)
        .unwrap_or(0);
    if controls::dropdown(ui, &definition.name, &mut index, &labels, width, true) {
        if let Some(option) = definition.options.get(index) {
            return vec![Edit::SetParam {
                node: node.id.clone(),
                name: definition.name.clone(),
                value: ParamValue::String(option.clone()),
            }];
        }
    }
    Vec::new()
}

fn vector(
    ui: &mut Ui,
    width: f32,
    node: &GraphNode,
    definition: &ParamDefinition,
    stored: Option<&ParamValue>,
) -> Vec<Edit> {
    let count = match definition.kind {
        ParamType::Vector2 => 2,
        ParamType::Vector3 => 3,
        _ => 4,
    };
    let labels = ["X", "Y", "Z", "W"];
    let mut values = vector_of(stored, count);
    let mut changed = false;
    // Laying out a row returns the cursor to the panel's own left edge, not to where the
    // row began, so each component is anchored to the edge the first one set.
    let left = ui.cursor_screen_position()[0];
    for index in 0..count {
        let label_width = 14.0;
        let origin = [left, ui.cursor_screen_position()[1]];
        ui.draw_list().text_with_face(
            [origin[0], origin[1] + 8.0],
            theme::TEXT_BRIGHT.with_alpha(0.5),
            theme::face::VALUE,
            labels[index],
        );
        ui.set_cursor_screen_position([origin[0] + label_width + 6.0, origin[1]]);
        let mut text = trim_number(values[index] as f32);
        if controls::text_input(
            ui,
            &format!("{}-{index}", definition.name),
            &mut text,
            "",
            width - label_width - 6.0,
        ) {
            if let Ok(parsed) = text.trim().parse::<f64>() {
                values[index] = parsed;
                changed = true;
            }
        }
        ui.dummy([width, 6.0]);
    }
    if changed {
        return vec![Edit::SetParam {
            node: node.id.clone(),
            name: definition.name.clone(),
            value: ParamValue::Vector(values),
        }];
    }
    Vec::new()
}

/// Reads a numeric parameter, whatever numeric shape it is stored in.
pub fn number_of(value: Option<&ParamValue>) -> Option<f32> {
    match value? {
        ParamValue::Int(value) => Some(*value as f32),
        ParamValue::Number(value) => Some(*value as f32),
        _ => None,
    }
}

/// The hex a `color-picker` parameter falls back to, which `hexToRgba` reads as black.
fn hex_default(definition: &ParamDefinition) -> String {
    match definition.default.as_ref() {
        Some(ParamValue::String(text)) => text.clone(),
        _ => "#000000".into(),
    }
}

fn default_number(definition: &ParamDefinition) -> f32 {
    number_of(definition.default.as_ref()).unwrap_or(0.0)
}

/// Stores a number back in the shape the definition expects.
pub fn number_value(definition: &ParamDefinition, value: f32) -> ParamValue {
    if definition.kind == ParamType::Int {
        ParamValue::Int(value.round() as i64)
    } else {
        ParamValue::Number(f64::from(value))
    }
}

/// Reads a vector parameter, padding to the requested length.
pub fn vector_of(value: Option<&ParamValue>, count: usize) -> Vec<f64> {
    let mut values = match value {
        Some(ParamValue::Vector(values)) => values.clone(),
        _ => Vec::new(),
    };
    values.resize(count.max(values.len()), 0.0);
    values.truncate(count.max(1));
    values
}

/// Formats a number the way the editor's fields show it: no trailing zeroes.
pub fn trim_number(value: f32) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        let text = format!("{value:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The small outlined reset control.
fn reset_button(ui: &mut Ui, width: f32) -> bool {
    let height = 18.0;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button("##reset", [width, height]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
    }
    let list = ui.draw_list();
    let max = [origin[0] + width, origin[1] + height];
    list.rect_outline(
        origin,
        max,
        if hovered {
            theme::COLOR_SUCCESS.mix(60.0, bite_imgui::Color::TRANSPARENT)
        } else {
            theme::BORDER
        },
        3.0,
        Rounding::All,
        1.0,
    );
    let size = list.measure(theme::face::SMALL_MONO, "Reset");
    list.text_with_face(
        [
            origin[0] + (width - size[0]) / 2.0,
            origin[1] + (height - size[1]) / 2.0,
        ],
        if hovered {
            theme::COLOR_SUCCESS_TEXT
        } else {
            theme::TEXT_BRIGHT
        },
        theme::face::SMALL_MONO,
        "Reset",
    );
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{GraphEdge, NodeData, Position, Viewport};

    fn definition(name: &str, kind: ParamType, default: Option<ParamValue>) -> ParamDefinition {
        ParamDefinition {
            name: name.into(),
            label: name.into(),
            kind,
            widget: None,
            default,
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
        }
    }

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            kind: bite_schema::NodeKind::Processing(bite_schema::ProcessingNodeKind::Process),
            position: Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: NodeData {
                label: String::new(),
                definition_id: "resize".into(),
                params: Default::default(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        }
    }

    fn graph(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> bite_schema::Graph {
        bite_schema::Graph {
            nodes,
            edges,
            viewport: Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        }
    }

    #[test]
    fn an_incoming_wire_replaces_the_control() {
        let target = node("b");
        let edge = GraphEdge {
            id: "e".into(),
            source: "a".into(),
            source_handle: "param:value".into(),
            target: "b".into(),
            target_handle: "param:width".into(),
        };
        let graph = graph(vec![node("a"), target.clone()], vec![edge]);
        let definition = definition("width", ParamType::Int, None);
        assert_eq!(
            row_state(&definition, &target, &graph, false),
            RowState::Wired
        );
    }

    #[test]
    fn a_computed_parameter_is_shown_rather_than_edited() {
        let node = node("a");
        let graph = graph(vec![node.clone()], Vec::new());
        let mut definition = definition("result", ParamType::Numeric, None);
        definition.readonly = true;
        assert_eq!(
            row_state(&definition, &node, &graph, true),
            RowState::Computed
        );
        // Without an executor the Svelte editor treats it as an ordinary row.
        assert_eq!(
            row_state(&definition, &node, &graph, false),
            RowState::Editable
        );
    }

    /// A Float, String or Boolean node carries one readonly value so the card gains an
    /// output port. Nothing computes it, so the inspector must still offer the control.
    #[test]
    fn a_constant_is_edited_while_a_worked_out_value_is_only_shown() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = crate::studio::Studio::load_registry(&root).unwrap();
        let state = |definition_id: &str, parameter: &str| {
            let entry = &registry.nodes[definition_id];
            let definition = entry
                .definition
                .params
                .iter()
                .find(|param| param.name == parameter)
                .unwrap();
            let mut node = node("a");
            node.data.definition_id = definition_id.into();
            let graph = graph(vec![node.clone()], Vec::new());
            row_state(
                definition,
                &node,
                &graph,
                is_computed(&entry.definition.implementation, parameter),
            )
        };
        assert_eq!(state("value_float", "value"), RowState::Editable);
        assert_eq!(state("value_string", "value"), RowState::Editable);
        assert_eq!(state("value_boolean", "value"), RowState::Editable);
        assert_eq!(state("value_vector4", "vec"), RowState::Editable);
        // The ones a node really does work out stay as readings.
        assert_eq!(state("math_add", "result"), RowState::Computed);
        assert_eq!(state("prop_dimensions", "width"), RowState::Computed);
        assert_eq!(state("mean_value", "value"), RowState::Computed);
    }

    #[test]
    fn the_reset_control_appears_only_for_editable_rows_with_a_default() {
        let with_default = definition("width", ParamType::Int, Some(ParamValue::Int(10)));
        let without = definition("width", ParamType::Int, None);
        assert!(shows_reset(&with_default, RowState::Editable));
        assert!(!shows_reset(&without, RowState::Editable));
        assert!(!shows_reset(&with_default, RowState::Wired));
        let mut checkbox = with_default.clone();
        checkbox.widget = Some(WidgetType::Checkbox);
        assert!(!shows_reset(&checkbox, RowState::Editable));
    }

    #[test]
    fn a_value_equal_to_the_default_hides_the_reset_control() {
        let definition = definition("width", ParamType::Int, Some(ParamValue::Int(10)));
        assert!(at_default(&definition, Some(&ParamValue::Int(10))));
        assert!(!at_default(&definition, Some(&ParamValue::Int(20))));
        assert!(!at_default(&definition, None));
    }

    #[test]
    fn the_wired_value_is_read_from_the_source_parameter() {
        let mut source = node("a");
        source
            .data
            .params
            .insert("value".into(), ParamValue::Number(2.5));
        let target = node("b");
        let edge = GraphEdge {
            id: "e".into(),
            source: "a".into(),
            source_handle: "param:value".into(),
            target: "b".into(),
            target_handle: "param:width".into(),
        };
        let graph = graph(vec![source, target.clone()], vec![edge]);
        let definition = definition("width", ParamType::Int, None);
        assert_eq!(
            wired_value(&definition, &target, &graph),
            Some("2.5".to_string())
        );
    }

    #[test]
    fn integers_are_stored_as_integers_and_floats_as_numbers() {
        let integer = definition("width", ParamType::Int, None);
        let float = definition("scale", ParamType::Float, None);
        assert_eq!(number_value(&integer, 12.6), ParamValue::Int(13));
        assert_eq!(number_value(&float, 2.5), ParamValue::Number(2.5));
    }

    #[test]
    fn numbers_are_shown_without_trailing_zeroes() {
        assert_eq!(trim_number(3.0), "3");
        assert_eq!(trim_number(2.5), "2.5");
        assert_eq!(trim_number(-1.25), "-1.25");
    }

    #[test]
    fn vectors_are_padded_to_the_requested_length() {
        assert_eq!(vector_of(None, 3), vec![0.0, 0.0, 0.0]);
        assert_eq!(
            vector_of(Some(&ParamValue::Vector(vec![1.0, 2.0])), 2),
            vec![1.0, 2.0]
        );
    }
}
