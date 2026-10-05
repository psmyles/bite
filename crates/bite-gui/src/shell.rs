//! The fixed panel layout from `src/renderer/App.svelte`, including its drag splitters.
//!
//! Electron arranges the window as a left column, a centre canvas with a filmstrip beneath
//! it, and a right column holding the inspector above the preview. The gaps between panels
//! are the drag targets; no visible bar is drawn.
use crate::{persist::PanelSizes, theme};
use bite_imgui::{MouseButton, MouseCursor, Ui, Vec2};

/// One panel's rectangle in screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn size(self) -> Vec2 {
        [self.max[0] - self.min[0], self.max[1] - self.min[1]]
    }

    pub fn width(self) -> f32 {
        self.max[0] - self.min[0]
    }

    pub fn height(self) -> f32 {
        self.max[1] - self.min[1]
    }

    pub fn contains(self, point: Vec2) -> bool {
        point[0] >= self.min[0]
            && point[0] <= self.max[0]
            && point[1] >= self.min[1]
            && point[1] <= self.max[1]
    }
}

/// Where each panel sits this frame.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub library: Rect,
    pub canvas: Rect,
    pub filmstrip: Rect,
    pub inspector: Rect,
    pub preview: Rect,
    pub left_splitter: Rect,
    pub right_splitter: Rect,
    pub filmstrip_splitter: Rect,
    pub inspector_splitter: Rect,
}

/// Which splitter is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Splitter {
    Left,
    Right,
    Filmstrip,
    Inspector,
}

/// The height available to the right column once padding and its splitter are removed.
pub fn right_column_available(shell_height: f32) -> f32 {
    (shell_height - 2.0 * theme::SHELL_PADDING - theme::RESIZE_HANDLE_SIZE).max(1.0)
}

/// Computes every rectangle for a window of `size`, starting below `menu_height`.
pub fn compute(size: Vec2, menu_height: f32, panels: PanelSizes) -> Layout {
    let handle = theme::RESIZE_HANDLE_SIZE;
    let pad = theme::SHELL_PADDING;
    let shell_top = menu_height;
    let shell_height = (size[1] - menu_height).max(1.0);

    let content_left = pad;
    let content_right = (size[0] - pad).max(content_left + 1.0);
    let content_top = shell_top + pad;
    let content_bottom = (shell_top + shell_height - pad).max(content_top + 1.0);

    // The right column takes its width from the panel size; the main area keeps the rest.
    let right_width = panels
        .right
        .clamp(theme::RIGHT_PANEL_MIN, theme::RIGHT_PANEL_MAX)
        .min((content_right - content_left - handle - 120.0).max(theme::RIGHT_PANEL_MIN));
    let right_start = content_right - right_width;
    let right_splitter = Rect {
        min: [right_start - handle, content_top],
        max: [right_start, content_bottom],
    };

    let main_right = right_splitter.min[0];
    let left_width = panels
        .left
        .clamp(theme::LEFT_PANEL_MIN, theme::LEFT_PANEL_MAX)
        .min((main_right - content_left - handle - 120.0).max(theme::LEFT_PANEL_MIN));

    let filmstrip_height = panels
        .filmstrip
        .clamp(theme::FILMSTRIP_MIN, theme::FILMSTRIP_MAX)
        .min((content_bottom - content_top - handle - 120.0).max(theme::FILMSTRIP_MIN));
    let filmstrip_top = content_bottom - filmstrip_height;
    let filmstrip_splitter = Rect {
        min: [content_left, filmstrip_top - handle],
        max: [main_right, filmstrip_top],
    };

    let top_row_bottom = filmstrip_splitter.min[1];
    let library = Rect {
        min: [content_left, content_top],
        max: [content_left + left_width, top_row_bottom],
    };
    let left_splitter = Rect {
        min: [library.max[0], content_top],
        max: [library.max[0] + handle, top_row_bottom],
    };
    let canvas = Rect {
        min: [left_splitter.max[0], content_top],
        max: [main_right, top_row_bottom],
    };
    let filmstrip = Rect {
        min: [content_left, filmstrip_top],
        max: [main_right, content_bottom],
    };

    // The inspector height is a fraction of the right column, so it rescales with the window.
    let available = right_column_available(shell_height);
    let inspector_height = (panels
        .inspector_fraction
        .clamp(theme::INSPECTOR_MIN, theme::INSPECTOR_MAX)
        * available)
        .round()
        .clamp(
            40.0,
            (content_bottom - content_top - handle - 40.0).max(40.0),
        );
    let inspector = Rect {
        min: [right_start, content_top],
        max: [content_right, content_top + inspector_height],
    };
    let inspector_splitter = Rect {
        min: [right_start, inspector.max[1]],
        max: [content_right, inspector.max[1] + handle],
    };
    let preview = Rect {
        min: [right_start, inspector_splitter.max[1]],
        max: [content_right, content_bottom],
    };

    Layout {
        library,
        canvas,
        filmstrip,
        inspector,
        preview,
        left_splitter,
        right_splitter,
        filmstrip_splitter,
        inspector_splitter,
    }
}

/// Drag state for the splitters, held across frames.
#[derive(Default)]
pub struct SplitterState {
    active: Option<Splitter>,
    /// Pointer position and panel sizes captured when the drag began.
    origin: Vec2,
    start: PanelSizes,
}

impl SplitterState {
    pub fn is_dragging(&self) -> bool {
        self.active.is_some()
    }

    /// Draws the invisible hit regions and applies any drag. Returns true when sizes changed.
    pub fn run(
        &mut self,
        ui: &mut Ui,
        layout: &Layout,
        panels: &mut PanelSizes,
        shell_height: f32,
    ) -> bool {
        let targets = [
            (Splitter::Left, layout.left_splitter, true),
            (Splitter::Right, layout.right_splitter, true),
            (Splitter::Filmstrip, layout.filmstrip_splitter, false),
            (Splitter::Inspector, layout.inspector_splitter, false),
        ];

        let pointer = ui.mouse_position();
        let mut hovered = None;
        for (splitter, rect, _) in targets {
            if rect.contains(pointer) {
                hovered = Some(splitter);
            }
        }

        if self.active.is_none() {
            if let Some(splitter) = hovered {
                if ui.mouse_clicked(MouseButton::Left) {
                    self.active = Some(splitter);
                    self.origin = pointer;
                    self.start = *panels;
                }
            }
        }

        // The cursor follows either the hovered gap or the gap being dragged.
        let shape_for = |splitter: Splitter| match splitter {
            Splitter::Left | Splitter::Right => MouseCursor::ResizeEastWest,
            Splitter::Filmstrip | Splitter::Inspector => MouseCursor::ResizeNorthSouth,
        };
        if let Some(splitter) = self.active.or(hovered) {
            ui.set_mouse_cursor(shape_for(splitter));
        }

        if !ui.mouse_down(MouseButton::Left) {
            self.active = None;
            return false;
        }
        let Some(splitter) = self.active else {
            return false;
        };

        let delta = [pointer[0] - self.origin[0], pointer[1] - self.origin[1]];
        let before = *panels;
        match splitter {
            Splitter::Left => {
                panels.left = (self.start.left + delta[0])
                    .clamp(theme::LEFT_PANEL_MIN, theme::LEFT_PANEL_MAX);
            }
            // Dragging the right gap leftwards widens the right column.
            Splitter::Right => {
                panels.right = (self.start.right - delta[0])
                    .clamp(theme::RIGHT_PANEL_MIN, theme::RIGHT_PANEL_MAX);
            }
            // Dragging the filmstrip gap upwards makes the filmstrip taller.
            Splitter::Filmstrip => {
                panels.filmstrip = (self.start.filmstrip - delta[1])
                    .clamp(theme::FILMSTRIP_MIN, theme::FILMSTRIP_MAX);
            }
            Splitter::Inspector => {
                let available = right_column_available(shell_height);
                panels.inspector_fraction = (self.start.inspector_fraction + delta[1] / available)
                    .clamp(theme::INSPECTOR_MIN, theme::INSPECTOR_MAX);
            }
        }
        *panels != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_for(width: f32, height: f32) -> Layout {
        compute([width, height], 0.0, PanelSizes::default())
    }

    #[test]
    fn default_layout_uses_the_stylesheet_panel_widths() {
        let layout = layout_for(1600.0, 1000.0);
        assert_eq!(layout.library.width(), theme::LEFT_PANEL_DEFAULT);
        assert_eq!(layout.inspector.width(), theme::RIGHT_PANEL_DEFAULT);
        assert_eq!(layout.filmstrip.height(), theme::FILMSTRIP_DEFAULT);
    }

    #[test]
    fn panels_are_separated_by_exactly_one_gap() {
        let layout = layout_for(1600.0, 1000.0);
        let gap = theme::RESIZE_HANDLE_SIZE;
        assert_eq!(layout.canvas.min[0] - layout.library.max[0], gap);
        assert_eq!(layout.inspector.min[0] - layout.canvas.max[0], gap);
        assert_eq!(layout.filmstrip.min[1] - layout.canvas.max[1], gap);
        assert_eq!(layout.preview.min[1] - layout.inspector.max[1], gap);
    }

    #[test]
    fn shell_padding_surrounds_the_panels() {
        let layout = layout_for(1600.0, 1000.0);
        assert_eq!(layout.library.min[0], theme::SHELL_PADDING);
        assert_eq!(layout.library.min[1], theme::SHELL_PADDING);
        assert_eq!(layout.inspector.max[0], 1600.0 - theme::SHELL_PADDING);
        assert_eq!(layout.filmstrip.max[1], 1000.0 - theme::SHELL_PADDING);
    }

    #[test]
    fn the_inspector_split_is_a_fraction_that_rescales_with_the_window() {
        let short = layout_for(1600.0, 800.0);
        let tall = layout_for(1600.0, 1200.0);
        let ratio = |layout: Layout| {
            layout.inspector.height() / (layout.inspector.height() + layout.preview.height())
        };
        assert!((ratio(short) - ratio(tall)).abs() < 0.02);
        assert!(ratio(short) > 0.6 && ratio(short) < 0.7);
    }

    #[test]
    fn the_filmstrip_spans_the_library_and_the_canvas() {
        let layout = layout_for(1600.0, 1000.0);
        assert_eq!(layout.filmstrip.min[0], layout.library.min[0]);
        assert_eq!(layout.filmstrip.max[0], layout.canvas.max[0]);
    }

    #[test]
    fn a_narrow_window_still_leaves_a_usable_canvas() {
        let layout = layout_for(640.0, 480.0);
        assert!(layout.canvas.width() > 0.0);
        assert!(layout.canvas.height() > 0.0);
        assert!(layout.preview.height() > 0.0);
    }
}
