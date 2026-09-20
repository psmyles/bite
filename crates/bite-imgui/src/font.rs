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

    fn data(self) -> &'static [u8] {
        match (self.family, self.weight) {
            (Family::Ui, Weight::Regular) => UI_REGULAR,
            (Family::Ui, Weight::SemiBold) => UI_SEMIBOLD,
            (Family::Ui, Weight::Bold) => UI_BOLD,
            (Family::Mono, Weight::Regular) => MONO_REGULAR,
            (Family::Mono, Weight::SemiBold) => MONO_SEMIBOLD,
            (Family::Mono, Weight::Bold) => MONO_BOLD,
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

/// Sizes that also carry a merged fallback covering scripts outside Latin, so that text
/// entered through an input method renders in the fields that accept free text.
fn wants_fallback(face: Face) -> bool {
    matches!(
        (face.family, face.weight, face.size),
        (Family::Ui, Weight::Regular, 12..=14) | (Family::Mono, Weight::Regular, 11..=12)
    )
}

/// A platform font used to cover glyphs the bundled faces lack.
fn fallback_font() -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    let candidates: Vec<std::path::PathBuf> = {
        let root = std::env::var_os("WINDIR").map(std::path::PathBuf::from)?;
        ["Fonts/msyh.ttc", "Fonts/meiryo.ttc", "Fonts/simsun.ttc"]
            .iter()
            .map(|name| root.join(name))
            .collect()
    };
    #[cfg(target_os = "macos")]
    let candidates: Vec<std::path::PathBuf> = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ]
    .iter()
    .map(std::path::PathBuf::from)
    .collect();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let candidates: Vec<std::path::PathBuf> = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ]
    .iter()
    .map(std::path::PathBuf::from)
    .collect();

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|path| std::fs::read(path).ok())
}

/// The factor between a face's CSS pixel size and the size Dear ImGui must be asked for.
///
/// `ImFontConfig::SizePixels` is fed to `stbtt_ScaleForPixelHeight`, which scales the font so
/// that ascender minus descender equals that many pixels. A stylesheet's `font-size` is the em
/// size instead. The two differ by `(ascender - descender) / unitsPerEm`, which is 1.32 for
/// JetBrains Mono: asking for twelve there would draw an em of nine, three quarters of the
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

/// Handle to one face inside the current atlas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FontId(pub(crate) usize);

/// The built atlas: the face list, the matching ImGui font pointers and the texture pixels.
pub struct Fonts {
    pub(crate) faces: Vec<Face>,
    pub(crate) handles: Vec<*mut sys::ImFont>,
    /// The size each face was asked for, in logical pixels. This is the face's CSS size
    /// corrected by [`em_to_pixel_height`], and it is what drawing and measuring must use.
    pub(crate) heights: Vec<f32>,

    scale: f32,
    /// Held for as long as the atlas keeps a pointer into it.
    _fallback: Option<Vec<u8>>,
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
        let size = self.heights.get(id.0).copied().unwrap_or_default() * scale;
        let text = std::ffi::CString::new(text).unwrap_or_default();
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

    /// Rebuilds the atlas at `scale`, rasterizing every face at its physical size.
    pub(crate) fn build(faces: Vec<Face>, scale: f32) -> Self {
        let atlas = unsafe { (*sys::igGetIO()).Fonts };
        unsafe { sys::ImFontAtlas_Clear(atlas) };
        let fallback = fallback_font();
        let mut handles = Vec::with_capacity(faces.len());
        let mut heights = Vec::with_capacity(faces.len());
        for face in &faces {
            let data = face.data();
            // The height that draws an em of exactly the size the stylesheet names, and the
            // whole physical pixel the atlas is rasterized at. Dear ImGui truncates the size
            // it is given, so the atlas can only be built at a whole number; rounding there
            // and drawing at the exact height instead leaves the em right to the pixel, where
            // taking the rounded height for both put it out by up to five per cent.
            let exact = f32::from(face.size) * em_to_pixel_height(data);
            let physical = (exact * scale).round().max(1.0);
            heights.push(exact);
            let mut config = unsafe { *sys::ImFontConfig_ImFontConfig() };
            // The atlas must not free memory that Rust owns.
            config.FontDataOwnedByAtlas = false;
            config.PixelSnapH = false;
            let handle = unsafe {
                sys::ImFontAtlas_AddFontFromMemoryTTF(
                    atlas,
                    data.as_ptr() as *mut _,
                    data.len() as i32,
                    physical,
                    &config,
                    std::ptr::null(),
                )
            };
            if let Some(bytes) = fallback.as_ref().filter(|_| wants_fallback(*face)) {
                let mut merge = unsafe { *sys::ImFontConfig_ImFontConfig() };
                merge.FontDataOwnedByAtlas = false;
                merge.MergeMode = true;
                merge.PixelSnapH = false;
                unsafe {
                    let ranges = sys::ImFontAtlas_GetGlyphRangesChineseSimplifiedCommon(atlas);
                    sys::ImFontAtlas_AddFontFromMemoryTTF(
                        atlas,
                        bytes.as_ptr() as *mut _,
                        bytes.len() as i32,
                        // The fallback is corrected against its own metrics, so its em
                        // matches the face it merges into.
                        (f32::from(face.size) * em_to_pixel_height(bytes) * scale)
                            .round()
                            .max(1.0),
                        &merge,
                        ranges,
                    );
                }
            }
            // Dear ImGui sizes its own widget text from the atlas size, which is the rounded
            // one. This factor brings that back to the exact height, so a menu row and a
            // label painted through a draw list are the same size.
            if !handle.is_null() {
                unsafe { (*handle).Scale = exact * scale / physical };
            }
            handles.push(handle);
        }
        Self {
            faces,
            handles,
            heights,
            scale,
            _fallback: fallback,
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

    /// Red, green, blue and alpha pixels for the atlas texture, with its dimensions.
    pub fn texture(&self) -> (u32, u32, Vec<u8>) {
        let atlas = unsafe { (*sys::igGetIO()).Fonts };
        let mut pixels: *mut u8 = std::ptr::null_mut();
        let (mut width, mut height, mut bytes_per_pixel) = (0, 0, 0);
        unsafe {
            sys::ImFontAtlas_GetTexDataAsRGBA32(
                atlas,
                &mut pixels,
                &mut width,
                &mut height,
                &mut bytes_per_pixel,
            );
            let len = width as usize * height as usize * 4;
            (
                width as u32,
                height as u32,
                std::slice::from_raw_parts(pixels, len).to_vec(),
            )
        }
    }

    /// Binds the uploaded texture identifier to the atlas.
    pub fn set_texture_id(&self, id: u64) {
        unsafe {
            let atlas = (*sys::igGetIO()).Fonts;
            sys::ImFontAtlas_SetTexID(atlas, id as sys::ImTextureID);
        }
    }
}
