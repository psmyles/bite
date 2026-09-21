//! Measurement reports the size the stylesheet asks for.
//!
//! `Fonts::measure` used to round the size it asked Dear ImGui about, on the belief that a
//! fractional size is not what gets drawn. It is: `GetFontBaked` rounds only the size a glyph is
//! *rasterized* at, and `CalcTextSizeA` and `RenderText` both scale what they produce by
//! `size / baked->Size`. So drawing was already exact while measuring was not, and every box
//! drawn around a run, every centred or right-aligned label, and every glyph position in
//! letter-spaced text inherited the difference - four and a half per cent at eleven pixels.
//!
//! These are all one test because a Dear ImGui context is process-wide and a test binary runs
//! its tests in one process.

use bite_imgui::Context;
use bite_imgui::font::Face;

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

    // The interface face's advances for this string sum to 5.964 em, which is the 65.6 pixels
    // `ProcessNode.svelte` gives the previewing badge at `--font-size-xs`.
    let badge = fonts.measure(Face::ui(11), 1.0, TEXT)[0];
    assert!(
        (badge - 65.6).abs() < 0.05,
        "the badge text is 65.6 pixels wide in the stylesheet, not {badge}"
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
}
