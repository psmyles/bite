//! The preview panel, from `src/renderer/components/Preview.svelte`.
//!
//! The image is letterboxed on a black surface with no zoom or pan, and an information
//! overlay along the bottom names the file and its dimensions.
use crate::{controls, shell::Rect, theme};
use bite_imgui::{MouseCursor, Rounding, Ui, Vec2, WindowFlags};

/// What the panel is showing.
#[derive(Clone, Debug, Default)]
pub struct PreviewImage {
    pub texture: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub format: String,
    pub bytes: u64,
}

/// Fits `content` inside `available` without cropping, as `object-fit: contain` does.
pub fn contain(content: Vec2, available: Vec2) -> Vec2 {
    if content[0] <= 0.0 || content[1] <= 0.0 {
        return [0.0, 0.0];
    }
    let scale = (available[0] / content[0]).min(available[1] / content[1]);
    [content[0] * scale, content[1] * scale]
}

/// The file size string the overlay shows.
pub fn format_size(bytes: u64) -> String {
    const KILOBYTE: f64 = 1024.0;
    const MEGABYTE: f64 = KILOBYTE * 1024.0;
    let bytes = bytes as f64;
    if bytes < KILOBYTE {
        format!("{} B", bytes as u64)
    } else if bytes < MEGABYTE {
        format!("{:.1} KB", bytes / KILOBYTE)
    } else {
        format!("{:.1} MB", bytes / MEGABYTE)
    }
}

/// The second overlay line: the format, the dimensions and the size.
pub fn meta_line(image: &PreviewImage) -> String {
    format!(
        "{} · {} x {} · {}",
        image.format.to_uppercase(),
        image.width,
        image.height,
        format_size(image.bytes)
    )
}

/// Draws the panel. Returns true when the information toggle was clicked.
pub fn draw(ui: &mut Ui, rect: Rect, image: Option<&PreviewImage>, show_info: bool) -> bool {
    let mut toggled = false;
    ui.set_next_window_position(rect.min);
    ui.set_next_window_size(rect.size());
    let mut flags = WindowFlags::panel();
    flags.no_background = true;
    flags.no_scrollbar = true;

    ui.window_with("##preview", flags, |ui| {
        ui.draw_list().rect(
            rect.min,
            rect.max,
            theme::PREVIEW_BG,
            theme::PANEL_RADIUS,
            Rounding::All,
        );
        ui.set_cursor_screen_position(rect.min);
        controls::panel_header(ui, "Preview", None);

        // The information toggle only appears once an image is selected.
        if image.is_some() {
            let label = "Info";
            let size = controls::measure(ui, theme::face::SMALL_MONO, label);
            let width = size[0] + 16.0;
            let height = 20.0;
            let origin = [
                rect.max[0] - 8.0 - width,
                rect.min[1] + (theme::PANEL_HEADER_HEIGHT - height) / 2.0,
            ];
            let max = [origin[0] + width, origin[1] + height];
            let hovered = controls::point_in(ui.mouse_position(), origin, max);
            if hovered {
                ui.set_mouse_cursor(MouseCursor::Hand);
                if ui.mouse_clicked(bite_imgui::MouseButton::Left) {
                    toggled = true;
                }
            }
            let list = ui.draw_list();
            if show_info {
                list.rect(
                    origin,
                    max,
                    theme::COLOR_SUCCESS.mix(18.0, theme::PANEL_HEADER_BG),
                    3.0,
                    Rounding::All,
                );
            }
            list.rect_outline(
                origin,
                max,
                if show_info {
                    theme::COLOR_SUCCESS.mix(60.0, bite_imgui::Color::TRANSPARENT)
                } else if hovered {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
                3.0,
                Rounding::All,
                1.0,
            );
            list.text_with_face(
                [origin[0] + 8.0, origin[1] + (height - size[1]) / 2.0],
                if show_info {
                    theme::COLOR_SUCCESS_MUTED
                } else {
                    theme::TEXT_BRIGHT
                },
                theme::face::SMALL_MONO,
                label,
            );
        }

        let area_min = [rect.min[0], rect.min[1] + theme::PANEL_HEADER_HEIGHT];
        let area = [rect.width(), rect.max[1] - area_min[1]];
        let Some(image) = image else {
            let text = "No image selected.";
            let size = controls::measure(ui, theme::face::HINT, text);
            ui.draw_list().text_with_face(
                [
                    area_min[0] + (area[0] - size[0]) / 2.0,
                    area_min[1] + (area[1] - size[1]) / 2.0,
                ],
                theme::TEXT_BRIGHT.with_alpha(0.5),
                theme::face::HINT,
                text,
            );
            return;
        };

        if let Some(texture) = image.texture {
            let fitted = contain([image.width as f32, image.height as f32], area);
            let min = [
                area_min[0] + (area[0] - fitted[0]) / 2.0,
                area_min[1] + (area[1] - fitted[1]) / 2.0,
            ];
            ui.draw_list()
                .image(texture, min, [min[0] + fitted[0], min[1] + fitted[1]]);
        }

        if show_info {
            draw_info_overlay(ui, rect, area_min, image);
        }
    });
    toggled
}

/// The gradient strip along the bottom carrying the file name and its details.
fn draw_info_overlay(ui: &Ui, rect: Rect, area_min: Vec2, image: &PreviewImage) {
    let list = ui.draw_list();
    let height = 52.0;
    let top = (rect.max[1] - height).max(area_min[1]);
    list.rect_gradient(
        [rect.min[0], top],
        rect.max,
        bite_imgui::Color([0.0, 0.0, 0.0, 0.0]),
        bite_imgui::Color([0.0, 0.0, 0.0, 0.72]),
    );
    let name_face = bite_imgui::Face::ui_weight(theme::FONT_SIZE_SM, bite_imgui::Weight::SemiBold);
    controls::draw_ellipsized(
        ui,
        [rect.min[0] + 12.0, rect.max[1] - 34.0],
        theme::TEXT_BRIGHT,
        name_face,
        &image.name,
        rect.width() - 24.0,
    );
    controls::draw_ellipsized(
        ui,
        [rect.min[0] + 12.0, rect.max[1] - 17.0],
        theme::TEXT_MUTED,
        theme::face::SMALL_MONO,
        &meta_line(image),
        rect.width() - 24.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_image_is_letterboxed_by_width() {
        let fitted = contain([2000.0, 1000.0], [400.0, 400.0]);
        assert_eq!(fitted, [400.0, 200.0]);
    }

    #[test]
    fn a_tall_image_is_letterboxed_by_height() {
        let fitted = contain([1000.0, 2000.0], [400.0, 400.0]);
        assert_eq!(fitted, [200.0, 400.0]);
    }

    #[test]
    fn an_image_without_dimensions_takes_no_space() {
        assert_eq!(contain([0.0, 0.0], [400.0, 400.0]), [0.0, 0.0]);
    }

    #[test]
    fn file_sizes_switch_units_at_the_thresholds() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn the_meta_line_reads_format_then_dimensions_then_size() {
        let image = PreviewImage {
            texture: None,
            width: 1920,
            height: 1080,
            name: "shot.png".into(),
            format: "png".into(),
            bytes: 2048,
        };
        assert_eq!(meta_line(&image), "PNG · 1920 x 1080 · 2.0 KB");
    }
}
