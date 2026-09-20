//! The filmstrip, from `src/renderer/components/Filmstrip.svelte`.
//!
//! The thumbnail size follows the panel height rather than a fixed value, the strip scrolls
//! horizontally, and a status bar reports how many images the active branch holds.
use crate::{controls, shell::Rect, theme};
use bite_imgui::{MouseCursor, Rounding, Ui, Vec2, WindowFlags};

/// One imported image as the strip shows it.
#[derive(Clone, Debug)]
pub struct Thumbnail {
    pub path: String,
    pub name: String,
    /// The uploaded texture, or none while the thumbnail is still being generated.
    pub texture: Option<u64>,
    /// The thumbnail's own measurements, so it can stand in for the preview.
    pub size: [u32; 2],
    /// The file it was made from, which the preview overlay describes.
    pub source: crate::work::SourceImage,
}

/// The status bar along the bottom, which reports the count.
pub const STATUS_HEIGHT: f32 = 24.0;
/// The strip's own inset inside the panel, above it and below it.
const STRIP_INSET: f32 = 9.0;
/// The file name under a thumbnail.
const LABEL_HEIGHT: f32 = 14.0;
/// The padding an item keeps around its thumbnail and its name.
const ITEM_PADDING: f32 = 9.0;

/// The chrome the panel reserves around a thumbnail.
///
/// Everything that is not the thumbnail itself is counted here, the horizontal scrollbar
/// included: it takes its height out of the strip, and a thumbnail sized without it
/// overflows by exactly that much and raises a vertical scrollbar beside it.
pub const CHROME: f32 =
    STATUS_HEIGHT + STRIP_INSET + LABEL_HEIGHT + ITEM_PADDING + theme::SCROLLBAR_WIDTH;
/// The gap between items, which the stride adds to the thumbnail size.
pub const ITEM_GAP: f32 = 12.0;

/// The thumbnail edge length for a panel of `height`.
pub fn thumbnail_size(height: f32) -> f32 {
    (height - CHROME).max(32.0)
}

/// The count line the status bar shows.
///
/// Electron shows this at every count, including none, so the bar never goes blank.
pub fn status_text(count: usize) -> String {
    match count {
        1 => "1 image".to_string(),
        many => format!("{many} images"),
    }
}

/// The half-open range of items that can be visible, with the overscan the strip applies.
pub fn visible_range(scroll: f32, width: f32, stride: f32, count: usize) -> (usize, usize) {
    const OVERSCAN: usize = 8;
    if count == 0 || stride <= 0.0 {
        return (0, 0);
    }
    let first = ((scroll / stride).floor() as isize - OVERSCAN as isize).max(0) as usize;
    let last = (((scroll + width) / stride).ceil() as usize + OVERSCAN).min(count);
    (first.min(count), last)
}

/// What the panel reports back.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Selected(usize),
    /// The empty-state prompt was clicked, which opens the image dialog.
    RequestImport,
    /// Files were dropped on the strip.
    Dropped(Vec<String>),
}

/// Draws the filmstrip.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    thumbnails: &[Thumbnail],
    selected: Option<usize>,
    scroll: &mut f32,
) -> Option<Outcome> {
    let mut outcome = None;
    ui.set_next_window_position(rect.min);
    ui.set_next_window_size(rect.size());
    let mut flags = WindowFlags::panel();
    flags.no_background = true;
    flags.no_scrollbar = true;

    ui.window_with("##filmstrip", flags, |ui| {
        ui.draw_list().rect(
            rect.min,
            rect.max,
            theme::PANEL_BG,
            theme::PANEL_RADIUS,
            Rounding::All,
        );

        let status_height = STATUS_HEIGHT;
        let strip_height = (rect.height() - status_height).max(1.0);
        let size = thumbnail_size(rect.height());
        let stride = size + ITEM_GAP;

        if thumbnails.is_empty() {
            let text = "Drop images here or click to open...";
            let measured = controls::measure(ui, theme::face::HINT, text);
            let position = [
                rect.min[0] + (rect.width() - measured[0]) / 2.0,
                rect.min[1] + (strip_height - measured[1]) / 2.0,
            ];
            let hovered = controls::point_in(
                ui.mouse_position(),
                position,
                [position[0] + measured[0], position[1] + measured[1]],
            );
            if hovered {
                ui.set_mouse_cursor(MouseCursor::Hand);
                if ui.mouse_clicked(bite_imgui::MouseButton::Left) {
                    outcome = Some(Outcome::RequestImport);
                }
            }
            ui.draw_list().text_with_face(
                position,
                theme::TEXT_BRIGHT.with_alpha(if hovered { 1.0 } else { 0.5 }),
                theme::face::HINT,
                text,
            );
        } else {
            ui.set_cursor_screen_position([rect.min[0] + 8.0, rect.min[1] + 5.0]);
            let strip_flags = WindowFlags {
                horizontal_scrollbar: true,
                ..WindowFlags::default()
            };
            ui.child_with(
                "filmstrip-strip",
                [rect.width() - 16.0, strip_height - 9.0],
                false,
                strip_flags,
                |ui| {
                    // A vertical wheel gesture scrolls the strip sideways.
                    let wheel = ui.mouse_wheel();
                    if ui.window_hovered() && wheel[1].abs() > 0.0 {
                        let target = ui.scroll_x() - wheel[1] * stride;
                        ui.set_scroll_x(target.clamp(0.0, ui.scroll_max_x().max(0.0)));
                    }
                    *scroll = ui.scroll_x();
                    let (first, last) =
                        visible_range(*scroll, rect.width(), stride, thumbnails.len());
                    // Spacers keep the scroll range correct while only drawing what shows.
                    if first > 0 {
                        ui.dummy([first as f32 * stride, 1.0]);
                        ui.same_line();
                    }
                    for (offset, thumbnail) in thumbnails[first..last].iter().enumerate() {
                        let index = first + offset;
                        if item(ui, thumbnail, size, selected == Some(index)) {
                            outcome = Some(Outcome::Selected(index));
                        }
                        ui.same_line();
                    }
                    let remaining = thumbnails.len().saturating_sub(last);
                    if remaining > 0 {
                        ui.dummy([remaining as f32 * stride, 1.0]);
                    }
                },
            );
        }

        // The status bar reports the count for the active branch.
        let status_top = rect.max[1] - status_height;
        let list = ui.draw_list();
        list.rect(
            [rect.min[0], status_top],
            rect.max,
            theme::PANEL_HEADER_BG,
            theme::PANEL_RADIUS,
            Rounding::Bottom,
        );
        list.line(
            [rect.min[0], status_top],
            [rect.max[0], status_top],
            theme::BORDER.mix(40.0, bite_imgui::Color::TRANSPARENT),
            1.0,
        );
        // The count shows even at zero, as `countLabel` does.
        let text = status_text(thumbnails.len());
        let size = list.measure(theme::face::SMALL_MONO, &text);
        list.text_with_face(
            [rect.min[0] + 10.0, status_top + (status_height - size[1]) / 2.0],
            theme::TEXT_BRIGHT.with_alpha(0.5),
            theme::face::SMALL_MONO,
            &text,
        );
    });
    outcome
}

/// One thumbnail with its file name beneath it.
fn item(ui: &mut Ui, thumbnail: &Thumbnail, size: f32, selected: bool) -> bool {
    let label_height = LABEL_HEIGHT;
    let total = [size + 6.0, size + label_height + ITEM_PADDING];
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##thumb-{}", thumbnail.path), total);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }

    let list = ui.draw_list();
    let border = if selected {
        theme::ACCENT
    } else if hovered {
        theme::BORDER
    } else {
        bite_imgui::Color::TRANSPARENT
    };
    list.rect_outline(
        origin,
        [origin[0] + total[0], origin[1] + total[1]],
        border,
        4.0,
        Rounding::All,
        1.0,
    );
    let image_min = [origin[0] + 3.0, origin[1] + 3.0];
    let image_max = [image_min[0] + size, image_min[1] + size];
    match thumbnail.texture {
        Some(texture) => list.image_rounded(texture, image_min, image_max, 2.0),
        // Until the thumbnail arrives the strip shows a plain placeholder box.
        None => list.rect(
            image_min,
            image_max,
            theme::PANEL_HEADER_BG,
            2.0,
            Rounding::All,
        ),
    }
    controls::draw_ellipsized(
        ui,
        [image_min[0], image_max[1] + 3.0],
        theme::TEXT_BRIGHT.with_alpha(if selected { 1.0 } else { 0.6 }),
        theme::face::THUMB_NAME,
        &thumbnail.name,
        size,
    );
    clicked
}

/// Draws the drag-over tint and dashed outline the panel shows while files hover it.
pub fn draw_drag_highlight(ui: &Ui, rect: Rect) {
    let list = ui.draw_list();
    list.rect(
        rect.min,
        rect.max,
        theme::ACCENT.mix(12.0, theme::PANEL_BG),
        theme::PANEL_RADIUS,
        Rounding::All,
    );
    // The stylesheet insets the outline by three pixels.
    list.rect_outline(
        [rect.min[0] + 3.0, rect.min[1] + 3.0],
        [rect.max[0] - 3.0, rect.max[1] - 3.0],
        theme::ACCENT,
        theme::PANEL_RADIUS,
        Rounding::All,
        1.0,
    );
}

/// The file name shown under a thumbnail.
pub fn display_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// The panel size in logical pixels, for callers that need it before drawing.
pub fn preferred_size(rect: Rect) -> Vec2 {
    [rect.width(), rect.height()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumbnail_size_follows_the_panel_height() {
        // The default panel is one hundred and twenty pixels tall.
        assert_eq!(thumbnail_size(120.0), 120.0 - CHROME);
        assert_eq!(thumbnail_size(220.0), 220.0 - CHROME);
    }

    #[test]
    fn a_very_short_panel_keeps_a_usable_thumbnail() {
        assert_eq!(thumbnail_size(40.0), 32.0);
    }

    #[test]
    fn the_status_line_uses_the_singular_for_one_image() {
        assert_eq!(status_text(1), "1 image");
        assert_eq!(status_text(0), "0 images");
        assert_eq!(status_text(27), "27 images");
    }

    #[test]
    fn the_visible_range_covers_the_strip_plus_the_overscan() {
        let (first, last) = visible_range(0.0, 400.0, 80.0, 100);
        assert_eq!(first, 0);
        assert!((5..=100).contains(&last));
        let (first, _) = visible_range(1600.0, 400.0, 80.0, 100);
        assert_eq!(first, 12);
    }

    #[test]
    fn an_empty_strip_has_an_empty_range() {
        assert_eq!(visible_range(0.0, 400.0, 80.0, 0), (0, 0));
    }

    #[test]
    fn display_names_drop_the_directory_on_both_separators() {
        assert_eq!(display_name("C:\\images\\shot.png"), "shot.png");
        assert_eq!(display_name("/home/user/shot.png"), "shot.png");
        assert_eq!(display_name("shot.png"), "shot.png");
    }
}
