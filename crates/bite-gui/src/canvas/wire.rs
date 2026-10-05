//! Wire colors and the bezier geometry Svelte Flow draws between ports.
use crate::theme;
use bite_core::graph::WireType;
use bite_imgui::{Color, Vec2};

/// The stroke color for a wire type, from the `--port-color-*` tokens.
pub fn color(wire: WireType) -> Color {
    match wire {
        WireType::Image => theme::PORT_COLOR_IMAGE,
        WireType::Mask => theme::PORT_COLOR_MASK,
        WireType::Number => theme::PORT_COLOR_NUMBER,
        WireType::String => theme::PORT_COLOR_STRING,
        WireType::Bool => theme::PORT_COLOR_BOOLEAN,
        WireType::Color => theme::PORT_COLOR_COLOR,
        WireType::Vector2 => theme::PORT_COLOR_VECTOR2,
        WireType::Vector3 => theme::PORT_COLOR_VECTOR3,
        WireType::Vector4 => theme::PORT_COLOR_VECTOR4,
        WireType::Numeric => theme::PORT_COLOR_NUMERIC,
        WireType::Path => theme::PORT_COLOR_PATH,
        WireType::Value => theme::PORT_COLOR_ANY,
    }
}

/// The four points of the cubic curve between an output on the left and an input on the
/// right, using the curvature Svelte Flow applies to a bezier edge.
pub fn bezier_points(from: Vec2, to: Vec2) -> [Vec2; 4] {
    const CURVATURE: f32 = 0.25;
    // Svelte Flow's `calculateControlOffset`: a forward wire puts its handles halfway
    // along the span, which is what gives the Electron edges their bow. A backwards wire
    // falls back to the curvature term so it bulges out instead of doubling back through
    // the cards.
    let distance = (to[0] - from[0]).abs();
    let offset = if to[0] >= from[0] {
        distance * 0.5
    } else {
        CURVATURE * 25.0 * distance.sqrt()
    };
    [
        from,
        [from[0] + offset, from[1]],
        [to[0] - offset, to[1]],
        to,
    ]
}

/// Samples a cubic curve at `t`.
pub fn sample(points: [Vec2; 4], t: f32) -> Vec2 {
    let inverse = 1.0 - t;
    let a = inverse * inverse * inverse;
    let b = 3.0 * inverse * inverse * t;
    let c = 3.0 * inverse * t * t;
    let d = t * t * t;
    [
        a * points[0][0] + b * points[1][0] + c * points[2][0] + d * points[3][0],
        a * points[0][1] + b * points[1][1] + c * points[2][1] + d * points[3][1],
    ]
}

/// The distance from `point` to the curve, used to decide whether a click selected a wire.
pub fn distance_to(points: [Vec2; 4], point: Vec2) -> f32 {
    const STEPS: usize = 24;
    let mut best = f32::MAX;
    let mut previous = sample(points, 0.0);
    for step in 1..=STEPS {
        let current = sample(points, step as f32 / STEPS as f32);
        best = best.min(distance_to_segment(point, previous, current));
        previous = current;
    }
    best
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = [end[0] - start[0], end[1] - start[1]];
    let length_squared = segment[0] * segment[0] + segment[1] * segment[1];
    if length_squared <= f32::EPSILON {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }
    let t = (((point[0] - start[0]) * segment[0] + (point[1] - start[1]) * segment[1])
        / length_squared)
        .clamp(0.0, 1.0);
    let closest = [start[0] + segment[0] * t, start[1] + segment[1] * t];
    ((point[0] - closest[0]).powi(2) + (point[1] - closest[1]).powi(2)).sqrt()
}

/// The dashed preview drawn while a wire is being dragged, from the creation menu code.
pub fn preview_points(from: Vec2, to: Vec2) -> [Vec2; 4] {
    let control = ((to[0] - from[0]).abs() * 0.4).max(50.0);
    [
        from,
        [from[0] + control, from[1]],
        [to[0] - control, to[1]],
        to,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_colors_come_from_the_stylesheet_tokens() {
        assert_eq!(color(WireType::Image), Color::from_hex("#ff8c3f"));
        assert_eq!(color(WireType::Number), Color::from_hex("#22d3ee"));
        assert_eq!(color(WireType::Bool), Color::from_hex("#eab308"));
        assert_eq!(color(WireType::Value), Color::from_hex("#ffffff"));
        assert_eq!(color(WireType::Path), Color::from_hex("#86efac"));
    }

    #[test]
    fn a_forward_wire_leaves_the_source_going_right() {
        let points = bezier_points([0.0, 0.0], [200.0, 0.0]);
        assert!(points[1][0] > points[0][0]);
        assert!(points[2][0] < points[3][0]);
        assert_eq!(points[1][1], points[0][1]);
    }

    #[test]
    fn a_forward_wire_bows_out_by_half_the_span() {
        // Svelte Flow's forward control offset, which is what gives an edge between two
        // cards on different rows its S.
        let points = bezier_points([0.0, 0.0], [200.0, 120.0]);
        assert_eq!(points[1][0], 100.0);
        assert_eq!(points[2][0], 100.0);
    }

    #[test]
    fn a_backwards_wire_falls_back_to_the_curvature_term() {
        let points = bezier_points([200.0, 0.0], [100.0, 0.0]);
        assert_eq!(points[1][0], 200.0 + 0.25 * 25.0 * 10.0);
    }

    #[test]
    fn the_curve_starts_and_ends_at_the_ports() {
        let points = bezier_points([10.0, 20.0], [150.0, 80.0]);
        let start = sample(points, 0.0);
        let end = sample(points, 1.0);
        assert!((start[0] - 10.0).abs() < 1e-3 && (start[1] - 20.0).abs() < 1e-3);
        assert!((end[0] - 150.0).abs() < 1e-3 && (end[1] - 80.0).abs() < 1e-3);
    }

    #[test]
    fn a_point_on_the_curve_measures_as_touching_it() {
        let points = bezier_points([0.0, 0.0], [200.0, 100.0]);
        let midpoint = sample(points, 0.5);
        assert!(distance_to(points, midpoint) < 1.0);
        assert!(distance_to(points, [midpoint[0], midpoint[1] + 60.0]) > 20.0);
    }

    #[test]
    fn the_drag_preview_keeps_a_minimum_control_offset() {
        let points = preview_points([0.0, 0.0], [10.0, 0.0]);
        assert_eq!(points[1][0], 50.0);
    }
}
