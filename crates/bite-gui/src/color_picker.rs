//! The inline colour editor from `src/renderer/components/ColorPicker.svelte`.
//!
//! Dear ImGui's own picker is a wheel or a square with its own bars and text boxes; the
//! Electron editor has a saturation square, a hue bar, a mode drop-down, one gradient slider
//! and number box per channel, an alpha row and a hex row, all laid out inline in the
//! inspector rather than behind a swatch. This draws that, and the conversions are ported
//! from `colorConversions.ts` so the numbers a mode shows are the same ones.
use crate::{controls, theme};
use bite_imgui::{Color, DrawListRef, InputFlags, MouseCursor, Rounding, Ui, Vec2};
use std::collections::HashMap;

// -- Conversions, from `colorConversions.ts` ------------------------------------------

pub fn c01(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

pub fn c255(value: f32) -> u8 {
    (c01(value) * 255.0).round() as u8
}

pub fn rgb_to_hsv(red: f32, green: f32, blue: f32) -> [f32; 3] {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let mut hue = 0.0;
    if delta > 0.0 {
        hue = if max == red {
            (((green - blue) / delta) % 6.0 + 6.0) % 6.0
        } else if max == green {
            (blue - red) / delta + 2.0
        } else {
            (red - green) / delta + 4.0
        } * 60.0;
    }
    [hue, if max > 0.0 { delta / max } else { 0.0 }, max]
}

pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let chroma = value * saturation;
    let second = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let base = value - chroma;
    let sector = (hue / 60.0).floor().rem_euclid(6.0) as usize;
    let [red, green, blue] = match sector {
        0 => [chroma, second, 0.0],
        1 => [second, chroma, 0.0],
        2 => [0.0, chroma, second],
        3 => [0.0, second, chroma],
        4 => [second, 0.0, chroma],
        _ => [chroma, 0.0, second],
    };
    [red + base, green + base, blue + base]
}

/// sRGB to linear light.
fn slin(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light back to sRGB.
fn sdelin(channel: f32) -> f32 {
    let value = channel.max(0.0);
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub fn rgb_to_lab(red: f32, green: f32, blue: f32) -> [f32; 3] {
    let x = 0.412_456_4 * slin(red) + 0.357_576_1 * slin(green) + 0.180_437_5 * slin(blue);
    let y = 0.212_672_9 * slin(red) + 0.715_152_2 * slin(green) + 0.072_175 * slin(blue);
    let z = 0.019_333_9 * slin(red) + 0.119_192 * slin(green) + 0.950_304_1 * slin(blue);
    let f = |t: f32| {
        if t > 0.008_856 {
            t.cbrt()
        } else {
            (903.3 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn lab_to_rgb(lightness: f32, a: f32, b: f32) -> [f32; 3] {
    let fy = (lightness + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let inverse = |t: f32| {
        if t > 0.2069 {
            t * t * t
        } else {
            (116.0 * t - 16.0) / 903.3
        }
    };
    let (x, y, z) = (0.950_47 * inverse(fx), inverse(fy), 1.088_83 * inverse(fz));
    [
        c01(sdelin(3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z)),
        c01(sdelin(-0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z)),
        c01(sdelin(0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z)),
    ]
}

pub fn rgb_to_cmyk(red: f32, green: f32, blue: f32) -> [f32; 4] {
    let key = 1.0 - red.max(green).max(blue);
    if key >= 1.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let ink = 1.0 - key;
    [
        c01((1.0 - red - key) / ink),
        c01((1.0 - green - key) / ink),
        c01((1.0 - blue - key) / ink),
        key,
    ]
}

pub fn cmyk_to_rgb(cyan: f32, magenta: f32, yellow: f32, key: f32) -> [f32; 3] {
    [
        c01((1.0 - cyan) * (1.0 - key)),
        c01((1.0 - magenta) * (1.0 - key)),
        c01((1.0 - yellow) * (1.0 - key)),
    ]
}

// -- Modes ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Rgb01,
    Rgb255,
    Hsv,
    Lab,
    Cmyk,
}

pub const MODE_ORDER: [Mode; 5] = [Mode::Rgb01, Mode::Rgb255, Mode::Hsv, Mode::Lab, Mode::Cmyk];

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rgb01 => "RGB 0-1",
            Self::Rgb255 => "RGB 0-255",
            Self::Hsv => "HSV",
            Self::Lab => "LAB",
            Self::Cmyk => "CMYK",
        }
    }
}

/// One editable channel of a mode.
#[derive(Clone, Copy, Debug)]
pub struct Chan {
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub decimals: usize,
}

const fn chan(label: &'static str, min: f32, max: f32, decimals: usize) -> Chan {
    Chan {
        label,
        min,
        max,
        decimals,
    }
}

const RGB01: [Chan; 3] = [
    chan("R", 0.0, 1.0, 3),
    chan("G", 0.0, 1.0, 3),
    chan("B", 0.0, 1.0, 3),
];
const RGB255: [Chan; 3] = [
    chan("R", 0.0, 255.0, 0),
    chan("G", 0.0, 255.0, 0),
    chan("B", 0.0, 255.0, 0),
];
const HSV: [Chan; 3] = [
    chan("H", 0.0, 360.0, 1),
    chan("S", 0.0, 1.0, 3),
    chan("V", 0.0, 1.0, 3),
];
const LAB: [Chan; 3] = [
    chan("L", 0.0, 100.0, 1),
    chan("a", -128.0, 127.0, 1),
    chan("b", -128.0, 127.0, 1),
];
const CMYK: [Chan; 4] = [
    chan("C", 0.0, 1.0, 3),
    chan("M", 0.0, 1.0, 3),
    chan("Y", 0.0, 1.0, 3),
    chan("K", 0.0, 1.0, 3),
];

pub fn channels(mode: Mode) -> &'static [Chan] {
    match mode {
        Mode::Rgb01 => &RGB01,
        Mode::Rgb255 => &RGB255,
        Mode::Hsv => &HSV,
        Mode::Lab => &LAB,
        Mode::Cmyk => &CMYK,
    }
}

/// The colour in the terms the mode shows. HSV reads the picker's own hue so that a grey
/// still remembers which hue it came from, as the Svelte component does.
pub fn to_mode(mode: Mode, rgb: [f32; 3], hsv: [f32; 3]) -> Vec<f32> {
    match mode {
        Mode::Rgb01 => rgb.to_vec(),
        Mode::Rgb255 => rgb.iter().map(|channel| channel * 255.0).collect(),
        Mode::Hsv => hsv.to_vec(),
        Mode::Lab => rgb_to_lab(rgb[0], rgb[1], rgb[2]).to_vec(),
        Mode::Cmyk => rgb_to_cmyk(rgb[0], rgb[1], rgb[2]).to_vec(),
    }
}

pub fn from_mode(mode: Mode, values: &[f32]) -> [f32; 3] {
    let at = |index: usize| values.get(index).copied().unwrap_or(0.0);
    match mode {
        Mode::Rgb01 => [c01(at(0)), c01(at(1)), c01(at(2))],
        Mode::Rgb255 => [c01(at(0) / 255.0), c01(at(1) / 255.0), c01(at(2) / 255.0)],
        Mode::Hsv => hsv_to_rgb(at(0).clamp(0.0, 360.0), c01(at(1)), c01(at(2))),
        Mode::Lab => lab_to_rgb(at(0), at(1), at(2)),
        Mode::Cmyk => cmyk_to_rgb(c01(at(0)), c01(at(1)), c01(at(2)), c01(at(3))),
    }
}

pub fn to_hex(rgb: [f32; 3]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        c255(rgb[0]),
        c255(rgb[1]),
        c255(rgb[2])
    )
}

/// The colour a six-digit hex string names, or nothing when it is not one.
pub fn from_hex(text: &str) -> Option<[f32; 3]> {
    let digits = text.trim().trim_start_matches('#').trim();
    if digits.len() != 6 {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&digits[range], 16)
            .ok()
            .map(|byte| f32::from(byte) / 255.0)
    };
    Some([channel(0..2)?, channel(2..4)?, channel(4..6)?])
}

/// The value a channel box shows, to the decimals its mode asks for.
pub fn format_channel(value: f32, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

// -- Per-picker state -----------------------------------------------------------------

/// What one picker remembers between frames.
///
/// The hue and saturation are kept beside the colour rather than derived from it every
/// frame: black and white have no hue of their own, so dragging the square down to the
/// bottom edge and back up would otherwise come back red.
#[derive(Clone, Debug)]
pub struct State {
    pub mode: Mode,
    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
    /// True while the square or the hue bar is being dragged, which holds the hue steady.
    pub picking: bool,
    hex_draft: String,
    hex_editing: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            hue: 0.0,
            saturation: 1.0,
            brightness: 1.0,
            picking: false,
            hex_draft: String::new(),
            hex_editing: false,
        }
    }
}

impl State {
    /// Takes the hue, saturation and brightness from `rgb` unless a drag is holding them.
    pub fn sync(&mut self, rgb: [f32; 3]) {
        if self.picking {
            return;
        }
        let [hue, saturation, brightness] = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
        self.hue = hue;
        self.saturation = saturation;
        self.brightness = brightness;
    }

    pub fn rgb(&self) -> [f32; 3] {
        hsv_to_rgb(self.hue, self.saturation, self.brightness)
    }
}

/// The pickers a panel has on screen, one per parameter.
#[derive(Default)]
pub struct States(HashMap<String, State>);

impl States {
    pub fn get(&mut self, id: &str) -> &mut State {
        self.0.entry(id.to_string()).or_default()
    }
}

// -- Geometry, from the component's stylesheet ----------------------------------------

/// `.cpk` rounds itself by six and clips, so the square's top corners are cut.
const PICKER_RADIUS: f32 = 6.0;
/// `.sq` is `aspect-ratio: 4 / 3`.
const SQUARE_ASPECT: f32 = 3.0 / 4.0;
const SQUARE_CURSOR: f32 = 12.0;
const HUE_HEIGHT: f32 = 12.0;
const HUE_MARGIN_X: f32 = 8.0;
const HUE_MARGIN_TOP: f32 = 8.0;
const HUE_MARGIN_BOTTOM: f32 = 2.0;
const HUE_THUMB: f32 = 14.0;
const MODE_PADDING_TOP: f32 = 6.0;
const MODE_PADDING_BOTTOM: f32 = 2.0;
const ROW_PADDING_X: f32 = 8.0;
const ROW_PADDING_Y: f32 = 3.0;
const ROW_GAP: f32 = 6.0;
const LABEL_WIDTH: f32 = 12.0;
const TRACK_HEIGHT: f32 = 6.0;
const TRACK_THUMB: f32 = 12.0;
const NUMBER_WIDTH: f32 = 58.0;
const HEX_MARGIN_TOP: f32 = 4.0;
const HEX_PADDING_TOP: f32 = 6.0;
const HEX_PADDING_BOTTOM: f32 = 8.0;
const HEX_GAP: f32 = 7.0;
const HEX_LABEL_WIDTH: f32 = 20.0;
const HEX_SWATCH: f32 = 30.0;
/// The number of steps a channel ramp is built from, as `chanGradient` uses.
const RAMP_STEPS: usize = 6;

/// The height the picker takes at a given width, so a caller can lay out around it.
pub fn height(width: f32, mode: Mode) -> f32 {
    let rows = channels(mode).len() + 1;
    (width * SQUARE_ASPECT).round()
        + HUE_MARGIN_TOP
        + HUE_HEIGHT
        + HUE_MARGIN_BOTTOM
        + MODE_PADDING_TOP
        + theme::INPUT_HEIGHT
        + MODE_PADDING_BOTTOM
        + rows as f32 * (theme::INPUT_HEIGHT + ROW_PADDING_Y * 2.0)
        + HEX_MARGIN_TOP
        + HEX_PADDING_TOP
        + HEX_SWATCH
        + HEX_PADDING_BOTTOM
}

// -- Drawing --------------------------------------------------------------------------

/// Paints `background` back over the corners of an unrounded fill.
///
/// Dear ImGui's gradient fills take no corner radius, so a gradient drawn inside a rounded
/// box spills into the corners. Each corner is covered with a fan of triangles between the
/// square corner and the arc the fill should have followed.
fn round_corners(
    list: &DrawListRef<'_>,
    min: Vec2,
    max: Vec2,
    radius: f32,
    background: Color,
    corners: Rounding,
) {
    if radius <= 0.0 {
        return;
    }
    let top = matches!(corners, Rounding::All | Rounding::Top);
    let bottom = matches!(corners, Rounding::All | Rounding::Bottom);
    // Each corner, the angle its arc starts at, and whether this rounding includes it.
    let wanted = [
        ([min[0], min[1]], [min[0] + radius, min[1] + radius], 180.0, top),
        ([max[0], min[1]], [max[0] - radius, min[1] + radius], 270.0, top),
        ([max[0], max[1]], [max[0] - radius, max[1] - radius], 0.0, bottom),
        ([min[0], max[1]], [min[0] + radius, max[1] - radius], 90.0, bottom),
    ];
    for (corner, centre, start, include) in wanted {
        if !include {
            continue;
        }
        const SEGMENTS: usize = 8;
        let mut previous = None;
        for step in 0..=SEGMENTS {
            let angle = (start + 90.0 * step as f32 / SEGMENTS as f32).to_radians();
            let point = [
                centre[0] + radius * angle.cos(),
                centre[1] + radius * angle.sin(),
            ];
            if let Some(last) = previous {
                list.triangle(corner, last, point, background);
            }
            previous = Some(point);
        }
    }
}

/// A horizontal ramp through `stops`, rounded by `radius` against `background`.
fn ramp(
    list: &DrawListRef<'_>,
    min: Vec2,
    max: Vec2,
    stops: &[Color],
    radius: f32,
    background: Color,
) {
    if stops.len() < 2 {
        return;
    }
    let span = (max[0] - min[0]) / (stops.len() - 1) as f32;
    for (index, pair) in stops.windows(2).enumerate() {
        let left = min[0] + span * index as f32;
        list.rect_gradient_x([left, min[1]], [left + span, max[1]], pair[0], pair[1]);
    }
    round_corners(list, min, max, radius, background, Rounding::All);
}

/// The white circle a slider or the hue bar rides on, with the stylesheet's dark ring.
fn thumb(list: &DrawListRef<'_>, centre: Vec2, size: f32) {
    let radius = size / 2.0;
    list.circle(centre, radius, Color([0.0, 0.0, 0.0, 0.35]));
    list.circle(centre, radius - 1.0, Color::rgb(0xff, 0xff, 0xff));
}

/// Draws the picker and reports whether the colour changed. `background` is the surface it
/// sits on, which the rounded corners are cut back to.
pub fn draw(
    ui: &mut Ui,
    id: &str,
    value: &mut [f32; 4],
    width: f32,
    readonly: bool,
    states: &mut States,
    background: Color,
) -> bool {
    let state = states.get(id);
    state.sync([value[0], value[1], value[2]]);
    let mut changed = false;
    let _scope = ui.push_id(id);

    // -- Saturation and brightness square --
    let square_height = (width * SQUARE_ASPECT).round();
    let origin = ui.cursor_screen_position();
    let square_max = [origin[0] + width, origin[1] + square_height];
    ui.invisible_button("##square", [width, square_height]);
    let square_active = ui.item_active() && !readonly;
    if ui.item_hovered() && !readonly {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    if square_active {
        let mouse = ui.mouse_position();
        let state = states.get(id);
        state.saturation = c01((mouse[0] - origin[0]) / width.max(1.0));
        state.brightness = c01(1.0 - (mouse[1] - origin[1]) / square_height.max(1.0));
        changed = true;
    }

    let state = states.get(id);
    let hue_rgb = hsv_to_rgb(state.hue, 1.0, 1.0);
    let hue_color = Color([hue_rgb[0], hue_rgb[1], hue_rgb[2], 1.0]);
    let (saturation, brightness) = (state.saturation, state.brightness);
    {
        let list = ui.draw_list();
        list.rect(origin, square_max, hue_color, 0.0, Rounding::None);
        // `.sq-white` runs white to transparent across, `.sq-black` transparent to black down.
        list.rect_gradient_x(
            origin,
            square_max,
            Color::rgb(0xff, 0xff, 0xff),
            Color([1.0, 1.0, 1.0, 0.0]),
        );
        list.rect_gradient(
            origin,
            square_max,
            Color([0.0, 0.0, 0.0, 0.0]),
            Color::rgb(0, 0, 0),
        );
        round_corners(
            &list,
            origin,
            square_max,
            PICKER_RADIUS,
            background,
            Rounding::Top,
        );
        let cursor = [
            origin[0] + saturation * width,
            origin[1] + (1.0 - brightness) * square_height,
        ];
        // `.sq` clips, so the ring is cut off at the edges rather than spilling over the
        // label above it when the colour sits in a corner.
        list.push_clip(origin, square_max, true);
        let radius = SQUARE_CURSOR / 2.0;
        list.circle_outline(cursor, radius + 0.5, Color([0.0, 0.0, 0.0, 0.5]), 1.0);
        list.circle_outline(cursor, radius - 1.0, Color::rgb(0xff, 0xff, 0xff), 2.0);
        list.pop_clip();
    }

    // -- Hue bar --
    ui.set_cursor_screen_position([origin[0], origin[1] + square_height]);
    ui.dummy([width, HUE_MARGIN_TOP]);
    // `ui.dummy` puts the cursor back at the window's left edge, so every x below comes
    // from the picker's own origin rather than from the cursor.
    let hue_origin = [
        origin[0] + HUE_MARGIN_X,
        ui.cursor_screen_position()[1],
    ];
    let hue_width = (width - HUE_MARGIN_X * 2.0).max(1.0);
    ui.set_cursor_screen_position(hue_origin);
    ui.invisible_button("##hue", [hue_width, HUE_HEIGHT]);
    let hue_active = ui.item_active() && !readonly;
    if ui.item_hovered() && !readonly {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    if hue_active {
        let mouse_x = ui.mouse_position()[0];
        let state = states.get(id);
        state.hue = c01((mouse_x - hue_origin[0]) / hue_width) * 360.0;
        changed = true;
    }
    let state = states.get(id);
    state.picking = square_active || hue_active;
    let hue = state.hue;
    {
        // Thirteen stops thirty degrees apart, as the `linear-gradient` lists them.
        let stops: Vec<Color> = (0..=12)
            .map(|step| {
                let rgb = hsv_to_rgb(step as f32 * 30.0, 1.0, 1.0);
                Color([rgb[0], rgb[1], rgb[2], 1.0])
            })
            .collect();
        let list = ui.draw_list();
        ramp(
            &list,
            hue_origin,
            [hue_origin[0] + hue_width, hue_origin[1] + HUE_HEIGHT],
            &stops,
            HUE_HEIGHT / 2.0,
            background,
        );
        thumb(
            &list,
            [
                hue_origin[0] + hue / 360.0 * hue_width,
                hue_origin[1] + HUE_HEIGHT / 2.0,
            ],
            HUE_THUMB,
        );
    }
    ui.set_cursor_screen_position([origin[0], hue_origin[1] + HUE_HEIGHT]);
    ui.dummy([width, HUE_MARGIN_BOTTOM]);

    // -- Mode drop-down --
    ui.dummy([width, MODE_PADDING_TOP]);
    let mode_top = ui.cursor_screen_position()[1];
    ui.set_cursor_screen_position([origin[0] + ROW_PADDING_X, mode_top]);
    let labels: Vec<String> = MODE_ORDER
        .iter()
        .map(|mode| mode.label().to_string())
        .collect();
    let mut selected = MODE_ORDER
        .iter()
        .position(|mode| *mode == states.get(id).mode)
        .unwrap_or(0);
    let mode_width = (width - ROW_PADDING_X * 2.0).max(1.0);
    if controls::dropdown(ui, "mode", &mut selected, &labels, mode_width, !readonly) {
        states.get(id).mode = MODE_ORDER[selected];
    }
    ui.set_cursor_screen_position([origin[0], mode_top + theme::INPUT_HEIGHT]);
    ui.dummy([width, MODE_PADDING_BOTTOM]);

    // -- One row per channel of the mode, then alpha --
    let state = states.get(id);
    let mode = state.mode;
    let rgb = state.rgb();
    let hsv = [state.hue, state.saturation, state.brightness];
    let shown = to_mode(mode, rgb, hsv);
    for (index, channel) in channels(mode).iter().enumerate() {
        let current = shown.get(index).copied().unwrap_or(0.0);
        let stops: Vec<Color> = (0..=RAMP_STEPS)
            .map(|step| {
                let mut values = shown.clone();
                values[index] =
                    channel.min + (step as f32 / RAMP_STEPS as f32) * (channel.max - channel.min);
                let stop = if mode == Mode::Hsv {
                    hsv_to_rgb(values[0], values[1], values[2])
                } else {
                    from_mode(mode, &values)
                };
                Color([stop[0], stop[1], stop[2], 1.0])
            })
            .collect();
        if let Some(next) = channel_row(
            ui,
            origin[0],
            width,
            channel,
            current,
            &stops,
            readonly,
            background,
        ) {
            let state = states.get(id);
            if mode == Mode::Hsv {
                // HSV edits the picker's own state, so nothing is lost converting round.
                match index {
                    0 => state.hue = next.clamp(0.0, 360.0),
                    1 => state.saturation = c01(next),
                    _ => state.brightness = c01(next),
                }
            } else {
                let mut values = shown.clone();
                values[index] = next;
                let updated = from_mode(mode, &values);
                let [hue, saturation, brightness] = rgb_to_hsv(updated[0], updated[1], updated[2]);
                state.hue = hue;
                state.saturation = saturation;
                state.brightness = brightness;
            }
            changed = true;
        }
    }

    let alpha_stops = [
        Color([rgb[0], rgb[1], rgb[2], 0.0]),
        Color([rgb[0], rgb[1], rgb[2], 1.0]),
    ];
    let alpha = chan("A", 0.0, 1.0, 3);
    if let Some(next) = channel_row(
        ui,
        origin[0],
        width,
        &alpha,
        value[3],
        &alpha_stops,
        readonly,
        background,
    ) {
        value[3] = c01(next);
        changed = true;
    }

    // -- Hex --
    let hex = to_hex(states.get(id).rgb());
    ui.dummy([width, HEX_MARGIN_TOP]);
    let separator = [origin[0], ui.cursor_screen_position()[1]];
    ui.draw_list().line(
        separator,
        [separator[0] + width, separator[1]],
        theme::BORDER.mix(40.0, Color::TRANSPARENT),
        1.0,
    );
    ui.dummy([width, HEX_PADDING_TOP]);
    let hex_top = ui.cursor_screen_position()[1];
    let mut cursor_x = origin[0] + ROW_PADDING_X;
    controls::draw_in_row(
        ui,
        cursor_x,
        hex_top,
        HEX_SWATCH,
        theme::TEXT_BRIGHT.with_alpha(0.55),
        theme::face::SMALL_MONO,
        "Hex",
    );
    cursor_x += HEX_LABEL_WIDTH + HEX_GAP;
    {
        let list = ui.draw_list();
        let swatch_min = [cursor_x, hex_top];
        let swatch_max = [cursor_x + HEX_SWATCH, hex_top + HEX_SWATCH];
        list.rect(
            swatch_min,
            swatch_max,
            Color([rgb[0], rgb[1], rgb[2], 1.0]),
            theme::INPUT_RADIUS,
            Rounding::All,
        );
        list.rect_outline(
            swatch_min,
            swatch_max,
            theme::BORDER,
            theme::INPUT_RADIUS,
            Rounding::All,
            theme::INPUT_BORDER_WIDTH,
        );
    }
    cursor_x += HEX_SWATCH + HEX_GAP;
    let field_width = (origin[0] + width - ROW_PADDING_X - cursor_x).max(1.0);
    ui.set_cursor_screen_position([cursor_x, hex_top]);
    let state = states.get(id);
    if !state.hex_editing {
        state.hex_draft = hex;
    }
    let mut draft = state.hex_draft.clone();
    let entered = controls::text_input_with(
        ui,
        "hex",
        &mut draft,
        "",
        field_width,
        InputFlags {
            read_only: readonly,
            enter_returns_true: true,
            ..InputFlags::default()
        },
    );
    let editing = ui.item_active();
    let finished = entered || ui.item_deactivated();
    let state = states.get(id);
    state.hex_draft = draft;
    state.hex_editing = editing;
    // `applyHex` runs on blur and on Enter, and leaves the colour alone when the text is
    // not six digits.
    if finished && !readonly {
        if let Some(parsed) = from_hex(&state.hex_draft) {
            let [hue, saturation, brightness] = rgb_to_hsv(parsed[0], parsed[1], parsed[2]);
            state.hue = hue;
            state.saturation = saturation;
            state.brightness = brightness;
            changed = true;
        }
        state.hex_editing = false;
    }
    ui.set_cursor_screen_position([origin[0], hex_top + HEX_SWATCH]);
    ui.dummy([width, HEX_PADDING_BOTTOM]);

    if changed {
        let rgb = states.get(id).rgb();
        value[0] = rgb[0];
        value[1] = rgb[1];
        value[2] = rgb[2];
    }
    changed
}

/// One `.ch-row`: a mono label, a ramped slider and a right-aligned number box.
#[allow(clippy::too_many_arguments)]
fn channel_row(
    ui: &mut Ui,
    left: f32,
    width: f32,
    channel: &Chan,
    value: f32,
    stops: &[Color],
    readonly: bool,
    background: Color,
) -> Option<f32> {
    let height = theme::INPUT_HEIGHT + ROW_PADDING_Y * 2.0;
    let top = ui.cursor_screen_position()[1];
    let mut result = None;
    let _scope = ui.push_id(channel.label);

    controls::draw_in_row(
        ui,
        left + ROW_PADDING_X,
        top,
        height,
        theme::TEXT_BRIGHT.with_alpha(0.55),
        theme::face::SMALL_MONO,
        channel.label,
    );

    let track_left = left + ROW_PADDING_X + LABEL_WIDTH + ROW_GAP;
    let track_right = left + width - ROW_PADDING_X - NUMBER_WIDTH - ROW_GAP;
    let track_width = (track_right - track_left).max(1.0);
    // The hit region is the row's full height, not the six-pixel track, so the slider is
    // no harder to grab than the browser's.
    ui.set_cursor_screen_position([track_left, top + ROW_PADDING_Y]);
    ui.invisible_button("##track", [track_width, theme::INPUT_HEIGHT]);
    let active = ui.item_active() && !readonly;
    if ui.item_hovered() && !readonly {
        ui.set_mouse_cursor(MouseCursor::Hand);
    }
    let mut shown = value;
    if active {
        let travel = (track_width - TRACK_THUMB).max(1.0);
        let fraction = c01((ui.mouse_position()[0] - track_left - TRACK_THUMB / 2.0) / travel);
        shown = channel.min + fraction * (channel.max - channel.min);
        result = Some(shown);
    }

    let centre = top + height / 2.0;
    {
        let list = ui.draw_list();
        ramp(
            &list,
            [track_left, centre - TRACK_HEIGHT / 2.0],
            [track_right, centre + TRACK_HEIGHT / 2.0],
            stops,
            TRACK_HEIGHT / 2.0,
            background,
        );
        let span = (channel.max - channel.min).abs().max(f32::EPSILON);
        let fraction = ((shown - channel.min) / span).clamp(0.0, 1.0);
        // A range input keeps its thumb wholly inside the track, unlike `.hue-thumb`,
        // which is placed by percentage and so hangs half off at either end.
        let travel = (track_width - TRACK_THUMB).max(0.0);
        thumb(
            &list,
            [track_left + TRACK_THUMB / 2.0 + fraction * travel, centre],
            TRACK_THUMB,
        );
    }

    let number_left = left + width - ROW_PADDING_X - NUMBER_WIDTH;
    ui.set_cursor_screen_position([number_left, centre - theme::INPUT_HEIGHT / 2.0]);
    let mut text = format_channel(shown, channel.decimals);
    if controls::text_input_with(
        ui,
        "num",
        &mut text,
        "",
        NUMBER_WIDTH,
        InputFlags {
            read_only: readonly,
            ..InputFlags::default()
        },
    ) {
        if let Ok(parsed) = text.trim().parse::<f32>() {
            result = Some(parsed.clamp(channel.min, channel.max));
        }
    }

    ui.set_cursor_screen_position([left, top + height]);
    ui.dummy([width, 0.0]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn hue_saturation_and_value_survive_a_round_trip() {
        for colour in [[0.9, 0.35, 0.08], [0.2, 0.7, 0.4], [0.05, 0.1, 0.8]] {
            let [hue, saturation, brightness] = rgb_to_hsv(colour[0], colour[1], colour[2]);
            for (actual, expected) in hsv_to_rgb(hue, saturation, brightness)
                .iter()
                .zip(colour.iter())
            {
                close(*actual, *expected);
            }
        }
    }

    #[test]
    fn the_primaries_land_on_the_hues_the_gradient_names() {
        close(rgb_to_hsv(1.0, 0.0, 0.0)[0], 0.0);
        close(rgb_to_hsv(0.0, 1.0, 0.0)[0], 120.0);
        close(rgb_to_hsv(0.0, 0.0, 1.0)[0], 240.0);
        // The bar's last stop is three hundred and sixty, which wraps to red rather than
        // falling off the end of the sector table.
        let wrapped = hsv_to_rgb(360.0, 1.0, 1.0);
        close(wrapped[0], 1.0);
        close(wrapped[1], 0.0);
        close(wrapped[2], 0.0);
    }

    #[test]
    fn lab_and_cmyk_come_back_where_they_started() {
        let colour = [0.4, 0.62, 0.17];
        let lab = rgb_to_lab(colour[0], colour[1], colour[2]);
        for (actual, expected) in lab_to_rgb(lab[0], lab[1], lab[2]).iter().zip(colour.iter()) {
            close(*actual, *expected);
        }
        let cmyk = rgb_to_cmyk(colour[0], colour[1], colour[2]);
        for (actual, expected) in cmyk_to_rgb(cmyk[0], cmyk[1], cmyk[2], cmyk[3])
            .iter()
            .zip(colour.iter())
        {
            close(*actual, *expected);
        }
    }

    #[test]
    fn white_is_lightness_one_hundred_and_black_is_all_key() {
        close(rgb_to_lab(1.0, 1.0, 1.0)[0], 100.0);
        assert_eq!(rgb_to_cmyk(0.0, 0.0, 0.0), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn hex_is_read_and_written_the_way_the_component_does() {
        assert_eq!(to_hex([1.0, 0.0, 0.5]), "#ff0080");
        assert_eq!(from_hex("#ff0080").map(|colour| colour[0]), Some(1.0));
        assert_eq!(
            from_hex("ff0080").map(|colour| colour[2]),
            Some(128.0 / 255.0)
        );
        // Anything that is not six digits leaves the colour alone.
        assert_eq!(from_hex("#fff"), None);
        assert_eq!(from_hex("zzzzzz"), None);
    }

    #[test]
    fn every_mode_names_its_channels_and_round_trips_through_them() {
        let rgb = [0.9, 0.35, 0.08];
        let hsv = rgb_to_hsv(rgb[0], rgb[1], rgb[2]);
        for mode in MODE_ORDER {
            let shown = to_mode(mode, rgb, hsv);
            assert_eq!(shown.len(), channels(mode).len());
            for (value, channel) in shown.iter().zip(channels(mode)) {
                assert!(
                    *value >= channel.min - 1e-3 && *value <= channel.max + 1e-3,
                    "{} out of range in {mode:?}",
                    channel.label
                );
            }
            for (actual, expected) in from_mode(mode, &shown).iter().zip(rgb.iter()) {
                close(*actual, *expected);
            }
        }
    }

    #[test]
    fn a_grey_keeps_the_hue_it_was_dragged_from_while_picking() {
        let mut state = State {
            hue: 210.0,
            saturation: 0.5,
            brightness: 0.5,
            picking: true,
            ..State::default()
        };
        state.sync([0.0, 0.0, 0.0]);
        close(state.hue, 210.0);
        // Once the drag ends the colour is read back, as the Svelte effect does.
        state.picking = false;
        state.sync([0.0, 0.0, 0.0]);
        close(state.hue, 0.0);
    }

    #[test]
    fn a_channel_box_shows_the_decimals_its_mode_asks_for() {
        assert_eq!(format_channel(0.65, 3), "0.650");
        assert_eq!(format_channel(165.4, 0), "165");
        assert_eq!(format_channel(12.34, 1), "12.3");
    }

    #[test]
    fn the_picker_is_one_row_taller_for_the_mode_with_a_fourth_channel() {
        let row = theme::INPUT_HEIGHT + ROW_PADDING_Y * 2.0;
        close(height(240.0, Mode::Cmyk) - height(240.0, Mode::Rgb01), row);
    }

    #[test]
    fn each_picker_keeps_its_own_mode() {
        let mut states = States::default();
        states.get("background").mode = Mode::Hsv;
        assert_eq!(states.get("background").mode, Mode::Hsv);
        assert_eq!(states.get("tint").mode, Mode::Rgb01);
    }
}
