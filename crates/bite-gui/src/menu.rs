//! The application menu.
//!
//! Every item is always enabled and the only dynamic label is the performance timer toggle,
//! which is the shape the menu has always had; `docs/spec.md` is where that contract is
//! written down.
//!
//! On macOS the system menu bar beside this one (`crate::menubar`) carries only what AppKit
//! must own - the application menu and Quit. This is the editor's menu on both platforms, so
//! the two must not both claim an accelerator; see that module.
use crate::{controls, theme};
use bite_imgui::{Color, StyleColor, StyleVar, Ui};

/// Every command the menu can raise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    New,
    RunWorkflow,
    OpenWorkflow,
    SaveWorkflow,
    SaveWorkflowAs,
    ExportPowerShell,
    ExportBash,
    ExportCmd,
    Exit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    ActualSize,
    ZoomIn,
    ZoomOut,
    ToggleFullScreen,
    TogglePerformanceTimers,
    ViewLog,
    OpenTempFolder,
    ClearCache,
    ShowAllUiElements,
    About,
    Documentation,
    ReportBug,
    Credits,
    CheckForUpdates,
}

/// The height the in-window menu bar is laid out for, from `MenuBar.svelte`. Dear ImGui
/// sizes the real bar from the font and its frame padding, so [`draw`] reports what it
/// actually drew and the shell pads below that; this is only the fallback for tests.
pub const MENU_BAR_HEIGHT: f32 = 30.0;

/// What one frame of the menu bar produced: the command the user picked, if any, and the
/// height the bar took, so the panels below it sit one shell gap away rather than a gap
/// plus whatever the constant above mis-estimates.
pub struct Bar {
    pub command: Option<Command>,
    pub height: f32,
}

/// Draws the menu bar and returns the command the user picked, if any.
pub fn draw(ui: &mut Ui, title: &str, timers_enabled: bool, show_developer_items: bool) -> Bar {
    let mut command = None;
    let mut height = MENU_BAR_HEIGHT;
    ui.with_style(
        &[
            StyleVar::WindowPadding([8.0, 0.0]),
            StyleVar::FramePadding([8.0, 6.0]),
            StyleVar::ItemSpacing([2.0, 0.0]),
            StyleVar::PopupRounding(theme::CTX_RADIUS),
            StyleVar::PopupBorderSize(1.0),
            StyleVar::WindowRounding(0.0),
        ],
        |ui| {
            ui.with_colors(
                &[
                    (StyleColor::WindowBg, theme::PANEL_HEADER_BG),
                    (StyleColor::PopupBg, theme::CTX_BG),
                    (StyleColor::Border, theme::CTX_BORDER),
                    (StyleColor::Text, theme::CTX_TEXT),
                    (StyleColor::Header, theme::LIBRARY_ITEM_HOVER_BG),
                    (StyleColor::HeaderHovered, theme::CTX_ITEM_HOVER_BG),
                    (StyleColor::HeaderActive, theme::CTX_ITEM_HOVER_BG),
                ],
                |ui| {
                    ui.main_menu_bar(|ui| {
                        ui.with_face(theme::face::BODY, |ui| {
                            // A dropdown's own padding and row spacing, from
                            // `.dropdown` and `.dropdown li button`. Without the vertical
                            // spacing the rows sit line against line, because a menu row's
                            // height is its text and nothing else; Dear ImGui grows the
                            // highlight into half the spacing on each side, which is the
                            // five pixels the stylesheet pads with.
                            ui.with_style(
                                &[
                                    StyleVar::WindowPadding(theme::MENU_DROPDOWN_PADDING),
                                    StyleVar::ItemSpacing([
                                        theme::MENU_ITEM_GAP,
                                        theme::MENU_ITEM_PADDING_Y * 2.0,
                                    ]),
                                ],
                                |ui| {
                                    command = menus(ui, timers_enabled, show_developer_items);
                                },
                            );
                        });
                        height = ui.window_size()[1];
                        draw_title(ui, title, height);
                    });
                },
            )
        },
    );
    Bar { command, height }
}

/// The current document title, shown at the right of the bar in muted monospace.
fn draw_title(ui: &mut Ui, title: &str, bar_height: f32) {
    if title.is_empty() {
        return;
    }
    let available = ui.content_region_available()[0];
    let size = controls::measure(ui, theme::face::SMALL_MONO, title);
    if available < size[0] + 16.0 {
        return;
    }
    let origin = ui.cursor_screen_position();
    ui.draw_list().text_with_face(
        [
            origin[0] + available - size[0] - 8.0,
            origin[1] + (bar_height - size[1]) / 2.0 - 2.0,
        ],
        theme::TEXT.with_alpha(0.6),
        theme::face::SMALL_MONO,
        title,
    );
}

/// Spells an accelerator whose modifier is the platform's primary one: Cmd on macOS, Ctrl
/// everywhere else. `primary!("N")` is `"Cmd+N"` or `"Ctrl+N"`, decided at compile time, so the
/// menu rows below stay one list rather than two.
///
/// This is only how the chord is *written*. What the editor acts on is [`shortcut`], which takes
/// `Modifiers::primary` - and `Ui::primary_modifier` is already Ctrl-or-Cmd, so the Mac chords
/// worked before these labels admitted it.
#[cfg(target_os = "macos")]
macro_rules! primary {
    ($keys:literal) => {
        concat!("Cmd+", $keys)
    };
}
#[cfg(not(target_os = "macos"))]
macro_rules! primary {
    ($keys:literal) => {
        concat!("Ctrl+", $keys)
    };
}

/// Quitting is the one row whose chord is not the same key on both platforms: Alt+F4 is a
/// Windows window-manager binding, and on macOS the menu bar owns Cmd+Q (see `menubar`).
#[cfg(target_os = "macos")]
const EXIT_ACCELERATOR: &str = "Cmd+Q";
#[cfg(not(target_os = "macos"))]
const EXIT_ACCELERATOR: &str = "Alt+F4";

/// What the Exit row is called. The Mac convention names the application; Windows does not.
#[cfg(target_os = "macos")]
const EXIT_LABEL: &str = "Quit Bite";
#[cfg(not(target_os = "macos"))]
const EXIT_LABEL: &str = "Exit";

fn menus(ui: &mut Ui, timers_enabled: bool, show_developer_items: bool) -> Option<Command> {
    let mut command = None;
    // A free function keeps the accumulator out of a closure the menu bodies also borrow.
    fn item(
        ui: &mut Ui,
        command: &mut Option<Command>,
        label: &str,
        shortcut: &str,
        value: Command,
    ) {
        if ui.menu_item(label, shortcut, false, true) {
            *command = Some(value);
        }
    }

    ui.menu("File", |ui| {
        item(ui, &mut command, "New", primary!("N"), Command::New);
        ui.separator();
        item(
            ui,
            &mut command,
            "Run Workflow",
            primary!("R"),
            Command::RunWorkflow,
        );
        ui.separator();
        item(
            ui,
            &mut command,
            "Open Workflow",
            primary!("O"),
            Command::OpenWorkflow,
        );
        item(
            ui,
            &mut command,
            "Save Workflow",
            primary!("S"),
            Command::SaveWorkflow,
        );
        item(
            ui,
            &mut command,
            "Save Workflow As",
            primary!("Shift+S"),
            Command::SaveWorkflowAs,
        );
        ui.separator();
        let mut nested = None;
        ui.menu("Export CLI Script", |ui| {
            if ui.menu_item("PowerShell", "", false, true) {
                nested = Some(Command::ExportPowerShell);
            }
            if ui.menu_item("Bash", "", false, true) {
                nested = Some(Command::ExportBash);
            }
            if ui.menu_item("Windows Command Prompt", "", false, true) {
                nested = Some(Command::ExportCmd);
            }
        });
        if let Some(nested) = nested {
            command = Some(nested);
        }
        ui.separator();
        item(
            ui,
            &mut command,
            EXIT_LABEL,
            EXIT_ACCELERATOR,
            Command::Exit,
        );
    });

    ui.menu("Edit", |ui| {
        item(ui, &mut command, "Undo", primary!("Z"), Command::Undo);
        item(ui, &mut command, "Redo", primary!("Y"), Command::Redo);
        ui.separator();
        item(ui, &mut command, "Cut", primary!("X"), Command::Cut);
        item(ui, &mut command, "Copy", primary!("C"), Command::Copy);
        item(ui, &mut command, "Paste", primary!("V"), Command::Paste);
        ui.separator();
        item(
            ui,
            &mut command,
            "Duplicate",
            primary!("D"),
            Command::Duplicate,
        );
        item(ui, &mut command, "Delete", "Delete", Command::Delete);
        ui.separator();
        item(
            ui,
            &mut command,
            "Select All",
            primary!("A"),
            Command::SelectAll,
        );
    });

    ui.menu("View", |ui| {
        item(
            ui,
            &mut command,
            "Actual Size",
            primary!("0"),
            Command::ActualSize,
        );
        item(
            ui,
            &mut command,
            "Zoom In",
            primary!("Plus"),
            Command::ZoomIn,
        );
        item(
            ui,
            &mut command,
            "Zoom Out",
            primary!("-"),
            Command::ZoomOut,
        );
        ui.separator();
        item(
            ui,
            &mut command,
            "Toggle Full Screen",
            "F11",
            Command::ToggleFullScreen,
        );
    });

    ui.menu("Debug", |ui| {
        let label = if timers_enabled {
            "Disable Performance Timers"
        } else {
            "Enable Performance Timers"
        };
        item(
            ui,
            &mut command,
            label,
            "",
            Command::TogglePerformanceTimers,
        );
        ui.separator();
        item(ui, &mut command, "View Log", "", Command::ViewLog);
        ui.separator();
        item(
            ui,
            &mut command,
            "Open Temp Folder",
            "",
            Command::OpenTempFolder,
        );
        item(ui, &mut command, "Clear Cache", "", Command::ClearCache);
        if show_developer_items {
            ui.separator();
            item(
                ui,
                &mut command,
                "Show All UI Elements",
                "",
                Command::ShowAllUiElements,
            );
        }
    });

    ui.menu("Help", |ui| {
        item(ui, &mut command, "About", "", Command::About);
        item(
            ui,
            &mut command,
            "Documentation",
            "",
            Command::Documentation,
        );
        item(ui, &mut command, "Report a bug", "", Command::ReportBug);
        item(ui, &mut command, "Credits", "", Command::Credits);
        ui.separator();
        item(
            ui,
            &mut command,
            "Check for Updates",
            "",
            Command::CheckForUpdates,
        );
    });

    command
}

/// The web addresses the Help menu opens.
pub const DOCUMENTATION_URL: &str = "https://github.com/psmyles/bite/tree/main/docs";
pub const REPORT_BUG_URL: &str = "https://github.com/psmyles/bite/issues/new";
pub const RELEASES_URL: &str = "https://github.com/psmyles/bite/releases";

/// The keyboard shortcut a key press maps to, given the modifier state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Modifiers {
    pub primary: bool,
    pub shift: bool,
}

/// Resolves a key press to a menu command, matching the accelerators above.
pub fn shortcut(key: &str, modifiers: Modifiers) -> Option<Command> {
    let Modifiers { primary, shift } = modifiers;
    Some(match (key, primary, shift) {
        ("n", true, false) => Command::New,
        ("o", true, false) => Command::OpenWorkflow,
        ("s", true, false) => Command::SaveWorkflow,
        ("s", true, true) => Command::SaveWorkflowAs,
        ("r", true, false) => Command::RunWorkflow,
        ("z", true, false) => Command::Undo,
        ("z", true, true) => Command::Redo,
        ("y", true, false) => Command::Redo,
        ("x", true, false) => Command::Cut,
        ("c", true, false) => Command::Copy,
        ("v", true, false) => Command::Paste,
        ("d", true, false) => Command::Duplicate,
        ("a", true, false) => Command::SelectAll,
        ("0", true, false) => Command::ActualSize,
        ("+" | "=", true, _) => Command::ZoomIn,
        ("-", true, false) => Command::ZoomOut,
        ("F11", false, false) => Command::ToggleFullScreen,
        _ => return None,
    })
}

/// A colored label used by the log window's level badges.
pub fn level_color(level: &str) -> Color {
    match level {
        "warn" | "warning" => theme::COLOR_WARNING_TEXT,
        "error" => theme::COLOR_ERROR_TEXT,
        _ => theme::TEXT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str, primary: bool, shift: bool) -> Option<Command> {
        shortcut(key, Modifiers { primary, shift })
    }

    #[test]
    fn save_and_save_as_are_separated_by_the_shift_modifier() {
        assert_eq!(press("s", true, false), Some(Command::SaveWorkflow));
        assert_eq!(press("s", true, true), Some(Command::SaveWorkflowAs));
    }

    #[test]
    fn both_redo_accelerators_are_accepted() {
        assert_eq!(press("y", true, false), Some(Command::Redo));
        assert_eq!(press("z", true, true), Some(Command::Redo));
        assert_eq!(press("z", true, false), Some(Command::Undo));
    }

    #[test]
    fn keys_without_the_modifier_are_not_menu_commands() {
        assert_eq!(press("s", false, false), None);
        assert_eq!(press("n", false, false), None);
    }

    #[test]
    fn the_zoom_accelerators_cover_both_plus_spellings() {
        assert_eq!(press("+", true, false), Some(Command::ZoomIn));
        assert_eq!(press("=", true, false), Some(Command::ZoomIn));
        assert_eq!(press("-", true, false), Some(Command::ZoomOut));
        assert_eq!(press("0", true, false), Some(Command::ActualSize));
    }

    #[test]
    fn fullscreen_needs_no_modifier() {
        assert_eq!(press("F11", false, false), Some(Command::ToggleFullScreen));
    }
}
