//! Bounded header reader ported from the legacy fast path.
use std::{io::Read, path::Path};
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(131072)
        .read_to_end(&mut data)
        .ok()?;
    parse(
        &data,
        path.extension().and_then(|s| s.to_str()).unwrap_or(""),
    )
}
pub fn parse(b: &[u8], extension: &str) -> Option<(u32, u32)> {
    let be32 = |i| {
        b.get(i..i + 4)
            .map(|s| u32::from_be_bytes(s.try_into().unwrap()))
    };
    let le32 = |i| {
        b.get(i..i + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    };
    let be16 = |i| {
        b.get(i..i + 2)
            .map(|s| u16::from_be_bytes(s.try_into().unwrap()) as u32)
    };
    let le16 = |i| {
        b.get(i..i + 2)
            .map(|s| u16::from_le_bytes(s.try_into().unwrap()) as u32)
    };
    let result = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some((be32(16)?, be32(20)?))
    } else if b.starts_with(b"BM") {
        let w = le32(18)? as i32;
        let h = le32(22)? as i32;
        Some((w.unsigned_abs(), h.unsigned_abs()))
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        match b.get(12..16)? {
            b"VP8 " if b.get(23..26) == Some(b"\x9d\x01\x2a") => {
                Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff))
            }
            b"VP8L" if b.get(20) == Some(&0x2f) => {
                let n = le32(21)?;
                Some(((n & 0x3fff) + 1, ((n >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => {
                let data = b.get(24..30)?;
                Some((
                    u32::from_le_bytes([data[0], data[1], data[2], 0]) + 1,
                    u32::from_le_bytes([data[3], data[4], data[5], 0]) + 1,
                ))
            }
            _ => None,
        }
    } else if b.starts_with(b"\xff\xd8") {
        let mut i = 2;
        let mut result = None;
        while i + 1 < b.len() {
            if b[i] != 0xff {
                break;
            }
            while b.get(i) == Some(&0xff) {
                i += 1;
            }
            let marker = *b.get(i)?;
            i += 1;
            if matches!(marker, 0xd0..=0xd9) {
                continue;
            }
            if marker == 0xda {
                break;
            }
            let len = be16(i)? as usize;
            if len < 2 {
                break;
            }
            if matches!(marker,0xc0..=0xc3|0xc5..=0xc7|0xc9..=0xcb|0xcd..=0xcf) {
                result = Some((be16(i + 5)?, be16(i + 3)?));
                break;
            }
            i += len;
        }
        result
    } else if ["tga", "targa"].contains(&extension.to_lowercase().as_str())
        && b.get(2)
            .is_some_and(|t| [0, 1, 2, 3, 9, 10, 11].contains(t))
    {
        Some((le16(12)?, le16(14)?))
    } else {
        None
    };
    result.filter(|(w, h)| *w > 0 && *h > 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_headers_never_panic() {
        for n in 0..64 {
            let bytes = vec![0xff; n];
            assert_eq!(parse(&bytes, "png"), None);
            assert_eq!(parse(&bytes, "tga"), None);
        }
    }
    #[test]
    fn png_and_webp() {
        let mut p = vec![0; 24];
        p[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        p[16..20].copy_from_slice(&128u32.to_be_bytes());
        p[20..24].copy_from_slice(&75u32.to_be_bytes());
        assert_eq!(parse(&p, "png"), Some((128, 75)));
        let mut w = vec![0; 25];
        w[..4].copy_from_slice(b"RIFF");
        w[8..16].copy_from_slice(b"WEBPVP8L");
        w[20] = 0x2f;
        w[21..25].copy_from_slice(&(63u32 | (31 << 14)).to_le_bytes());
        assert_eq!(parse(&w, "webp"), Some((64, 32)));
    }

    #[test]
    fn jpeg_bmp_and_tga_dimensions() {
        let jpeg = [
            0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 75, 0, 128, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0,
        ];
        assert_eq!(parse(&jpeg, "jpg"), Some((128, 75)));

        let mut bmp = vec![0; 26];
        bmp[..2].copy_from_slice(b"BM");
        bmp[18..22].copy_from_slice(&128i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-75i32).to_le_bytes());
        assert_eq!(parse(&bmp, "bmp"), Some((128, 75)));

        let mut tga = vec![0; 18];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&128u16.to_le_bytes());
        tga[14..16].copy_from_slice(&75u16.to_le_bytes());
        assert_eq!(parse(&tga, "tga"), Some((128, 75)));
    }
}
