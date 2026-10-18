//! Measurement reports the size the stylesheet asks for.
//!
//! `Fonts::measure` used to round the size it asked Dear ImGui about, on the belief that a
//! fractional size is not what gets drawn. It is: `GetFontBaked` rounds only the size a glyph is
//! *rasterized* at, and `CalcTextSizeA` and `RenderText` both scale what they produce by
//! `size / baked->Size`. So drawing was already exact while measuring was not, and every box
//! drawn around a run, every centred or right-aligned label, and every glyph position in
//! letter-spaced text inherited the difference - a few per cent at eleven pixels.
//!
//! These are all one test because a Dear ImGui context is process-wide and a test binary runs
//! its tests in one process.

use bite_imgui::font::Face;
use bite_imgui::Context;

/// Ten characters, which makes a per-character advance easy to read off a total.
const TEXT: &str = "PREVIEWING";

#[test]
fn measurement_reports_the_size_the_stylesheet_asks_for() {
    let context = Context::new(1.0).expect("a Dear ImGui context");
    let fonts = context.fonts();

    // JetBrains Mono is monospaced at 0.6 em, so twelve pixel text advances exactly 7.2 per
    // character whatever the rasterizer does. This is the assertion that needs no font tables to
    // check: if the size ever drifts again, this is off by the drift.
    let mono = fonts.measure(Face::mono(12), 1.0, TEXT)[0];
    assert!(
        (mono - 72.0).abs() < 0.05,
        "twelve pixel mono should advance 0.6 em per character, so 72.0 for ten of them, not {mono}"
    );

    // The interface face's advances for this string sum to 12690 / 2048 em, which is 68.16
    // pixels for the previewing badge at `--font-size-xs`.
    let badge = fonts.measure(Face::ui(11), 1.0, TEXT)[0];
    assert!(
        (badge - 68.16).abs() < 0.05,
        "the badge text is 68.16 pixels wide in the interface face, not {badge}"
    );

    // Canvas text is measured at the zoom it is drawn at, so measurement has to be proportional
    // to it. Rounding broke this worst of all: at zoom one the eleven pixel face rounded up and
    // at zoom two it rounded down, so a card's type changed proportion as it was zoomed.
    for scale in [0.5, 2.0, 3.0] {
        let scaled = fonts.measure(Face::ui(11), scale, TEXT)[0];
        assert!(
            (scaled - badge * scale).abs() < 0.05,
            "measuring at {scale} times the size should be {scale} times the width, not {scaled}"
        );
    }

    // Rounding put `--font-size-xs` and `--font-size-sm` on the same whole pixel, so the two
    // steps of the type scale measured identically even though they drew a size apart.
    let steps: Vec<f32> = [10, 11, 12, 13, 14, 15]
        .iter()
        .map(|size| fonts.measure(Face::ui(*size), 1.0, TEXT)[0])
        .collect();
    for pair in steps.windows(2) {
        assert!(
            pair[1] - pair[0] > 1.0,
            "each step of the type scale should measure wider than the one below it: {steps:?}"
        );
    }

    // Dear ImGui does not kern, so the measurement adds the font's own pair kerning: Inter draws
    // "To" 160 units of its 2048 to the em closer than "T" and "o" set apart, and JetBrains Mono,
    // which has no kerning, keeps every character on its 0.6 em grid.
    let separate = |face: Face, text: &str| -> f32 {
        text.chars()
            .map(|character| fonts.measure(face, 1.0, &character.to_string())[0])
            .sum()
    };
    let kerned = fonts.measure(Face::ui(13), 1.0, "To")[0] - separate(Face::ui(13), "To");
    assert!(
        (kerned - -160.0 / 2048.0 * 13.0).abs() < 0.001,
        "\"To\" should kern by -1.016 pixels at thirteen, not {kerned}"
    );
    let mono = fonts.measure(Face::mono(12), 1.0, "To")[0] - separate(Face::mono(12), "To");
    assert_eq!(mono, 0.0, "monospaced text must not be kerned");
    // Each line of a multi-line run is kerned on its own, and the run is as wide as its widest.
    let lines = fonts.measure(Face::ui(13), 1.0, "To\nTo")[0];
    assert!((lines - fonts.measure(Face::ui(13), 1.0, "To")[0]).abs() < 0.001);
}
