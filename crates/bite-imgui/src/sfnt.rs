//! The parts of a TrueType file the font module reads for itself: the table directory, the
//! vertical metrics and the character map. Dear ImGui reads the same files to rasterize them, but
//! exposes none of this.
use std::collections::HashMap;

/// A big-endian view of font data.
///
/// Every read is bounds-checked and comes back `None` past the end, so a truncated or malformed
/// table reads as missing rather than panicking.
#[derive(Clone, Copy)]
pub(crate) struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    pub fn u16(self, at: usize) -> Option<u16> {
        let bytes = self.0.get(at..at.checked_add(2)?)?;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn i16(self, at: usize) -> Option<i16> {
        self.u16(at).map(|value| value as i16)
    }

    pub fn u32(self, at: usize) -> Option<u32> {
        let bytes = self.0.get(at..at.checked_add(4)?)?;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn tag(self, at: usize) -> Option<&'a [u8]> {
        self.0.get(at..at.checked_add(4)?)
    }

    /// The data from `at` on, for following an offset to a subtable.
    pub fn from(self, at: usize) -> Option<Self> {
        self.0.get(at..).map(Bytes)
    }

    /// The data from the offset stored as a `u16` at `at`, which is how most tables point at
    /// their subtables. An offset of zero is OpenType's null, and points at nothing.
    pub fn follow(self, at: usize) -> Option<Self> {
        match self.u16(at)? {
            0 => None,
            offset => self.from(usize::from(offset)),
        }
    }
}

/// The table tagged `tag`, found through the font's table directory.
pub(crate) fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<Bytes<'a>> {
    let font = Bytes(font);
    (0..usize::from(font.u16(4)?)).find_map(|index| {
        let record = 12 + index * 16;
        if font.tag(record)? != tag {
            return None;
        }
        let start = font.u32(record + 8)? as usize;
        let length = font.u32(record + 12)? as usize;
        font.0.get(start..start.checked_add(length)?).map(Bytes)
    })
}

/// The units the font's outlines and metrics are measured in, per em.
pub(crate) fn units_per_em(font: &[u8]) -> Option<f32> {
    let units = table(font, b"head")?.u16(18)?;
    (units > 0).then_some(f32::from(units))
}

/// The factor between a face's CSS pixel size and the size Dear ImGui must be asked for, and the
/// ascender, both in ems.
///
/// The size handed to `AddFontFromMemoryTTF` - and, since 1.92, to `igPushFont` - is fed to the
/// rasterizer as "make ascender minus descender this many pixels". A stylesheet's `font-size` is
/// the em size instead. The two differ by `(ascender - descender) / unitsPerEm`, which is 1.32
/// for JetBrains Mono: asking for twelve there would draw an em of nine, three quarters of the
/// size the stylesheet asked for. Multiplying by this factor makes the em come out right.
///
/// The ascender is how far below the top of a drawn run its baseline sits, since Dear ImGui
/// places glyphs from the `hhea` ascent.
pub(crate) fn vertical_metrics(font: &[u8]) -> Option<(f32, f32)> {
    let units_per_em = units_per_em(font)?;
    let horizontal = table(font, b"hhea")?;
    let ascender = f32::from(horizontal.i16(4)?);
    let descender = f32::from(horizontal.i16(6)?);
    let height = ascender - descender;
    (height > 0.0).then_some((height / units_per_em, ascender / units_per_em))
}

/// Which glyph draws each character, from the font's Unicode character map.
///
/// Format 12 covers every plane and is preferred; format 4 covers the Basic Multilingual Plane,
/// which is all an older font has. A font with neither maps nothing.
pub(crate) fn character_map(font: &[u8]) -> HashMap<char, u16> {
    let Some(cmap) = table(font, b"cmap") else {
        return HashMap::new();
    };
    let unicode = |format: u16| {
        (0..usize::from(cmap.u16(2)?)).find_map(|index| {
            let record = 4 + index * 8;
            // Platform 0 is Unicode; platform 3 is Windows, where encodings 1 and 10 are Unicode.
            let unicode = matches!(
                (cmap.u16(record)?, cmap.u16(record + 2)?),
                (0, _) | (3, 1) | (3, 10)
            );
            let subtable = cmap.from(cmap.u32(record + 4)? as usize)?;
            (unicode && subtable.u16(0)? == format).then_some(subtable)
        })
    };
    let mut glyphs = HashMap::new();
    if let Some(subtable) = unicode(12) {
        read_segmented_coverage(subtable, &mut glyphs);
    } else if let Some(subtable) = unicode(4) {
        read_segment_mapping(subtable, &mut glyphs);
    }
    glyphs
}

/// Format 12: runs of consecutive characters drawn by consecutive glyphs.
fn read_segmented_coverage(subtable: Bytes, glyphs: &mut HashMap<char, u16>) -> Option<()> {
    for group in 0..subtable.u32(12)? as usize {
        let record = 16 + group * 12;
        let (first, last) = (subtable.u32(record)?, subtable.u32(record + 4)?);
        let start_glyph = subtable.u32(record + 8)?;
        for code in first..=last.min(char::MAX as u32) {
            let glyph = start_glyph + (code - first);
            if let (Some(character), Ok(glyph)) = (char::from_u32(code), u16::try_from(glyph)) {
                glyphs.insert(character, glyph);
            }
        }
    }
    Some(())
}

/// Format 4: segments of the Basic Multilingual Plane, each mapped by a delta or through an
/// array of glyph ids.
fn read_segment_mapping(subtable: Bytes, glyphs: &mut HashMap<char, u16>) -> Option<()> {
    let segments = usize::from(subtable.u16(6)? / 2);
    let ends = 14;
    let starts = ends + segments * 2 + 2;
    let deltas = starts + segments * 2;
    let range_offsets = deltas + segments * 2;
    for segment in 0..segments {
        let end = subtable.u16(ends + segment * 2)?;
        let start = subtable.u16(starts + segment * 2)?;
        let delta = subtable.u16(deltas + segment * 2)?;
        let range_offset_at = range_offsets + segment * 2;
        let range_offset = usize::from(subtable.u16(range_offset_at)?);
        for code in start..=end {
            // The last segment ends at 0xFFFF only to close the table.
            if code == 0xFFFF {
                break;
            }
            let glyph = if range_offset == 0 {
                code.wrapping_add(delta)
            } else {
                // The offset is counted from where it is stored, in bytes.
                let at = range_offset_at + range_offset + usize::from(code - start) * 2;
                match subtable.u16(at)? {
                    0 => 0,
                    glyph => glyph.wrapping_add(delta),
                }
            };
            if let (Some(character), true) = (char::from_u32(u32::from(code)), glyph != 0) {
                glyphs.insert(character, glyph);
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{MONO_REGULAR, UI_REGULAR};

    #[test]
    fn inter_lays_a_line_out_on_its_typographic_metrics() {
        let (height, ascender) = vertical_metrics(UI_REGULAR).unwrap();
        assert_eq!(height, (1984.0 + 494.0) / 2048.0);
        assert_eq!(ascender, 1984.0 / 2048.0);
        assert_eq!(units_per_em(MONO_REGULAR), Some(1000.0));
    }

    #[test]
    fn every_printable_character_has_a_glyph() {
        for font in [UI_REGULAR, MONO_REGULAR] {
            let glyphs = character_map(font);
            for character in ' '..='~' {
                assert!(glyphs.get(&character).is_some_and(|&glyph| glyph != 0));
            }
            // Distinct characters draw through distinct glyphs.
            assert_ne!(glyphs[&'A'], glyphs[&'B']);
        }
    }

    #[test]
    fn a_missing_table_reads_as_absent_rather_than_failing() {
        assert!(table(UI_REGULAR, b"none").is_none());
        assert!(vertical_metrics(&UI_REGULAR[..64]).is_none());
        assert!(character_map(&[]).is_empty());
    }
}
