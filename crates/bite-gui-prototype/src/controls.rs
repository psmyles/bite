//! The Electron widget set, drawn to match `theme.css` rather than Dear ImGui's defaults.
//!
//! Each control here is hand-drawn on the window draw list over an invisible hit region, so
//! that heights, radii, border widths and hover colors match the stylesheet exactly.
use crate::theme::{self, ButtonStyle};
use bite_imgui::{
    Color, Face, InputFlags, MouseButton, MouseCursor, Rounding, StyleColor, StyleVar, Ui, Vec2,
};

/// Which of the three button styles to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    Neutral,
    Primary,
    Danger,
}

impl ButtonKind {
    fn style(self) -> ButtonStyle {
        match self {
            Self::Neutral => theme::button_neutral(),
            Self::Primary => theme::button_primary(),
            Self::Danger => theme::button_danger(),
        }
    }
}

/// Draws a button of the shared thirty-pixel height. `width` of zero sizes to the label.
pub fn button(ui: &mut Ui, label: &str, kind: ButtonKind, width: f32, enabled: bool) -> bool {
    let text_size = measure(ui, theme::face::BUTTON, label);
    let width = if width > 0.0 {
        width
    } else {
        text_size[0] + theme::BUTTON_PADDING_X * 2.0
    };
    let origin = ui.cursor_screen_position();
    let pressed = ui.invisible_button(&format!("##button-{label}"), [width, theme::BUTTON_HEIGHT])
        && enabled;
    let hovered = ui.item_hovered() && enabled;
    if hovered {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }

    let style = if enabled {
        kind.style()
    } else {
        theme::button_disabled()
    };
    let (background, border, text) = if hovered {
        (style.hover_background, style.hover_border, style.hover_text)
    } else {
        (style.background, style.border, style.text)
    };

    let list = ui.draw_list();
    let max = [origin[0] + width, origin[1] + theme::BUTTON_HEIGHT];
    list.rect(origin, max, background, theme::BUTTON_RADIUS, Rounding::All);
    list.rect_outline(
        origin,
        max,
        border,
        theme::BUTTON_RADIUS,
        Rounding::All,
        theme::INPUT_BORDER_WIDTH,
    );
    let text_position = [
        origin[0] + (width - text_size[0]) / 2.0,
        origin[1] + (theme::BUTTON_HEIGHT - text_size[1]) / 2.0,
    ];
    list.text_with_face(text_position, text, theme::face::BUTTON, label);
    pressed
}

/// A full-width button, as used by the inspector actions.
pub fn button_full(ui: &mut Ui, label: &str, kind: ButtonKind, enabled: bool) -> bool {
    let width = ui.content_region_available()[0];
    button(ui, label, kind, width, enabled)
}

/// Draws a text field with the stylesheet's two-pixel border and monospaced value.
pub fn text_input(ui: &mut Ui, id: &str, value: &mut String, hint: &str, width: f32) -> bool {
    text_input_with(ui, id, value, hint, width, InputFlags::default())
}

pub fn text_input_with(
    ui: &mut Ui,
    id: &str,
    value: &mut String,
    hint: &str,
    width: f32,
    flags: InputFlags,
) -> bool {
    let width = if width > 0.0 {
        width
    } else {
        ui.content_region_available()[0]
    };
    let origin = ui.cursor_screen_position();
    let list = ui.draw_list();
    let max = [origin[0] + width, origin[1] + theme::INPUT_HEIGHT];
    list.rect(
        origin,
        max,
        theme::TEXT_FIELD_BG,
        theme::INPUT_RADIUS,
        Rounding::All,
    );

    ui.set_next_item_width(width);
    let changed = ui.with_style(
        &[
            StyleVar::FrameRounding(theme::INPUT_RADIUS),
            StyleVar::FrameBorderSize(0.0),
            StyleVar::FramePadding([
                theme::INPUT_PADDING_X,
                (theme::INPUT_HEIGHT - 12.0) / 2.0,
            ]),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::FrameBg, Color::TRANSPARENT),
                    (StyleColor::FrameBgHovered, Color::TRANSPARENT),
                    (StyleColor::FrameBgActive, Color::TRANSPARENT),
                    (StyleColor::Text, theme::TEXT_BRIGHT),
                    (StyleColor::TextDisabled, theme::TEXT.with_alpha(0.5)),
                ],
                |ui| {
                    ui.with_face(theme::face::VALUE, |ui| {
                        ui.input_text_with(&format!("##{id}"), value, hint, flags)
                    })
                },
            )
        },
    );
    let active = ui.item_active();

    let list = ui.draw_list();
    list.rect_outline(
        origin,
        max,
        if active { theme::ACCENT } else { theme::BORDER },
        theme::INPUT_RADIUS,
        Rounding::All,
        theme::INPUT_BORDER_WIDTH,
    );
    changed
}

/// The search field used by the library and the creation menu: a darker fill, no border,
/// and a one-pixel accent ring while focused.
pub fn search_input(ui: &mut Ui, id: &str, value: &mut String, hint: &str, width: f32) -> bool {
    let width = if width > 0.0 {
        width
    } else {
        ui.content_region_available()[0]
    };
    let height = 26.0;
    let origin = ui.cursor_screen_position();
    let max = [origin[0] + width, origin[1] + height];
    ui.draw_list().rect(
        origin,
        max,
        theme::SEARCH_BG,
        theme::LIBRARY_SEARCH_RADIUS,
        Rounding::All,
    );
    ui.set_next_item_width(width);
    let changed = ui.with_style(
        &[
            StyleVar::FrameRounding(theme::LIBRARY_SEARCH_RADIUS),
            StyleVar::FrameBorderSize(0.0),
            StyleVar::FramePadding([10.0, (height - 13.0) / 2.0]),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::FrameBg, Color::TRANSPARENT),
                    (StyleColor::FrameBgHovered, Color::TRANSPARENT),
                    (StyleColor::FrameBgActive, Color::TRANSPARENT),
                    (StyleColor::Text, theme::TEXT_BRIGHT),
                    (StyleColor::TextDisabled, theme::TEXT.with_alpha(0.5)),
                ],
                |ui| {
                    ui.with_face(theme::face::SEARCH, |ui| {
                        ui.input_text_with(&format!("##{id}"), value, hint, InputFlags::default())
                    })
                },
            )
        },
    );
    if ui.item_active() {
        ui.draw_list().rect_outline(
            origin,
            max,
            theme::ACCENT,
            theme::LIBRARY_SEARCH_RADIUS,
            Rounding::All,
            1.0,
        );
    }
    changed
}

/// A sixteen-pixel checkbox with the stylesheet's green tick.
pub fn checkbox(ui: &mut Ui, id: &str, value: &mut bool) -> bool {
    let size = theme::CHECKBOX_SIZE;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##check-{id}"), [size, size]);
    if clicked {
        *value = !*value;
    }
    if ui.item_hovered() {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    let list = ui.draw_list();
    let max = [origin[0] + size, origin[1] + size];
    let border = if *value {
        theme::COLOR_SUCCESS
    } else {
        theme::TEXT_BRIGHT
    };
    list.rect_outline(
        origin,
        max,
        border,
        theme::CHECKBOX_RADIUS,
        Rounding::All,
        theme::CHECKBOX_BORDER_WIDTH,
    );
    if *value {
        // The stylesheet draws a five by nine tick rotated through forty five degrees.
        let left = [origin[0] + size * 0.28, origin[1] + size * 0.52];
        let middle = [origin[0] + size * 0.44, origin[1] + size * 0.70];
        let right = [origin[0] + size * 0.74, origin[1] + size * 0.30];
        list.line(left, middle, theme::COLOR_SUCCESS, 2.0);
        list.line(middle, right, theme::COLOR_SUCCESS, 2.0);
    }
    clicked
}

/// The drop-down from `Dropdown.svelte`: a thirty-pixel button with a rotating chevron.
pub fn dropdown(
    ui: &mut Ui,
    id: &str,
    current: &mut usize,
    labels: &[String],
    width: f32,
    enabled: bool,
) -> bool {
    if labels.is_empty() {
        return false;
    }
    let width = if width > 0.0 {
        width
    } else {
        ui.content_region_available()[0]
    };
    let origin = ui.cursor_screen_position();
    let index = (*current).min(labels.len() - 1);

    ui.set_next_item_width(width);
    let changed = ui.disabled(!enabled, |ui| {
        ui.with_style(
            &[
                StyleVar::FrameRounding(theme::INPUT_RADIUS),
                StyleVar::FrameBorderSize(0.0),
                StyleVar::FramePadding([
                    theme::INPUT_PADDING_X,
                    (theme::INPUT_HEIGHT - 13.0) / 2.0,
                ]),
                StyleVar::PopupRounding(theme::INPUT_RADIUS),
                StyleVar::PopupBorderSize(theme::INPUT_BORDER_WIDTH),
                StyleVar::ItemSpacing([0.0, 0.0]),
                // `.dd-list` pads its rows; without this the open list's own window would
                // take the zero padding above, leaving the first row against the border.
                StyleVar::WindowPadding([
                    theme::DROPDOWN_ITEM_PADDING_X,
                    theme::DROPDOWN_LIST_PADDING_Y,
                ]),
            ],
            |ui| {
                ui.with_colors(
                    &[
                        (StyleColor::FrameBg, theme::DROPDOWN_BG),
                        (StyleColor::FrameBgHovered, theme::DROPDOWN_HOVER_BG),
                        (StyleColor::FrameBgActive, theme::DROPDOWN_HOVER_BG),
                        (StyleColor::Text, theme::TEXT_BRIGHT),
                        (StyleColor::PopupBg, theme::DROPDOWN_BG),
                        (StyleColor::Border, theme::BORDER),
                        (StyleColor::Header, theme::DROPDOWN_HOVER_BG),
                        (StyleColor::HeaderHovered, theme::DROPDOWN_ACTIVE_BG),
                        (StyleColor::HeaderActive, theme::DROPDOWN_ACTIVE_BG),
                    ],
                    |ui| {
                        ui.with_face(theme::face::BODY, |ui| {
                            dropdown_list(ui, id, current, labels, index)
                        })
                    },
                )
            },
        )
    });

    let hovered = ui.item_hovered();
    let list = ui.draw_list();
    let max = [origin[0] + width, origin[1] + theme::INPUT_HEIGHT];
    list.rect_outline(
        origin,
        max,
        if hovered && enabled {
            theme::ACCENT
        } else {
            theme::BORDER
        },
        theme::INPUT_RADIUS,
        Rounding::All,
        theme::INPUT_BORDER_WIDTH,
    );
    let _ = index;
    changed
}

/// The open list of a dropdown.
///
/// A row's height is its text alone, so without vertical spacing the options sit line
/// against line. Dear ImGui grows a row's highlight into half the spacing on each side,
/// which is the padding `.dd-item` gives it.
fn dropdown_list(
    ui: &mut Ui,
    id: &str,
    current: &mut usize,
    labels: &[String],
    index: usize,
) -> bool {
    let mut changed = false;
    // Each row is given its own height rather than leaning on item spacing: the list window
    // sizes itself from its content, and a row whose height is only its text would leave the
    // options stacked line against line.
    let row = ui.calc_text_size("Ag")[1] + theme::DROPDOWN_ITEM_PADDING_Y * 2.0;
    // Left to itself a combo caps its list at eight of Dear ImGui's rows, which is shorter
    // than eight of these and would cut the list off partway down a row.
    let visible = labels.len().min(theme::DROPDOWN_VISIBLE_ROWS) as f32;
    ui.set_next_window_size_constraints(
        [0.0, 0.0],
        [f32::MAX, visible * row + theme::DROPDOWN_LIST_PADDING_Y * 2.0],
    );
    if ui.begin_combo(&format!("##{id}"), &labels[index]) {
        for (position, label) in labels.iter().enumerate() {
            let selected = position == index;
            if ui.selectable_sized(label, selected, [0.0, row]) {
                *current = position;
                changed = true;
            }
            if selected {
                ui.set_item_default_focus();
            }
        }
        ui.end_combo();
    }
    changed
}

/// A panel header: a thirty-six pixel bar with the stylesheet's letter-spaced title.
pub fn panel_header(ui: &mut Ui, title: &str, trailing: Option<&str>) {
    let width = ui.content_region_available()[0];
    let origin = ui.cursor_screen_position();
    let max = [origin[0] + width, origin[1] + theme::PANEL_HEADER_HEIGHT];
    let list = ui.draw_list();
    list.rect(origin, max, theme::PANEL_HEADER_BG, 0.0, Rounding::None);
    // The stylesheet letter-spaces the title, which is drawn per character here.
    let title_position = [
        origin[0] + 12.0,
        origin[1] + (theme::PANEL_HEADER_HEIGHT - 13.0) / 2.0 - 1.0,
    ];
    draw_tracked_text(
        ui,
        title_position,
        theme::TEXT_BRIGHT,
        theme::face::PANEL_HEADER,
        title,
        0.06 * 13.0,
    );
    if let Some(trailing) = trailing {
        let size = measure(ui, theme::face::SMALL_MONO, trailing);
        let available = width * 0.6;
        let text_x = (max[0] - 10.0 - size[0]).max(origin[0] + 12.0 + available * 0.2);
        ui.draw_list().text_with_face(
            [text_x, origin[1] + (theme::PANEL_HEADER_HEIGHT - size[1]) / 2.0],
            theme::TEXT_BRIGHT.with_alpha(0.5),
            theme::face::SMALL_MONO,
            trailing,
        );
    }
    ui.dummy([width, theme::PANEL_HEADER_HEIGHT]);
}

/// Draws text with extra spacing between characters, as the `letter-spacing` rules ask for.
pub fn draw_tracked_text(
    ui: &Ui,
    position: Vec2,
    color: Color,
    face: Face,
    text: &str,
    tracking: f32,
) {
    draw_tracked_text_scaled(ui, position, color, face, 1.0, text, tracking)
}

/// The same, with every face size multiplied by `scale`, for canvas content that zooms.
pub fn draw_tracked_text_scaled(
    ui: &Ui,
    position: Vec2,
    color: Color,
    face: Face,
    scale: f32,
    text: &str,
    tracking: f32,
) {
    let list = ui.draw_list().scaled(scale);
    // The spacing is part of the type, so it zooms with it.
    let tracking = tracking * scale;
    if tracking.abs() < 0.01 {
        list.text_with_face(position, color, face, text);
        return;
    }
    let mut x = position[0];
    let mut buffer = [0u8; 4];
    for character in text.chars() {
        let glyph = character.encode_utf8(&mut buffer);
        list.text_with_face([x, position[1]], color, face, glyph);
        x += list.measure(face, glyph)[0] + tracking;
    }
}

/// The width of `text` in `face`, including letter spacing when given.
pub fn measure(ui: &Ui, face: Face, text: &str) -> Vec2 {
    measure_scaled(ui, face, 1.0, text)
}

/// Measures without a draw list, so it is safe before any window has begun.
pub fn measure_scaled(ui: &Ui, face: Face, scale: f32, text: &str) -> Vec2 {
    ui.fonts().measure(face, scale, text)
}

pub fn measure_tracked(ui: &Ui, face: Face, text: &str, tracking: f32) -> Vec2 {
    measure_tracked_scaled(ui, face, 1.0, text, tracking)
}

/// The same, with the face size and the spacing both multiplied by `scale`.
pub fn measure_tracked_scaled(ui: &Ui, face: Face, scale: f32, text: &str, tracking: f32) -> Vec2 {
    let base = measure_scaled(ui, face, scale, text);
    let count = text.chars().count() as f32;
    [base[0] + tracking * scale * count.max(1.0), base[1]]
}

/// Draws text truncated with an ellipsis so that it fits `width`.
pub fn draw_ellipsized(ui: &Ui, position: Vec2, color: Color, face: Face, text: &str, width: f32) {
    draw_ellipsized_scaled(ui, position, color, face, 1.0, text, width)
}

/// The same, with every face size multiplied by `scale`, for canvas content that zooms.
pub fn draw_ellipsized_scaled(
    ui: &Ui,
    position: Vec2,
    color: Color,
    face: Face,
    scale: f32,
    text: &str,
    width: f32,
) {
    let list = ui.draw_list().scaled(scale);
    if list.measure(face, text)[0] <= width {
        list.text_with_face(position, color, face, text);
        return;
    }
    let ellipsis = "...";
    let reserve = list.measure(face, ellipsis)[0];
    let mut end = text.len();
    while end > 0 {
        if !text.is_char_boundary(end) {
            end -= 1;
            continue;
        }
        if list.measure(face, &text[..end])[0] + reserve <= width {
            break;
        }
        end -= 1;
    }
    list.text_with_face(position, color, face, &format!("{}{ellipsis}", &text[..end]));
}

/// A label drawn in the inspector's row style: twelve pixels at sixty percent opacity.
pub fn row_label(ui: &mut Ui, text: &str) {
    ui.with_face(theme::face::LABEL, |ui| {
        ui.with_colors(
            &[(StyleColor::Text, theme::TEXT_BRIGHT.with_alpha(0.6))],
            |ui| ui.text(text),
        )
    });
}

/// A hint line: thirteen pixel interface text at half opacity.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.with_face(theme::face::HINT, |ui| {
        ui.with_colors(
            &[(StyleColor::Text, theme::TEXT_BRIGHT.with_alpha(0.5))],
            |ui| ui.text_wrapped(text),
        )
    });
}

/// A one-pixel divider in the inspector's row color.
pub fn row_separator(ui: &mut Ui) {
    let width = ui.content_region_available()[0];
    let origin = ui.cursor_screen_position();
    ui.draw_list().line(
        origin,
        [origin[0] + width, origin[1]],
        theme::inspector_row_border(),
        1.0,
    );
    ui.dummy([width, 1.0]);
}

/// The square close button shared by every modal header.
pub fn close_button(ui: &mut Ui, id: &str) -> bool {
    let size = theme::MODAL_CLOSE_BTN_SIZE;
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##close-{id}"), [size, size]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    let list = ui.draw_list();
    let max = [origin[0] + size, origin[1] + size];
    if hovered {
        list.rect(
            origin,
            max,
            theme::CTX_ITEM_HOVER_BG,
            theme::MODAL_CLOSE_BTN_RADIUS,
            Rounding::All,
        );
    }
    list.rect_outline(
        origin,
        max,
        if hovered { theme::ACCENT } else { theme::BORDER },
        theme::MODAL_CLOSE_BTN_RADIUS,
        Rounding::All,
        theme::MODAL_CLOSE_BTN_BORDER_WIDTH,
    );
    let colour = if hovered {
        theme::TEXT_BRIGHT
    } else {
        theme::TEXT
    };
    let inset = size * 0.32;
    list.line(
        [origin[0] + inset, origin[1] + inset],
        [max[0] - inset, max[1] - inset],
        colour,
        1.6,
    );
    list.line(
        [max[0] - inset, origin[1] + inset],
        [origin[0] + inset, max[1] - inset],
        colour,
        1.6,
    );
    clicked
}

/// A badge such as the `wired` or `out` markers beside an inspector label.
pub fn badge(ui: &mut Ui, text: &str, color: Color) {
    let face = theme::face::SMALL_MONO;
    let size = measure(ui, face, text);
    let width = size[0] + 6.0;
    let height = 14.0;
    let origin = ui.cursor_screen_position();
    // The label beside it is centred on its own line, so the badge is centred to match.
    let line = ui.calc_text_size("Ag")[1].max(height);
    let top = origin[1] + (line - height) / 2.0;
    let list = ui.draw_list();
    let max = [origin[0] + width, top + height];
    list.rect_outline([origin[0], top], max, color, 3.0, Rounding::All, 1.0);
    list.text_with_face(
        [origin[0] + 3.0, top + (height - size[1]) / 2.0],
        color,
        face,
        text,
    );
    ui.dummy([width, line]);
}

/// The slider from `InspectorParamEditor.svelte`: a three pixel track with a round accent
/// thumb, and no value printed on the track. ImGui's own slider draws a grab rectangle with
/// the number inside it, which is a different control.
///
/// `height` is the row height the track centres itself in, so the slider lines up with the
/// number box beside it.
pub fn slider(
    ui: &mut Ui,
    id: &str,
    value: &mut f32,
    min: f32,
    max: f32,
    width: f32,
    height: f32,
) -> bool {
    let origin = ui.cursor_screen_position();
    let clicked = ui.invisible_button(&format!("##{id}-slider"), [width, height]);
    let active = ui.item_active();
    let hovered = ui.item_hovered();
    if hovered || active {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }

    let mut changed = clicked;
    if active {
        let fraction = slider_fraction(ui.mouse_position()[0] - origin[0], width);
        let next = min + fraction * (max - min);
        if (next - *value).abs() > f32::EPSILON {
            *value = next;
            changed = true;
        }
    }

    let span = (max - min).abs().max(f32::EPSILON);
    let fraction = ((*value - min) / span).clamp(0.0, 1.0);
    let centre_y = origin[1] + height / 2.0;
    let track_top = centre_y - theme::SLIDER_TRACK_HEIGHT / 2.0;
    let list = ui.draw_list();
    list.rect(
        [origin[0], track_top],
        [
            origin[0] + width,
            track_top + theme::SLIDER_TRACK_HEIGHT,
        ],
        theme::BORDER.mix(theme::SLIDER_TRACK_MIX, theme::PANEL_BG),
        theme::SLIDER_TRACK_RADIUS,
        Rounding::All,
    );
    let half = theme::SLIDER_THUMB_SIZE / 2.0;
    let thumb_x = origin[0] + half + fraction * (width - theme::SLIDER_THUMB_SIZE).max(0.0);
    list.circle(
        [thumb_x, centre_y],
        half,
        theme::ACCENT,
    );
    changed
}

/// A colour row: a full-width swatch that opens the picker, sized like the other controls.
///
/// ImGui's inline editor packs four drag fields into the row instead, which is illegible at
/// an inspector's width.
pub fn color_row(ui: &mut Ui, id: &str, value: &mut [f32; 4], width: f32) -> bool {
    let origin = ui.cursor_screen_position();
    let max = [origin[0] + width, origin[1] + theme::INPUT_HEIGHT];
    let opened = ui.color_button(&format!("##{id}-swatch"), Color(*value), [
        width,
        theme::INPUT_HEIGHT,
    ]);
    ui.draw_list().rect_outline(
        origin,
        max,
        if ui.item_hovered() {
            theme::ACCENT
        } else {
            theme::BORDER
        },
        theme::INPUT_RADIUS,
        Rounding::All,
        theme::INPUT_BORDER_WIDTH,
    );
    let popup = format!("##{id}-picker");
    if opened {
        ui.open_popup(&popup);
    }
    let mut changed = false;
    ui.popup(&popup, |ui| {
        changed = ui.color_picker4(&format!("##{id}-wheel"), value);
    });
    changed
}

/// Draws `text` centred vertically in a row `height` tall whose top edge is `top`.
///
/// Faces differ in line height, so two faces sharing a row cannot both sit at a fixed
/// offset from its top; centring each one against the row keeps them on the same line.
pub fn draw_in_row(
    ui: &Ui,
    x: f32,
    top: f32,
    height: f32,
    color: Color,
    face: Face,
    text: &str,
) {
    let list = ui.draw_list();
    let line = list.measure(face, text)[1];
    list.text_with_face([x, top + (height - line) / 2.0], color, face, text);
}

/// The x a right-aligned run of `text` starts at, given the row's right edge.
pub fn right_aligned(ui: &Ui, right: f32, face: Face, text: &str) -> f32 {
    right - measure(ui, face, text)[0]
}

/// Where along the track a pointer `offset` pixels into it falls.
///
/// The thumb's centre travels between the inset ends, so the usable span is the track width
/// less one thumb; without the inset the thumb would hang off both ends.
pub fn slider_fraction(offset: f32, width: f32) -> f32 {
    let half = theme::SLIDER_THUMB_SIZE / 2.0;
    let travel = (width - theme::SLIDER_THUMB_SIZE).max(1.0);
    ((offset - half) / travel).clamp(0.0, 1.0)
}

/// Tracks how long the pointer has rested on the last item, for the delayed tooltips.
#[derive(Default)]
pub struct HoverTimer {
    target: Option<String>,
    elapsed: f32,
}

impl HoverTimer {
    /// Advances the timer for `id` and reports whether the delay has elapsed.
    pub fn poll(&mut self, id: &str, hovered: bool, delta: f32, delay: f32) -> bool {
        if !hovered {
            if self.target.as_deref() == Some(id) {
                self.target = None;
                self.elapsed = 0.0;
            }
            return false;
        }
        if self.target.as_deref() != Some(id) {
            self.target = Some(id.to_string());
            self.elapsed = 0.0;
        }
        self.elapsed += delta;
        self.elapsed >= delay
    }

    pub fn reset(&mut self) {
        self.target = None;
        self.elapsed = 0.0;
    }
}

/// True when the pointer is inside the rectangle, for hand-drawn hover states.
pub fn point_in(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point[0] >= min[0] && point[0] <= max[0] && point[1] >= min[1] && point[1] <= max[1]
}

/// Reports a left click inside `min`..`max` without consuming ImGui's own hit testing.
pub fn clicked_in(ui: &Ui, min: Vec2, max: Vec2) -> bool {
    ui.mouse_clicked(MouseButton::Left) && point_in(ui.mouse_position(), min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slider_track_is_inset_by_half_a_thumb_at_each_end() {
        let width = 112.0;
        assert_eq!(slider_fraction(theme::SLIDER_THUMB_SIZE / 2.0, width), 0.0);
        assert_eq!(
            slider_fraction(width - theme::SLIDER_THUMB_SIZE / 2.0, width),
            1.0
        );
        let middle = slider_fraction(width / 2.0, width);
        assert!((middle - 0.5).abs() < 0.001, "midpoint was {middle}");
    }

    #[test]
    fn the_slider_clamps_a_pointer_dragged_past_either_end() {
        assert_eq!(slider_fraction(-40.0, 100.0), 0.0);
        assert_eq!(slider_fraction(400.0, 100.0), 1.0);
    }

    #[test]
    fn point_containment_includes_the_edges() {
        assert!(point_in([10.0, 10.0], [10.0, 10.0], [20.0, 20.0]));
        assert!(point_in([20.0, 20.0], [10.0, 10.0], [20.0, 20.0]));
        assert!(!point_in([9.9, 15.0], [10.0, 10.0], [20.0, 20.0]));
    }

    #[test]
    fn the_hover_timer_fires_only_after_the_delay() {
        let mut timer = HoverTimer::default();
        assert!(!timer.poll("a", true, 0.1, 0.2));
        assert!(timer.poll("a", true, 0.15, 0.2));
    }

    #[test]
    fn moving_to_another_target_restarts_the_hover_timer() {
        let mut timer = HoverTimer::default();
        assert!(!timer.poll("a", true, 0.19, 0.2));
        assert!(!timer.poll("b", true, 0.05, 0.2));
        assert!(timer.poll("b", true, 0.2, 0.2));
    }

    #[test]
    fn leaving_the_target_clears_the_hover_timer() {
        let mut timer = HoverTimer::default();
        assert!(timer.poll("a", true, 1.0, 0.2));
        assert!(!timer.poll("a", false, 0.1, 0.2));
        assert!(!timer.poll("a", true, 0.1, 0.2));
    }
}
