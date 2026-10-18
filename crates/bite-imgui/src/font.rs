//! Font atlas construction: Inter for interface text and JetBrains Mono for monospaced text,
//! in the weights `theme.css` uses.
use crate::{kerning::Kerning, sfnt};
use bite_imgui_sys as sys;
use std::rc::Rc;

pub const UI_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
pub const UI_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");
pub const UI_BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");
pub const MONO_REGULAR: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");
pub const MONO_SEMIBOLD: &[u8] = include_bytes!("../fonts/JetBrainsMono-SemiBold.ttf");
pub const MONO_BOLD: &[u8] = include_bytes!("../fonts/JetBrainsMono-Bold.ttf");

/// A font family, as named by the `--font-ui` and `--font-mono` tokens.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Family {
    Ui,
    Mono,
}

/// The CSS font weights the stylesheet asks for.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Weight {
    Regular,
    SemiBold,
    Bold,
}

impl Weight {
    /// Maps a CSS numeric weight onto the faces that ship with the application.
    pub fn from_css(weight: u16) -> Self {
        match weight {
            0..=549 => Self::Regular,
            550..=649 => Self::SemiBold,
            _ => Self::Bold,
        }
    }
}

/// One rasterized face: family, weight and logical pixel size.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Face {
    pub family: Family,
    pub weight: Weight,
    /// Logical, unscaled pixel size, exactly as written in `theme.css`.
    pub size: u16,
}

impl Face {
    pub const fn ui(size: u16) -> Self {
        Self {
            family: Family::Ui,
            weight: Weight::Regular,
            size,
        }
    }

    pub const fn ui_weight(size: u16, weight: Weight) -> Self {
        Self {
            family: Family::Ui,
            weight,
            size,
        }
    }

    pub const fn mono(size: u16) -> Self {
        Self {
            family: Family::Mono,
            weight: Weight::Regular,
            size,
        }
    }

    pub const fn mono_weight(size: u16, weight: Weight) -> Self {
        Self {
            family: Family::Mono,
            weight,
            size,
        }
    }
}

/// Every face the interface needs, derived from the size and weight tokens in `theme.css`.
pub fn default_faces() -> Vec<Face> {
    use Weight::{Bold, Regular, SemiBold};
    let mut faces = Vec::new();
    for size in [10u16, 11, 12, 13, 14, 15, 17, 20, 28] {
        faces.push(Face::ui_weight(size, Regular));
    }
    for size in [10u16, 11, 12, 13, 14] {
        faces.push(Face::ui_weight(size, SemiBold));
    }
    for size in [11u16, 13, 14, 17, 20, 28] {
        faces.push(Face::ui_weight(size, Bold));
    }
    for size in [10u16, 11, 12, 13, 14] {
        faces.push(Face::mono_weight(size, Regular));
    }
    for size in [11u16, 12, 13, 28] {
        faces.push(Face::mono_weight(size, SemiBold));
    }
    for size in [11u16, 13] {
        faces.push(Face::mono_weight(size, Bold));
    }
    faces.sort_unstable();
    faces.dedup();
    faces
}

/// The codepoints the faces may be rasterized for: all of them.
///
/// Since 1.92 Dear ImGui rasterizes a glyph the first time it is drawn, so a range is no longer a
/// list of what to bake - it is only a filter on what *may* be baked. Passing none is the 1.92
/// default: the interface is English, an unseen codepoint costs nothing until something asks for
/// it, and a codepoint the font has no glyph for still falls back to a missing-glyph box.
///
/// The old list stopped at the end of Latin-1 plus General Punctuation, chosen so the em dash,
/// the ellipsis and the curly quotes would not be boxes in an atlas that had to be sized up
/// front. On demand, that concern goes away.
const NO_GLYPH_RANGES: *const sys::ImWchar = std::ptr::null();

/// The size the six fonts are registered at.
///
/// Every draw names its own size, so this is only the value `ImFont::LegacySize` reports and is
/// never what anything is drawn at.
const REGISTERED_SIZE: f32 = 13.0;

/// Handle to one face in the face list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FontId(pub(crate) usize);

/// How many fonts are registered: the two families in three weights each.
const REGISTERED: usize = 6;

/// Which of the registered fonts a family and weight is.
fn slot(family: Family, weight: Weight) -> usize {
    let family = match family {
        Family::Ui => 0,
        Family::Mono => 1,
    };
    let weight = match weight {
        Weight::Regular => 0,
        Weight::SemiBold => 1,
        Weight::Bold => 2,
    };
    family * 3 + weight
}

/// The font file behind one of the registered slots.
fn slot_data(slot: usize) -> &'static [u8] {
    match slot {
        0 => UI_REGULAR,
        1 => UI_SEMIBOLD,
        2 => UI_BOLD,
        3 => MONO_REGULAR,
        4 => MONO_SEMIBOLD,
        _ => MONO_BOLD,
    }
}

/// The registered fonts: the face list the stylesheet names, and the `ImFont` and logical size
/// each of those faces resolves to.
///
/// Since 1.92 a font is no longer a size. One `ImFont` per family and weight covers every size
/// the stylesheet asks for, because the size is chosen at `igPushFont` time and the glyphs for it
/// are rasterized when they are first drawn. So [`REGISTERED`] fonts are registered - not one per
/// [`Face`] - and `handles` maps each face onto whichever of the six it draws through.
///
/// Registration rasterizes nothing, which is why there is no atlas to build at startup and none
/// to rebuild when the display scale changes.
pub struct Fonts {
    pub(crate) faces: Vec<Face>,
    /// The `ImFont` each face draws through: one of the six, repeated.
    pub(crate) handles: Vec<*mut sys::ImFont>,
    /// The size each face is pushed at, in logical pixels. This is the face's CSS size corrected
    /// by [`sfnt::vertical_metrics`], and it is what drawing and measuring must use.
    pub(crate) heights: Vec<f32>,
    /// The distance from the top of each face's line to its baseline, in logical pixels.
    ascents: Vec<f32>,
    /// The kerning of the font each face draws through, read once per font and shared.
    kerning: Vec<Rc<Kerning>>,
    /// The display scale the interface is being drawn at, which the host reads back.
    ///
    /// It is baked into nothing. Glyph *metrics* stay logical because the pushed size is logical,
    /// and glyph *bitmaps* follow `io.DisplayFramebufferScale`, which Dear ImGui turns into the
    /// rasterizer density once the renderer claims `ImGuiBackendFlags_RendererHasTextures` -
    /// for the current font only, so [`Fonts::set_density`] hands it to the rest each frame.
    scale: f32,
}

impl Fonts {
    /// The logical pixel size a face is drawn at, which is its CSS size corrected so the em
    /// comes out right.
    pub(crate) fn height(&self, id: FontId) -> f32 {
        self.heights.get(id.0).copied().unwrap_or_default()
    }

    /// Measures `text` in `face` at `scale` times its size.
    ///
    /// This goes straight to the font, so it works before any window has begun. Measuring
    /// through a draw list would need a current window, and asking for one outside a frame's
    /// windows leaves the context in a state that shows up as stray popups and clipped text.
    ///
    /// The size asked for here is the exact one, fraction and all, because that is the size the
    /// draw path asks for. Dear ImGui rounds only the size it *rasterizes* at: `GetFontBaked`
    /// picks a whole pixel bake, and then `CalcTextSizeA` and `RenderText` both scale what they
    /// return by `size / baked->Size`, so the geometry comes back at the size that was asked
    /// for. Rounding here would describe a bake that nothing draws - at eleven pixels the
    /// interface face is 13.31, and calling that 13 would make every box drawn around a run,
    /// and every glyph position in letter-spaced text, over two per cent too narrow.
    ///
    /// Kerning is included, so this is the width [`crate::DrawListRef::text_with_face`] draws.
    pub fn measure(&self, face: Face, scale: f32, text: &str) -> [f32; 2] {
        let id = self.id(face);
        let [width, height] = self.measure_unkerned(id, scale, text);
        let Some(kerning) = self.kerning(id) else {
            return [width, height];
        };
        let em = self.em(id) * scale;
        // Kerning changes the width of the line it falls on, and a run is as wide as its widest
        // line.
        let width = if text.contains('\n') {
            text.split('\n')
                .map(|line| self.measure_unkerned(id, scale, line)[0] + kerning.total(line) * em)
                .fold(0.0, f32::max)
        } else {
            width + kerning.total(text) * em
        };
        [width, height]
    }

    /// What Dear ImGui measures `text` as, which leaves kerning out.
    pub(crate) fn measure_unkerned(&self, id: FontId, scale: f32, text: &str) -> [f32; 2] {
        let Some(handle) = self.handles.get(id.0).copied().filter(|h| !h.is_null()) else {
            return [0.0, 0.0];
        };
        let size = self.height(id) * scale;
        let text = std::ffi::CString::new(text).unwrap_or_default();
        let out = unsafe {
            sys::ImFont_CalcTextSizeA(
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

    /// The kerning of the font a face draws through, or `None` when it has none.
    pub(crate) fn kerning(&self, id: FontId) -> Option<&Kerning> {
        self.kerning
            .get(id.0)
            .map(Rc::as_ref)
            .filter(|kerning| !kerning.is_empty())
    }

    /// The logical size of a face's em, which kerning is a fraction of.
    pub(crate) fn em(&self, id: FontId) -> f32 {
        self.faces
            .get(id.0)
            .map_or(0.0, |face| f32::from(face.size))
    }

    /// How far below the point text is drawn at its baseline falls, for `face` at `scale` times
    /// its size. Runs in different faces share a baseline when each is drawn this far above it.
    pub fn ascent(&self, face: Face, scale: f32) -> f32 {
        self.ascents
            .get(self.id(face).0)
            .copied()
            .unwrap_or_default()
            * scale
    }

    /// Registers the six fonts and maps every face onto one of them.
    ///
    /// Nothing is rasterized here: `AddFontFromMemoryTTF` in 1.92 reads the font tables and
    /// returns. The first glyph of a given font and size is rasterized when it is first drawn and
    /// uploaded by the renderer inside that frame, which is why the editor pays no atlas cost
    /// before its first frame and none at all for text it never shows.
    pub(crate) fn build(faces: Vec<Face>, scale: f32) -> Self {
        let atlas = unsafe { (*sys::igGetIO_Nil()).Fonts };
        unsafe { sys::ImFontAtlas_Clear(atlas) };

        let mut registered = [std::ptr::null_mut(); REGISTERED];
        for (index, handle) in registered.iter_mut().enumerate() {
            let data = slot_data(index);
            let mut config = unsafe { *sys::ImFontConfig_ImFontConfig() };
            // The atlas must not free memory that Rust owns.
            config.FontDataOwnedByAtlas = false;
            config.PixelSnapH = false;
            *handle = unsafe {
                sys::ImFontAtlas_AddFontFromMemoryTTF(
                    atlas,
                    data.as_ptr() as *mut _,
                    data.len() as i32,
                    REGISTERED_SIZE,
                    &config,
                    NO_GLYPH_RANGES,
                )
            };
        }

        let by_font: Vec<Rc<Kerning>> = (0..REGISTERED)
            .map(|index| Rc::new(Kerning::read(slot_data(index))))
            .collect();
        let mut handles = Vec::with_capacity(faces.len());
        let mut heights = Vec::with_capacity(faces.len());
        let mut ascents = Vec::with_capacity(faces.len());
        let mut kerning = Vec::with_capacity(faces.len());
        for face in &faces {
            let index = slot(face.family, face.weight);
            // A font whose tables cannot be read is rasterized as Dear ImGui would have done.
            let (height, ascent) = sfnt::vertical_metrics(slot_data(index)).unwrap_or((1.0, 0.8));
            handles.push(registered[index]);
            heights.push(f32::from(face.size) * height);
            ascents.push(f32::from(face.size) * ascent);
            kerning.push(Rc::clone(&by_font[index]));
        }
        Self {
            faces,
            handles,
            heights,
            ascents,
            kerning,
            scale,
        }
    }

    /// Looks up a face, falling back to the nearest size in the same family and weight.
    pub fn id(&self, face: Face) -> FontId {
        if let Some(index) = self.faces.iter().position(|candidate| *candidate == face) {
            return FontId(index);
        }
        let nearest = self
            .faces
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.family == face.family && candidate.weight == face.weight
            })
            .min_by_key(|(_, candidate)| candidate.size.abs_diff(face.size));
        if let Some((index, _)) = nearest {
            return FontId(index);
        }
        let same_family = self
            .faces
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.family == face.family)
            .min_by_key(|(_, candidate)| candidate.size.abs_diff(face.size));
        FontId(same_family.map(|(index, _)| index).unwrap_or(0))
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Rasterizes every font's glyphs at `density` pixels per logical pixel from here on.
    ///
    /// Dear ImGui keeps the density on each font and brings it up to date only for the font
    /// that is current - the one pushed, or the default. Nearly all of the interface draws a
    /// face straight into a draw list instead, so a font that was never pushed kept the 1.0 it
    /// was registered with: on a Retina display its glyphs were rasterized at half resolution
    /// and stretched, and bold and semibold text, which is never pushed, drew blurred beside
    /// the regular text around it.
    pub(crate) fn set_density(&self, density: f32) {
        for &handle in &self.handles {
            if !handle.is_null() {
                unsafe { (*handle).CurrentRasterizerDensity = density };
            }
        }
    }

    /// Records the display scale the interface is being drawn at.
    ///
    /// Nothing is rebuilt. The rasterizer density follows `io.DisplayFramebufferScale`, which the
    /// frame sets, so a monitor change is this one assignment and the glyphs for the new density
    /// are rasterized as they are next drawn.
    pub(crate) fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }
}
