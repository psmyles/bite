//! A safe, owned wrapper over Dear ImGui. It carries no BITE workflow or pipeline types.
//!
//! The wrapper exposes the parts of the library the native editor needs to reproduce the
//! Electron interface: measurement, styling, draw lists, popups, tooltips and the full
//! widget set, plus the font atlas described in [`font`].
mod draw;
pub mod font;
mod input;
mod style;
mod widgets;

pub use draw::{DrawListRef, Rounding};
pub use font::{Face, Family, FontId, Fonts, Weight};
pub use input::{Key, MouseButton, MouseCursor};
pub use style::{Color, Style, StyleColor, StyleVar};
pub use widgets::{IdGuard, InputFlags, WindowFlags};

use bite_imgui_sys as sys;
use std::{
    ffi::CString,
    marker::PhantomData,
    rc::Rc,
    sync::{Mutex, MutexGuard},
};

static OWNER: Mutex<()> = Mutex::new(());

/// Builds a C string, replacing interior nul bytes so that any caller string is accepted.
pub(crate) fn c(text: &str) -> CString {
    CString::new(text.replace('\0', "\u{fffd}")).unwrap_or_default()
}

/// A two-component vector in logical pixels.
pub type Vec2 = [f32; 2];

pub(crate) fn v(value: Vec2) -> sys::ImVec2 {
    sys::ImVec2 {
        x: value[0],
        y: value[1],
    }
}

pub(crate) fn from_v(value: sys::ImVec2) -> Vec2 {
    [value.x, value.y]
}

/// One vertex of the generated geometry, laid out for direct upload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
}

/// One draw call: a slice of the index buffer, a texture and a scissor rectangle.
#[derive(Clone, Copy, Debug, Default)]
pub struct Command {
    pub count: u32,
    pub index_offset: u32,
    pub vertex_offset: u32,
    /// Left, top, right and bottom in logical pixels.
    pub clip: [f32; 4],
    pub texture: u64,
}

/// The geometry of one ImGui draw list, owned by Rust.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<Command>,
}

/// Everything produced by one frame.
pub type DrawData = Vec<DrawList>;

/// The ImGui context. Only one may exist at a time.
pub struct Context {
    raw: *mut sys::ImGuiContext,
    fonts: Fonts,
    _owner: MutexGuard<'static, ()>,
    _thread: PhantomData<Rc<()>>,
}

impl Context {
    /// Creates a context at the given display scale with the default face list.
    pub fn new(scale: f32) -> Result<Self, String> {
        Self::with_faces(scale, font::default_faces())
    }

    pub fn with_faces(scale: f32, faces: Vec<Face>) -> Result<Self, String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("display scale must be a positive finite number".into());
        }
        let owner = OWNER
            .try_lock()
            .map_err(|_| "only one ImGui context may be active")?;
        let raw = unsafe { sys::igCreateContext(std::ptr::null_mut()) };
        if raw.is_null() {
            return Err("ImGui context creation failed".into());
        }
        unsafe {
            let io = sys::igGetIO();
            (*io).IniFilename = std::ptr::null();
            (*io).LogFilename = std::ptr::null();
            (*io).BackendFlags |= sys::ImGuiBackendFlags_RendererHasVtxOffset;
            (*io).ConfigInputTextCursorBlink = true;
            // Layout is authored in logical pixels; faces are rasterized at the physical size.
            (*io).FontGlobalScale = 1.0 / scale;
        }
        let fonts = Fonts::build(faces, scale);
        Ok(Self {
            raw,
            fonts,
            _owner: owner,
            _thread: PhantomData,
        })
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// Rebuilds the atlas for a new display scale, for example after a monitor change.
    pub fn set_scale(&mut self, scale: f32) -> Result<(), String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("display scale must be a positive finite number".into());
        }
        if (scale - self.fonts.scale()).abs() < f32::EPSILON {
            return Ok(());
        }
        unsafe {
            (*sys::igGetIO()).FontGlobalScale = 1.0 / scale;
        }
        let faces = self.fonts.faces.clone();
        self.fonts = Fonts::build(faces, scale);
        Ok(())
    }

    pub fn mouse_position(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::ImGuiIO_AddMousePosEvent(sys::igGetIO(), x, y) }
        }
    }

    pub fn mouse_button(&mut self, button: MouseButton, down: bool) {
        unsafe { sys::ImGuiIO_AddMouseButtonEvent(sys::igGetIO(), button as i32, down) }
    }

    pub fn mouse_wheel(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::ImGuiIO_AddMouseWheelEvent(sys::igGetIO(), x, y) }
        }
    }

    pub fn key(&mut self, key: Key, down: bool) {
        unsafe { sys::ImGuiIO_AddKeyEvent(sys::igGetIO(), key.code(), down) }
    }

    pub fn text_input(&mut self, text: &str) {
        unsafe { sys::ImGuiIO_AddInputCharactersUTF8(sys::igGetIO(), c(text).as_ptr()) }
    }

    pub fn focus(&mut self, focused: bool) {
        unsafe { sys::ImGuiIO_AddFocusEvent(sys::igGetIO(), focused) }
    }

    /// True while a text field is accepting keystrokes, so shortcuts must stand down.
    pub fn want_text_input(&self) -> bool {
        unsafe { (*sys::igGetIO()).WantTextInput }
    }

    /// True while ImGui is using the pointer, so the host must not act on it.
    pub fn want_capture_mouse(&self) -> bool {
        unsafe { (*sys::igGetIO()).WantCaptureMouse }
    }

    pub fn want_capture_keyboard(&self) -> bool {
        unsafe { (*sys::igGetIO()).WantCaptureKeyboard }
    }

    /// The cursor shape the interface asks the window to display.
    pub fn mouse_cursor(&self) -> MouseCursor {
        MouseCursor::from_raw(unsafe { sys::igGetMouseCursor() })
    }

    /// Starts a frame. `width` and `height` are logical pixels.
    pub fn frame(&mut self, width: f32, height: f32, scale: f32, delta: f32) -> Frame<'_> {
        let width = width.max(1.0);
        let height = height.max(1.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let delta = delta.clamp(1.0 / 1000.0, 1.0);
        unsafe {
            let io = sys::igGetIO();
            (*io).DisplaySize = v([width, height]);
            (*io).DisplayFramebufferScale = v([scale, scale]);
            (*io).DeltaTime = delta;
            sys::igNewFrame();
        }
        Frame {
            context: self,
            rendered: false,
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { sys::igDestroyContext(self.raw) }
    }
}

/// An in-progress frame. Dropping it renders and discards the geometry.
pub struct Frame<'a> {
    context: &'a mut Context,
    rendered: bool,
}

impl Frame<'_> {
    pub fn ui(&mut self) -> Ui<'_> {
        Ui {
            fonts: &self.context.fonts,
            _frame: PhantomData,
        }
    }

    pub fn fonts(&self) -> &Fonts {
        &self.context.fonts
    }

    /// Ends the frame and copies the geometry out of ImGui.
    pub fn render(mut self) -> DrawData {
        unsafe { sys::igRender() };
        self.rendered = true;
        let data = unsafe { sys::igGetDrawData() };
        if data.is_null() || !unsafe { (*data).Valid } {
            return Vec::new();
        }
        let mut out = Vec::new();
        unsafe {
            let count = (*data).CmdListsCount.max(0) as usize;
            let lists = (*data).CmdLists.Data;
            for index in 0..count {
                let list = *lists.add(index);
                if list.is_null() {
                    continue;
                }
                let vertex_count = (*list).VtxBuffer.Size.max(0) as usize;
                let index_count = (*list).IdxBuffer.Size.max(0) as usize;
                let vertices = std::slice::from_raw_parts((*list).VtxBuffer.Data, vertex_count)
                    .iter()
                    .map(|vertex| Vertex {
                        pos: [vertex.pos.x, vertex.pos.y],
                        uv: [vertex.uv.x, vertex.uv.y],
                        color: vertex.col,
                    })
                    .collect();
                let indices = std::slice::from_raw_parts((*list).IdxBuffer.Data, index_count)
                    .iter()
                    .map(|value| u32::from(*value))
                    .collect();
                let command_count = (*list).CmdBuffer.Size.max(0) as usize;
                let commands = std::slice::from_raw_parts((*list).CmdBuffer.Data, command_count)
                    .iter()
                    .filter(|command| command.UserCallback.is_none() && command.ElemCount > 0)
                    .map(|command| Command {
                        count: command.ElemCount,
                        index_offset: command.IdxOffset,
                        vertex_offset: command.VtxOffset,
                        clip: [
                            command.ClipRect.x,
                            command.ClipRect.y,
                            command.ClipRect.z,
                            command.ClipRect.w,
                        ],
                        texture: command.TextureId as u64,
                    })
                    .collect();
                out.push(DrawList {
                    vertices,
                    indices,
                    commands,
                });
            }
        }
        out
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        if !self.rendered {
            unsafe { sys::igRender() };
        }
    }
}

/// The frame-scoped interface handle that every widget call goes through.
pub struct Ui<'a> {
    pub(crate) fonts: &'a Fonts,
    _frame: PhantomData<&'a mut ()>,
}

impl Ui<'_> {
    pub fn fonts(&self) -> &Fonts {
        self.fonts
    }

    /// Reads back the text a widget wrote into a fixed buffer.
    pub(crate) fn buffer_string(buffer: &[u8]) -> String {
        let end = buffer.iter().position(|byte| *byte == 0).unwrap_or(0);
        String::from_utf8_lossy(&buffer[..end]).into_owned()
    }

}

/// Runs `body` with a value restored afterwards, which keeps push and pop pairs balanced.
pub(crate) struct Guard<F: FnMut()>(pub(crate) F);

impl<F: FnMut()> Drop for Guard<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only one ImGui context may exist, so the context-backed checks share one test.
    #[test]
    fn context_frame_fonts_and_measurement() {
        let mut context = Context::new(1.0).unwrap();
        let (width, height, pixels) = context.fonts().texture();
        // One byte a pixel: the atlas is coverage, not color.
        assert_eq!(pixels.len(), (width * height) as usize);
        context.fonts().set_texture_id(1);

        let exact = context.fonts().id(Face::ui(13));
        assert_eq!(context.fonts().faces[exact.0], Face::ui(13));
        let missing = context.fonts().id(Face::ui(200));
        assert_eq!(context.fonts().faces[missing.0].family, Family::Ui);
        assert_eq!(context.fonts().faces[missing.0].weight, Weight::Regular);

        let mut data = Vec::new();
        for _ in 0..3 {
            let mut frame = context.frame(1280.0, 720.0, 1.0, 1.0 / 60.0);
            let mut ui = frame.ui();
            ui.window("probe", |ui| {
                ui.text("Node Library");
                let size = ui.calc_text_size("Node Library");
                assert!(size[0] > 0.0 && size[1] > 0.0);
                let list = ui.draw_list();
                let measured = list.measure(Face::mono(11), "output-image-1");
                assert!(measured[0] > 0.0);

                // The rendered em must equal the size the stylesheet names.
                // `ImFontConfig::SizePixels` is the distance from ascender to descender, not
                // the em, and the two differ by a third for JetBrains Mono. Asking Dear ImGui
                // for the stylesheet's number directly drew monospaced text at three quarters
                // of its intended size. JetBrains Mono advances six hundred units on a
                // thousand unit em.
                const MONO_ADVANCE: f32 = 0.6;
                for size in [11u16, 12, 13] {
                    let width = list.measure(Face::mono(size), "MMMMMMMMMM")[0] / 10.0;
                    let ratio = width / f32::from(size);
                    assert!(
                        (ratio - MONO_ADVANCE).abs() < 0.05,
                        "mono {size} advanced {ratio:.3} of its size, wanted {MONO_ADVANCE}"
                    );
                }
            });
            data = frame.render();
        }
        assert!(data.iter().any(|list| !list.vertices.is_empty()));

        context.set_scale(2.0).unwrap();
        let (scaled_width, scaled_height, scaled_pixels) = context.fonts().texture();
        assert_eq!(scaled_pixels.len(), (scaled_width * scaled_height) as usize);
        assert!(scaled_width >= width && scaled_height >= height);
        assert_eq!(context.fonts().scale(), 2.0);
    }
}
