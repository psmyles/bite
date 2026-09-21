//! Debug > Show All UI Elements, from `src/renderer/showcase/Showcase.svelte`.
//!
//! One window holding every control the editor draws, so a change to a token or a widget can
//! be seen against all of its uses at once. The dialogs are not redrawn here: the buttons in
//! the last section open the real ones, which is the only way to be sure they still match.
use crate::{
    color_picker,
    controls::{self, ButtonKind},
    modals, theme,
};
use bite_imgui::{Color, Face, Rounding, Ui};

/// The gap between two rows that draw their own text.
const ROW_GAP: f32 = 4.0;

/// The mutable values the sample controls are bound to.
pub struct Showcase {
    pub open: bool,
    text: String,
    search: String,
    slider: f32,
    number: String,
    dropdown: usize,
    checkbox: bool,
    color: [f32; 4],
    pickers: color_picker::States,
}

impl Default for Showcase {
    fn default() -> Self {
        // The seeds `Showcase.svelte` opens with, so the two windows show the same values.
        Self {
            open: false,
            text: "output-image-1".into(),
            search: String::new(),
            slider: 0.65,
            number: "0.65".into(),
            dropdown: 1,
            checkbox: true,
            color: [0.9, 0.35, 0.08, 1.0],
            pickers: color_picker::States::default(),
        }
    }
}

/// What the window asks the editor to do.
pub enum Outcome {
    /// Open one of the real dialogs.
    Show(modals::Modal),
}

/// The filter options the sample dropdown offers, as the showcase seeds them.
fn dropdown_labels() -> Vec<String> {
    ["Nearest Neighbour", "Bilinear", "Bicubic", "Lanczos"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Every text role the stylesheet names, with the token it comes from.
const TYPE_SCALE: &[(&str, Face)] = &[
    ("Panel header", theme::face::PANEL_HEADER),
    ("Body", theme::face::BODY),
    ("Library item", theme::face::NODE_ITEM),
    ("Inspector label", theme::face::LABEL),
    ("Value", theme::face::VALUE),
    ("Node header", theme::face::NODE_HEAD),
    ("Port label", theme::face::PORT_TAG),
    ("Small mono", theme::face::SMALL_MONO),
    ("Badge", theme::face::BADGE),
];

/// The surfaces and accents worth seeing side by side.
fn palette() -> Vec<(&'static str, Color)> {
    vec![
        ("--bg", theme::BG),
        ("--gap-color", theme::GAP_COLOR),
        ("--panel-bg", theme::PANEL_BG),
        ("--panel-header-bg", theme::PANEL_HEADER_BG),
        ("--node-bg", theme::NODE_BG),
        ("--border", theme::BORDER),
        ("--text", theme::TEXT),
        ("--text-bright", theme::TEXT_BRIGHT),
        ("--color-warning", theme::COLOR_WARNING),
        ("--color-danger", theme::COLOR_DANGER),
        ("--color-success", theme::COLOR_SUCCESS),
        ("--badge-preview-color", theme::BADGE_PREVIEW_COLOR),
    ]
}

/// The port colors, which say what a wire carries.
fn ports() -> Vec<(&'static str, Color)> {
    vec![
        ("image", theme::PORT_COLOR_IMAGE),
        ("number", theme::PORT_COLOR_NUMBER),
        ("boolean", theme::PORT_COLOR_BOOLEAN),
        ("string", theme::PORT_COLOR_STRING),
        ("color", theme::PORT_COLOR_COLOR),
        ("mask", theme::PORT_COLOR_MASK),
        ("vector3", theme::PORT_COLOR_VECTOR3),
    ]
}

/// Draws the window when it is open.
pub fn draw(ui: &mut Ui, state: &mut Showcase) -> Option<Outcome> {
    if !state.open {
        return None;
    }
    let mut outcome = None;
    let mut open = state.open;
    controls::debug_window(ui, "All UI Elements", [620.0, 700.0], &mut open, |ui| {
        if ui.collapsing_header("Type scale", true) {
            for (name, face) in TYPE_SCALE {
                let origin = ui.cursor_screen_position();
                let sample = format!("{name} - {}px", face.size);
                ui.draw_list()
                    .text_with_face(origin, theme::TEXT_BRIGHT, *face, &sample);
                // A sample is as tall as its own type, so the rows need a gap of their own
                // or the larger faces sit against the ones above them.
                let size = controls::measure(ui, *face, &sample);
                ui.dummy([size[0], size[1] + ROW_GAP]);
            }
        }
        if ui.collapsing_header("Palette", true) {
            for (name, color) in palette() {
                swatch(ui, name, color);
            }
        }
        if ui.collapsing_header("Ports", true) {
            for (name, color) in ports() {
                swatch(ui, name, color);
            }
        }
        if ui.collapsing_header("Controls", true) {
            controls::button(ui, "Primary", ButtonKind::Primary, 110.0, true);
            ui.same_line();
            controls::button(ui, "Neutral", ButtonKind::Neutral, 110.0, true);
            ui.same_line();
            controls::button(ui, "Danger", ButtonKind::Danger, 110.0, true);
            ui.same_line();
            controls::button(ui, "Disabled", ButtonKind::Neutral, 110.0, false);

            controls::text_input(ui, "showcase-text", &mut state.text, "Name...", 240.0);
            controls::search_input(
                ui,
                "showcase-search",
                &mut state.search,
                "Filter nodes...",
                240.0,
            );
            controls::slider(
                ui,
                "showcase-slider",
                &mut state.slider,
                0.0,
                1.0,
                180.0,
                24.0,
            );
            ui.same_line();
            controls::text_input(ui, "showcase-number", &mut state.number, "", 60.0);
            controls::dropdown(
                ui,
                "showcase-dropdown",
                &mut state.dropdown,
                &dropdown_labels(),
                240.0,
                true,
            );
            ui.checkbox("Preserve aspect ratio", &mut state.checkbox);
            color_picker::draw(
                ui,
                "showcase-color",
                &mut state.color,
                240.0,
                false,
                &mut state.pickers,
                theme::BG,
            );
            controls::badge(ui, "wired", theme::PORT_COLOR_NUMBER);
            ui.same_line();
            controls::badge(ui, "out", theme::TEXT);
        }
        if ui.collapsing_header("Dialogs", true) {
            for (label, modal) in dialogs() {
                if ui.button(label) {
                    outcome = Some(Outcome::Show(modal));
                }
                ui.same_line();
            }
            ui.dummy([1.0, 1.0]);
        }
    });
    state.open = open;
    outcome
}

/// The dialogs the buttons open, seeded as `Showcase.svelte` seeds them.
fn dialogs() -> Vec<(&'static str, modals::Modal)> {
    vec![
        (
            "Confirm",
            modals::Modal::Confirm {
                message: modals::confirm_message(modals::PendingAction::New),
                pending: modals::PendingAction::New,
            },
        ),
        (
            "About",
            modals::Modal::About {
                versions: crate::commands::about_versions(),
            },
        ),
        ("Credits", modals::Modal::Credits),
        (
            "Update",
            modals::Modal::Update(modals::UpdateState::Available {
                version: "1.4.0".into(),
                body: "Faster imports and a reworked inspector.".into(),
                url: "https://example.invalid/release".into(),
            }),
        ),
        (
            "Batch summary",
            modals::Modal::BatchSummary(modals::BatchSummary {
                processed: 24,
                skipped: 2,
                failed: 1,
                elapsed_ms: Some(4230),
                errors: vec!["corrupted_scan.jpg: decode error - unsupported colour space".into()],
                output_dir: None,
            }),
        ),
        ("Import", modals::Modal::ImportProgress),
    ]
}

/// One colour with its token name beside it.
fn swatch(ui: &mut Ui, name: &str, color: Color) {
    let origin = ui.cursor_screen_position();
    let size = 16.0;
    let list = ui.draw_list();
    list.rect(
        origin,
        [origin[0] + size * 2.0, origin[1] + size],
        color,
        3.0,
        Rounding::All,
    );
    list.rect_outline(
        origin,
        [origin[0] + size * 2.0, origin[1] + size],
        theme::BORDER,
        3.0,
        Rounding::All,
        1.0,
    );
    list.text_with_face(
        [origin[0] + size * 2.0 + 10.0, origin[1]],
        theme::TEXT_BRIGHT,
        theme::face::SMALL_MONO,
        name,
    );
    ui.dummy([240.0, size + ROW_GAP]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_opens_with_the_values_the_svelte_showcase_seeds() {
        let state = Showcase::default();
        assert!(!state.open);
        assert_eq!(state.color, [0.9, 0.35, 0.08, 1.0]);
        assert_eq!(dropdown_labels()[state.dropdown], "Bilinear");
    }

    #[test]
    fn every_dialog_the_editor_draws_has_a_button() {
        let names: Vec<&str> = dialogs().into_iter().map(|(name, _)| name).collect();
        assert!(names.contains(&"Confirm"));
        assert!(names.contains(&"About"));
        assert!(names.contains(&"Credits"));
        assert!(names.contains(&"Update"));
        assert!(names.contains(&"Batch summary"));
        assert!(names.contains(&"Import"));
    }

    #[test]
    fn the_type_scale_covers_every_role_without_repeating_a_name() {
        let mut names: Vec<&str> = TYPE_SCALE.iter().map(|(name, _)| *name).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before);
        assert!(before >= 8);
    }
}
