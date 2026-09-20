//! The dialogs, matching the modal components under `src/renderer/components`.
//!
//! They share one visual language: a dimmed backdrop, a panel with a two-pixel border and
//! an eight-pixel radius, a titled header, a body, and a right-aligned footer.
use crate::{
    controls::{self, ButtonKind},
    theme,
};
use bite_imgui::{Rounding, StyleColor, StyleVar, Ui, Vec2, WindowFlags};

/// Which dialog is showing. Only one is ever open at a time.
#[derive(Clone, Debug, PartialEq)]
pub enum Modal {
    None,
    /// The unsaved-changes prompt, carrying the action it guards.
    Confirm {
        message: String,
        pending: PendingAction,
    },
    About {
        versions: Vec<(String, String)>,
    },
    Credits,
    Update(UpdateState),
    IncompatibleVersion {
        file_version: Option<String>,
    },
    /// The choice dialog shown when some outputs are ready and others are not.
    RunWorkflow {
        nodes: Vec<RunCandidate>,
    },
    BatchProgress,
    BatchSummary(BatchSummary),
    ImportProgress,
    /// A plain message, used for the failures the Electron build reports with an alert.
    Message {
        title: String,
        body: String,
    },
}

/// The action an unsaved-changes prompt is guarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    New,
    Open,
    Exit,
    OpenPath,
}

/// The four states of the update dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Checking,
    Available {
        version: String,
        body: String,
        url: String,
    },
    Latest {
        version: String,
        body: String,
    },
    Failed,
}

impl UpdateState {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Checking => "Checking for Updates",
            Self::Available { .. } => "Update Available",
            Self::Latest { .. } => "You're up to date",
            Self::Failed => "Update Check Failed",
        }
    }
}

/// One output node in the run dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct RunCandidate {
    pub id: String,
    pub label: String,
    /// The reasons the node cannot run; an empty list means it is ready.
    pub reasons: Vec<String>,
}

impl RunCandidate {
    pub fn valid(&self) -> bool {
        self.reasons.is_empty()
    }
}

/// The batch result the summary dialog reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchSummary {
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub elapsed_ms: Option<u64>,
    pub errors: Vec<String>,
    pub output_dir: Option<String>,
}

/// Progress for the batch and import dialogs.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub completed: usize,
    pub total: usize,
    pub elapsed_seconds: f32,
    pub error: Option<String>,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.completed as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    pub fn percent(&self) -> i32 {
        (self.fraction() * 100.0).round() as i32
    }
}

/// Formats an elapsed time the way the progress and summary dialogs show it.
pub fn format_elapsed(seconds: f32) -> String {
    if seconds < 60.0 {
        format!("{}s", seconds.round() as i64)
    } else {
        let whole = seconds.round() as i64;
        format!("{}m {}s", whole / 60, whole % 60)
    }
}

/// The message the unsaved-changes prompt shows for each guarded action.
pub fn confirm_message(action: PendingAction) -> String {
    let verb = match action {
        PendingAction::New => "Start a new workflow",
        PendingAction::Open | PendingAction::OpenPath => "Open a different workflow",
        PendingAction::Exit => "Exit",
    };
    format!("You have unsaved changes. {verb} anyway?")
}

/// What the user chose in a dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Dismissed,
    ConfirmSave(PendingAction),
    ConfirmDiscard(PendingAction),
    RunSelected,
    CancelBatch,
    CancelImport,
    OpenOutputFolder(String),
    OpenUrl(String),
}

/// Draws whichever dialog is open. The dimmed backdrop comes from the modal popup itself,
/// so everything behind it is both darkened and unclickable.
pub fn draw(ui: &mut Ui, modal: &Modal, progress: &Progress, _viewport: Vec2) -> Option<Outcome> {
    if matches!(modal, Modal::None) {
        return None;
    }
    match modal {
        Modal::None => None,
        Modal::Confirm { message, pending } => confirm(ui, message, *pending),
        Modal::About { versions } => about(ui, versions),
        Modal::Credits => credits(ui),
        Modal::Update(state) => update(ui, state),
        Modal::IncompatibleVersion { file_version } => {
            incompatible_version(ui, file_version.as_deref())
        }
        Modal::RunWorkflow { nodes } => run_workflow(ui, nodes),
        Modal::BatchProgress => batch_progress(ui, progress),
        Modal::BatchSummary(summary) => batch_summary(ui, summary),
        Modal::ImportProgress => import_progress(ui, progress),
        Modal::Message { title, body } => message(ui, title, body),
    }
}

/// The shared panel chrome. `body` returns the outcome.
fn panel(
    ui: &mut Ui,
    id: &str,
    title: &str,
    width: f32,
    show_close: bool,
    body: impl FnOnce(&mut Ui, f32) -> Option<Outcome>,
) -> Option<Outcome> {
    let mut outcome = None;
    // Opening the popup by identifier closes any dialog that was showing before it.
    if !ui.popup_open(id) {
        ui.open_popup(id);
    }
    ui.center_next_window();
    ui.set_next_window_size_once([width, 0.0]);
    let flags = WindowFlags {
        always_auto_resize: true,
        no_scrollbar: true,
        no_title_bar: true,
        no_resize: true,
        no_move: true,
        no_saved_settings: true,
        ..WindowFlags::default()
    };

    ui.with_style(
        &[
            StyleVar::WindowPadding([0.0, 0.0]),
            StyleVar::WindowRounding(theme::PANEL_RADIUS),
            StyleVar::WindowBorderSize(2.0),
            StyleVar::ItemSpacing([0.0, 0.0]),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::WindowBg, theme::CTX_BG),
                    (StyleColor::Border, theme::CTX_BORDER),
                    (StyleColor::Text, theme::TEXT),
                ],
                |ui| {
                    ui.modal_without_close(id, flags, |ui| {
                        // Header.
                        ui.dummy([width, 12.0]);
                        ui.set_cursor_screen_position([
                            ui.cursor_screen_position()[0] + 14.0,
                            ui.cursor_screen_position()[1],
                        ]);
                        ui.group(|ui| {
                            let origin = ui.cursor_screen_position();
                            controls::draw_tracked_text(
                                ui,
                                origin,
                                theme::TEXT_BRIGHT,
                                theme::face::MODAL_TITLE,
                                title,
                                0.04 * f32::from(theme::FONT_SIZE_BASE),
                            );
                            ui.dummy([width - 28.0, 16.0]);
                            if show_close {
                                ui.set_cursor_screen_position([
                                    origin[0] + width - 28.0 - theme::MODAL_CLOSE_BTN_SIZE,
                                    origin[1] - 5.0,
                                ]);
                                if controls::close_button(ui, id) {
                                    outcome = Some(Outcome::Dismissed);
                                }
                            }
                        });
                        ui.dummy([width, 11.0]);
                        let divider = ui.cursor_screen_position();
                        ui.draw_list().rect(
                            divider,
                            [divider[0] + width, divider[1] + 2.0],
                            theme::CTX_BORDER,
                            0.0,
                            Rounding::None,
                        );
                        ui.dummy([width, 2.0]);

                        if let Some(result) = body(ui, width) {
                            outcome = Some(result);
                            // The dialog closes itself once a choice has been made.
                            ui.close_current_popup();
                        }
                    });
                },
            )
        },
    );
    outcome
}

/// Removes the body inset, then draws the divider and a right-aligned button row.
fn footer(
    ui: &mut Ui,
    width: f32,
    padding: f32,
    buttons: &[(&str, ButtonKind, bool)],
) -> Option<usize> {
    ui.unindent(padding);
    let origin = ui.cursor_screen_position();
    ui.draw_list().rect(
        origin,
        [origin[0] + width, origin[1] + 2.0],
        theme::CTX_BORDER,
        0.0,
        Rounding::None,
    );
    ui.dummy([width, 2.0]);
    ui.dummy([width, 12.0]);

    let mut widths = Vec::new();
    for (label, _, _) in buttons {
        widths.push(controls::measure(ui, theme::face::BUTTON, label)[0] + theme::BUTTON_PADDING_X * 2.0);
    }
    let total: f32 = widths.iter().sum::<f32>() + 8.0 * (buttons.len().max(1) - 1) as f32;
    ui.set_cursor_screen_position([
        ui.cursor_screen_position()[0] + width - 16.0 - total,
        ui.cursor_screen_position()[1],
    ]);

    let mut pressed = None;
    ui.group(|ui| {
        for (index, (label, kind, enabled)) in buttons.iter().enumerate() {
            if controls::button(ui, label, *kind, widths[index], *enabled) {
                pressed = Some(index);
            }
            if index + 1 < buttons.len() {
                ui.same_line_at(0.0, 8.0);
            }
        }
    });
    ui.dummy([width, 12.0]);
    pressed
}

/// Opens the body with a top margin and a horizontal inset, and reports the width left for
/// content. The inset is an indent so that it survives every line break, and [`footer`]
/// removes it again.
fn body_start(ui: &mut Ui, width: f32, padding: f32, top: f32) -> f32 {
    ui.dummy([width, top]);
    ui.indent(padding);
    width - padding * 2.0
}

fn confirm(ui: &mut Ui, message: &str, pending: PendingAction) -> Option<Outcome> {
    // The header reads only the application name, as `ConfirmModal.svelte` does.
    panel(ui, "confirm", "Bite", 320.0, false, |ui, width| {
        let inner = body_start(ui, width, 20.0, 20.0);
        ui.with_face(theme::face::BODY, |ui| ui.text_wrapped(message));
        ui.dummy([inner, 20.0]);
        // Cancel comes first so that the safe choice is the default.
        let pressed = footer(
            ui,
            width,
            20.0,
            &[
                ("Cancel", ButtonKind::Neutral, true),
                ("OK", ButtonKind::Danger, true),
            ],
        );
        match pressed {
            Some(0) => Some(Outcome::Dismissed),
            Some(1) => Some(Outcome::ConfirmDiscard(pending)),
            _ => None,
        }
    })
}

fn message(ui: &mut Ui, title: &str, text: &str) -> Option<Outcome> {
    panel(ui, "message", title, 340.0, true, |ui, width| {
        let inner = body_start(ui, width, 20.0, 18.0);
        ui.with_face(theme::face::BODY, |ui| {
            ui.set_next_item_width(inner);
            ui.text_wrapped(text);
        });
        ui.dummy([inner, 18.0]);
        footer(ui, width, 20.0, &[("Close", ButtonKind::Neutral, true)])
            .map(|_| Outcome::Dismissed)
    })
}

fn about(ui: &mut Ui, versions: &[(String, String)]) -> Option<Outcome> {
    panel(ui, "about", "About Bite", 320.0, true, |ui, width| {
        let inner = body_start(ui, width, 20.0, 20.0);
        ui.with_face(theme::face::BODY, |ui| {
            ui.set_next_item_width(inner);
            ui.text_wrapped("A node-based image processing tool powered by ImageMagick.");
        });
        ui.dummy([inner, 6.0]);
        ui.with_face(theme::face::SMALL_MONO, |ui| {
            ui.with_colors(&[(StyleColor::Text, theme::ACCENT)], |ui| {
                ui.text(&format!("Version {}", crate::updates::current_version()))
            })
        });
        ui.dummy([inner, 12.0]);
        let origin = ui.cursor_screen_position();
        ui.draw_list().line(
            origin,
            [origin[0] + inner, origin[1]],
            theme::CTX_BORDER,
            1.0,
        );
        ui.dummy([inner, 12.0]);
        for (name, version) in versions {
            let row = ui.cursor_screen_position();
            let list = ui.draw_list();
            list.text_with_face(row, theme::TEXT_MUTED, theme::face::SMALL_MONO, name);
            list.text_with_face(
                [row[0] + inner / 2.0, row[1]],
                theme::TEXT,
                theme::face::SMALL_MONO,
                version,
            );
            ui.dummy([inner, 18.0]);
        }
        ui.dummy([inner, 8.0]);
        footer(ui, width, 20.0, &[("Close", ButtonKind::Neutral, true)])
            .map(|_| Outcome::Dismissed)
    })
}

/// One credited dependency: its name, its license and where to read about it.
pub type Credit = (&'static str, &'static str, &'static str);
/// A headed group of credits.
pub type CreditSection = (&'static str, Vec<Credit>);

/// The libraries and fonts the native build ships, with their licenses.
pub fn credit_entries() -> Vec<CreditSection> {
    vec![
        (
            "Open Source Libraries",
            vec![
                ("Dear ImGui", "MIT", "https://github.com/ocornut/imgui"),
                ("wgpu", "MIT/Apache-2.0", "https://github.com/gfx-rs/wgpu"),
                ("winit", "Apache-2.0", "https://github.com/rust-windowing/winit"),
                ("Rust", "MIT/Apache-2.0", "https://www.rust-lang.org"),
                (
                    "ImageMagick",
                    "ImageMagick",
                    "https://imagemagick.org",
                ),
                ("serde", "MIT/Apache-2.0", "https://serde.rs"),
                ("rfd", "MIT", "https://github.com/PolyMeilex/rfd"),
                ("ureq", "MIT/Apache-2.0", "https://github.com/algesten/ureq"),
            ],
        ),
        (
            "Fonts",
            vec![
                (
                    "JetBrains Mono",
                    "SIL OFL 1.1",
                    "https://www.jetbrains.com/lp/mono/",
                ),
                (
                    "Atkinson Hyperlegible Next",
                    "SIL OFL 1.1",
                    "https://www.brailleinstitute.org/freefont/",
                ),
            ],
        ),
    ]
}

fn credits(ui: &mut Ui) -> Option<Outcome> {
    let mut outcome = None;
    let result = panel(ui, "credits", "Credits", 420.0, true, |ui, width| {
        let inner = body_start(ui, width, 16.0, 14.0);
        for (heading, entries) in credit_entries() {
            controls::draw_tracked_text(
                ui,
                ui.cursor_screen_position(),
                // The stylesheet uses a muted teal for these headings.
                bite_imgui::Color::from_hex("#6fb8cc"),
                bite_imgui::Face::ui_weight(theme::FONT_SIZE_XS, bite_imgui::Weight::Bold),
                &heading.to_uppercase(),
                0.08 * 11.0,
            );
            ui.dummy([inner, 20.0]);
            for (name, license, url) in entries {
                let row = ui.cursor_screen_position();
                let clicked = ui.invisible_button(&format!("##credit-{name}"), [inner, 20.0]);
                let hovered = ui.item_hovered();
                if hovered {
                    ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
                }
                let list = ui.draw_list();
                list.text_with_face(
                    [row[0], row[1] + 4.0],
                    if hovered {
                        theme::TEXT_BRIGHT
                    } else {
                        theme::CTX_TEXT
                    },
                    theme::face::BODY,
                    name,
                );
                let size = list.measure(theme::face::SMALL_MONO, license);
                list.text_with_face(
                    [row[0] + inner - size[0], row[1] + 5.0],
                    theme::CTX_TEXT_MUTED,
                    theme::face::SMALL_MONO,
                    license,
                );
                list.line(
                    [row[0], row[1] + 20.0],
                    [row[0] + inner, row[1] + 20.0],
                    theme::CTX_BORDER,
                    1.0,
                );
                if clicked {
                    outcome = Some(Outcome::OpenUrl(url.to_string()));
                }
            }
            ui.dummy([inner, 18.0]);
        }
        footer(ui, width, 16.0, &[("Close", ButtonKind::Neutral, true)])
            .map(|_| Outcome::Dismissed)
    });
    outcome.or(result)
}

fn update(ui: &mut Ui, state: &UpdateState) -> Option<Outcome> {
    let closable = !matches!(state, UpdateState::Checking);
    let mut link = None;
    let result = panel(ui, "update", state.title(), 420.0, closable, |ui, width| {
        let inner = body_start(ui, width, 20.0, 18.0);
        match state {
            UpdateState::Checking => {
                ui.with_face(theme::face::BODY, |ui| ui.text("Contacting GitHub..."));
                ui.dummy([inner, 18.0]);
                return None;
            }
            UpdateState::Available { version, body, url } => {
                ui.with_face(theme::face::BODY, |ui| {
                    ui.text("A new version is available:")
                });
                ui.same_line_at(0.0, 6.0);
                ui.with_face(
                    bite_imgui::Face::mono_weight(theme::FONT_SIZE_BASE, bite_imgui::Weight::SemiBold),
                    |ui| {
                        ui.with_colors(&[(StyleColor::Text, theme::ACCENT)], |ui| {
                            ui.text(&format!("v{version}"))
                        })
                    },
                );
                ui.dummy([inner, 14.0]);
                if controls::button(ui, "Update", ButtonKind::Primary, 0.0, true) {
                    link = Some(url.clone());
                }
                ui.same_line_at(0.0, 8.0);
                if controls::button(ui, "Later", ButtonKind::Neutral, 0.0, true) {
                    return Some(Outcome::Dismissed);
                }
                ui.dummy([inner, 14.0]);
                release_notes(ui, inner, body);
            }
            UpdateState::Latest { version, body } => {
                ui.with_face(theme::face::BODY, |ui| {
                    ui.text(&format!("Bite v{version} is the latest version."))
                });
                ui.dummy([inner, 14.0]);
                release_notes(ui, inner, body);
            }
            UpdateState::Failed => {
                ui.with_face(theme::face::BODY, |ui| {
                    ui.with_colors(&[(StyleColor::Text, theme::TEXT_MUTED)], |ui| {
                        ui.set_next_item_width(inner);
                        ui.text_wrapped(
                            "Could not reach GitHub. Check your internet connection and try again.",
                        );
                    })
                });
            }
        }
        ui.dummy([inner, 18.0]);
        footer(ui, width, 20.0, &[("Close", ButtonKind::Neutral, true)])
            .map(|_| Outcome::Dismissed)
    });
    link.map(Outcome::OpenUrl).or(result)
}

/// The release notes block beneath an update state.
fn release_notes(ui: &mut Ui, width: f32, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let origin = ui.cursor_screen_position();
    ui.draw_list()
        .line(origin, [origin[0] + width, origin[1]], theme::CTX_BORDER, 1.0);
    ui.dummy([width, 14.0]);
    controls::draw_tracked_text(
        ui,
        ui.cursor_screen_position(),
        theme::TEXT_MUTED,
        bite_imgui::Face::ui_weight(theme::FONT_SIZE_XS, bite_imgui::Weight::SemiBold),
        "RELEASE NOTES",
        0.06 * 11.0,
    );
    ui.dummy([width, 18.0]);
    ui.with_face(theme::face::BODY, |ui| {
        ui.set_next_item_width(width);
        // The notes are plain text here; only the first twenty lines are shown.
        let excerpt: String = body.lines().take(20).collect::<Vec<_>>().join("\n");
        ui.text_wrapped(&excerpt);
    });
}

fn incompatible_version(ui: &mut Ui, file_version: Option<&str>) -> Option<Outcome> {
    let current = crate::updates::current_version();
    let message = match file_version {
        Some(version) => format!(
            "This workflow was created with Bite v{version}, which is not compatible with the current version (v{current})."
        ),
        None => format!(
            "This workflow was created with an older version of Bite that does not include version information. It is not compatible with the current version (v{current})."
        ),
    };
    let mut link = None;
    let result = panel(
        ui,
        "incompatible",
        "Incompatible Workflow Version",
        360.0,
        true,
        |ui, width| {
            let inner = body_start(ui, width, 20.0, 18.0);
            ui.with_face(theme::face::LABEL, |ui| {
                ui.set_next_item_width(inner);
                ui.text_wrapped(&message);
                ui.dummy([inner, 10.0]);
                ui.set_next_item_width(inner);
                ui.text_wrapped(
                    "Please download an older compatible version of Bite from GitHub to open this file.",
                );
            });
            ui.dummy([inner, 18.0]);
            let pressed = footer(
                ui,
                width,
                20.0,
                &[
                    ("Close", ButtonKind::Neutral, true),
                    ("View Older Releases", ButtonKind::Primary, true),
                ],
            );
            match pressed {
                Some(0) => Some(Outcome::Dismissed),
                Some(1) => {
                    link = Some(crate::menu::RELEASES_URL.to_string());
                    None
                }
                _ => None,
            }
        },
    );
    link.map(Outcome::OpenUrl).or(result)
}

fn run_workflow(ui: &mut Ui, nodes: &[RunCandidate]) -> Option<Outcome> {
    let valid = nodes.iter().filter(|node| node.valid()).count();
    panel(ui, "run", "Run Workflow", 340.0, false, |ui, width| {
        let inner = body_start(ui, width, 14.0, 14.0);
        let mut description = format!(
            "{valid} of {} output node{} ready to run.",
            nodes.len(),
            if nodes.len() == 1 { "" } else { "s" }
        );
        if valid < nodes.len() {
            description.push_str(" Invalid nodes will be skipped.");
        }
        ui.with_face(theme::face::BODY, |ui| {
            ui.set_next_item_width(inner);
            ui.text_wrapped(&description);
        });
        ui.dummy([inner, 10.0]);

        for node in nodes {
            let ok = node.valid();
            let reasons = node.reasons.join(" - ");
            let height = if ok { 26.0 } else { 42.0 };
            let origin = ui.cursor_screen_position();
            let list = ui.draw_list();
            let max = [origin[0] + inner, origin[1] + height];
            let (fill, border, accent) = if ok {
                (
                    theme::COLOR_SUCCESS.mix(8.0, theme::CTX_BG),
                    theme::COLOR_SUCCESS.mix(25.0, bite_imgui::Color::TRANSPARENT),
                    theme::COLOR_SUCCESS,
                )
            } else {
                (
                    theme::COLOR_WARNING.mix(8.0, theme::CTX_BG),
                    theme::COLOR_WARNING.mix(25.0, bite_imgui::Color::TRANSPARENT),
                    theme::COLOR_WARNING,
                )
            };
            list.rect(origin, max, fill, 4.0, Rounding::All);
            list.rect_outline(origin, max, border, 4.0, Rounding::All, 1.0);
            // A ready node carries a round marker and an unready one a square.
            if ok {
                list.circle([origin[0] + 12.0, origin[1] + 11.0], 4.0, accent);
            } else {
                list.rect(
                    [origin[0] + 8.0, origin[1] + 7.0],
                    [origin[0] + 16.0, origin[1] + 15.0],
                    accent,
                    2.0,
                    Rounding::All,
                );
            }
            let name_face =
                bite_imgui::Face::ui_weight(theme::FONT_SIZE_SM, bite_imgui::Weight::SemiBold);
            controls::draw_ellipsized(
                ui,
                [origin[0] + 24.0, origin[1] + 5.0],
                theme::TEXT_BRIGHT,
                name_face,
                &node.label,
                inner - 32.0,
            );
            if !ok {
                controls::draw_ellipsized(
                    ui,
                    [origin[0] + 24.0, origin[1] + 23.0],
                    theme::COLOR_WARNING_TEXT,
                    theme::face::TINY_MONO,
                    &reasons,
                    inner - 32.0,
                );
            }
            ui.dummy([inner, height + 6.0]);
        }

        ui.dummy([inner, 8.0]);
        let run_label = format!("Run {valid} node{}", if valid == 1 { "" } else { "s" });
        let pressed = footer(
            ui,
            width,
            14.0,
            &[
                ("Cancel", ButtonKind::Neutral, true),
                (&run_label, ButtonKind::Primary, valid > 0),
            ],
        );
        match pressed {
            Some(0) => Some(Outcome::Dismissed),
            Some(1) => Some(Outcome::RunSelected),
            _ => None,
        }
    })
}

fn batch_progress(ui: &mut Ui, progress: &Progress) -> Option<Outcome> {
    let failed = progress.error.is_some();
    let title = if failed {
        "Workflow Error"
    } else {
        "Running Workflow..."
    };
    panel(ui, "batch", title, 340.0, false, |ui, width| {
        let inner = body_start(ui, width, 24.0, 24.0);
        if let Some(error) = &progress.error {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::COLOR_ERROR_TEXT)], |ui| {
                    ui.set_next_item_width(inner);
                    ui.text_wrapped(error);
                })
            });
            ui.dummy([inner, 20.0]);
            return footer(ui, width, 24.0, &[("Close", ButtonKind::Neutral, true)])
                .map(|_| Outcome::Dismissed);
        }
        if progress.total == 0 {
            let text = format!("Starting... {}", format_elapsed(progress.elapsed_seconds));
            let size = controls::measure(ui, theme::face::VALUE, &text);
            ui.draw_list().text_with_face(
                [
                    ui.cursor_screen_position()[0] + (inner - size[0]) / 2.0,
                    ui.cursor_screen_position()[1],
                ],
                theme::TEXT,
                theme::face::VALUE,
                &text,
            );
            ui.dummy([inner, size[1] + 20.0]);
        } else {
            counter_row(ui, inner, progress.completed, progress.total);
            ui.dummy([inner, 12.0]);
            progress_bar(ui, inner, progress.fraction(), theme::COLOR_SUCCESS);
            ui.dummy([inner, 8.0]);
            let label = format!(
                "{}%  -  {}",
                progress.percent(),
                format_elapsed(progress.elapsed_seconds)
            );
            let size = controls::measure(ui, theme::face::SMALL_MONO, &label);
            ui.draw_list().text_with_face(
                [
                    ui.cursor_screen_position()[0] + inner - size[0],
                    ui.cursor_screen_position()[1],
                ],
                theme::TEXT,
                theme::face::SMALL_MONO,
                &label,
            );
            ui.dummy([inner, size[1] + 20.0]);
        }
        footer(ui, width, 24.0, &[("Cancel", ButtonKind::Danger, true)])
            .map(|_| Outcome::CancelBatch)
    })
}

/// The large completed-over-total counter the progress dialogs show.
fn counter_row(ui: &mut Ui, width: f32, completed: usize, total: usize) {
    let origin = ui.cursor_screen_position();
    let list = ui.draw_list();
    let completed_text = completed.to_string();
    let total_text = total.to_string();
    let completed_size = list.measure(theme::face::STAT_VALUE, &completed_text);
    let mut x = origin[0];
    list.text_with_face(
        [x, origin[1]],
        theme::COLOR_SUCCESS_MUTED,
        theme::face::STAT_VALUE,
        &completed_text,
    );
    x += completed_size[0] + 6.0;
    let slash_face = bite_imgui::Face::mono(theme::FONT_SIZE_XL);
    list.text_with_face(
        [x, origin[1] + 8.0],
        theme::TEXT,
        slash_face,
        "/",
    );
    x += list.measure(slash_face, "/")[0] + 6.0;
    let total_face = bite_imgui::Face::mono(theme::FONT_SIZE_2XL);
    list.text_with_face([x, origin[1] + 6.0], theme::TEXT_BRIGHT, total_face, &total_text);
    x += list.measure(total_face, &total_text)[0] + 8.0;
    list.text_with_face(
        [x, origin[1] + 12.0],
        theme::TEXT,
        theme::face::BODY,
        "images",
    );
    ui.dummy([width, completed_size[1]]);
}

/// The six-pixel bar shared by the batch and import dialogs.
fn progress_bar(ui: &mut Ui, width: f32, fraction: f32, color: bite_imgui::Color) {
    let origin = ui.cursor_screen_position();
    let height = 6.0;
    let list = ui.draw_list();
    list.rect(
        origin,
        [origin[0] + width, origin[1] + height],
        theme::BORDER,
        3.0,
        Rounding::All,
    );
    let filled = width * fraction.clamp(0.0, 1.0);
    if filled > 0.5 {
        list.rect(
            origin,
            [origin[0] + filled, origin[1] + height],
            color,
            3.0,
            Rounding::All,
        );
    }
    ui.dummy([width, height]);
}

fn batch_summary(ui: &mut Ui, summary: &BatchSummary) -> Option<Outcome> {
    let mut open_folder = None;
    let result = panel(ui, "summary", "Batch Complete", 320.0, true, |ui, width| {
        let inner = body_start(ui, width, 20.0, 20.0);
        let mut stats: Vec<(&str, String, bite_imgui::Color)> = vec![(
            "PROCESSED",
            summary.processed.to_string(),
            theme::COLOR_SUCCESS_MUTED,
        )];
        if summary.skipped > 0 {
            stats.push(("SKIPPED", summary.skipped.to_string(), theme::TEXT));
        }
        if summary.failed > 0 {
            stats.push((
                "FAILED",
                summary.failed.to_string(),
                theme::COLOR_ERROR_TEXT,
            ));
        }
        let column = inner / stats.len() as f32;
        let origin = ui.cursor_screen_position();
        for (index, (label, value, colour)) in stats.iter().enumerate() {
            let list = ui.draw_list();
            let centre = origin[0] + column * index as f32 + column / 2.0;
            let value_size = list.measure(theme::face::STAT_VALUE, value);
            list.text_with_face(
                [centre - value_size[0] / 2.0, origin[1]],
                *colour,
                theme::face::STAT_VALUE,
                value,
            );
            let label_face =
                bite_imgui::Face::ui_weight(theme::FONT_SIZE_XS, bite_imgui::Weight::Regular);
            let label_size = controls::measure_tracked(ui, label_face, label, 0.06 * 11.0);
            controls::draw_tracked_text(
                ui,
                [centre - label_size[0] / 2.0, origin[1] + value_size[1] + 2.0],
                theme::TEXT,
                label_face,
                label,
                0.06 * 11.0,
            );
            if index + 1 < stats.len() {
                let divider = origin[0] + column * (index + 1) as f32;
                list.line(
                    [divider, origin[1] + 4.0],
                    [divider, origin[1] + 44.0],
                    theme::CTX_BORDER,
                    1.0,
                );
            }
        }
        ui.dummy([inner, 56.0]);

        if let Some(elapsed) = summary.elapsed_ms {
            let row = ui.cursor_screen_position();
            let list = ui.draw_list();
            list.rect(
                row,
                [row[0] + inner, row[1] + 2.0],
                theme::CTX_BORDER,
                0.0,
                Rounding::None,
            );
            list.text_with_face(
                [row[0], row[1] + 10.0],
                theme::TEXT,
                theme::face::BODY,
                "Total time",
            );
            let text = format_elapsed(elapsed as f32 / 1000.0);
            let size = list.measure(theme::face::VALUE, &text);
            list.text_with_face(
                [row[0] + inner - size[0], row[1] + 10.0],
                theme::TEXT_BRIGHT,
                theme::face::VALUE,
                &text,
            );
            ui.dummy([inner, 32.0]);
        }

        if !summary.errors.is_empty() {
            let row = ui.cursor_screen_position();
            let height = 120.0_f32.min(summary.errors.len() as f32 * 16.0 + 12.0);
            let list = ui.draw_list();
            let max = [row[0] + inner, row[1] + height];
            list.rect(row, max, theme::COLOR_ERROR_BG, 4.0, Rounding::All);
            list.rect_outline(row, max, theme::COLOR_ERROR_BORDER, 4.0, Rounding::All, 1.0);
            for (index, error) in summary.errors.iter().enumerate() {
                if (index as f32 + 1.0) * 16.0 > height - 12.0 {
                    break;
                }
                controls::draw_ellipsized(
                    ui,
                    [row[0] + 8.0, row[1] + 6.0 + index as f32 * 16.0],
                    theme::COLOR_ERROR_TEXT,
                    theme::face::TINY_MONO,
                    error,
                    inner - 16.0,
                );
            }
            ui.dummy([inner, height + 10.0]);
        } else if summary.failed > 0 {
            ui.with_face(theme::face::SMALL_MONO, |ui| {
                ui.with_colors(&[(StyleColor::Text, theme::COLOR_ERROR_TEXT)], |ui| {
                    ui.set_next_item_width(inner);
                    ui.text_wrapped(
                        "Check the log window for per-image error details.",
                    );
                })
            });
            ui.dummy([inner, 12.0]);
        }

        ui.dummy([inner, 8.0]);
        let mut buttons: Vec<(&str, ButtonKind, bool)> = Vec::new();
        if summary.output_dir.is_some() {
            buttons.push(("Open Output Folder", ButtonKind::Neutral, true));
        }
        buttons.push(("Close", ButtonKind::Neutral, true));
        let pressed = footer(ui, width, 20.0, &buttons);
        match (pressed, summary.output_dir.as_ref()) {
            (Some(0), Some(path)) => {
                open_folder = Some(path.clone());
                None
            }
            (Some(_), _) => Some(Outcome::Dismissed),
            _ => None,
        }
    });
    open_folder.map(Outcome::OpenOutputFolder).or(result)
}

fn import_progress(ui: &mut Ui, progress: &Progress) -> Option<Outcome> {
    let done = progress.total > 0 && progress.completed >= progress.total;
    let title = if done {
        "Import Complete"
    } else {
        "Importing Images"
    };
    panel(ui, "import", title, 340.0, false, |ui, width| {
        let inner = body_start(ui, width, 24.0, 24.0);
        if done {
            // A round tick badge marks the finished state.
            let origin = ui.cursor_screen_position();
            let centre = [origin[0] + inner / 2.0, origin[1] + 22.0];
            let list = ui.draw_list();
            list.circle(
                centre,
                22.0,
                theme::COLOR_SUCCESS.mix(20.0, theme::CTX_BG),
            );
            list.circle_outline(centre, 22.0, theme::COLOR_SUCCESS, 2.0);
            list.line(
                [centre[0] - 8.0, centre[1] + 1.0],
                [centre[0] - 2.0, centre[1] + 7.0],
                theme::COLOR_SUCCESS,
                3.0,
            );
            list.line(
                [centre[0] - 2.0, centre[1] + 7.0],
                [centre[0] + 9.0, centre[1] - 6.0],
                theme::COLOR_SUCCESS,
                3.0,
            );
            ui.dummy([inner, 56.0]);
            let text = format!(
                "Imported {} image(s) in {:.1} sec",
                progress.total, progress.elapsed_seconds
            );
            let size = controls::measure(ui, theme::face::VALUE, &text);
            ui.draw_list().text_with_face(
                [
                    ui.cursor_screen_position()[0] + (inner - size[0]) / 2.0,
                    ui.cursor_screen_position()[1],
                ],
                theme::TEXT_BRIGHT,
                theme::face::VALUE,
                &text,
            );
            ui.dummy([inner, size[1] + 20.0]);
            return footer(ui, width, 24.0, &[("OK", ButtonKind::Primary, true)])
                .map(|_| Outcome::Dismissed);
        }
        counter_row(ui, inner, progress.completed, progress.total);
        ui.dummy([inner, 12.0]);
        progress_bar(ui, inner, progress.fraction(), theme::COLOR_SUCCESS);
        ui.dummy([inner, 8.0]);
        let label = format!("{}%  -  {:.1}s", progress.percent(), progress.elapsed_seconds);
        let size = controls::measure(ui, theme::face::SMALL_MONO, &label);
        ui.draw_list().text_with_face(
            [
                ui.cursor_screen_position()[0] + inner - size[0],
                ui.cursor_screen_position()[1],
            ],
            theme::TEXT,
            theme::face::SMALL_MONO,
            &label,
        );
        ui.dummy([inner, size[1] + 20.0]);
        footer(ui, width, 24.0, &[("Cancel Import", ButtonKind::Danger, true)])
            .map(|_| Outcome::CancelImport)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unsaved_prompt_names_the_action_it_guards() {
        assert_eq!(
            confirm_message(PendingAction::New),
            "You have unsaved changes. Start a new workflow anyway?"
        );
        assert_eq!(
            confirm_message(PendingAction::Open),
            "You have unsaved changes. Open a different workflow anyway?"
        );
        assert_eq!(
            confirm_message(PendingAction::Exit),
            "You have unsaved changes. Exit anyway?"
        );
    }

    #[test]
    fn elapsed_times_switch_to_minutes_after_a_minute() {
        assert_eq!(format_elapsed(5.0), "5s");
        assert_eq!(format_elapsed(59.4), "59s");
        assert_eq!(format_elapsed(75.0), "1m 15s");
        assert_eq!(format_elapsed(120.0), "2m 0s");
    }

    #[test]
    fn progress_reports_a_fraction_and_a_whole_percentage() {
        let progress = Progress {
            completed: 3,
            total: 4,
            elapsed_seconds: 1.0,
            error: None,
        };
        assert_eq!(progress.fraction(), 0.75);
        assert_eq!(progress.percent(), 75);
    }

    #[test]
    fn an_empty_batch_reports_no_progress_rather_than_dividing_by_zero() {
        let progress = Progress::default();
        assert_eq!(progress.fraction(), 0.0);
        assert_eq!(progress.percent(), 0);
    }

    #[test]
    fn a_candidate_without_reasons_is_ready_to_run() {
        let ready = RunCandidate {
            id: "a".into(),
            label: "Image Output".into(),
            reasons: Vec::new(),
        };
        let blocked = RunCandidate {
            id: "b".into(),
            label: "Text Output".into(),
            reasons: vec!["Output file path is empty".into()],
        };
        assert!(ready.valid());
        assert!(!blocked.valid());
    }

    #[test]
    fn the_update_dialog_titles_every_state() {
        assert_eq!(UpdateState::Checking.title(), "Checking for Updates");
        assert_eq!(
            UpdateState::Available {
                version: "1.0.0".into(),
                body: String::new(),
                url: String::new()
            }
            .title(),
            "Update Available"
        );
        assert_eq!(
            UpdateState::Latest {
                version: "1.0.0".into(),
                body: String::new()
            }
            .title(),
            "You're up to date"
        );
        assert_eq!(UpdateState::Failed.title(), "Update Check Failed");
    }

    #[test]
    fn the_credits_list_names_the_libraries_and_the_fonts() {
        let sections = credit_entries();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].0, "Open Source Libraries");
        assert_eq!(sections[1].0, "Fonts");
        assert!(sections[1].1.iter().any(|(name, _, _)| *name == "JetBrains Mono"));
    }
}
