//! Colors, style variables and scoped push and pop helpers.
use crate::{FontId, Ui, Vec2, v};
use bite_imgui_sys as sys;

/// A straight red, green, blue and alpha color in the zero to one range.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const TRANSPARENT: Self = Self([0.0, 0.0, 0.0, 0.0]);

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            1.0,
        ])
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            alpha as f32 / 255.0,
        ])
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`, returning black when the text is malformed.
    pub fn from_hex(text: &str) -> Self {
        let text = text.trim().trim_start_matches('#');
        let parse = |slice: &str| u8::from_str_radix(slice, 16).unwrap_or(0);
        match text.len() {
            3 => {
                let expand = |index: usize| {
                    let digit = &text[index..index + 1];
                    parse(&format!("{digit}{digit}"))
                };
                Self::rgb(expand(0), expand(1), expand(2))
            }
            6 => Self::rgb(parse(&text[0..2]), parse(&text[2..4]), parse(&text[4..6])),
            8 => Self::rgba(
                parse(&text[0..2]),
                parse(&text[2..4]),
                parse(&text[4..6]),
                parse(&text[6..8]),
            ),
            _ => Self([0.0, 0.0, 0.0, 1.0]),
        }
    }

    /// The same color at a different opacity, as the stylesheet's `opacity` rules do.
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self([self.0[0], self.0[1], self.0[2], alpha])
    }

    /// Multiplies the existing opacity, for nesting a faded element inside a faded parent.
    pub fn fade(self, factor: f32) -> Self {
        Self([self.0[0], self.0[1], self.0[2], self.0[3] * factor])
    }

    /// Reproduces the stylesheet's `color-mix(in srgb, self percent%, other)`.
    pub fn mix(self, percent: f32, other: Self) -> Self {
        let weight = (percent / 100.0).clamp(0.0, 1.0);
        let blend = |a: f32, b: f32| a * weight + b * (1.0 - weight);
        Self([
            blend(self.0[0], other.0[0]),
            blend(self.0[1], other.0[1]),
            blend(self.0[2], other.0[2]),
            blend(self.0[3], other.0[3]),
        ])
    }

    /// Packs to the ABGR word the draw list expects.
    pub fn packed(self) -> u32 {
        let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        channel(self.0[0])
            | (channel(self.0[1]) << 8)
            | (channel(self.0[2]) << 16)
            | (channel(self.0[3]) << 24)
    }

    pub(crate) fn raw(self) -> sys::ImVec4 {
        sys::ImVec4 {
            x: self.0[0],
            y: self.0[1],
            z: self.0[2],
            w: self.0[3],
        }
    }
}

/// The style colors the editor overrides.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StyleColor {
    Text,
    TextDisabled,
    WindowBg,
    ChildBg,
    PopupBg,
    Border,
    FrameBg,
    FrameBgHovered,
    FrameBgActive,
    Button,
    ButtonHovered,
    ButtonActive,
    Header,
    HeaderHovered,
    HeaderActive,
    Separator,
    CheckMark,
    SliderGrab,
    SliderGrabActive,
    ScrollbarBg,
    ScrollbarGrab,
    ScrollbarGrabHovered,
    ScrollbarGrabActive,
    TextSelectedBg,
    ModalWindowDimBg,
    NavHighlight,
    PlotHistogram,
}

impl StyleColor {
    fn raw(self) -> sys::ImGuiCol {
        match self {
            Self::Text => sys::ImGuiCol_Text,
            Self::TextDisabled => sys::ImGuiCol_TextDisabled,
            Self::WindowBg => sys::ImGuiCol_WindowBg,
            Self::ChildBg => sys::ImGuiCol_ChildBg,
            Self::PopupBg => sys::ImGuiCol_PopupBg,
            Self::Border => sys::ImGuiCol_Border,
            Self::FrameBg => sys::ImGuiCol_FrameBg,
            Self::FrameBgHovered => sys::ImGuiCol_FrameBgHovered,
            Self::FrameBgActive => sys::ImGuiCol_FrameBgActive,
            Self::Button => sys::ImGuiCol_Button,
            Self::ButtonHovered => sys::ImGuiCol_ButtonHovered,
            Self::ButtonActive => sys::ImGuiCol_ButtonActive,
            Self::Header => sys::ImGuiCol_Header,
            Self::HeaderHovered => sys::ImGuiCol_HeaderHovered,
            Self::HeaderActive => sys::ImGuiCol_HeaderActive,
            Self::Separator => sys::ImGuiCol_Separator,
            Self::CheckMark => sys::ImGuiCol_CheckMark,
            Self::SliderGrab => sys::ImGuiCol_SliderGrab,
            Self::SliderGrabActive => sys::ImGuiCol_SliderGrabActive,
            Self::ScrollbarBg => sys::ImGuiCol_ScrollbarBg,
            Self::ScrollbarGrab => sys::ImGuiCol_ScrollbarGrab,
            Self::ScrollbarGrabHovered => sys::ImGuiCol_ScrollbarGrabHovered,
            Self::ScrollbarGrabActive => sys::ImGuiCol_ScrollbarGrabActive,
            Self::TextSelectedBg => sys::ImGuiCol_TextSelectedBg,
            Self::ModalWindowDimBg => sys::ImGuiCol_ModalWindowDimBg,
            Self::NavHighlight => sys::ImGuiCol_NavHighlight,
            Self::PlotHistogram => sys::ImGuiCol_PlotHistogram,
        }
    }
}

/// The style variables the editor overrides, each carrying its value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StyleVar {
    Alpha(f32),
    DisabledAlpha(f32),
    WindowPadding(Vec2),
    WindowRounding(f32),
    WindowBorderSize(f32),
    WindowMinSize(Vec2),
    ChildRounding(f32),
    ChildBorderSize(f32),
    PopupRounding(f32),
    PopupBorderSize(f32),
    FramePadding(Vec2),
    FrameRounding(f32),
    FrameBorderSize(f32),
    ItemSpacing(Vec2),
    ItemInnerSpacing(Vec2),
    IndentSpacing(f32),
    ScrollbarSize(f32),
    ScrollbarRounding(f32),
    GrabMinSize(f32),
    GrabRounding(f32),
    ButtonTextAlign(Vec2),
    SelectableTextAlign(Vec2),
}

impl StyleVar {
    fn push(self) {
        unsafe {
            match self {
                Self::Alpha(value) => sys::igPushStyleVar_Float(sys::ImGuiStyleVar_Alpha, value),
                Self::DisabledAlpha(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_DisabledAlpha, value)
                }
                Self::WindowPadding(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_WindowPadding, v(value))
                }
                Self::WindowRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_WindowRounding, value)
                }
                Self::WindowBorderSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_WindowBorderSize, value)
                }
                Self::WindowMinSize(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_WindowMinSize, v(value))
                }
                Self::ChildRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ChildRounding, value)
                }
                Self::ChildBorderSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ChildBorderSize, value)
                }
                Self::PopupRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_PopupRounding, value)
                }
                Self::PopupBorderSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_PopupBorderSize, value)
                }
                Self::FramePadding(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_FramePadding, v(value))
                }
                Self::FrameRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_FrameRounding, value)
                }
                Self::FrameBorderSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_FrameBorderSize, value)
                }
                Self::ItemSpacing(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ItemSpacing, v(value))
                }
                Self::ItemInnerSpacing(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ItemInnerSpacing, v(value))
                }
                Self::IndentSpacing(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_IndentSpacing, value)
                }
                Self::ScrollbarSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ScrollbarSize, value)
                }
                Self::ScrollbarRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_ScrollbarRounding, value)
                }
                Self::GrabMinSize(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabMinSize, value)
                }
                Self::GrabRounding(value) => {
                    sys::igPushStyleVar_Float(sys::ImGuiStyleVar_GrabRounding, value)
                }
                Self::ButtonTextAlign(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_ButtonTextAlign, v(value))
                }
                Self::SelectableTextAlign(value) => {
                    sys::igPushStyleVar_Vec2(sys::ImGuiStyleVar_SelectableTextAlign, v(value))
                }
            }
        }
    }
}

impl Ui<'_> {
    /// Runs `body` with the given style colors applied, then restores them.
    pub fn with_colors<R>(
        &mut self,
        colors: &[(StyleColor, Color)],
        body: impl FnOnce(&mut Self) -> R,
    ) -> R {
        for (slot, color) in colors {
            unsafe { sys::igPushStyleColor_Vec4(slot.raw(), color.raw()) };
        }
        let result = body(self);
        unsafe { sys::igPopStyleColor(colors.len() as i32) };
        result
    }

    /// Runs `body` with the given style variables applied, then restores them.
    pub fn with_style<R>(&mut self, vars: &[StyleVar], body: impl FnOnce(&mut Self) -> R) -> R {
        for var in vars {
            var.push();
        }
        let result = body(self);
        unsafe { sys::igPopStyleVar(vars.len() as i32) };
        result
    }

    /// Runs `body` with a face selected, then restores the previous one.
    pub fn with_font<R>(&mut self, font: FontId, body: impl FnOnce(&mut Self) -> R) -> R {
        let handle = self.fonts.handles.get(font.0).copied();
        match handle {
            Some(handle) if !handle.is_null() => {
                unsafe { sys::igPushFont(handle) };
                let result = body(self);
                unsafe { sys::igPopFont() };
                result
            }
            _ => body(self),
        }
    }

    /// Selects a face by description, using the nearest available size.
    pub fn with_face<R>(&mut self, face: crate::Face, body: impl FnOnce(&mut Self) -> R) -> R {
        let id = self.fonts.id(face);
        self.with_font(id, body)
    }

    /// Runs `body` with widgets dimmed and non-interactive when `disabled` is true.
    pub fn disabled<R>(&mut self, disabled: bool, body: impl FnOnce(&mut Self) -> R) -> R {
        unsafe { sys::igBeginDisabled(disabled) };
        let result = body(self);
        unsafe { sys::igEndDisabled() };
        result
    }

    /// Applies the base style once, at startup.
    pub fn apply_base_style(&mut self, setup: impl FnOnce(&mut Style)) {
        let mut style = Style;
        setup(&mut style);
    }
}

/// Write access to the persistent style, used once during setup.
pub struct Style;

impl Style {
    pub fn set_color(&mut self, slot: StyleColor, color: Color) {
        unsafe {
            let style = sys::igGetStyle();
            (*style).Colors[slot.raw() as usize] = color.raw();
        }
    }

    pub fn set(&mut self, var: StyleVar) {
        unsafe {
            let style = sys::igGetStyle();
            match var {
                StyleVar::Alpha(value) => (*style).Alpha = value,
                StyleVar::DisabledAlpha(value) => (*style).DisabledAlpha = value,
                StyleVar::WindowPadding(value) => (*style).WindowPadding = v(value),
                StyleVar::WindowRounding(value) => (*style).WindowRounding = value,
                StyleVar::WindowBorderSize(value) => (*style).WindowBorderSize = value,
                StyleVar::WindowMinSize(value) => (*style).WindowMinSize = v(value),
                StyleVar::ChildRounding(value) => (*style).ChildRounding = value,
                StyleVar::ChildBorderSize(value) => (*style).ChildBorderSize = value,
                StyleVar::PopupRounding(value) => (*style).PopupRounding = value,
                StyleVar::PopupBorderSize(value) => (*style).PopupBorderSize = value,
                StyleVar::FramePadding(value) => (*style).FramePadding = v(value),
                StyleVar::FrameRounding(value) => (*style).FrameRounding = value,
                StyleVar::FrameBorderSize(value) => (*style).FrameBorderSize = value,
                StyleVar::ItemSpacing(value) => (*style).ItemSpacing = v(value),
                StyleVar::ItemInnerSpacing(value) => (*style).ItemInnerSpacing = v(value),
                StyleVar::IndentSpacing(value) => (*style).IndentSpacing = value,
                StyleVar::ScrollbarSize(value) => (*style).ScrollbarSize = value,
                StyleVar::ScrollbarRounding(value) => (*style).ScrollbarRounding = value,
                StyleVar::GrabMinSize(value) => (*style).GrabMinSize = value,
                StyleVar::GrabRounding(value) => (*style).GrabRounding = value,
                StyleVar::ButtonTextAlign(value) => (*style).ButtonTextAlign = v(value),
                StyleVar::SelectableTextAlign(value) => (*style).SelectableTextAlign = v(value),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn hex_parsing_matches_the_stylesheet_notation() {
        assert_eq!(Color::from_hex("#141414"), Color::rgb(20, 20, 20));
        assert_eq!(Color::from_hex("585858"), Color::rgb(88, 88, 88));
        assert_eq!(Color::from_hex("#fff"), Color::rgb(255, 255, 255));
    }

    #[test]
    fn mixing_reproduces_the_node_header_recipe() {
        // color-mix(in srgb, #22c55e 18%, #3d3d3d)
        let mixed = Color::from_hex("#22c55e").mix(18.0, Color::from_hex("#3d3d3d"));
        let expected = [
            (0x22 as f32 / 255.0) * 0.18 + (0x3d as f32 / 255.0) * 0.82,
            (0xc5 as f32 / 255.0) * 0.18 + (0x3d as f32 / 255.0) * 0.82,
            (0x5e as f32 / 255.0) * 0.18 + (0x3d as f32 / 255.0) * 0.82,
        ];
        for (actual, want) in mixed.0.iter().zip(expected.iter()) {
            assert!((actual - want).abs() < 1e-5);
        }
    }

    #[test]
    fn packing_places_alpha_in_the_high_byte() {
        assert_eq!(Color::rgba(0x11, 0x22, 0x33, 0x44).packed(), 0x4433_2211);
    }
}
