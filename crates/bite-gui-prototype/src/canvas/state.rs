//! Canvas interaction state: selection, drags, rubber band and the pending connection.
//!
//! Everything here is pure so that the gesture rules can be tested without a window.
use super::viewport::Viewport;
use crate::theme;
use bite_imgui::Vec2;
use std::collections::BTreeSet;

/// A rectangle in graph coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: [a[0].min(b[0]), a[1].min(b[1])],
            max: [a[0].max(b[0]), a[1].max(b[1])],
        }
    }

    pub fn contains(self, point: Vec2) -> bool {
        point[0] >= self.min[0]
            && point[0] <= self.max[0]
            && point[1] >= self.min[1]
            && point[1] <= self.max[1]
    }

    /// True when the two rectangles overlap at all, which is the partial selection mode
    /// Svelte Flow uses: touching a node selects it.
    pub fn intersects(self, other: Self) -> bool {
        self.min[0] <= other.max[0]
            && self.max[0] >= other.min[0]
            && self.min[1] <= other.max[1]
            && self.max[1] >= other.min[1]
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])],
            max: [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])],
        }
    }
}

/// Which end of a connection the pointer started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireEnd {
    Source,
    Target,
}

/// A connection being dragged from a port.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingWire {
    pub node: String,
    pub handle: String,
    pub end: WireEnd,
    /// The port's position in graph coordinates, where the curve starts.
    pub origin: Vec2,
    pub wire: bite_core::graph::WireType,
}

/// What the pointer is currently doing on the canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    Idle,
    /// Panning with the left or middle button held on empty canvas.
    Panning,
    /// Dragging the selected nodes. Holds the pointer position where the drag began.
    MovingNodes {
        origin: Vec2,
        started: bool,
    },
    /// Shift and drag on empty canvas.
    RubberBand {
        origin: Vec2,
        current: Vec2,
    },
    /// Dragging a wire out of a port.
    Connecting(PendingWire),
    /// Resizing a group or comment from its corner.
    Resizing {
        node: String,
        origin: Vec2,
        start: Vec2,
    },
}

/// The canvas view state that survives between frames.
pub struct CanvasState {
    pub viewport: Viewport,
    pub selected_nodes: BTreeSet<String>,
    pub selected_edges: BTreeSet<String>,
    pub gesture: Gesture,
    /// The node the pointer is over, used for hover styling and the header tooltip.
    pub hovered_node: Option<String>,
    pub hovered_port: Option<(String, String)>,
    pub hovered_edge: Option<String>,
    /// The last pointer position in graph coordinates, where Space and Tab place a node.
    pub last_pointer: Vec2,
    /// How long the pointer has rested on the hovered node header.
    pub hover_elapsed: f32,
    /// Set when a node must be scrolled into view after being created or pasted.
    pub focus_request: Option<String>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            viewport: Viewport::default(),
            selected_nodes: BTreeSet::new(),
            selected_edges: BTreeSet::new(),
            gesture: Gesture::Idle,
            hovered_node: None,
            hovered_port: None,
            hovered_edge: None,
            last_pointer: [0.0, 0.0],
            hover_elapsed: 0.0,
            focus_request: None,
        }
    }
}

impl CanvasState {
    /// Applies a click on a node, following the modifier rules the editor uses: a plain
    /// click selects one node, and the platform modifier adds to or removes from the set.
    pub fn click_node(&mut self, id: &str, additive: bool) {
        self.selected_edges.clear();
        if additive {
            if !self.selected_nodes.remove(id) {
                self.selected_nodes.insert(id.to_string());
            }
        } else if !self.selected_nodes.contains(id) {
            self.selected_nodes.clear();
            self.selected_nodes.insert(id.to_string());
        }
    }

    pub fn click_edge(&mut self, id: &str, additive: bool) {
        self.selected_nodes.clear();
        if additive {
            if !self.selected_edges.remove(id) {
                self.selected_edges.insert(id.to_string());
            }
        } else {
            self.selected_edges.clear();
            self.selected_edges.insert(id.to_string());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_nodes.clear();
        self.selected_edges.clear();
    }

    pub fn select_only(&mut self, ids: impl IntoIterator<Item = String>) {
        self.selected_nodes = ids.into_iter().collect();
        self.selected_edges.clear();
    }

    /// Replaces the selection with every node the rubber band touches.
    pub fn apply_rubber_band(&mut self, band: Bounds, nodes: &[(String, Bounds)], additive: bool) {
        if !additive {
            self.selected_nodes.clear();
        }
        self.selected_edges.clear();
        for (id, bounds) in nodes {
            if band.intersects(*bounds) {
                self.selected_nodes.insert(id.clone());
            }
        }
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selected_nodes.contains(id)
    }

    /// True once a node drag has passed the one-pixel threshold Svelte Flow applies.
    pub fn drag_passed_threshold(origin: Vec2, current: Vec2) -> bool {
        let dx = current[0] - origin[0];
        let dy = current[1] - origin[1];
        (dx * dx + dy * dy).sqrt() >= theme::NODE_DRAG_THRESHOLD
    }
}

/// Finds the port nearest `point` within the twenty-pixel snap radius Svelte Flow uses.
pub fn nearest_port<'a>(
    point: Vec2,
    ports: impl IntoIterator<Item = (&'a str, &'a str, Vec2)>,
    zoom: f32,
) -> Option<(&'a str, &'a str)> {
    let radius = theme::CONNECTION_RADIUS * zoom.max(0.01);
    let mut best: Option<(f32, &str, &str)> = None;
    for (node, handle, position) in ports {
        let distance =
            ((point[0] - position[0]).powi(2) + (point[1] - position[1]).powi(2)).sqrt();
        if distance <= radius && best.is_none_or(|(current, _, _)| distance < current) {
            best = Some((distance, node, handle));
        }
    }
    best.map(|(_, node, handle)| (node, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds {
        Bounds {
            min: [x, y],
            max: [x + width, y + height],
        }
    }

    #[test]
    fn a_plain_click_replaces_the_selection() {
        let mut state = CanvasState::default();
        state.click_node("a", false);
        state.click_node("b", false);
        assert_eq!(state.selected_nodes.len(), 1);
        assert!(state.is_selected("b"));
    }

    #[test]
    fn the_modifier_adds_and_removes_from_the_selection() {
        let mut state = CanvasState::default();
        state.click_node("a", false);
        state.click_node("b", true);
        assert_eq!(state.selected_nodes.len(), 2);
        state.click_node("a", true);
        assert_eq!(state.selected_nodes.len(), 1);
        assert!(state.is_selected("b"));
    }

    #[test]
    fn clicking_a_node_already_in_a_multiple_selection_keeps_it() {
        let mut state = CanvasState::default();
        state.select_only(["a".into(), "b".into()]);
        state.click_node("a", false);
        // The whole selection has to survive so that the group can be dragged together.
        assert_eq!(state.selected_nodes.len(), 2);
    }

    #[test]
    fn the_rubber_band_selects_every_node_it_touches() {
        let mut state = CanvasState::default();
        let nodes = vec![
            ("a".to_string(), bounds(0.0, 0.0, 150.0, 60.0)),
            ("b".to_string(), bounds(400.0, 0.0, 150.0, 60.0)),
            ("c".to_string(), bounds(140.0, 50.0, 150.0, 60.0)),
        ];
        let band = Bounds::from_corners([10.0, 10.0], [200.0, 80.0]);
        state.apply_rubber_band(band, &nodes, false);
        assert!(state.is_selected("a"));
        assert!(state.is_selected("c"), "touching a corner must select");
        assert!(!state.is_selected("b"));
    }

    #[test]
    fn selecting_a_node_clears_any_selected_wire() {
        let mut state = CanvasState::default();
        state.click_edge("edge-1", false);
        assert_eq!(state.selected_edges.len(), 1);
        state.click_node("a", false);
        assert!(state.selected_edges.is_empty());
    }

    #[test]
    fn a_drag_needs_to_pass_one_pixel_before_it_counts() {
        assert!(!CanvasState::drag_passed_threshold(
            [10.0, 10.0],
            [10.5, 10.0]
        ));
        assert!(CanvasState::drag_passed_threshold(
            [10.0, 10.0],
            [11.5, 10.0]
        ));
    }

    #[test]
    fn port_snapping_prefers_the_nearest_port_inside_the_radius() {
        let ports = vec![
            ("a", "out:output", [100.0, 100.0]),
            ("b", "in:input", [108.0, 100.0]),
        ];
        let found = nearest_port([106.0, 100.0], ports.clone(), 1.0);
        assert_eq!(found, Some(("b", "in:input")));
        assert_eq!(nearest_port([400.0, 400.0], ports, 1.0), None);
    }

    #[test]
    fn the_snap_radius_follows_the_zoom() {
        let ports = vec![("a", "out:output", [100.0, 100.0])];
        // Thirty pixels away is outside the radius at one hundred percent and inside at two.
        assert_eq!(nearest_port([130.0, 100.0], ports.clone(), 1.0), None);
        assert!(nearest_port([130.0, 100.0], ports, 2.0).is_some());
    }

    #[test]
    fn bounds_union_covers_both_rectangles() {
        let combined = bounds(0.0, 0.0, 10.0, 10.0).union(bounds(100.0, 50.0, 10.0, 10.0));
        assert_eq!(combined.min, [0.0, 0.0]);
        assert_eq!(combined.max, [110.0, 60.0]);
    }
}
