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

/// Wraps a texture identifier as the `ImTextureRef` every 1.92 drawing call takes.
///
/// 1.92 lets a draw command name either a texture the backend already owns (an `ImTextureID`)
/// or one Dear ImGui is still managing (an `ImTextureData*`, the font atlas being the only one
/// here). Everything this wrapper draws is the host's, already uploaded, so the data side is
/// always null and the reference is the identifier.
pub(crate) fn texture_ref(id: u64) -> sys::ImTextureRef {
    sys::ImTextureRef {
        _TexData: std::ptr::null_mut(),
        _TexID: id as sys::ImTextureID,
    }
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

/// The identifier space Dear ImGui's own textures are given, so that they cannot collide with
/// the host's.
///
/// The host keys its textures on whatever it likes - the editor hashes a string - and Dear
/// ImGui's are numbered from one by the library. Setting the top bit on ImGui's side makes the
/// two spaces disjoint by construction rather than by luck, and the host masks the bit off its
/// own identifiers to hold up its end.
pub const IMGUI_TEXTURE_ID_BIT: u64 = 1 << 63;

/// One thing the host must do to a texture before the frame just rendered can be drawn.
///
/// Since 1.92 the font atlas is Dear ImGui's to grow, not the host's to build: glyphs are
/// rasterized the first time they are drawn, so a frame that shows a size or a character no
/// earlier frame did arrives with an upload attached. Requests are collected after the frame is
/// rendered and must be honoured before its geometry is drawn.
#[derive(Clone, Debug)]
pub struct TextureRequest {
    /// The identifier the geometry of this frame refers to the texture by. Always carries
    /// [`IMGUI_TEXTURE_ID_BIT`].
    pub id: u64,
    pub action: TextureAction,
}

/// What a [`TextureRequest`] asks for.
#[derive(Clone, Debug)]
pub enum TextureAction {
    /// Create this texture, or replace it wholesale.
    ///
    /// Dear ImGui also reports partial updates - the rectangle of an atlas a new glyph landed
    /// in - but it keeps the whole pixel buffer either way, so this hands over the whole thing
    /// and lets the host upload it in one call. That is a megabyte at the atlas sizes in play,
    /// on the rare frames that bake a glyph, against a sub-rectangle upload path in every
    /// renderer that would ever have to exist.
    Upload {
        width: u32,
        height: u32,
        /// True when the pixels are one byte of coverage each rather than four of colour.
        coverage: bool,
        pixels: Vec<u8>,
    },
    /// Release this texture; nothing refers to it any more.
    Destroy,
}

/// The ImGui context. Only one may exist at a time.
pub struct Context {
    raw: *mut sys::ImGuiContext,
    fonts: Fonts,
    /// What the last rendered frame needs done to its textures, waiting to be collected by
    /// [`Context::texture_requests`].
    pending_textures: Vec<TextureRequest>,
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
            let io = sys::igGetIO_Nil();
            (*io).IniFilename = std::ptr::null();
            (*io).LogFilename = std::ptr::null();
            // `RendererHasTextures` is the 1.92 contract: the host honours the texture requests
            // on each frame's draw data (see [`Context::texture_requests`]), and in return Dear
            // ImGui rasterizes glyphs on demand instead of baking an atlas up front - and drives
            // the rasterizer density from `DisplayFramebufferScale`, which is what keeps layout
            // in logical pixels while the glyphs are drawn at the display's resolution.
            (*io).BackendFlags |= sys::ImGuiBackendFlags_RendererHasVtxOffset
                | sys::ImGuiBackendFlags_RendererHasTextures;
            (*io).ConfigInputTextCursorBlink = true;
        }
        Fonts::prefer_coverage_texture();
        let fonts = Fonts::build(faces, scale);
        Ok(Self {
            raw,
            fonts,
            pending_textures: Vec::new(),
            _owner: owner,
            _thread: PhantomData,
        })
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    /// Records a new display scale, for example after a monitor change.
    ///
    /// Nothing is rebuilt and nothing is re-uploaded. Since 1.92 the atlas grows on demand and
    /// the rasterizer density follows `io.DisplayFramebufferScale`, which [`Context::frame`]
    /// sets from the scale it is given; the glyphs for the new density are rasterized as they
    /// are next drawn. Before 1.92 this rebuilt every face and the host re-uploaded the atlas.
    pub fn set_scale(&mut self, scale: f32) -> Result<(), String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("display scale must be a positive finite number".into());
        }
        self.fonts.set_scale(scale);
        Ok(())
    }

    pub fn mouse_position(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::ImGuiIO_AddMousePosEvent(sys::igGetIO_Nil(), x, y) }
        }
    }

    pub fn mouse_button(&mut self, button: MouseButton, down: bool) {
        unsafe { sys::ImGuiIO_AddMouseButtonEvent(sys::igGetIO_Nil(), button as i32, down) }
    }

    pub fn mouse_wheel(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            unsafe { sys::ImGuiIO_AddMouseWheelEvent(sys::igGetIO_Nil(), x, y) }
        }
    }

    pub fn key(&mut self, key: Key, down: bool) {
        unsafe { sys::ImGuiIO_AddKeyEvent(sys::igGetIO_Nil(), key.code(), down) }
    }

    pub fn text_input(&mut self, text: &str) {
        unsafe { sys::ImGuiIO_AddInputCharactersUTF8(sys::igGetIO_Nil(), c(text).as_ptr()) }
    }

    pub fn focus(&mut self, focused: bool) {
        unsafe { sys::ImGuiIO_AddFocusEvent(sys::igGetIO_Nil(), focused) }
    }

    /// True while a text field is accepting keystrokes, so shortcuts must stand down.
    pub fn want_text_input(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).WantTextInput }
    }

    /// True while ImGui is using the pointer, so the host must not act on it.
    pub fn want_capture_mouse(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).WantCaptureMouse }
    }

    pub fn want_capture_keyboard(&self) -> bool {
        unsafe { (*sys::igGetIO_Nil()).WantCaptureKeyboard }
    }

    /// The cursor shape the interface asks the window to display.
    pub fn mouse_cursor(&self) -> MouseCursor {
        MouseCursor::from_raw(unsafe { sys::igGetMouseCursor() })
    }

    /// The texture work the frame just rendered needs done before its geometry can be drawn.
    ///
    /// Call this after [`Frame::render`] and act on every request before drawing. The identifier
    /// in each one is what the frame's draw commands name, so a host that ignores a request
    /// draws a frame that refers to a texture nobody created.
    ///
    /// Empty on almost every frame: it has something in it only when a glyph was rasterized for
    /// the first time, which at startup is the text of the first frame and afterwards is a size
    /// or a character the interface had not shown before.
    pub fn texture_requests(&mut self) -> Vec<TextureRequest> {
        std::mem::take(&mut self.pending_textures)
    }

    /// Collects the frame's texture requests and tells Dear ImGui they are settled.
    ///
    /// This has to run *before* the draw commands are read, not after: from 1.92 a command names
    /// its texture through `ImDrawCmd_GetTexID`, which asserts that the identifier has been
    /// filled in. Marking the requests done here and handing them to the host afterwards keeps
    /// both true - the commands can be read, and the pixels reach the device before anything is
    /// drawn with them.
    fn collect_texture_requests(&mut self) {
        let data = unsafe { sys::igGetDrawData() };
        if data.is_null() {
            return;
        }
        let textures = unsafe { (*data).Textures };
        if textures.is_null() {
            return;
        }
        let requests = &mut self.pending_textures;
        unsafe {
            let count = (*textures).Size.max(0) as usize;
            for index in 0..count {
                let texture = *(*textures).Data.add(index);
                if texture.is_null() {
                    continue;
                }
                let id = IMGUI_TEXTURE_ID_BIT | (*texture).UniqueID as u32 as u64;
                match (*texture).Status {
                    // A new atlas, or one that grew: both hand over the whole buffer.
                    sys::ImTextureStatus_WantCreate | sys::ImTextureStatus_WantUpdates => {
                        let width = (*texture).Width.max(0) as u32;
                        let height = (*texture).Height.max(0) as u32;
                        let bytes = (*texture).BytesPerPixel.max(0) as usize;
                        let length = width as usize * height as usize * bytes;
                        if (*texture).Pixels.is_null() || length == 0 {
                            continue;
                        }
                        requests.push(TextureRequest {
                            id,
                            action: TextureAction::Upload {
                                width,
                                height,
                                coverage: bytes == 1,
                                pixels: std::slice::from_raw_parts((*texture).Pixels, length)
                                    .to_vec(),
                            },
                        });
                        sys::ImTextureData_SetTexID(texture, id as sys::ImTextureID);
                        sys::ImTextureData_SetStatus(texture, sys::ImTextureStatus_OK);
                    }
                    sys::ImTextureStatus_WantDestroy => {
                        requests.push(TextureRequest {
                            id,
                            action: TextureAction::Destroy,
                        });
                        sys::ImTextureData_SetTexID(texture, 0);
                        sys::ImTextureData_SetStatus(texture, sys::ImTextureStatus_Destroyed);
                    }
                    _ => {}
                }
            }
        }
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
            let io = sys::igGetIO_Nil();
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
    ///
    /// Any texture work the frame generated is settled first and left for the host to pick up
    /// with [`Context::texture_requests`] - see [`Context::collect_texture_requests`] for why
    /// the order matters.
    pub fn render(mut self) -> DrawData {
        unsafe { sys::igRender() };
        self.rendered = true;
        self.context.collect_texture_requests();
        let data = unsafe { sys::igGetDrawData() };
        if data.is_null() || !unsafe { (*data).Valid } {
            return Vec::new();
        }
        let mut out = Vec::new();
        unsafe {
            let count = (*data).CmdLists.Size.max(0) as usize;
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
                        texture: sys::ImDrawCmd_GetTexID(
                            command as *const sys::ImDrawCmd as *mut _,
                        ) as u64,
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
            // A frame nobody asked the geometry of still baked whatever glyphs it drew, and
            // Dear ImGui will keep asking until the host has been told.
            self.context.collect_texture_requests();
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

        // Registering the faces rasterizes nothing, so there is no texture to hand over yet:
        // the first request arrives with the first frame that draws text.
        assert!(context.texture_requests().is_empty());

        let exact = context.fonts().id(Face::ui(13));
        assert_eq!(context.fonts().faces[exact.0], Face::ui(13));
        let missing = context.fonts().id(Face::ui(200));
        assert_eq!(context.fonts().faces[missing.0].family, Family::Ui);
        assert_eq!(context.fonts().faces[missing.0].weight, Weight::Regular);

        let mut data = Vec::new();
        let mut uploads = Vec::new();
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
            uploads.extend(context.texture_requests());
        }
        assert!(data.iter().any(|list| !list.vertices.is_empty()));

        // Drawing text rasterized glyphs, which is a texture the host has to create. It must be
        // coverage, one byte a pixel - the four-channel form is 1.92's default and four times
        // the upload for the same information - and it must be in Dear ImGui's half of the
        // identifier space, so it can never be mistaken for one of the editor's own images.
        let upload = uploads
            .iter()
            .find_map(|request| match &request.action {
                TextureAction::Upload {
                    width,
                    height,
                    coverage,
                    pixels,
                } => Some((request.id, *width, *height, *coverage, pixels.len())),
                TextureAction::Destroy => None,
            })
            .expect("drawing text must ask the host to create the atlas texture");
        let (id, width, height, coverage, length) = upload;
        assert!(id & IMGUI_TEXTURE_ID_BIT != 0, "atlas id {id:#x} is host-side");
        assert!(coverage, "the atlas must be asked for as coverage");
        assert_eq!(length, (width * height) as usize);

        // A scale change rebuilds nothing; glyphs for the new density are rasterized as they are
        // next drawn. Before 1.92 this rebuilt every face and the host re-uploaded the atlas.
        context.set_scale(2.0).unwrap();
        assert_eq!(context.fonts().scale(), 2.0);
        assert!(context.texture_requests().is_empty());
    }
}
