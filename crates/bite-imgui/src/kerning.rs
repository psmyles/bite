//! Pair kerning from the font's `GPOS` table, which Dear ImGui does not apply.
//!
//! Dear ImGui advances every glyph by its own width. The stb_truetype it bundles can look kerning
//! up, but Dear ImGui never asks it to - and stb_truetype reads only pair lookups stored directly
//! in `GPOS`, while Inter keeps its Latin kerning inside an extension lookup, so it would find
//! nothing anyway. This reads the pairs the font's `kern` feature names for Latin text, for the
//! font module to measure and draw with.
use crate::sfnt::{self, Bytes};
use std::collections::HashMap;

/// A font's kerning: how far each pair of characters moves apart, in ems.
pub(crate) struct Kerning {
    glyphs: HashMap<char, u16>,
    /// The `kern` feature's lookups in the order they apply, each as its subtables in order.
    lookups: Vec<Vec<PairAdjustment>>,
    units_per_em: f32,
    /// Every pair of printable ASCII characters, looked up once when the font is read.
    ///
    /// Nearly all of the interface's text is ASCII, and every run's pairs are asked about on
    /// every frame it is measured or drawn; an index here keeps that to the cost of measuring
    /// the text at all, where walking the lookups for each pair cost many times more.
    ascii: Vec<f32>,
}

/// The printable ASCII characters, which [`Kerning::ascii`] holds every pair of.
const ASCII: std::ops::RangeInclusive<char> = ' '..='~';
const ASCII_COUNT: usize = 95;

fn ascii_index(character: char) -> Option<usize> {
    ASCII
        .contains(&character)
        .then(|| character as usize - ' ' as usize)
}

/// One pair adjustment subtable, holding horizontal advance changes in font units.
enum PairAdjustment {
    /// Format 1: an adjustment for each listed second glyph, per covered first glyph.
    Glyphs {
        coverage: ByGlyph,
        /// Each covered glyph's second glyphs, sorted, with their adjustments.
        pairs: Vec<Vec<(u16, i16)>>,
    },
    /// Format 2: an adjustment for each pair of glyph classes.
    Classes {
        coverage: ByGlyph,
        first: ByGlyph,
        second: ByGlyph,
        second_classes: usize,
        values: Vec<i16>,
    },
}

impl PairAdjustment {
    /// The adjustment for a pair, or `None` when this subtable does not cover it and the next
    /// one should be asked.
    fn get(&self, left: u16, right: u16) -> Option<i16> {
        match self {
            Self::Glyphs { coverage, pairs } => {
                let seconds = pairs.get(usize::from(coverage.get(left)?))?;
                let index = seconds
                    .binary_search_by_key(&right, |&(glyph, _)| glyph)
                    .ok()?;
                Some(seconds[index].1)
            }
            Self::Classes {
                coverage,
                first,
                second,
                second_classes,
                values,
            } => {
                coverage.get(left)?;
                // A glyph no class lists is in class zero.
                let row = usize::from(first.get(left).unwrap_or(0));
                let column = usize::from(second.get(right).unwrap_or(0));
                values.get(row * second_classes + column).copied()
            }
        }
    }
}

/// A value for some of a font's glyphs - a coverage index, or a class - indexed by glyph id.
///
/// Glyph ids are small and dense, so a table is quicker to fill and to read than a map.
#[derive(Default)]
struct ByGlyph(Vec<Option<u16>>);

impl ByGlyph {
    fn get(&self, glyph: u16) -> Option<u16> {
        self.0.get(usize::from(glyph)).copied().flatten()
    }

    fn insert(&mut self, glyph: u16, value: u16) {
        let index = usize::from(glyph);
        if index >= self.0.len() {
            self.0.resize(index + 1, None);
        }
        self.0[index] = Some(value);
    }
}

impl Kerning {
    /// Reads a font's kerning. A font without any - a monospaced one, or one whose tables cannot
    /// be read - has none, and costs nothing to ask.
    pub fn read(font: &[u8]) -> Self {
        let lookups = sfnt::table(font, b"GPOS")
            .and_then(read_lookups)
            .unwrap_or_default();
        let mut kerning = Self {
            glyphs: if lookups.is_empty() {
                HashMap::new()
            } else {
                sfnt::character_map(font)
            },
            lookups,
            units_per_em: sfnt::units_per_em(font).unwrap_or(1000.0),
            ascii: Vec::new(),
        };
        if !kerning.is_empty() {
            kerning.ascii = ASCII
                .flat_map(|left| ASCII.map(move |right| (left, right)))
                .map(|(left, right)| kerning.look_up(left, right))
                .collect();
        }
        kerning
    }

    pub fn is_empty(&self) -> bool {
        self.lookups.is_empty()
    }

    /// How far `right` moves when it follows `left`, in ems; negative draws the two closer.
    pub fn adjustment(&self, left: char, right: char) -> f32 {
        match (ascii_index(left), ascii_index(right)) {
            (Some(left), Some(right)) => self
                .ascii
                .get(left * ASCII_COUNT + right)
                .copied()
                .unwrap_or(0.0),
            _ => self.look_up(left, right),
        }
    }

    /// The adjustment for a pair, from the lookups themselves.
    ///
    /// Every lookup adds its adjustment, and within a lookup the first subtable that covers the
    /// pair is the one that applies.
    fn look_up(&self, left: char, right: char) -> f32 {
        let (Some(&left), Some(&right)) = (self.glyphs.get(&left), self.glyphs.get(&right)) else {
            return 0.0;
        };
        let units: i32 = self
            .lookups
            .iter()
            .filter_map(|subtables| {
                subtables
                    .iter()
                    .find_map(|subtable| subtable.get(left, right))
            })
            .map(i32::from)
            .sum();
        units as f32 / self.units_per_em
    }

    /// Each kerned pair in `text`: the byte index of its second character and its adjustment in
    /// ems. Pairs that adjust nothing are left out, so this is empty for most runs.
    pub fn pairs<'a>(&'a self, text: &'a str) -> impl Iterator<Item = (usize, f32)> + 'a {
        let mut previous = None;
        text.char_indices().filter_map(move |(index, character)| {
            let left = previous.replace(character)?;
            let adjustment = self.adjustment(left, character);
            (adjustment != 0.0).then_some((index, adjustment))
        })
    }

    /// The total adjustment across `text`, in ems.
    pub fn total(&self, text: &str) -> f32 {
        self.pairs(text).map(|(_, adjustment)| adjustment).sum()
    }
}

/// The pair adjustment lookups the `kern` feature applies to Latin text, in lookup order.
fn read_lookups(gpos: Bytes) -> Option<Vec<Vec<PairAdjustment>>> {
    let mut indices = kern_lookup_indices(gpos)?;
    indices.sort_unstable();
    indices.dedup();
    let list = gpos.follow(8)?;
    let lookups = indices
        .into_iter()
        .filter_map(|index| read_lookup(list.follow(2 + usize::from(index) * 2)?))
        .filter(|subtables| !subtables.is_empty())
        .collect();
    Some(lookups)
}

/// The lookups the `kern` feature names for the Latin script, or the default script when the
/// font gives Latin no entry of its own.
fn kern_lookup_indices(gpos: Bytes) -> Option<Vec<u16>> {
    let scripts = gpos.follow(4)?;
    let records = 0..usize::from(scripts.u16(0)?);
    let script = |tag: &[u8]| {
        records
            .clone()
            .find(|record| scripts.tag(2 + record * 6) == Some(tag))
            .and_then(|record| scripts.follow(2 + record * 6 + 4))
    };
    let language = script(b"latn").or_else(|| script(b"DFLT"))?.follow(0)?;
    let features = gpos.follow(6)?;
    let mut indices = Vec::new();
    for slot in 0..usize::from(language.u16(4)?) {
        let record = 2 + usize::from(language.u16(6 + slot * 2)?) * 6;
        if features.tag(record)? != b"kern" {
            continue;
        }
        let feature = features.follow(record + 4)?;
        for lookup in 0..usize::from(feature.u16(2)?) {
            indices.push(feature.u16(4 + lookup * 2)?);
        }
    }
    Some(indices)
}

/// A lookup's pair adjustment subtables, unwrapping extension lookups. A lookup of any other
/// kind has none.
fn read_lookup(lookup: Bytes) -> Option<Vec<PairAdjustment>> {
    const PAIR_ADJUSTMENT: u16 = 2;
    const EXTENSION: u16 = 9;
    let kind = lookup.u16(0)?;
    let subtables = (0..usize::from(lookup.u16(4)?))
        .filter_map(|index| {
            let subtable = lookup.follow(6 + index * 2)?;
            match kind {
                PAIR_ADJUSTMENT => read_pair_adjustment(subtable),
                // An extension holds the real subtable at a 32-bit offset, for tables too big
                // for the 16-bit ones.
                EXTENSION if subtable.u16(2)? == PAIR_ADJUSTMENT => {
                    read_pair_adjustment(subtable.from(subtable.u32(4)? as usize)?)
                }
                _ => None,
            }
        })
        .collect();
    Some(subtables)
}

fn read_pair_adjustment(subtable: Bytes) -> Option<PairAdjustment> {
    let coverage = read_coverage(subtable.follow(2)?)?;
    let first_format = subtable.u16(4)?;
    let second_format = subtable.u16(6)?;
    // Kerning is the advance of the first glyph; a subtable that changes anything else is not
    // kerning, and is left out.
    let advance = advance_offset(first_format)?;
    let record = value_record_size(first_format) + value_record_size(second_format);
    match subtable.u16(0)? {
        1 => {
            let mut pairs = Vec::new();
            for set in 0..usize::from(subtable.u16(8)?) {
                let set = subtable.follow(10 + set * 2)?;
                let mut seconds = Vec::new();
                for pair in 0..usize::from(set.u16(0)?) {
                    let at = 2 + pair * (2 + record);
                    seconds.push((set.u16(at)?, set.i16(at + 2 + advance)?));
                }
                // The font lists them in order already; sorting keeps a font that does not from
                // breaking the search.
                seconds.sort_unstable_by_key(|&(glyph, _)| glyph);
                pairs.push(seconds);
            }
            Some(PairAdjustment::Glyphs { coverage, pairs })
        }
        2 => {
            let first_classes = usize::from(subtable.u16(12)?);
            let second_classes = usize::from(subtable.u16(14)?);
            let values = (0..first_classes * second_classes)
                .map(|cell| subtable.i16(16 + cell * record + advance))
                .collect::<Option<_>>()?;
            Some(PairAdjustment::Classes {
                coverage,
                first: read_class_definition(subtable.follow(8)?)?,
                second: read_class_definition(subtable.follow(10)?)?,
                second_classes,
                values,
            })
        }
        _ => None,
    }
}

/// Where a value record's advance sits, or `None` when the record has no advance. Each field
/// the format flags takes two bytes, in flag order, and the advance comes after the two
/// placements.
fn advance_offset(format: u16) -> Option<usize> {
    const X_ADVANCE: u16 = 0x0004;
    (format & X_ADVANCE != 0).then(|| (format & 0x0003).count_ones() as usize * 2)
}

fn value_record_size(format: u16) -> usize {
    (format & 0x00FF).count_ones() as usize * 2
}

/// The glyphs a subtable covers, each with its index into the subtable's own arrays.
fn read_coverage(table: Bytes) -> Option<ByGlyph> {
    let mut coverage = ByGlyph::default();
    match table.u16(0)? {
        1 => {
            for index in 0..table.u16(2)? {
                coverage.insert(table.u16(4 + usize::from(index) * 2)?, index);
            }
        }
        2 => {
            for range in 0..usize::from(table.u16(2)?) {
                let at = 4 + range * 6;
                let (first, last) = (table.u16(at)?, table.u16(at + 2)?);
                let start = table.u16(at + 4)?;
                for glyph in first..=last {
                    coverage.insert(glyph, start.checked_add(glyph - first)?);
                }
            }
        }
        _ => return None,
    }
    Some(coverage)
}

/// Which class each listed glyph is in.
fn read_class_definition(table: Bytes) -> Option<ByGlyph> {
    let mut classes = ByGlyph::default();
    match table.u16(0)? {
        1 => {
            let first = table.u16(2)?;
            for index in 0..table.u16(4)? {
                classes.insert(
                    first.checked_add(index)?,
                    table.u16(6 + usize::from(index) * 2)?,
                );
            }
        }
        2 => {
            for range in 0..usize::from(table.u16(2)?) {
                let at = 4 + range * 6;
                let class = table.u16(at + 4)?;
                for glyph in table.u16(at)?..=table.u16(at + 2)? {
                    classes.insert(glyph, class);
                }
            }
        }
        _ => return None,
    }
    Some(classes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{MONO_REGULAR, UI_BOLD, UI_REGULAR};

    /// Inter's own values, in its 2048 units to the em, as the font's tables give them.
    fn units(kerning: &Kerning, pair: &str) -> f32 {
        let mut characters = pair.chars();
        let (left, right) = (characters.next().unwrap(), characters.next().unwrap());
        (kerning.adjustment(left, right) * 2048.0).round()
    }

    #[test]
    fn inter_kerns_the_pairs_its_tables_name() {
        let regular = Kerning::read(UI_REGULAR);
        assert_eq!(units(&regular, "To"), -160.0);
        assert_eq!(units(&regular, "LT"), -197.0);
        assert_eq!(units(&regular, "AV"), -140.0);
        assert_eq!(units(&regular, "rk"), 32.0);
        assert_eq!(units(&regular, "es"), 0.0);
        // Each weight carries its own values.
        assert_eq!(units(&Kerning::read(UI_BOLD), "Yo"), -224.0);
    }

    #[test]
    fn the_ascii_table_holds_what_the_lookups_give() {
        let kerning = Kerning::read(UI_REGULAR);
        assert_eq!(kerning.ascii.len(), ASCII_COUNT * ASCII_COUNT);
        for left in ASCII {
            for right in ASCII {
                assert_eq!(
                    kerning.adjustment(left, right),
                    kerning.look_up(left, right)
                );
            }
        }
        // Outside ASCII the lookups are walked: an accented capital kerns as its base letter does.
        assert_eq!(units(&kerning, "\u{C0}V"), units(&kerning, "AV"));
    }

    #[test]
    fn a_monospaced_font_has_no_kerning() {
        let mono = Kerning::read(MONO_REGULAR);
        assert!(mono.is_empty());
        assert_eq!(mono.adjustment('T', 'o'), 0.0);
    }

    #[test]
    fn only_the_pairs_that_move_are_reported() {
        let kerning = Kerning::read(UI_REGULAR);
        // "To" draws in by 160 units and "ta" out by 10; "ot" and "al" are left alone.
        let pairs: Vec<usize> = kerning.pairs("Total").map(|(index, _)| index).collect();
        assert_eq!(pairs, [1, 3]);
        assert_eq!(kerning.total("Total"), -150.0 / 2048.0);
        // Nothing kerns across a line break, or with a character the font lacks.
        assert_eq!(kerning.total("T\no"), 0.0);
        assert_eq!(kerning.adjustment('T', '\u{10FFFF}'), 0.0);
    }

    #[test]
    fn unreadable_data_has_no_kerning() {
        assert!(Kerning::read(&UI_REGULAR[..1024]).is_empty());
        assert!(Kerning::read(&[]).is_empty());
    }
}
