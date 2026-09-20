//! Drawing and gesture handling for the node canvas.
use super::{
    layout::{self, Card, CardContext, Row},
    state::{Bounds, CanvasState, Gesture, PendingWire, WireEnd, nearest_port},
    viewport::Viewport,
    wire,
};
use crate::{
    controls::{self, HoverTimer},
    shell::Rect,
    theme,
};
use bite_core::{Registry, graph::WireType};
use bite_imgui::{
    Color, MouseButton, MouseCursor, Rounding, Ui, Vec2, WindowFlags,
};
use bite_schema::{BuiltinNodeKind, Graph, GraphNode, NodeKind, ParamValue, ProcessingNodeKind};
use std::collections::BTreeMap;

/// Everything the canvas needs that does not live in the graph.
pub struct CanvasContext<'a> {
    pub registry: &'a Registry,
    /// Live values from the preview run, keyed by node then parameter.
    pub resolved: &'a BTreeMap<String, BTreeMap<String, ParamValue>>,
    /// Image counts per Input node, for the card footer.
    pub image_counts: &'a BTreeMap<String, usize>,
    pub preview_node: Option<String>,
    pub running_node: Option<String>,
    pub running_file: Option<String>,
    pub delta: f32,
}

/// What the canvas asks the application to do. The canvas never edits the graph itself, so
/// that history and validation stay in one place.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A node drag started; the application opens one history transaction.
    BeginDrag,
    /// Nodes moved to new positions, in graph coordinates.
    MoveNodes(Vec<(String, [f32; 2])>),
    EndDrag,
    Connect {
        source: String,
        source_handle: String,
        target: String,
        target_handle: String,
    },
    DeleteEdges(Vec<String>),
    /// The creation menu should open, optionally completing a dropped wire.
    OpenCreateMenu {
        position: [f32; 2],
        pending: Option<PendingWire>,
    },
    /// Double clicking a node toggles which node the preview follows.
    TogglePreview(String),
    ToggleBypass(String),
    ResizeNode {
        id: String,
        width: f32,
        height: f32,
    },
    /// An inline edit of a comment heading or body, or a group name.
    SetParam {
        node: String,
        name: String,
        value: ParamValue,
    },
    /// A library item was dropped on the canvas at this graph position.
    DropDefinition {
        payload: String,
        position: [f32; 2],
    },
    SelectionChanged,
}

/// Cached geometry for one node this frame.
struct Placed {
    id: String,
    card: Card,
    /// Absolute graph position, with any parent offset already applied.
    position: Vec2,
    kind: NodeKind,
    label: String,
    enabled: bool,
    is_group: bool,
    is_comment: bool,
}

impl Placed {
    fn bounds(&self) -> Bounds {
        Bounds {
            min: self.position,
            max: [
                self.position[0] + self.card.width,
                self.position[1] + self.card.height,
            ],
        }
    }
}

/// Per-frame canvas surface, holding the state that survives between frames.
#[derive(Default)]
pub struct Canvas {
    pub state: CanvasState,
    pub tooltip: HoverTimer,
    /// Node positions captured when a drag began, so the move is applied as one delta.
    drag_start: BTreeMap<String, Vec2>,
    /// The comment or group whose text is being edited inline.
    pub editing: Option<(String, String)>,
    pub editing_buffer: String,
}

impl Canvas {
    /// Draws the canvas into `rect` and returns the actions the application must apply.
    pub fn run(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        graph: &Graph,
        context: &CanvasContext,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let placed = self.measure_nodes(graph, context);

        ui.set_next_window_position(rect.min);
        ui.set_next_window_size(rect.size());
        let mut flags = WindowFlags::panel();
        flags.no_scrollbar = true;
        flags.no_scroll_with_mouse = true;
        flags.no_background = true;

        ui.window_with("##canvas", flags, |ui| {
            let list = ui.draw_list();
            list.push_clip(rect.min, rect.max, true);
            list.rect(
                rect.min,
                rect.max,
                theme::GRAPH_BG_BASE_COLOR,
                theme::PANEL_RADIUS,
                Rounding::All,
            );
            self.draw_grid(ui, rect);

            // Groups sit behind everything so their children draw on top.
            for node in placed.iter().filter(|node| node.is_group) {
                self.draw_group(ui, rect, node, context);
            }
            self.draw_edges(ui, rect, graph, &placed, context);
            for node in placed.iter().filter(|node| !node.is_group) {
                self.draw_node(ui, rect, node, context);
            }
            self.draw_pending_wire(ui, rect);
            self.draw_rubber_band(ui, rect);

            actions.extend(self.handle_input(ui, rect, graph, &placed, context));

            ui.draw_list().pop_clip();
            self.draw_overlays(ui, rect, &placed);
            if let Some(payload) = self.accept_drop(ui, rect) {
                actions.push(payload);
            }
        });
        actions
    }

    /// Measures every node once, resolving group-relative positions to absolute ones.
    fn measure_nodes(&self, graph: &Graph, context: &CanvasContext) -> Vec<Placed> {
        let positions: BTreeMap<&str, Vec2> = graph
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.as_str(),
                    [node.position.x as f32, node.position.y as f32],
                )
            })
            .collect();
        graph
            .nodes
            .iter()
            .map(|node| {
                let mut position = [node.position.x as f32, node.position.y as f32];
                if let Some(parent) = node.parent_id.as_deref().and_then(|id| positions.get(id)) {
                    position = [position[0] + parent[0], position[1] + parent[1]];
                }
                let enabled_wired = graph.edges.iter().any(|edge| {
                    edge.target == node.id && edge.target_handle == "param:_enabled"
                });
                let resolved = context.resolved.get(&node.id);
                let visible = |_name: &str| true;
                let card = if matches!(
                    node.kind,
                    NodeKind::Builtin(BuiltinNodeKind::Group | BuiltinNodeKind::Comment)
                ) {
                    Card {
                        width: node.width.unwrap_or(280.0) as f32,
                        height: node.height.unwrap_or(120.0) as f32,
                        header_height: if node.kind
                            == NodeKind::Builtin(BuiltinNodeKind::Comment)
                        {
                            30.0
                        } else {
                            36.0
                        },
                        ports: Vec::new(),
                        rows: Vec::new(),
                        footer: None,
                        has_bypass_toggle: false,
                        separators: Vec::new(),
                    }
                } else {
                    layout::measure(
                        node,
                        &CardContext {
                            registry: context.registry,
                            resolved,
                            visible: &visible,
                            footer: footer_text(node, context),
                            enabled_wired,
                        },
                    )
                };
                Placed {
                    id: node.id.clone(),
                    card,
                    position,
                    kind: node.kind.clone(),
                    label: card_label(node, context.registry),
                    enabled: node_enabled(node, graph, context.registry),
                    is_group: node.kind == NodeKind::Builtin(BuiltinNodeKind::Group),
                    is_comment: node.kind == NodeKind::Builtin(BuiltinNodeKind::Comment),
                }
            })
            .collect()
    }

    /// The line grid the `Background` component paints at a thirty-two pixel spacing.
    fn draw_grid(&self, ui: &Ui, rect: Rect) {
        let list = ui.draw_list();
        let viewport = self.state.viewport;
        let gap = theme::GRAPH_BG_GAP * viewport.zoom;
        if gap < 4.0 {
            return;
        }
        let origin = [rect.min[0] + viewport.x, rect.min[1] + viewport.y];
        let first_x = origin[0] - ((origin[0] - rect.min[0]) / gap).ceil() * gap;
        let first_y = origin[1] - ((origin[1] - rect.min[1]) / gap).ceil() * gap;
        let mut x = first_x;
        while x <= rect.max[0] {
            list.line(
                [x, rect.min[1]],
                [x, rect.max[1]],
                theme::GRAPH_BG_COLOR,
                theme::GRAPH_BG_LINE_WIDTH,
            );
            x += gap;
        }
        let mut y = first_y;
        while y <= rect.max[1] {
            list.line(
                [rect.min[0], y],
                [rect.max[0], y],
                theme::GRAPH_BG_COLOR,
                theme::GRAPH_BG_LINE_WIDTH,
            );
            y += gap;
        }
    }

    /// Converts a graph point to a screen point inside `rect`.
    fn screen(&self, rect: Rect, point: Vec2) -> Vec2 {
        let local = self.state.viewport.to_screen(point);
        [rect.min[0] + local[0], rect.min[1] + local[1]]
    }

    /// Converts a screen point back to graph coordinates.
    fn graph_point(&self, rect: Rect, point: Vec2) -> Vec2 {
        self.state
            .viewport
            .to_graph([point[0] - rect.min[0], point[1] - rect.min[1]])
    }

    /// The screen position of a port handle.
    fn port_position(&self, rect: Rect, node: &Placed, handle: &str) -> Option<Vec2> {
        let port = node.card.port(handle)?;
        let x = if port.output {
            node.position[0] + node.card.width
        } else {
            node.position[0]
        };
        Some(self.screen(rect, [x, node.position[1] + port.offset_y]))
    }

    fn draw_edges(
        &self,
        ui: &Ui,
        rect: Rect,
        graph: &Graph,
        placed: &[Placed],
        _context: &CanvasContext,
    ) {
        let list = ui.draw_list();
        let by_id: BTreeMap<&str, &Placed> =
            placed.iter().map(|node| (node.id.as_str(), node)).collect();
        for edge in &graph.edges {
            let (Some(source), Some(target)) =
                (by_id.get(edge.source.as_str()), by_id.get(edge.target.as_str()))
            else {
                continue;
            };
            let (Some(from), Some(to)) = (
                self.port_position(rect, source, &edge.source_handle),
                self.port_position(rect, target, &edge.target_handle),
            ) else {
                continue;
            };
            // The color always comes from the source handle, so a reopened file keeps it.
            let wire_type = source
                .card
                .port(&edge.source_handle)
                .map(|port| port.wire)
                .unwrap_or(WireType::Value);
            let selected = self.state.selected_edges.contains(&edge.id);
            let points = wire::bezier_points(from, to);
            let colour = wire::color(wire_type);
            if selected {
                // The stylesheet applies a five pixel glow to the selected wire.
                for step in 1..=3 {
                    list.bezier(
                        points[0],
                        points[1],
                        points[2],
                        points[3],
                        colour.with_alpha(0.12),
                        theme::EDGE_WIDTH_SELECTED * self.state.viewport.zoom
                            + step as f32 * 2.5,
                    );
                }
            }
            list.bezier(
                points[0],
                points[1],
                points[2],
                points[3],
                colour,
                if selected {
                    theme::EDGE_WIDTH_SELECTED
                } else {
                    theme::EDGE_WIDTH
                } * self.state.viewport.zoom.max(0.5),
            );
        }
    }

    fn draw_pending_wire(&self, ui: &Ui, rect: Rect) {
        let Gesture::Connecting(pending) = &self.state.gesture else {
            return;
        };
        let from = self.screen(rect, pending.origin);
        let to = ui.mouse_position();
        let (from, to) = if pending.end == WireEnd::Source {
            (from, to)
        } else {
            (to, from)
        };
        let points = wire::preview_points(from, to);
        // The drag line is dashed, which is drawn as alternating samples of the curve.
        let list = ui.draw_list();
        let colour = wire::color(pending.wire).with_alpha(0.8);
        let steps = 48;
        for step in 0..steps {
            if step % 2 == 1 {
                continue;
            }
            let a = wire::sample(points, step as f32 / steps as f32);
            let b = wire::sample(points, (step + 1) as f32 / steps as f32);
            list.line(a, b, colour, theme::EDGE_WIDTH);
        }
    }

    fn draw_rubber_band(&self, ui: &Ui, _rect: Rect) {
        let Gesture::RubberBand { origin, current } = self.state.gesture else {
            return;
        };
        let list = ui.draw_list();
        let min = [origin[0].min(current[0]), origin[1].min(current[1])];
        let max = [origin[0].max(current[0]), origin[1].max(current[1])];
        list.rect(min, max, theme::ACCENT.with_alpha(0.12), 0.0, Rounding::None);
        list.rect_outline(
            min,
            max,
            theme::ACCENT.with_alpha(0.8),
            0.0,
            Rounding::None,
            1.0,
        );
    }

    fn draw_group(&self, ui: &Ui, rect: Rect, node: &Placed, _context: &CanvasContext) {
        let list = ui.draw_list();
        let min = self.screen(rect, node.position);
        let max = self.screen(
            rect,
            [
                node.position[0] + node.card.width,
                node.position[1] + node.card.height,
            ],
        );
        let selected = self.state.is_selected(&node.id);
        let border = if selected {
            theme::GROUP_SELECTED_BORDER
        } else {
            theme::GROUP_BORDER
        };
        let zoom = self.state.viewport.zoom;
        list.rect(
            min,
            max,
            theme::GROUP_BG,
            theme::NODE_RADIUS * zoom,
            Rounding::Bottom,
        );
        list.rect_outline(
            min,
            max,
            border,
            theme::NODE_RADIUS * zoom,
            Rounding::Bottom,
            1.5 * zoom,
        );
        // The label band floats above the frame and carries the rounded top corners.
        let band_height = 36.0 * zoom;
        let band_min = [min[0], min[1] - band_height];
        list.rect(
            band_min,
            [max[0], min[1]],
            theme::NODE_HEAD_BG,
            theme::NODE_RADIUS * zoom,
            Rounding::Top,
        );
        list.rect_outline(
            band_min,
            [max[0], min[1]],
            border,
            theme::NODE_RADIUS * zoom,
            Rounding::Top,
            1.5 * zoom,
        );
        if zoom > 0.4 {
            let text_height = list.measure(theme::face::NODE_HEAD, &node.label)[1];
            controls::draw_ellipsized(
                ui,
                [
                    band_min[0] + 12.0 * zoom,
                    band_min[1] + (band_height - text_height) / 2.0,
                ],
                theme::NODE_TEXT,
                theme::face::NODE_HEAD,
                &node.label,
                (max[0] - min[0] - 24.0 * zoom).max(10.0),
            );
        }
    }

    fn draw_node(&self, ui: &Ui, rect: Rect, node: &Placed, context: &CanvasContext) {
        if node.is_comment {
            return self.draw_comment(ui, rect, node);
        }
        let list = ui.draw_list();
        let zoom = self.state.viewport.zoom;
        let min = self.screen(rect, node.position);
        let max = self.screen(
            rect,
            [
                node.position[0] + node.card.width,
                node.position[1] + node.card.height,
            ],
        );
        if max[0] < rect.min[0] || min[0] > rect.max[0] || max[1] < rect.min[1] || min[1] > rect.max[1]
        {
            return;
        }
        let radius = theme::NODE_RADIUS * zoom;
        let alpha = if node.enabled { 1.0 } else { 0.45 };

        // A drop shadow stands in for the stylesheet's twelve pixel blur.
        list.rect(
            [min[0], min[1] + 4.0 * zoom],
            [max[0], max[1] + 4.0 * zoom],
            Color([0.0, 0.0, 0.0, 0.45 * alpha]),
            radius,
            Rounding::All,
        );
        list.rect(min, max, theme::NODE_BG.fade(alpha), radius, Rounding::All);

        let header_bottom = min[1] + node.card.header_height * zoom;
        list.rect(
            min,
            [max[0], header_bottom],
            header_color(&node.kind, &node.label).fade(alpha),
            radius,
            Rounding::Top,
        );
        list.line(
            [min[0], header_bottom],
            [max[0], header_bottom],
            theme::NODE_BORDER.fade(alpha),
            1.0,
        );

        if zoom > 0.35 {
            let text_size = list.measure(theme::face::NODE_HEAD, &node.label);
            let inset = if node.card.has_bypass_toggle {
                34.0 * zoom
            } else {
                12.0 * zoom
            };
            let available = (max[0] - min[0] - inset * 2.0).max(10.0);
            let centred = min[0] + inset + (available - text_size[0]).max(0.0) / 2.0;
            controls::draw_ellipsized(
                ui,
                [
                    centred,
                    min[1] + (node.card.header_height * zoom - text_size[1]) / 2.0,
                ],
                theme::NODE_TEXT.fade(alpha),
                theme::face::NODE_HEAD,
                &node.label,
                available,
            );
            self.draw_rows(ui, rect, node, alpha);
        }

        for offset in &node.card.separators {
            let y = min[1] + offset * zoom;
            list.line(
                [min[0], y],
                [max[0], y],
                theme::NODE_BORDER.fade(alpha),
                1.0,
            );
        }

        if let Some(footer) = &node.card.footer {
            if zoom > 0.35 {
                let y = max[1] - theme::NODE_FOOTER_H * zoom;
                let size = list.measure(theme::face::SMALL_MONO, footer);
                controls::draw_ellipsized(
                    ui,
                    [
                        min[0] + theme::NODE_ROW_INSET * zoom,
                        y + (theme::NODE_FOOTER_H * zoom - size[1]) / 2.0,
                    ],
                    theme::TEXT.fade(alpha),
                    theme::face::SMALL_MONO,
                    footer,
                    (max[0] - min[0] - theme::NODE_ROW_INSET * 2.0 * zoom).max(10.0),
                );
            }
        }

        let selected = self.state.is_selected(&node.id);
        list.rect_outline(
            min,
            max,
            theme::NODE_BORDER.fade(alpha),
            radius,
            Rounding::All,
            1.0,
        );
        if selected {
            list.rect_outline(
                [min[0] - 2.0, min[1] - 2.0],
                [max[0] + 2.0, max[1] + 2.0],
                theme::NODE_SELECTED_RING,
                radius + 2.0,
                Rounding::All,
                2.0,
            );
        }

        if node.card.has_bypass_toggle {
            self.draw_bypass_tick(ui, min, node, zoom);
        }
        self.draw_ports(ui, rect, node, alpha);
        self.draw_badges(ui, rect, node, context);
    }

    fn draw_rows(&self, ui: &Ui, rect: Rect, node: &Placed, alpha: f32) {
        let list = ui.draw_list();
        let zoom = self.state.viewport.zoom;
        let left = self.screen(rect, node.position)[0];
        let right = left + node.card.width * zoom;
        let inset = theme::NODE_ROW_INSET * zoom;
        for row in &node.card.rows {
            match row {
                Row::PortLabel {
                    left: left_label,
                    left_wire,
                    right: right_label,
                    right_wire,
                    top,
                } => {
                    let y = self.screen(rect, [0.0, node.position[1] + top])[1];
                    let height = theme::NODE_LAYOUT_PORT_ROW_H * zoom;
                    if let (Some(label), Some(wire_type)) = (left_label, left_wire) {
                        let size = list.measure(theme::face::PORT_TAG, label);
                        list.text_with_face(
                            [left + inset, y + (height - size[1]) / 2.0],
                            wire::color(*wire_type).fade(alpha),
                            theme::face::PORT_TAG,
                            label,
                        );
                    }
                    if let (Some(label), Some(wire_type)) = (right_label, right_wire) {
                        let size = list.measure(theme::face::PORT_TAG, label);
                        list.text_with_face(
                            [right - inset - size[0], y + (height - size[1]) / 2.0],
                            wire::color(*wire_type).fade(alpha),
                            theme::face::PORT_TAG,
                            label,
                        );
                    }
                }
                Row::Param {
                    label,
                    value,
                    wire: wire_type,
                    swatch,
                    top,
                    ..
                } => {
                    let y = self.screen(rect, [0.0, node.position[1] + top])[1];
                    let height = theme::NODE_LAYOUT_PARAM_ROW_H * zoom;
                    let mut x = left + inset;
                    if let Some(swatch) = swatch {
                        let size = 20.0 * zoom;
                        let swatch_min = [x - 6.0 * zoom, y + (height - size) / 2.0];
                        let swatch_max = [swatch_min[0] + size, swatch_min[1] + size];
                        list.rect(
                            swatch_min,
                            swatch_max,
                            Color(*swatch).fade(alpha),
                            3.0 * zoom,
                            Rounding::All,
                        );
                        list.rect_outline(
                            swatch_min,
                            swatch_max,
                            theme::INPUT_BORDER,
                            3.0 * zoom,
                            Rounding::All,
                            1.0,
                        );
                        x = swatch_max[0] + 6.0 * zoom;
                    }
                    let size = list.measure(theme::face::PORT_TAG, label);
                    let value_width = value
                        .as_ref()
                        .map(|text| list.measure(theme::face::PORT_TAG, text)[0] + 6.0 * zoom)
                        .unwrap_or(0.0);
                    controls::draw_ellipsized(
                        ui,
                        [x, y + (height - size[1]) / 2.0],
                        wire::color(*wire_type).fade(alpha),
                        theme::face::PORT_TAG,
                        label,
                        (right - inset - x - value_width).max(8.0),
                    );
                    if let Some(text) = value {
                        let size = list.measure(theme::face::PORT_TAG, text);
                        list.text_with_face(
                            [right - inset - size[0], y + (height - size[1]) / 2.0],
                            theme::TEXT.fade(alpha),
                            theme::face::PORT_TAG,
                            text,
                        );
                    }
                }
                Row::ReadonlyParam {
                    label,
                    value,
                    wire: wire_type,
                    top,
                    ..
                } => {
                    // A computed row puts the value on the left and the label on the right.
                    let y = self.screen(rect, [0.0, node.position[1] + top])[1];
                    let height = theme::NODE_LAYOUT_PARAM_ROW_H * zoom;
                    if let Some(text) = value {
                        let size = list.measure(theme::face::PORT_TAG, text);
                        list.text_with_face(
                            [left + inset, y + (height - size[1]) / 2.0],
                            theme::TEXT.fade(alpha),
                            theme::face::PORT_TAG,
                            text,
                        );
                    }
                    let size = list.measure(theme::face::PORT_TAG, label);
                    list.text_with_face(
                        [right - inset - size[0], y + (height - size[1]) / 2.0],
                        wire::color(*wire_type).fade(alpha),
                        theme::face::PORT_TAG,
                        label,
                    );
                }
                Row::Slot {
                    label,
                    value,
                    wire: wire_type,
                    top,
                    ..
                } => {
                    let y = self.screen(rect, [0.0, node.position[1] + top])[1];
                    let height = theme::NODE_LAYOUT_PORT_ROW_H * zoom;
                    if let Some(text) = value {
                        let size = list.measure(theme::face::PORT_TAG, text);
                        list.text_with_face(
                            [left + inset, y + (height - size[1]) / 2.0],
                            theme::TEXT.fade(alpha),
                            theme::face::PORT_TAG,
                            text,
                        );
                    }
                    let size = list.measure(theme::face::PORT_TAG, label);
                    list.text_with_face(
                        [right - inset - size[0], y + (height - size[1]) / 2.0],
                        wire::color(*wire_type).fade(alpha),
                        theme::face::PORT_TAG,
                        label,
                    );
                }
            }
        }
    }

    fn draw_ports(&self, ui: &Ui, rect: Rect, node: &Placed, alpha: f32) {
        let list = ui.draw_list();
        let zoom = self.state.viewport.zoom;
        let radius = theme::HANDLE_SIZE / 2.0 * zoom;
        for port in &node.card.ports {
            let Some(centre) = self.port_position(rect, node, &port.handle) else {
                continue;
            };
            let colour = wire::color(port.wire).fade(alpha);
            list.circle(centre, radius, colour);
            list.circle_outline(
                centre,
                radius,
                colour,
                theme::HANDLE_BORDER_WIDTH * zoom * 0.5,
            );
            let hovered = self
                .state
                .hovered_port
                .as_ref()
                .is_some_and(|(id, handle)| *id == node.id && *handle == port.handle);
            if hovered {
                list.circle_outline(centre, radius + 3.0, colour.with_alpha(0.7), 1.5);
            }
        }
    }

    fn draw_bypass_tick(&self, ui: &Ui, min: Vec2, node: &Placed, zoom: f32) {
        let list = ui.draw_list();
        let size = 14.0 * zoom;
        let origin = [
            min[0] + theme::NODE_ROW_INSET * zoom,
            min[1] + (node.card.header_height * zoom - size) / 2.0,
        ];
        let max = [origin[0] + size, origin[1] + size];
        let active = node.enabled;
        let border = if active {
            theme::PORT_COLOR_BOOLEAN
        } else {
            theme::NODE_TEXT.mix(35.0, Color::TRANSPARENT)
        };
        if active {
            list.rect(
                origin,
                max,
                theme::PORT_COLOR_BOOLEAN.mix(20.0, Color::TRANSPARENT),
                2.0 * zoom,
                Rounding::All,
            );
        }
        list.rect_outline(origin, max, border, 2.0 * zoom, Rounding::All, 1.5 * zoom);
        if active {
            let left = [origin[0] + size * 0.26, origin[1] + size * 0.52];
            let middle = [origin[0] + size * 0.44, origin[1] + size * 0.72];
            let right = [origin[0] + size * 0.76, origin[1] + size * 0.28];
            list.line(left, middle, theme::PORT_COLOR_BOOLEAN, 2.0 * zoom);
            list.line(middle, right, theme::PORT_COLOR_BOOLEAN, 2.0 * zoom);
        }
    }

    fn draw_badges(&self, ui: &Ui, rect: Rect, node: &Placed, context: &CanvasContext) {
        let zoom = self.state.viewport.zoom;
        if zoom < 0.4 {
            return;
        }
        let min = self.screen(rect, node.position);
        let width = node.card.width * zoom;
        if context.preview_node.as_deref() == Some(node.id.as_str()) {
            draw_badge(
                ui,
                [min[0] + width / 2.0, min[1] - 22.0 * zoom],
                "PREVIEWING",
                theme::BADGE_PREVIEW_COLOR,
                None,
            );
        }
        if context.running_node.as_deref() == Some(node.id.as_str()) {
            draw_badge(
                ui,
                [min[0] + width / 2.0, min[1] - 22.0 * zoom],
                "PROCESSING",
                theme::BADGE_PROCESSING_COLOR,
                context.running_file.as_deref(),
            );
        }
    }

    fn draw_comment(&self, ui: &Ui, rect: Rect, node: &Placed) {
        let list = ui.draw_list();
        let zoom = self.state.viewport.zoom;
        let min = self.screen(rect, node.position);
        let max = self.screen(
            rect,
            [
                node.position[0] + node.card.width,
                node.position[1] + node.card.height,
            ],
        );
        // The sticky note uses a two pixel radius and a layered shadow.
        list.rect(
            [min[0] + 3.0 * zoom, min[1] + 5.0 * zoom],
            [max[0] + 3.0 * zoom, max[1] + 5.0 * zoom],
            Color([0.0, 0.0, 0.0, 0.55]),
            2.0,
            Rounding::All,
        );
        list.rect(min, max, theme::COMMENT_BG, 2.0, Rounding::All);
        let header_bottom = min[1] + node.card.header_height * zoom;
        list.rect(
            min,
            [max[0], header_bottom],
            theme::COMMENT_HEADER_BG,
            2.0,
            Rounding::Top,
        );
        list.line(
            [min[0], header_bottom],
            [max[0], header_bottom],
            Color([0.0, 0.0, 0.0, 0.12]),
            1.0,
        );
        if self.state.is_selected(&node.id) {
            list.rect_outline(
                min,
                max,
                theme::COMMENT_SELECTED_BORDER,
                2.0,
                Rounding::All,
                2.0,
            );
        }
    }

    /// The zoom label, controls and minimap, which sit above the canvas content.
    fn draw_overlays(&mut self, ui: &mut Ui, rect: Rect, placed: &[Placed]) {
        self.draw_minimap(ui, rect, placed);
        let text = format!("{}%", self.state.viewport.zoom_percent());
        let list = ui.draw_list();
        let size = list.measure(theme::face::ZOOM_LABEL, &text);
        let centre = rect.max[0] - 10.0 - theme::MINIMAP_WIDTH / 2.0;
        list.text_with_face(
            [
                centre - size[0] / 2.0,
                rect.max[1] - theme::ZOOM_LABEL_BOTTOM - size[1],
            ],
            theme::ZOOM_LABEL_COLOR,
            theme::face::ZOOM_LABEL,
            &text,
        );
    }

    fn draw_minimap(&self, ui: &Ui, rect: Rect, placed: &[Placed]) {
        if placed.is_empty() {
            return;
        }
        let list = ui.draw_list();
        let width = theme::MINIMAP_WIDTH;
        let height = theme::MINIMAP_HEIGHT;
        let min = [
            rect.max[0] - width - 10.0,
            rect.max[1] - height - theme::ZOOM_LABEL_BOTTOM - 16.0,
        ];
        let max = [min[0] + width, min[1] + height];
        let hovered = controls::point_in(ui.mouse_position(), min, max);
        let alpha = if hovered { 1.0 } else { theme::OVERLAY_OPACITY };
        list.rect(
            min,
            max,
            theme::PANEL_HEADER_BG.with_alpha(0.85 * alpha),
            theme::MINIMAP_RADIUS,
            Rounding::All,
        );

        let mut bounds: Option<Bounds> = None;
        for node in placed {
            bounds = Some(match bounds {
                Some(current) => current.union(node.bounds()),
                None => node.bounds(),
            });
        }
        let Some(bounds) = bounds else { return };
        let span = [
            (bounds.max[0] - bounds.min[0]).max(1.0),
            (bounds.max[1] - bounds.min[1]).max(1.0),
        ];
        let pad = 8.0;
        let scale = ((width - pad * 2.0) / span[0]).min((height - pad * 2.0) / span[1]);
        let project = |point: Vec2| {
            [
                min[0] + pad + (point[0] - bounds.min[0]) * scale,
                min[1] + pad + (point[1] - bounds.min[1]) * scale,
            ]
        };
        for node in placed {
            let node_bounds = node.bounds();
            let colour = if node.is_comment {
                theme::COMMENT_HEADER_BG
            } else if node.is_group {
                theme::GROUP_BORDER
            } else {
                header_color(&node.kind, &node.label)
            };
            list.rect(
                project(node_bounds.min),
                project(node_bounds.max),
                colour.with_alpha(alpha),
                5.0 * scale.min(1.0),
                Rounding::All,
            );
        }
        // The viewport mask shows which part of the graph the canvas is showing.
        let view_min = self.graph_point(rect, rect.min);
        let view_max = self.graph_point(rect, rect.max);
        list.rect_outline(
            project(view_min),
            project(view_max),
            theme::TEXT_BRIGHT.with_alpha(0.6 * alpha),
            2.0,
            Rounding::All,
            1.0,
        );
    }

    fn accept_drop(&self, ui: &mut Ui, rect: Rect) -> Option<Action> {
        let payload = ui.drag_target("bite-node")?;
        let position = self.graph_point(rect, ui.mouse_position());
        Some(Action::DropDefinition { payload, position })
    }

    /// The gesture state machine.
    fn handle_input(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        graph: &Graph,
        placed: &[Placed],
        context: &CanvasContext,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let pointer = ui.mouse_position();
        let inside = rect.contains(pointer) && ui.window_hovered();
        if inside {
            self.state.last_pointer = self.graph_point(rect, pointer);
        }

        self.update_hover(ui, rect, graph, placed, inside);

        if inside {
            let wheel = ui.mouse_wheel()[1];
            if wheel.abs() > 0.0 {
                let factor = theme::ZOOM_STEP.powf(wheel);
                let local = [pointer[0] - rect.min[0], pointer[1] - rect.min[1]];
                self.state.viewport.zoom_about(local, factor);
            }
        }

        match self.state.gesture.clone() {
            Gesture::Idle => {
                if inside {
                    actions.extend(self.begin_gesture(ui, rect, placed, pointer, context));
                }
            }
            Gesture::Panning => {
                let holding =
                    ui.mouse_down(MouseButton::Left) || ui.mouse_down(MouseButton::Middle);
                if holding {
                    let button = if ui.mouse_down(MouseButton::Left) {
                        MouseButton::Left
                    } else {
                        MouseButton::Middle
                    };
                    let delta = ui.mouse_drag_delta(button, 0.0);
                    self.state.viewport.pan(delta);
                    ui.reset_mouse_drag_delta(button);
                    ui.set_mouse_cursor(MouseCursor::ResizeAll);
                } else {
                    self.state.gesture = Gesture::Idle;
                }
            }
            Gesture::MovingNodes { origin, started } => {
                if ui.mouse_down(MouseButton::Left) {
                    if !started && CanvasState::drag_passed_threshold(origin, pointer) {
                        self.state.gesture = Gesture::MovingNodes {
                            origin,
                            started: true,
                        };
                        actions.push(Action::BeginDrag);
                    }
                    if matches!(self.state.gesture, Gesture::MovingNodes { started: true, .. }) {
                        let start_graph = self.graph_point(rect, origin);
                        let now_graph = self.graph_point(rect, pointer);
                        let delta = [
                            now_graph[0] - start_graph[0],
                            now_graph[1] - start_graph[1],
                        ];
                        let moves = self
                            .drag_start
                            .iter()
                            .map(|(id, start)| {
                                (id.clone(), [start[0] + delta[0], start[1] + delta[1]])
                            })
                            .collect::<Vec<_>>();
                        if !moves.is_empty() {
                            actions.push(Action::MoveNodes(moves));
                        }
                        ui.set_mouse_cursor(MouseCursor::ResizeAll);
                    }
                } else {
                    if started {
                        actions.push(Action::EndDrag);
                    }
                    self.drag_start.clear();
                    self.state.gesture = Gesture::Idle;
                }
            }
            Gesture::RubberBand { origin, .. } => {
                if ui.mouse_down(MouseButton::Left) {
                    self.state.gesture = Gesture::RubberBand {
                        origin,
                        current: pointer,
                    };
                    let band = Bounds::from_corners(
                        self.graph_point(rect, origin),
                        self.graph_point(rect, pointer),
                    );
                    let bounds: Vec<(String, Bounds)> = placed
                        .iter()
                        .map(|node| (node.id.clone(), node.bounds()))
                        .collect();
                    self.state.apply_rubber_band(band, &bounds, false);
                } else {
                    self.state.gesture = Gesture::Idle;
                    actions.push(Action::SelectionChanged);
                }
            }
            Gesture::Connecting(pending) => {
                if !ui.mouse_down(MouseButton::Left) {
                    actions.extend(self.finish_connection(ui, rect, placed, &pending));
                    self.state.gesture = Gesture::Idle;
                }
            }
            Gesture::Resizing { node, origin, start } => {
                if ui.mouse_down(MouseButton::Left) {
                    let start_graph = self.graph_point(rect, origin);
                    let now_graph = self.graph_point(rect, pointer);
                    let (min_width, min_height) = resize_minimum(placed, &node);
                    actions.push(Action::ResizeNode {
                        id: node.clone(),
                        width: (start[0] + now_graph[0] - start_graph[0]).max(min_width),
                        height: (start[1] + now_graph[1] - start_graph[1]).max(min_height),
                    });
                    ui.set_mouse_cursor(MouseCursor::ResizeNorthWestSouthEast);
                } else {
                    self.state.gesture = Gesture::Idle;
                    actions.push(Action::EndDrag);
                }
            }
        }
        actions
    }

    fn update_hover(
        &mut self,
        ui: &Ui,
        rect: Rect,
        graph: &Graph,
        placed: &[Placed],
        inside: bool,
    ) {
        self.state.hovered_node = None;
        self.state.hovered_port = None;
        self.state.hovered_edge = None;
        if !inside {
            return;
        }
        let pointer = ui.mouse_position();
        // Ports take priority so that a wire can start from a card's edge.
        let viewport = self.state.viewport;
        let ports: Vec<(&str, &str, Vec2)> = placed
            .iter()
            .flat_map(|node| {
                node.card.ports.iter().map(move |port| {
                    let x = if port.output {
                        node.position[0] + node.card.width
                    } else {
                        node.position[0]
                    };
                    (
                        node.id.as_str(),
                        port.handle.as_str(),
                        to_screen(rect, viewport, [x, node.position[1] + port.offset_y]),
                    )
                })
            })
            .collect();
        if let Some((node, handle)) = nearest_port(pointer, ports, self.state.viewport.zoom) {
            self.state.hovered_port = Some((node.to_string(), handle.to_string()));
        }
        // Later nodes draw on top, so hit testing walks the list backwards.
        for node in placed.iter().rev() {
            let min = self.screen(rect, node.position);
            let max = self.screen(
                rect,
                [
                    node.position[0] + node.card.width,
                    node.position[1] + node.card.height,
                ],
            );
            let extended_min = if node.is_group {
                [min[0], min[1] - 36.0 * self.state.viewport.zoom]
            } else {
                min
            };
            if controls::point_in(pointer, extended_min, max) {
                self.state.hovered_node = Some(node.id.clone());
                break;
            }
        }
        if self.state.hovered_node.is_none() && self.state.hovered_port.is_none() {
            let by_id: BTreeMap<&str, &Placed> =
                placed.iter().map(|node| (node.id.as_str(), node)).collect();
            for edge in &graph.edges {
                let (Some(source), Some(target)) = (
                    by_id.get(edge.source.as_str()),
                    by_id.get(edge.target.as_str()),
                ) else {
                    continue;
                };
                let (Some(from), Some(to)) = (
                    self.port_position(rect, source, &edge.source_handle),
                    self.port_position(rect, target, &edge.target_handle),
                ) else {
                    continue;
                };
                if wire::distance_to(wire::bezier_points(from, to), pointer) <= 6.0 {
                    self.state.hovered_edge = Some(edge.id.clone());
                    break;
                }
            }
        }
    }

    fn begin_gesture(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        placed: &[Placed],
        pointer: Vec2,
        _context: &CanvasContext,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let additive = ui.primary_modifier();

        // Right click opens the creation menu on empty canvas or on a group.
        if ui.mouse_clicked(MouseButton::Right) {
            let on_group = self
                .state
                .hovered_node
                .as_ref()
                .and_then(|id| placed.iter().find(|node| node.id == *id))
                .is_some_and(|node| node.is_group);
            if self.state.hovered_node.is_none() || on_group {
                actions.push(Action::OpenCreateMenu {
                    position: self.graph_point(rect, pointer),
                    pending: None,
                });
            }
            return actions;
        }

        if ui.mouse_double_clicked(MouseButton::Left) {
            if let Some(id) = self.state.hovered_node.clone() {
                if let Some(node) = placed.iter().find(|node| node.id == id) {
                    let excluded = node.is_comment
                        || matches!(
                            node.kind,
                            NodeKind::Builtin(
                                BuiltinNodeKind::Input
                                    | BuiltinNodeKind::ImageOutput
                                    | BuiltinNodeKind::TextOutput
                                    | BuiltinNodeKind::FlipbookOutput
                            )
                        );
                    if !excluded && !node.is_group && node.enabled {
                        actions.push(Action::TogglePreview(id));
                    }
                }
            } else {
                // Double clicking empty canvas resets the zoom while keeping the pan.
                self.state.viewport.zoom = 1.0;
            }
            return actions;
        }

        if !ui.mouse_clicked(MouseButton::Left) && !ui.mouse_clicked(MouseButton::Middle) {
            return actions;
        }

        if ui.mouse_clicked(MouseButton::Middle) {
            self.state.gesture = Gesture::Panning;
            return actions;
        }

        // Starting a wire from a port takes priority over selecting the card.
        if let Some((node_id, handle)) = self.state.hovered_port.clone() {
            if let Some(node) = placed.iter().find(|node| node.id == node_id) {
                if let Some(port) = node.card.port(&handle) {
                    let x = if port.output {
                        node.position[0] + node.card.width
                    } else {
                        node.position[0]
                    };
                    self.state.gesture = Gesture::Connecting(PendingWire {
                        node: node_id,
                        handle,
                        end: if port.output {
                            WireEnd::Source
                        } else {
                            WireEnd::Target
                        },
                        origin: [x, node.position[1] + port.offset_y],
                        wire: port.wire,
                    });
                    return actions;
                }
            }
        }

        if let Some(id) = self.state.hovered_node.clone() {
            let node = placed.iter().find(|node| node.id == id);
            // A group or comment resizes from its bottom right corner while selected.
            if let Some(node) = node.filter(|node| node.is_group || node.is_comment) {
                if self.state.is_selected(&node.id) {
                    let max = self.screen(
                        rect,
                        [
                            node.position[0] + node.card.width,
                            node.position[1] + node.card.height,
                        ],
                    );
                    if controls::point_in(pointer, [max[0] - 14.0, max[1] - 14.0], max) {
                        self.state.gesture = Gesture::Resizing {
                            node: node.id.clone(),
                            origin: pointer,
                            start: [node.card.width, node.card.height],
                        };
                        actions.push(Action::BeginDrag);
                        return actions;
                    }
                }
            }

            if let Some(node) = node {
                if node.card.has_bypass_toggle {
                    let min = self.screen(rect, node.position);
                    let zoom = self.state.viewport.zoom;
                    let size = 14.0 * zoom;
                    let tick = [
                        min[0] + theme::NODE_ROW_INSET * zoom,
                        min[1] + (node.card.header_height * zoom - size) / 2.0,
                    ];
                    if controls::point_in(pointer, tick, [tick[0] + size, tick[1] + size]) {
                        actions.push(Action::ToggleBypass(node.id.clone()));
                        return actions;
                    }
                }
            }

            self.state.click_node(&id, additive);
            actions.push(Action::SelectionChanged);
            self.drag_start = self
                .state
                .selected_nodes
                .iter()
                .filter_map(|selected| {
                    placed
                        .iter()
                        .find(|node| node.id == *selected)
                        .map(|node| (node.id.clone(), node.position))
                })
                .collect();
            self.state.gesture = Gesture::MovingNodes {
                origin: pointer,
                started: false,
            };
            return actions;
        }

        if let Some(edge) = self.state.hovered_edge.clone() {
            self.state.click_edge(&edge, additive);
            actions.push(Action::SelectionChanged);
            return actions;
        }

        // Empty canvas: shift starts a rubber band, otherwise the canvas pans.
        if ui.shift_down() {
            self.state.gesture = Gesture::RubberBand {
                origin: pointer,
                current: pointer,
            };
        } else {
            self.state.clear_selection();
            actions.push(Action::SelectionChanged);
            self.state.gesture = Gesture::Panning;
        }
        actions
    }

    fn finish_connection(
        &mut self,
        ui: &Ui,
        rect: Rect,
        placed: &[Placed],
        pending: &PendingWire,
    ) -> Vec<Action> {
        let pointer = ui.mouse_position();
        let viewport = self.state.viewport;
        let wants_output = pending.end == WireEnd::Target;
        let ports: Vec<(&str, &str, Vec2)> = placed
            .iter()
            .filter(|node| node.id != pending.node)
            .flat_map(|node| {
                node.card.ports.iter().filter_map(move |port| {
                    // Only the opposite end of a wire is a valid drop target.
                    if port.output != wants_output {
                        return None;
                    }
                    let x = if port.output {
                        node.position[0] + node.card.width
                    } else {
                        node.position[0]
                    };
                    Some((
                        node.id.as_str(),
                        port.handle.as_str(),
                        to_screen(rect, viewport, [x, node.position[1] + port.offset_y]),
                    ))
                })
            })
            .collect();

        match nearest_port(pointer, ports, self.state.viewport.zoom) {
            Some((node, handle)) => {
                let action = if pending.end == WireEnd::Source {
                    Action::Connect {
                        source: pending.node.clone(),
                        source_handle: pending.handle.clone(),
                        target: node.to_string(),
                        target_handle: handle.to_string(),
                    }
                } else {
                    Action::Connect {
                        source: node.to_string(),
                        source_handle: handle.to_string(),
                        target: pending.node.clone(),
                        target_handle: pending.handle.clone(),
                    }
                };
                vec![action]
            }
            // Dropping on empty canvas offers to create a compatible node there.
            None if rect.contains(pointer) => vec![Action::OpenCreateMenu {
                position: self.graph_point(rect, pointer),
                pending: Some(pending.clone()),
            }],
            None => Vec::new(),
        }
    }
}

/// Projects a graph point into `rect` using `viewport`, without borrowing the canvas.
fn to_screen(rect: Rect, viewport: Viewport, point: Vec2) -> Vec2 {
    let local = viewport.to_screen(point);
    [rect.min[0] + local[0], rect.min[1] + local[1]]
}

/// The minimum size a group or comment may be dragged down to.
fn resize_minimum(placed: &[Placed], id: &str) -> (f32, f32) {
    match placed.iter().find(|node| node.id == id) {
        Some(node) if node.is_comment => (210.0, 80.0),
        _ => (120.0, 80.0),
    }
}

fn draw_badge(ui: &Ui, centre: Vec2, text: &str, color: Color, subtitle: Option<&str>) {
    let list = ui.draw_list();
    let size = controls::measure_tracked(ui, theme::face::BADGE, text, 0.08 * 11.0);
    let padding = [7.0, 2.0];
    let min = [
        centre[0] - size[0] / 2.0 - padding[0],
        centre[1] - size[1] / 2.0 - padding[1],
    ];
    let max = [
        centre[0] + size[0] / 2.0 + padding[0],
        centre[1] + size[1] / 2.0 + padding[1],
    ];
    list.rect(min, max, theme::BG.with_alpha(0.85), 3.0, Rounding::All);
    list.rect_outline(min, max, color, 3.0, Rounding::All, 1.0);
    controls::draw_tracked_text(
        ui,
        [min[0] + padding[0], min[1] + padding[1]],
        color,
        theme::face::BADGE,
        text,
        0.08 * 11.0,
    );
    if let Some(subtitle) = subtitle {
        controls::draw_ellipsized(
            ui,
            [centre[0] - 90.0, min[1] - 13.0],
            theme::TEXT.with_alpha(0.75),
            theme::face::TINY_MONO,
            subtitle,
            180.0,
        );
    }
}

/// The header tint for a node kind, from the `--node-accent-*` tokens.
pub fn header_color(kind: &NodeKind, _label: &str) -> Color {
    match kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => {
            theme::node_header(theme::NODE_ACCENT_INPUT, 18.0)
        }
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            theme::node_header(theme::NODE_ACCENT_IMAGE_OUTPUT, 18.0)
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            theme::node_header(theme::NODE_ACCENT_TEXT_OUTPUT, 18.0)
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
            theme::node_header(theme::NODE_ACCENT_FLIPBOOK_OUTPUT, 18.0)
        }
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => {
            theme::node_header(theme::NODE_ACCENT_FOLDER_PATH, 18.0)
        }
        NodeKind::Processing(ProcessingNodeKind::SetInput) => {
            theme::node_header(theme::NODE_ACCENT_SET_INPUT, 20.0)
        }
        _ => theme::NODE_HEAD_BG,
    }
}

/// The card title, which the built-in kinds name for themselves.
pub fn card_label(node: &GraphNode, registry: &Registry) -> String {
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => {
            if node.data.label.is_empty() {
                "Input".into()
            } else {
                node.data.label.clone()
            }
        }
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => "Image Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => "Text Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => "Flipbook Output".into(),
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => "Folder Path".into(),
        NodeKind::Builtin(BuiltinNodeKind::Comment) => match node.data.params.get("heading") {
            Some(ParamValue::String(text)) if !text.is_empty() => text.clone(),
            _ => "Comment".into(),
        },
        NodeKind::Builtin(BuiltinNodeKind::Group) => match node.data.params.get("name") {
            Some(ParamValue::String(text)) if !text.is_empty() => text.clone(),
            _ => "Group".into(),
        },
        NodeKind::Processing(ProcessingNodeKind::SetInput) => "Process As Set".into(),
        _ => registry
            .nodes
            .get(&node.data.definition_id)
            .map(|entry| entry.definition.label.clone())
            .unwrap_or_else(|| node.data.label.clone()),
    }
}

/// The footer line each built-in card shows.
fn footer_text(node: &GraphNode, context: &CanvasContext) -> Option<String> {
    match &node.kind {
        NodeKind::Builtin(BuiltinNodeKind::Input) => {
            let count = context.image_counts.get(&node.id).copied().unwrap_or(0);
            Some(match count {
                0 => "no images".to_string(),
                1 => "1 image".to_string(),
                many => format!("{many} images"),
            })
        }
        NodeKind::Builtin(BuiltinNodeKind::ImageOutput) => {
            Some(match node.data.params.get("outputPath") {
                Some(ParamValue::String(mode)) if mode == "source" => {
                    "same folder as source".to_string()
                }
                Some(ParamValue::String(mode)) if mode == "custom" => {
                    match node.data.params.get("customPath") {
                        Some(ParamValue::String(path)) if !path.is_empty() => path.clone(),
                        _ => "no path set".to_string(),
                    }
                }
                Some(ParamValue::String(other)) => other.clone(),
                _ => "same folder as source".to_string(),
            })
        }
        NodeKind::Builtin(BuiltinNodeKind::TextOutput) => {
            Some(match node.data.params.get("outputPath") {
                Some(ParamValue::String(path)) if !path.is_empty() => path.clone(),
                _ => "no output file set".to_string(),
            })
        }
        NodeKind::Builtin(BuiltinNodeKind::FlipbookOutput) => {
            let number = |name: &str, fallback: i64| match node.data.params.get(name) {
                Some(ParamValue::Int(value)) => *value,
                Some(ParamValue::Number(value)) => *value as i64,
                _ => fallback,
            };
            Some(format!(
                "{} x {} grid",
                number("cols", 4),
                number("rows", 4)
            ))
        }
        NodeKind::Builtin(BuiltinNodeKind::FolderPath) => {
            Some(match node.data.params.get("folderPath") {
                Some(ParamValue::String(path)) if !path.is_empty() => path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(path)
                    .to_string(),
                _ => "not set".to_string(),
            })
        }
        NodeKind::Processing(ProcessingNodeKind::SetInput) => Some(set_footer(node, context)),
        _ => None,
    }
}

/// The matched-set count the Process As Set card shows.
fn set_footer(node: &GraphNode, _context: &CanvasContext) -> String {
    let suffixes = match node.data.params.get("suffixes") {
        Some(ParamValue::Structured(bite_schema::StructuredParam::SetSuffixes { suffixes })) => {
            suffixes.clone()
        }
        _ => Vec::new(),
    };
    if suffixes.iter().all(|suffix| suffix.is_empty()) {
        return "no sets matched".into();
    }
    "no sets matched".into()
}

/// Whether a node is active, following the truthiness rule the Svelte helper applies.
pub fn node_enabled(node: &GraphNode, graph: &Graph, _registry: &Registry) -> bool {
    let wired = graph
        .edges
        .iter()
        .find(|edge| edge.target == node.id && edge.target_handle == "param:_enabled");
    if let Some(edge) = wired {
        let Some(source) = graph.nodes.iter().find(|candidate| candidate.id == edge.source) else {
            return true;
        };
        let Some(name) = edge.source_handle.strip_prefix("param:") else {
            return true;
        };
        return match source.data.params.get(name) {
            Some(ParamValue::Bool(false)) => false,
            Some(ParamValue::Int(0)) => false,
            Some(ParamValue::Number(value)) if *value == 0.0 => false,
            Some(ParamValue::Null) | None => false,
            _ => true,
        };
    }
    // Only an explicit false or zero disables a node; a missing value means enabled.
    !matches!(
        node.data.params.get("_enabled"),
        Some(ParamValue::Bool(false)) | Some(ParamValue::Int(0))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bite_schema::{NodeData, Position};

    fn node(id: &str, kind: NodeKind) -> GraphNode {
        GraphNode {
            id: id.into(),
            kind,
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

    fn graph_with(nodes: Vec<GraphNode>, edges: Vec<bite_schema::GraphEdge>) -> Graph {
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

    #[test]
    fn header_tints_differ_per_built_in_kind() {
        let input = header_color(&NodeKind::Builtin(BuiltinNodeKind::Input), "");
        let output = header_color(&NodeKind::Builtin(BuiltinNodeKind::ImageOutput), "");
        let process = header_color(&NodeKind::Processing(ProcessingNodeKind::Process), "");
        assert_ne!(input, output);
        assert_eq!(process, theme::NODE_HEAD_BG);
    }

    #[test]
    fn a_missing_enabled_parameter_means_the_node_is_active() {
        let node = node("a", NodeKind::Processing(ProcessingNodeKind::Process));
        let graph = graph_with(vec![node.clone()], Vec::new());
        let registry = Registry::default();
        assert!(node_enabled(&node, &graph, &registry));
    }

    #[test]
    fn an_explicit_false_bypasses_the_node() {
        let mut node = node("a", NodeKind::Processing(ProcessingNodeKind::Process));
        node.data
            .params
            .insert("_enabled".into(), ParamValue::Bool(false));
        let graph = graph_with(vec![node.clone()], Vec::new());
        assert!(!node_enabled(&node, &graph, &Registry::default()));
    }

    #[test]
    fn a_wired_bypass_port_overrides_the_manual_state() {
        let mut target = node("b", NodeKind::Processing(ProcessingNodeKind::Process));
        target
            .data
            .params
            .insert("_enabled".into(), ParamValue::Bool(true));
        let mut source = node("a", NodeKind::Processing(ProcessingNodeKind::Process));
        source
            .data
            .params
            .insert("value".into(), ParamValue::Bool(false));
        let edge = bite_schema::GraphEdge {
            id: "e".into(),
            source: "a".into(),
            source_handle: "param:value".into(),
            target: "b".into(),
            target_handle: "param:_enabled".into(),
        };
        let graph = graph_with(vec![source, target.clone()], vec![edge]);
        assert!(!node_enabled(&target, &graph, &Registry::default()));
    }

    #[test]
    fn built_in_cards_carry_their_own_titles() {
        let registry = Registry::default();
        assert_eq!(
            card_label(&node("a", NodeKind::Builtin(BuiltinNodeKind::ImageOutput)), &registry),
            "Image Output"
        );
        assert_eq!(
            card_label(
                &node("a", NodeKind::Processing(ProcessingNodeKind::SetInput)),
                &registry
            ),
            "Process As Set"
        );
    }
}
