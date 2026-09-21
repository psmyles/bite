//! Font atlas construction. Mirrors the Electron font stack: Atkinson Hyperlegible Next for
//! interface text and JetBrains Mono for monospaced text, in the weights `theme.css` uses.
use bite_imgui_sys as sys;

pub const UI_REGULAR: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegibleNext-Regular.ttf");
pub const UI_SEMIBOLD: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegibleNext-SemiBold.ttf");
pub const UI_BOLD: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegibleNext-Bold.ttf");
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
    for size in [11u16, 12, 28] {
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

/// The factor between a face's CSS pixel size and the size Dear ImGui must be asked for.
///
/// The size handed to `AddFontFromMemoryTTF` - and, since 1.92, to `igPushFont` - is fed to the
/// rasterizer as "make ascender minus descender this many pixels". A stylesheet's `font-size` is
/// the em size instead. The two differ by `(ascender - descender) / unitsPerEm`, which is 1.32
/// for JetBrains Mono: asking for twelve there would draw an em of nine, three quarters of the
/// size the stylesheet asked for. Multiplying by this factor makes the em come out right.
fn em_to_pixel_height(data: &[u8]) -> f32 {
    fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
        let bytes = data.get(offset..offset + 2)?;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    fn i16_at(data: &[u8], offset: usize) -> Option<i16> {
        u16_at(data, offset).map(|value| value as i16)
    }
    fn table(data: &[u8], tag: &[u8; 4]) -> Option<usize> {
        let count = usize::from(u16_at(data, 4)?);
        (0..count).find_map(|index| {
            let record = 12 + index * 16;
            (data.get(record..record + 4)? == tag).then(|| {
                let bytes = data.get(record + 8..record + 12)?;
                Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
            })?
        })
    }

    let parsed = (|| {
        let units_per_em = f32::from(u16_at(data, table(data, b"head")? + 18)?);
        let horizontal = table(data, b"hhea")?;
        let ascender = f32::from(i16_at(data, horizontal + 4)?);
        let descender = f32::from(i16_at(data, horizontal + 6)?);
        let height = ascender - descender;
        (units_per_em > 0.0 && height > 0.0).then_some(height / units_per_em)
    })();
    // A font whose tables cannot be read is rasterized as Dear ImGui would have done.
    parsed.unwrap_or(1.0)
}

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
    /// by [`em_to_pixel_height`], and it is what drawing and measuring must use.
    pub(crate) heights: Vec<f32>,
    /// The display scale the interface is being drawn at, which the host reads back.
    ///
    /// It is baked into nothing. Glyph *metrics* stay logical because the pushed size is logical,
    /// and glyph *bitmaps* follow `io.DisplayFramebufferScale`, which Dear ImGui turns into the
    /// rasterizer density itself once the renderer claims
    /// `ImGuiBackendFlags_RendererHasTextures` (`imgui.cpp`, `SetCurrentWindow`).
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
    pub fn measure(&self, face: Face, scale: f32, text: &str) -> [f32; 2] {
        let id = self.id(face);
        let Some(handle) = self.handles.get(id.0).copied().filter(|h| !h.is_null()) else {
            return [0.0, 0.0];
        };
        // Dear ImGui rounds the size it draws at (`GetRoundedFontSize`), so a measurement taken
        // at the unrounded size would not describe the glyphs that land on screen.
        let size = rounded(self.heights.get(id.0).copied().unwrap_or_default() * scale);
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

        let mut handles = Vec::with_capacity(faces.len());
        let mut heights = Vec::with_capacity(faces.len());
        for face in &faces {
            let index = slot(face.family, face.weight);
            handles.push(registered[index]);
            heights.push(f32::from(face.size) * em_to_pixel_height(slot_data(index)));
        }
        Self {
            faces,
            handles,
            heights,
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

    /// Records the display scale the interface is being drawn at.
    ///
    /// Nothing is rebuilt. The rasterizer density follows `io.DisplayFramebufferScale`, which the
    /// frame sets, so a monitor change is this one assignment and the glyphs for the new density
    /// are rasterized as they are next drawn.
    pub(crate) fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

}

/// The size Dear ImGui will actually draw at, given the one it is asked for.
///
/// `ImGui::GetRoundedFontSize` is `IM_ROUND`, applied to the size after every global scale
/// factor. The layout system does not yet handle fractional sizes, so this is not something a
/// caller can opt out of - it can only be matched, which is what measurement here does.
pub(crate) fn rounded(size: f32) -> f32 {
    (size + 0.5).floor()
}
