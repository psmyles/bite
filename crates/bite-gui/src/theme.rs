//! The design tokens from `src/renderer/assets/theme.css`, transcribed for the native editor.
//!
//! Every color, size, radius and font role in the interface must come from here, so that a
//! change to the stylesheet has exactly one place to land on this side.
use bite_imgui::{Color, Face, Weight};

// -- Application backgrounds ---------------------------------------------------------

pub const BG: Color = Color::rgb(0x14, 0x14, 0x14);
pub const GAP_COLOR: Color = Color::rgb(0x2b, 0x2b, 0x2b);
pub const PANEL_BG: Color = BG;
pub const PANEL_HEADER_BG: Color = Color::rgb(0x1c, 0x1c, 0x1c);
pub const SEARCH_BG: Color = Color::rgb(0x0e, 0x0e, 0x0e);
pub const TEXT_FIELD_BG: Color = Color::rgb(0x11, 0x11, 0x11);
pub const PREVIEW_BG: Color = Color::rgb(0x00, 0x00, 0x00);
pub const INPUT_BG: Color = Color([1.0, 1.0, 1.0, 0.06]);
pub const INPUT_BORDER: Color = Color([1.0, 1.0, 1.0, 0.12]);
pub const ITEM_HOVER_BG: Color = Color([1.0, 1.0, 1.0, 0.04]);
pub const BTN_SUBTLE_BG: Color = Color([1.0, 1.0, 1.0, 0.05]);

// -- Text, borders, handles and accent -----------------------------------------------

pub const TEXT: Color = Color::rgb(0xa8, 0xa8, 0xa8);
pub const TEXT_BRIGHT: Color = Color::rgb(0xff, 0xff, 0xff);
pub const TEXT_MUTED: Color = TEXT;
pub const BORDER: Color = Color::rgb(0x58, 0x58, 0x58);
pub const HANDLE: Color = Color::rgb(0x29, 0x29, 0x29);
pub const HANDLE_HOT: Color = BORDER;
/// The stylesheet aliases the accent to the body text color rather than a hue.
pub const ACCENT: Color = TEXT;

// -- Scrollbars ----------------------------------------------------------------------

/// The visible bar, a pixel over the stylesheet's five.
pub const SCROLLBAR_THUMB_WIDTH: f32 = 6.0;
/// The track the bar is drawn in, which is not what is seen.
///
/// Dear ImGui insets its grab inside the track by `trunc((width - 2) / 2)` on each side,
/// capped at three, so every track from eight pixels up draws a bar six pixels narrower
/// than itself. Asking for the stylesheet's five drew a three pixel bar, and asking for
/// nine drew exactly the same three.
pub const SCROLLBAR_WIDTH: f32 = SCROLLBAR_THUMB_WIDTH + 6.0;
pub const SCROLLBAR_RADIUS: f32 = SCROLLBAR_THUMB_WIDTH / 2.0;
pub const SCROLLBAR_THUMB: Color = BORDER;
pub const SCROLLBAR_THUMB_HOVER: Color = TEXT;

// -- Inspector text roles --------------------------------------------------------------

/// A name for something below or beside it: a section title, a parameter label. Every one
/// of these is `--text-bright` at six tenths in the stylesheet.
pub const INSPECTOR_LABEL: Color = Color([1.0, 1.0, 1.0, 0.6]);
/// Something the row says in its own right: a field's contents, a dropdown's choice, the
/// wording beside a checkbox. These take the bright text at full strength, so that they do
/// not read as another heading.
pub const INSPECTOR_VALUE: Color = TEXT_BRIGHT;

// -- Semantic status -----------------------------------------------------------------

pub const COLOR_SUCCESS: Color = Color::rgb(0x22, 0xc5, 0x5e);
pub const COLOR_SUCCESS_TEXT: Color = Color::rgb(0x86, 0xef, 0xac);
pub const COLOR_SUCCESS_TEXT_BRIGHT: Color = Color::rgb(0xbb, 0xf7, 0xd0);
pub const COLOR_SUCCESS_MUTED: Color = Color::rgb(0x81, 0xc7, 0x84);
pub const COLOR_ERROR: Color = Color::rgb(0xf8, 0x71, 0x71);
pub const COLOR_ERROR_TEXT: Color = Color::rgb(0xff, 0x90, 0x90);
pub const COLOR_ERROR_BG: Color = Color([1.0, 80.0 / 255.0, 80.0 / 255.0, 0.06]);
pub const COLOR_ERROR_BORDER: Color = Color::rgb(0x7a, 0x20, 0x20);
/// The log window's own palette, from `public/log-viewer.html`. It is darker and flatter
/// than the editor's, because it is a reading surface rather than a working one.
pub mod log {
    use super::Color;
    pub const TOOLBAR_BG: Color = Color::rgb(0x1e, 0x1e, 0x1e);
    pub const TOOLBAR_BORDER: Color = Color::rgb(0x2e, 0x2e, 0x2e);
    pub const LABEL: Color = Color::rgb(0x66, 0x66, 0x66);
    pub const ROW_HOVER: Color = Color::rgb(0x1c, 0x1c, 0x1c);
    pub const TIMESTAMP: Color = Color::rgb(0x77, 0x77, 0x77);
    pub const INFO: Color = Color::rgb(0xe0, 0xe0, 0xe0);
    pub const INFO_BADGE: Color = Color::rgb(0x77, 0x77, 0x77);
    pub const WARNING: Color = Color::rgb(0xf0, 0xc0, 0x40);
    pub const ERROR: Color = Color::rgb(0xe0, 0x5a, 0x5a);
    /// The `[tag]` a message opens with.
    pub const TAG: Color = Color::rgb(0x02, 0xcc, 0xff);
    /// The width the level badge column reserves.
    pub const BADGE_WIDTH: f32 = 36.0;
    pub const ROW_PADDING_X: f32 = 14.0;
    pub const COLUMN_GAP: f32 = 10.0;
    /// How close to the bottom the view must be to keep following new lines.
    pub const FOLLOW_MARGIN: f32 = 40.0;
    /// `line-height: 1.55` on a row, which is what keeps the lines apart.
    pub const LINE_HEIGHT: f32 = 1.55;
}

pub const COLOR_WARNING: Color = Color::rgb(0xf5, 0x9e, 0x0b);
pub const COLOR_WARNING_TEXT: Color = Color::rgb(0xfb, 0xbf, 0x24);
pub const COLOR_DANGER: Color = Color::rgb(0xc0, 0x39, 0x2b);
pub const COLOR_DANGER_TEXT: Color = Color::rgb(0xfc, 0xa5, 0xa5);
pub const COLOR_DANGER_TEXT_BRIGHT: Color = Color::rgb(0xfe, 0xca, 0xca);
pub const COLOR_RENAME_ORIG: Color = Color::rgb(0xb8, 0x9c, 0xfb);

pub const MODAL_OVERLAY_BG: Color = Color([0.0, 0.0, 0.0, 0.65]);

// -- Node type accents ---------------------------------------------------------------

pub const NODE_ACCENT_INPUT: Color = COLOR_SUCCESS;
pub const NODE_ACCENT_IMAGE_OUTPUT: Color = COLOR_WARNING;
pub const NODE_ACCENT_TEXT_OUTPUT: Color = Color::rgb(0x3b, 0x82, 0xf6);
pub const NODE_ACCENT_FLIPBOOK_OUTPUT: Color = Color::rgb(0xa8, 0x55, 0xf7);
pub const NODE_ACCENT_FOLDER_PATH: Color = COLOR_SUCCESS_TEXT;
pub const NODE_ACCENT_SET_INPUT: Color = COLOR_WARNING;

// -- Comment node --------------------------------------------------------------------

pub const COMMENT_BG: Color = Color::rgb(0xfe, 0xf0, 0x8a);
pub const COMMENT_HEADER_BG: Color = Color::rgb(0xfd, 0xe0, 0x47);
pub const COMMENT_TEXT: Color = Color::rgb(0x5c, 0x32, 0x00);
pub const COMMENT_TEXT_BODY: Color = Color::rgb(0x42, 0x20, 0x06);
pub const COMMENT_PLACEHOLDER: Color = Color([113.0 / 255.0, 63.0 / 255.0, 18.0 / 255.0, 0.4]);
pub const COMMENT_SELECTED_BORDER: Color = Color::rgb(0xca, 0x8a, 0x04);

// -- Group node ----------------------------------------------------------------------

const GREY_168: f32 = 168.0 / 255.0;
pub const GROUP_BG: Color = Color([GREY_168, GREY_168, GREY_168, 0.08]);
pub const GROUP_BORDER: Color = Color([GREY_168, GREY_168, GREY_168, 0.25]);
pub const GROUP_SELECTED_BORDER: Color = Color([220.0 / 255.0, 220.0 / 255.0, 220.0 / 255.0, 0.75]);
pub const GROUP_LABEL_INPUT_BORDER: Color = Color([GREY_168, GREY_168, GREY_168, 0.35]);

// -- Canvas node badges --------------------------------------------------------------

pub const BADGE_PREVIEW_COLOR: Color = Color::rgb(0x39, 0xff, 0x14);
pub const BADGE_PROCESSING_COLOR: Color = COLOR_WARNING;

// -- Modal close button --------------------------------------------------------------

pub const MODAL_CLOSE_BTN_SIZE: f32 = 26.0;
pub const MODAL_CLOSE_BTN_BORDER_WIDTH: f32 = 2.0;
pub const MODAL_CLOSE_BTN_RADIUS: f32 = 4.0;

// -- Dropdown surfaces ---------------------------------------------------------------

pub const DROPDOWN_BG: Color = Color::rgb(0x2e, 0x2e, 0x2e);
pub const DROPDOWN_HOVER_BG: Color = Color::rgb(0x38, 0x38, 0x38);
pub const DROPDOWN_ACTIVE_BG: Color = Color::rgb(0x48, 0x48, 0x48);
/// `.dd-item` pads five pixels above and below its label and ten to each side.
pub const DROPDOWN_ITEM_PADDING_Y: f32 = 5.0;
pub const DROPDOWN_ITEM_PADDING_X: f32 = 10.0;
/// The open list's own padding, above the first row and below the last.
pub const DROPDOWN_LIST_PADDING_Y: f32 = 4.0;
/// How many rows the list shows before it scrolls.
pub const DROPDOWN_VISIBLE_ROWS: usize = 8;

// -- Type sizes ----------------------------------------------------------------------

pub const FONT_SIZE_3XL: u16 = 28;
pub const FONT_SIZE_2XL: u16 = 20;
pub const FONT_SIZE_XL: u16 = 17;
pub const FONT_SIZE_LG: u16 = 15;
pub const FONT_SIZE_MD: u16 = 14;
pub const FONT_SIZE_BASE: u16 = 13;
pub const FONT_SIZE_SM: u16 = 12;
pub const FONT_SIZE_XS: u16 = 11;
pub const FONT_SIZE_XXS: u16 = 10;

/// The per-role faces the stylesheet defines, so callers name a role rather than a size.
pub mod face {
    use super::*;

    pub const PANEL_HEADER: Face = Face::ui_weight(FONT_SIZE_BASE, Weight::SemiBold);
    pub const CATEGORY_LABEL: Face = Face::ui_weight(FONT_SIZE_BASE, Weight::Bold);
    pub const NODE_ITEM: Face = Face::ui(FONT_SIZE_BASE);
    pub const SEARCH: Face = Face::ui(FONT_SIZE_BASE);
    pub const HINT: Face = Face::ui(FONT_SIZE_BASE);
    pub const NODE_HEAD: Face = Face::mono(FONT_SIZE_SM);
    pub const PORT_TAG: Face = Face::mono(FONT_SIZE_XS);
    pub const ZOOM_LABEL: Face = Face::mono(FONT_SIZE_MD);
    /// `Filmstrip.svelte` overrides `--text-thumb-name-size` with `--font-size-xs`, so the
    /// rendered size is eleven pixels and not the ten the token names.
    pub const THUMB_NAME: Face = Face::ui(FONT_SIZE_XS);
    pub const BODY: Face = Face::ui(FONT_SIZE_BASE);
    pub const LABEL: Face = Face::ui(FONT_SIZE_SM);
    pub const VALUE: Face = Face::mono(FONT_SIZE_SM);
    pub const SMALL_MONO: Face = Face::mono(FONT_SIZE_XS);
    pub const TINY_MONO: Face = Face::mono(FONT_SIZE_XXS);
    pub const BADGE: Face = Face::ui_weight(FONT_SIZE_XS, Weight::Bold);
    pub const COMMENT_HEADING: Face = Face::ui_weight(FONT_SIZE_MD, Weight::Bold);
    pub const COMMENT_BODY: Face = Face::ui(FONT_SIZE_MD);
    pub const MODAL_TITLE: Face = Face::ui_weight(FONT_SIZE_BASE, Weight::SemiBold);
    pub const STAT_VALUE: Face = Face::mono_weight(FONT_SIZE_3XL, Weight::SemiBold);
    pub const BUTTON: Face = Face::ui(FONT_SIZE_BASE);
}

// -- Node graph ----------------------------------------------------------------------

pub const NODE_BG: Color = Color::rgb(0x21, 0x21, 0x21);
pub const NODE_HEAD_BG: Color = Color::rgb(0x3d, 0x3d, 0x3d);
pub const NODE_BORDER: Color = BORDER;
pub const NODE_TEXT: Color = Color::rgb(0xe2, 0xe2, 0xe8);
pub const NODE_RADIUS: f32 = 6.0;
pub const NODE_MIN_WIDTH: f32 = 150.0;
/// The built-in workflow cards are wider than process cards.
pub const NODE_WORKFLOW_WIDTH: f32 = 190.0;
pub const NODE_COMMENT_MIN_WIDTH: f32 = 210.0;
/// A card grows to fit its content, so these are the paddings that content sits inside.
/// `.node-head` pads twelve pixels a side, or thirty-four when it carries the bypass tick.
pub const NODE_HEAD_INSET: f32 = 12.0;
pub const NODE_HEAD_TOGGLE_INSET: f32 = 34.0;
/// The least space left between a row's left and right halves, which `space-between` would
/// otherwise let close to nothing.
pub const NODE_ROW_MIN_GAP: f32 = 12.0;
/// `.param-value` pads six pixels away from the name beside it.
pub const NODE_VALUE_GAP: f32 = 6.0;
pub const NODE_SWATCH_SIZE: f32 = 20.0;
/// The ceiling a card label is ellipsized against, so one long value cannot stretch a card
/// across the canvas.
pub const NODE_MAX_WIDTH: f32 = 320.0;
pub const NODE_SELECTED_RING: Color = Color([1.0, 1.0, 1.0, 0.8]);
pub const NODE_SHADOW: Color = Color([0.0, 0.0, 0.0, 1.0]);

// Layout numerics, from the `--node-layout-*` tokens.
pub const NODE_LAYOUT_HEADER_H: f32 = 28.0;
pub const NODE_LAYOUT_PORT_PAD: f32 = 5.0;
pub const NODE_LAYOUT_PORT_ROW_H: f32 = 20.0;
pub const NODE_LAYOUT_PARAM_PAD: f32 = 4.0;
pub const NODE_LAYOUT_PARAM_ROW_H: f32 = 22.0;
pub const NODE_LAYOUT_SEP_H: f32 = 1.0;
pub const NODE_FOOTER_H: f32 = 22.0;
/// Horizontal inset for port labels and row content.
pub const NODE_ROW_INSET: f32 = 10.0;

pub const HANDLE_SIZE: f32 = 10.0;
pub const HANDLE_BORDER_WIDTH: f32 = 2.0;

// -- Canvas background ---------------------------------------------------------------

pub const GRAPH_BG_GAP: f32 = 32.0;
pub const GRAPH_BG_LINE_WIDTH: f32 = 1.0;
pub const GRAPH_BG_COLOR: Color = Color::rgb(32, 32, 32);
pub const GRAPH_BG_BASE_COLOR: Color = BG;

// -- Wires ---------------------------------------------------------------------------

pub const PORT_COLOR_IMAGE: Color = Color::rgb(0xff, 0x8c, 0x3f);
pub const PORT_COLOR_MASK: Color = Color::rgb(0xd8, 0xa4, 0xfc);
pub const PORT_COLOR_NUMBER: Color = Color::rgb(0x22, 0xd3, 0xee);
pub const PORT_COLOR_STRING: Color = Color::rgb(0x22, 0xc5, 0x5e);
pub const PORT_COLOR_BOOLEAN: Color = Color::rgb(0xea, 0xb3, 0x08);
pub const PORT_COLOR_COLOR: Color = Color::rgb(0xfc, 0x86, 0xbc);
pub const PORT_COLOR_VECTOR2: Color = Color::rgb(0xfb, 0x92, 0x3c);
pub const PORT_COLOR_VECTOR3: Color = Color::rgb(0xa5, 0xb4, 0xfc);
pub const PORT_COLOR_VECTOR4: Color = Color::rgb(0x2d, 0xd4, 0xbf);
pub const PORT_COLOR_NUMERIC: Color = Color::rgb(0x94, 0xa3, 0xb8);
pub const PORT_COLOR_ANY: Color = Color::rgb(0xff, 0xff, 0xff);
pub const PORT_COLOR_PATH: Color = Color::rgb(0x86, 0xef, 0xac);
/// The fallback used when a handle's type cannot be resolved.
pub const EDGE_STROKE: Color = Color::rgb(0x6b, 0x72, 0x80);
pub const EDGE_STROKE_SELECTED: Color = Color::rgb(0xff, 0xff, 0xff);
pub const EDGE_WIDTH: f32 = 2.0;
pub const EDGE_WIDTH_SELECTED: f32 = 3.0;

pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 2.0;
pub const ZOOM_STEP: f32 = 1.2;
pub const ZOOM_LABEL_BOTTOM: f32 = 22.0;
pub const ZOOM_LABEL_COLOR: Color = Color([1.0, 1.0, 1.0, 0.8]);

pub const CONNECTION_RADIUS: f32 = 20.0;
pub const NODE_DRAG_THRESHOLD: f32 = 1.0;
pub const GROUP_PADDING: f32 = 40.0;
pub const DUPLICATE_OFFSET: f32 = 20.0;

// -- Panels --------------------------------------------------------------------------

pub const LIBRARY_ITEM_HOVER_BG: Color = Color::rgb(0x3a, 0x3a, 0x3a);
pub const LIBRARY_SEARCH_RADIUS: f32 = 4.0;
/// The box `.collapse-icon` reserves for the minus and plus, which it centres them in.
pub const LIBRARY_COLLAPSE_ICON_WIDTH: f32 = 10.0;
pub const OVERLAY_OPACITY: f32 = 0.35;
/// The hover tooltip's wrapping width, from `NodeLibrary.svelte`'s placement arithmetic.
pub const TOOLTIP_WIDTH: f32 = 320.0;

// -- Menu bar dropdowns, from `MenuBar.svelte` ---------------------------------------

/// `.dropdown li button` pads five pixels above and below its label.
pub const MENU_ITEM_PADDING_Y: f32 = 5.0;
/// `.dropdown` pads four pixels above its first row and below its last.
pub const MENU_DROPDOWN_PADDING_Y: f32 = 4.0;
/// The window padding the dropdown is given.
///
/// The twelve is the row's own horizontal padding. The vertical figure carries the row's
/// padding as well as the dropdown's, because Dear ImGui places the first row's text at the
/// window padding and only grows its highlight into half the item spacing above it.
pub const MENU_DROPDOWN_PADDING: [f32; 2] =
    [12.0, MENU_DROPDOWN_PADDING_Y + MENU_ITEM_PADDING_Y];
/// The gap the row leaves between its label and its shortcut.
pub const MENU_ITEM_GAP: f32 = 8.0;
/// The fit-view control that stands in for the Electron minimap.
pub const FIT_BUTTON_HEIGHT: f32 = 26.0;
pub const FIT_VIEW_PADDING: f32 = 48.0;
pub const SHADOW_POPOVER: Color = Color([0.0, 0.0, 0.0, 0.4]);
pub const DISABLED_OPACITY: f32 = 0.4;

pub const PANEL_RADIUS: f32 = 8.0;
pub const PANEL_GAP: f32 = 6.0;
pub const SHELL_PADDING: f32 = 6.0;
pub const RESIZE_HANDLE_SIZE: f32 = PANEL_GAP;
pub const PANEL_HEADER_HEIGHT: f32 = 36.0;

pub const LEFT_PANEL_DEFAULT: f32 = 220.0;
pub const LEFT_PANEL_MIN: f32 = 160.0;
pub const LEFT_PANEL_MAX: f32 = 400.0;
pub const RIGHT_PANEL_DEFAULT: f32 = 280.0;
pub const RIGHT_PANEL_MIN: f32 = 200.0;
pub const RIGHT_PANEL_MAX: f32 = 480.0;
pub const FILMSTRIP_DEFAULT: f32 = 120.0;
pub const FILMSTRIP_MIN: f32 = 80.0;
pub const FILMSTRIP_MAX: f32 = 220.0;
pub const INSPECTOR_DEFAULT: f32 = 0.65;
pub const INSPECTOR_MIN: f32 = 0.30;
pub const INSPECTOR_MAX: f32 = 0.80;

// -- Context menu --------------------------------------------------------------------

pub const CTX_BG: Color = PANEL_HEADER_BG;
pub const CTX_BORDER: Color = Color::rgb(0x38, 0x38, 0x38);
pub const CTX_RADIUS: f32 = 6.0;
pub const CTX_SHADOW: Color = Color([0.0, 0.0, 0.0, 0.65]);
pub const CTX_WIDTH: f32 = 200.0;
pub const CTX_SUB_WIDTH: f32 = 190.0;
pub const CTX_MAX_HEIGHT: f32 = 420.0;
/// One row in the creation menu, its flyout and its action list.
pub const CTX_ROW_HEIGHT: f32 = 26.0;
/// The padding above the search field and below the last row.
pub const CTX_PADDING_Y: f32 = 4.0;
/// The gap between the search field and the first row.
pub const CTX_SEARCH_GAP: f32 = 4.0;
/// The creation menu's search field, which is shorter than an inspector's.
pub const CTX_SEARCH_HEIGHT: f32 = 26.0;
pub const CTX_SEARCH_BG: Color = TEXT_FIELD_BG;
pub const CTX_TEXT: Color = Color::rgb(0xc8, 0xc8, 0xc8);
pub const CTX_TEXT_MUTED: Color = TEXT;
pub const CTX_ITEM_HOVER_BG: Color = Color::rgb(0x2c, 0x2c, 0x2c);
pub const CTX_SEPARATOR: Color = CTX_BORDER;
/// Hover delay before a description appears in the creation menu and the library.
pub const TOOLTIP_DELAY_MENU: f32 = 0.2;
/// The slower delay used by node headers on the canvas.
pub const TOOLTIP_DELAY_NODE: f32 = 1.0;

// -- Inspector and sliders -----------------------------------------------------------

pub const INSPECTOR_PARAM_GAP: f32 = 5.0;
/// The gap `.param-label` puts between the label and its badge.
pub const INSPECTOR_LABEL_GAP: f32 = 6.0;
pub const INSPECTOR_PARAM_PADDING: [f32; 2] = [12.0, 7.0];
pub const INSPECTOR_ROW_BORDER_MIX: f32 = 25.0;
pub const SLIDER_WRAP_GAP: f32 = 8.0;
pub const SLIDER_TRACK_HEIGHT: f32 = 3.0;
pub const SLIDER_TRACK_RADIUS: f32 = 2.0;
pub const SLIDER_TRACK_MIX: f32 = 60.0;
pub const SLIDER_THUMB_SIZE: f32 = 12.0;
pub const SLIDER_VAL_WIDTH: f32 = 52.0;
pub const SLIDER_VAL_RADIUS: f32 = 4.0;
pub const SLIDER_VAL_PADDING: [f32; 2] = [6.0, 4.0];

// -- Controls ------------------------------------------------------------------------

pub const BUTTON_HEIGHT: f32 = 30.0;
pub const BUTTON_PADDING_X: f32 = 16.0;
pub const BUTTON_RADIUS: f32 = 4.0;
pub const INPUT_HEIGHT: f32 = 30.0;
pub const INPUT_PADDING_X: f32 = 8.0;
pub const INPUT_RADIUS: f32 = 4.0;
pub const INPUT_BORDER_WIDTH: f32 = 2.0;
pub const CHECKBOX_SIZE: f32 = 16.0;
pub const CHECKBOX_BORDER_WIDTH: f32 = 1.5;
pub const CHECKBOX_RADIUS: f32 = 3.0;

/// The three button styles: resting background, border and text for each.
pub struct ButtonStyle {
    pub background: Color,
    pub border: Color,
    pub text: Color,
    pub hover_background: Color,
    pub hover_border: Color,
    pub hover_text: Color,
}

pub fn button_neutral() -> ButtonStyle {
    ButtonStyle {
        background: PANEL_HEADER_BG,
        border: BORDER,
        text: TEXT_BRIGHT,
        hover_background: TEXT_BRIGHT.mix(6.0, PANEL_HEADER_BG),
        hover_border: ACCENT,
        hover_text: TEXT_BRIGHT,
    }
}

pub fn button_primary() -> ButtonStyle {
    ButtonStyle {
        background: COLOR_SUCCESS.mix(14.0, PANEL_HEADER_BG),
        border: COLOR_SUCCESS.mix(40.0, Color::TRANSPARENT),
        text: COLOR_SUCCESS_TEXT,
        hover_background: COLOR_SUCCESS.mix(22.0, PANEL_HEADER_BG),
        hover_border: COLOR_SUCCESS.mix(65.0, Color::TRANSPARENT),
        hover_text: COLOR_SUCCESS_TEXT_BRIGHT,
    }
}

pub fn button_danger() -> ButtonStyle {
    ButtonStyle {
        background: COLOR_DANGER.mix(14.0, PANEL_HEADER_BG),
        border: COLOR_DANGER.mix(40.0, Color::TRANSPARENT),
        text: COLOR_DANGER_TEXT,
        hover_background: COLOR_DANGER.mix(22.0, PANEL_HEADER_BG),
        hover_border: COLOR_DANGER.mix(65.0, Color::TRANSPARENT),
        hover_text: COLOR_DANGER_TEXT_BRIGHT,
    }
}

/// The disabled appearance shared by all three button styles.
pub fn button_disabled() -> ButtonStyle {
    
    ButtonStyle {
        background: PANEL_HEADER_BG,
        border: BORDER.mix(50.0, Color::TRANSPARENT),
        text: TEXT_BRIGHT.mix(30.0, Color::TRANSPARENT),
        hover_background: PANEL_HEADER_BG,
        hover_border: BORDER.mix(50.0, Color::TRANSPARENT),
        hover_text: TEXT_BRIGHT.mix(30.0, Color::TRANSPARENT),
    }
}

/// The border used between inspector rows.
pub fn inspector_row_border() -> Color {
    BORDER.mix(INSPECTOR_ROW_BORDER_MIX, Color::TRANSPARENT)
}

/// Tints a node header with its accent, reproducing the stylesheet's `color-mix`.
pub fn node_header(accent: Color, percent: f32) -> Color {
    accent.mix(percent, NODE_HEAD_BG)
}

/// Applies the persistent style once, at startup. Per-widget overrides are pushed and
/// popped where they are drawn.
pub fn apply_base_style() {
    use bite_imgui::{Style, StyleColor as C, StyleVar as V};
    let mut style = Style;
    style.set(V::WindowPadding([0.0, 0.0]));
    style.set(V::WindowRounding(PANEL_RADIUS));
    style.set(V::WindowBorderSize(0.0));
    style.set(V::ChildRounding(0.0));
    style.set(V::ChildBorderSize(0.0));
    style.set(V::PopupRounding(CTX_RADIUS));
    style.set(V::PopupBorderSize(1.0));
    style.set(V::FramePadding([INPUT_PADDING_X, 6.0]));
    style.set(V::FrameRounding(INPUT_RADIUS));
    style.set(V::FrameBorderSize(0.0));
    style.set(V::ItemSpacing([0.0, 0.0]));
    style.set(V::ItemInnerSpacing([6.0, 4.0]));
    style.set(V::ScrollbarSize(SCROLLBAR_WIDTH));
    style.set(V::ScrollbarRounding(SCROLLBAR_RADIUS));
    style.set(V::GrabMinSize(SLIDER_THUMB_SIZE));
    style.set(V::GrabRounding(SLIDER_THUMB_SIZE / 2.0));
    style.set(V::DisabledAlpha(DISABLED_OPACITY));

    style.set_color(C::Text, TEXT);
    style.set_color(C::TextDisabled, TEXT.with_alpha(0.5));
    style.set_color(C::WindowBg, PANEL_BG);
    style.set_color(C::ChildBg, Color::TRANSPARENT);
    style.set_color(C::PopupBg, CTX_BG);
    style.set_color(C::Border, CTX_BORDER);
    style.set_color(C::FrameBg, TEXT_FIELD_BG);
    style.set_color(C::FrameBgHovered, TEXT_FIELD_BG);
    style.set_color(C::FrameBgActive, TEXT_FIELD_BG);
    style.set_color(C::Button, PANEL_HEADER_BG);
    style.set_color(C::ButtonHovered, LIBRARY_ITEM_HOVER_BG);
    style.set_color(C::ButtonActive, DROPDOWN_ACTIVE_BG);
    style.set_color(C::Header, DROPDOWN_HOVER_BG);
    style.set_color(C::HeaderHovered, DROPDOWN_ACTIVE_BG);
    style.set_color(C::HeaderActive, DROPDOWN_ACTIVE_BG);
    style.set_color(C::Separator, BORDER);
    style.set_color(C::CheckMark, COLOR_SUCCESS);
    style.set_color(C::SliderGrab, ACCENT);
    style.set_color(C::SliderGrabActive, TEXT_BRIGHT);
    style.set_color(C::ScrollbarBg, Color::TRANSPARENT);
    style.set_color(C::ScrollbarGrab, SCROLLBAR_THUMB);
    style.set_color(C::ScrollbarGrabHovered, SCROLLBAR_THUMB_HOVER);
    style.set_color(C::ScrollbarGrabActive, SCROLLBAR_THUMB_HOVER);
    style.set_color(C::TextSelectedBg, ACCENT.with_alpha(0.35));
    style.set_color(C::NavHighlight, Color::TRANSPARENT);
    style.set_color(C::PlotHistogram, COLOR_SUCCESS);
    // The popup's own dimming is what darkens the editor behind a dialog.
    style.set_color(C::ModalWindowDimBg, MODAL_OVERLAY_BG);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dear ImGui insets the grab inside the track, capped at three pixels a side, so the
    /// bar that is actually seen is the track less six. This is the arithmetic that made
    /// five and nine draw the same three pixel bar.
    fn imgui_grab_width(track: f32) -> f32 {
        let inset = ((track - 2.0) / 2.0).trunc().clamp(0.0, 3.0);
        track - inset * 2.0
    }

    #[test]
    fn a_label_and_a_value_are_told_apart_in_the_inspector() {
        // A checkbox row was drawn in the label colour, which made its wording read as a
        // section title rather than as the thing the row says.
        assert_ne!(INSPECTOR_LABEL, INSPECTOR_VALUE);
        assert_eq!(INSPECTOR_VALUE, TEXT_BRIGHT);
        // The difference is opacity, as the stylesheet makes it: the same white, dimmed.
        assert_eq!(INSPECTOR_LABEL.0[..3], INSPECTOR_VALUE.0[..3]);
        assert_eq!(INSPECTOR_LABEL.0[3], 0.6);
    }

    #[test]
    fn the_scrollbar_track_is_wide_enough_to_draw_the_bar_it_names() {
        assert_eq!(imgui_grab_width(SCROLLBAR_WIDTH), SCROLLBAR_THUMB_WIDTH);
        // What the stylesheet's own number would have drawn, and what widening it to nine
        // drew: the same bar, which is why the first attempt changed nothing.
        assert_eq!(imgui_grab_width(5.0), 3.0);
        assert_eq!(imgui_grab_width(9.0), 3.0);
    }

    #[test]
    fn node_header_tints_match_the_stylesheet_percentages() {
        // Input, image output, text output, flipbook and folder path all mix at 18 percent,
        // while Process As Set mixes at 20.
        let input = node_header(NODE_ACCENT_INPUT, 18.0);
        let set = node_header(NODE_ACCENT_SET_INPUT, 20.0);
        assert!(input.0[1] > NODE_HEAD_BG.0[1], "green channel must lift");
        assert!(set.0[0] > NODE_HEAD_BG.0[0], "red channel must lift");
        assert_eq!(input.0[3], 1.0);
    }

    #[test]
    fn accent_and_border_alias_the_stylesheet_values() {
        assert_eq!(ACCENT, TEXT);
        assert_eq!(NODE_BORDER, BORDER);
        assert_eq!(PANEL_BG, BG);
        assert_eq!(CTX_BG, PANEL_HEADER_BG);
    }
}
