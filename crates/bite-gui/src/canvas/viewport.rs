//! The canvas pan and zoom transform, matching the Svelte Flow viewport the editor uses.
use crate::theme;
use bite_imgui::Vec2;

/// A pan offset in screen pixels and a zoom factor, exactly as stored in a `.bite` file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    /// Converts a graph position to a position inside the canvas rectangle.
    pub fn to_screen(self, point: Vec2) -> Vec2 {
        [point[0] * self.zoom + self.x, point[1] * self.zoom + self.y]
    }

    /// Converts a position inside the canvas rectangle back to graph coordinates.
    pub fn to_graph(self, point: Vec2) -> Vec2 {
        [
            (point[0] - self.x) / self.zoom,
            (point[1] - self.y) / self.zoom,
        ]
    }

    /// Scales a length from graph units to screen pixels.
    pub fn scale(self, length: f32) -> f32 {
        length * self.zoom
    }

    /// Zooms about `anchor`, which stays under the pointer. The factor is clamped to the
    /// range Svelte Flow uses.
    pub fn zoom_about(&mut self, anchor: Vec2, factor: f32) {
        let target = (self.zoom * factor).clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        if (target - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let graph = self.to_graph(anchor);
        self.zoom = target;
        self.x = anchor[0] - graph[0] * self.zoom;
        self.y = anchor[1] - graph[1] * self.zoom;
    }

    /// Sets an absolute zoom about `anchor`.
    pub fn set_zoom_about(&mut self, anchor: Vec2, zoom: f32) {
        let target = zoom.clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let graph = self.to_graph(anchor);
        self.zoom = target;
        self.x = anchor[0] - graph[0] * self.zoom;
        self.y = anchor[1] - graph[1] * self.zoom;
    }

    pub fn pan(&mut self, delta: Vec2) {
        self.x += delta[0];
        self.y += delta[1];
    }

    /// Frames `bounds` inside a canvas of `size`, as the fit-view control does.
    pub fn fit(bounds: Option<(Vec2, Vec2)>, size: Vec2, padding: f32) -> Self {
        let Some((min, max)) = bounds else {
            return Self::default();
        };
        let content = [(max[0] - min[0]).max(1.0), (max[1] - min[1]).max(1.0)];
        let available = [
            (size[0] - padding * 2.0).max(1.0),
            (size[1] - padding * 2.0).max(1.0),
        ];
        let zoom = (available[0] / content[0])
            .min(available[1] / content[1])
            .clamp(theme::ZOOM_MIN, theme::ZOOM_MAX);
        let centre = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        Self {
            x: size[0] / 2.0 - centre[0] * zoom,
            y: size[1] / 2.0 - centre[1] * zoom,
            zoom,
        }
    }

    /// The percentage the zoom label shows.
    pub fn zoom_percent(self) -> i32 {
        (self.zoom * 100.0).round() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_graph_conversions_are_inverses() {
        let viewport = Viewport {
            x: 120.0,
            y: -40.0,
            zoom: 1.35,
        };
        let point = [312.0, 88.0];
        let round_trip = viewport.to_graph(viewport.to_screen(point));
        assert!((round_trip[0] - point[0]).abs() < 1e-3);
        assert!((round_trip[1] - point[1]).abs() < 1e-3);
    }

    #[test]
    fn zooming_keeps_the_anchor_under_the_pointer() {
        let mut viewport = Viewport::default();
        let anchor = [400.0, 300.0];
        let before = viewport.to_graph(anchor);
        viewport.zoom_about(anchor, 1.2);
        let after = viewport.to_graph(anchor);
        assert!((before[0] - after[0]).abs() < 1e-3);
        assert!((before[1] - after[1]).abs() < 1e-3);
        assert!((viewport.zoom - 1.2).abs() < 1e-5);
    }

    #[test]
    fn zoom_is_clamped_to_the_range_svelte_flow_uses() {
        let mut viewport = Viewport::default();
        for _ in 0..40 {
            viewport.zoom_about([0.0, 0.0], theme::ZOOM_STEP);
        }
        assert_eq!(viewport.zoom, theme::ZOOM_MAX);
        for _ in 0..80 {
            viewport.zoom_about([0.0, 0.0], 1.0 / theme::ZOOM_STEP);
        }
        assert_eq!(viewport.zoom, theme::ZOOM_MIN);
    }

    #[test]
    fn fitting_centres_the_content() {
        let viewport = Viewport::fit(Some(([0.0, 0.0], [200.0, 100.0])), [800.0, 600.0], 40.0);
        let centre = viewport.to_screen([100.0, 50.0]);
        assert!((centre[0] - 400.0).abs() < 1e-3);
        assert!((centre[1] - 300.0).abs() < 1e-3);
    }

    #[test]
    fn an_empty_graph_fits_to_the_default_viewport() {
        assert_eq!(
            Viewport::fit(None, [800.0, 600.0], 40.0),
            Viewport::default()
        );
    }

    #[test]
    fn the_zoom_label_rounds_to_whole_percent() {
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 1.337,
        };
        assert_eq!(viewport.zoom_percent(), 134);
    }
}
