//! Direct drawing. The node canvas and every custom visual are painted through this.
use crate::{Color, Face, Ui, Vec2, c, v};
use bite_imgui_sys as sys;

/// Which corners a rounded rectangle rounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    All,
    Top,
    Bottom,
    None,
}

impl Rounding {
    fn flags(self) -> sys::ImDrawFlags {
        match self {
            Self::All => sys::ImDrawFlags_RoundCornersAll,
            Self::Top => sys::ImDrawFlags_RoundCornersTop,
            Self::Bottom => sys::ImDrawFlags_RoundCornersBottom,
            Self::None => sys::ImDrawFlags_RoundCornersNone,
        }
    }
}

/// A borrowed draw list. Coordinates are absolute screen positions in logical pixels.
pub struct DrawListRef<'ui> {
    raw: *mut sys::ImDrawList,
    fonts: &'ui crate::Fonts,
}

impl DrawListRef<'_> {
    pub fn line(&self, from: Vec2, to: Vec2, color: Color, thickness: f32) {
        unsafe { sys::ImDrawList_AddLine(self.raw, v(from), v(to), color.packed(), thickness) }
    }

    pub fn rect(&self, min: Vec2, max: Vec2, color: Color, radius: f32, corners: Rounding) {
        unsafe {
            sys::ImDrawList_AddRectFilled(
                self.raw,
                v(min),
                v(max),
                color.packed(),
                radius,
                corners.flags(),
            )
        }
    }

    pub fn rect_outline(
        &self,
        min: Vec2,
        max: Vec2,
        color: Color,
        radius: f32,
        corners: Rounding,
        thickness: f32,
    ) {
        unsafe {
            sys::ImDrawList_AddRect(
                self.raw,
                v(min),
                v(max),
                color.packed(),
                radius,
                corners.flags(),
                thickness,
            )
        }
    }

    /// A rectangle whose fill runs from `top` to `bottom`, as the preview overlay does.
    pub fn rect_gradient(&self, min: Vec2, max: Vec2, top: Color, bottom: Color) {
        unsafe {
            sys::ImDrawList_AddRectFilledMultiColor(
                self.raw,
                v(min),
                v(max),
                top.packed(),
                top.packed(),
                bottom.packed(),
                bottom.packed(),
            )
        }
    }

    pub fn circle(&self, center: Vec2, radius: f32, color: Color) {
        unsafe { sys::ImDrawList_AddCircleFilled(self.raw, v(center), radius, color.packed(), 0) }
    }

    pub fn circle_outline(&self, center: Vec2, radius: f32, color: Color, thickness: f32) {
        unsafe {
            sys::ImDrawList_AddCircle(self.raw, v(center), radius, color.packed(), 0, thickness)
        }
    }

    pub fn triangle(&self, a: Vec2, b: Vec2, point: Vec2, color: Color) {
        unsafe { sys::ImDrawList_AddTriangleFilled(self.raw, v(a), v(b), v(point), color.packed()) }
    }

    /// A cubic curve, used for every wire on the canvas.
    pub fn bezier(
        &self,
        from: Vec2,
        control_a: Vec2,
        control_b: Vec2,
        to: Vec2,
        color: Color,
        thickness: f32,
    ) {
        unsafe {
            sys::ImDrawList_AddBezierCubic(
                self.raw,
                v(from),
                v(control_a),
                v(control_b),
                v(to),
                color.packed(),
                thickness,
                0,
            )
        }
    }

    pub fn text(&self, position: Vec2, color: Color, text: &str) {
        let text = c(text);
        unsafe {
            sys::ImDrawList_AddText_Vec2(self.raw, v(position), color.packed(), text.as_ptr(), std::ptr::null())
        }
    }

    /// Draws text in a specific face without disturbing the surrounding font stack.
    pub fn text_with_face(&self, position: Vec2, color: Color, face: Face, text: &str) {
        let id = self.fonts.id(face);
        let Some(handle) = self.fonts.handles.get(id.0).copied() else {
            return self.text(position, color, text);
        };
        if handle.is_null() {
            return self.text(position, color, text);
        }
        let text = c(text);
        // The atlas is rasterized at the physical size, so ask for the logical size here.
        let size = f32::from(self.fonts.faces[id.0].size);
        unsafe {
            sys::ImDrawList_AddText_FontPtr(
                self.raw,
                handle,
                size,
                v(position),
                color.packed(),
                text.as_ptr(),
                std::ptr::null(),
                0.0,
                std::ptr::null(),
            )
        }
    }

    pub fn image(&self, texture: u64, min: Vec2, max: Vec2) {
        unsafe {
            sys::ImDrawList_AddImage(
                self.raw,
                texture as sys::ImTextureID,
                v(min),
                v(max),
                v([0.0, 0.0]),
                v([1.0, 1.0]),
                Color([1.0, 1.0, 1.0, 1.0]).packed(),
            )
        }
    }

    pub fn image_rounded(&self, texture: u64, min: Vec2, max: Vec2, radius: f32) {
        unsafe {
            sys::ImDrawList_AddImageRounded(
                self.raw,
                texture as sys::ImTextureID,
                v(min),
                v(max),
                v([0.0, 0.0]),
                v([1.0, 1.0]),
                Color([1.0, 1.0, 1.0, 1.0]).packed(),
                radius,
                sys::ImDrawFlags_RoundCornersAll,
            )
        }
    }

    /// Restricts later drawing to `min`..`max` until [`DrawListRef::pop_clip`].
    pub fn push_clip(&self, min: Vec2, max: Vec2, intersect: bool) {
        unsafe { sys::ImDrawList_PushClipRect(self.raw, v(min), v(max), intersect) }
    }

    pub fn pop_clip(&self) {
        unsafe { sys::ImDrawList_PopClipRect(self.raw) }
    }

    /// Splits the list into ordered layers so that later content can be drawn behind earlier
    /// content. Channel zero is drawn first.
    pub fn split(&self, channels: i32) -> Splitter<'_> {
        unsafe { sys::ImDrawList_ChannelsSplit(self.raw, channels) };
        Splitter::new(self.raw)
    }
}

/// Active layering on a draw list. Merging happens when this is dropped.
pub struct Splitter<'a> {
    list: *mut sys::ImDrawList,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Splitter<'_> {
    fn new(list: *mut sys::ImDrawList) -> Self {
        Self {
            list,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn set(&self, channel: i32) {
        unsafe { sys::ImDrawList_ChannelsSetCurrent(self.list, channel) }
    }
}

impl Drop for Splitter<'_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_ChannelsMerge(self.list) }
    }
}

impl<'ui> Ui<'ui> {
    /// The current window's draw list, painted under later widgets in the same window.
    pub fn draw_list(&self) -> DrawListRef<'ui> {
        DrawListRef {
            raw: unsafe { sys::igGetWindowDrawList() },
            fonts: self.fonts,
        }
    }

    /// A list painted above every window, used for tooltips and drag previews.
    pub fn foreground_draw_list(&self) -> DrawListRef<'ui> {
        DrawListRef {
            raw: unsafe { sys::igGetForegroundDrawList_ViewportPtr(std::ptr::null_mut()) },
            fonts: self.fonts,
        }
    }

    /// A list painted below every window, used for the canvas background.
    pub fn background_draw_list(&self) -> DrawListRef<'ui> {
        DrawListRef {
            raw: unsafe { sys::igGetBackgroundDrawList(std::ptr::null_mut()) },
            fonts: self.fonts,
        }
    }
}

// The splitter borrows the list for as long as it lives; construction is internal.
impl DrawListRef<'_> {
    /// Measures text in a specific face, matching what [`DrawListRef::text_with_face`] draws.
    pub fn measure(&self, face: Face, text: &str) -> Vec2 {
        let id = self.fonts.id(face);
        let Some(handle) = self.fonts.handles.get(id.0).copied().filter(|h| !h.is_null()) else {
            return [0.0, 0.0];
        };
        let size = f32::from(self.fonts.faces[id.0].size);
        let text = c(text);
        let mut out = sys::ImVec2::default();
        unsafe {
            sys::ImFont_CalcTextSizeA(
                &mut out,
                handle,
                size,
                f32::MAX,
                0.0,
                text.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        [out.x, out.y]
    }
}
