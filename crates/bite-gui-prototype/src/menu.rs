//! The application menu, matching `electron/main.ts` label for label.
//!
//! Every item in the Electron menu is always enabled, and the only dynamic label is the
//! performance timer toggle, so the native menu behaves the same way.
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

/// The height of the in-window menu bar, from `MenuBar.svelte`.
pub const MENU_BAR_HEIGHT: f32 = 30.0;

/// Draws the menu bar and returns the command the user picked, if any.
pub fn draw(
    ui: &mut Ui,
    title: &str,
    timers_enabled: bool,
    show_developer_items: bool,
) -> Option<Command> {
    let mut command = None;
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
                        draw_brand(ui);
                        ui.with_face(theme::face::BODY, |ui| {
                            command = menus(ui, timers_enabled, show_developer_items);
                        });
                        draw_title(ui, title);
                    });
                },
            )
        },
    );
    command
}

/// The application name at the left of the bar, in bold with wide letter spacing.
fn draw_brand(ui: &mut Ui) {
    let origin = ui.cursor_screen_position();
    let face = bite_imgui::Face::ui_weight(theme::FONT_SIZE_BASE, bite_imgui::Weight::Bold);
    let tracking = 0.05 * f32::from(theme::FONT_SIZE_BASE);
    let size = controls::measure_tracked(ui, face, "Bite", tracking);
    controls::draw_tracked_text(
        ui,
        [
            origin[0] + 4.0,
            origin[1] + (MENU_BAR_HEIGHT - size[1]) / 2.0,
        ],
        theme::TEXT_BRIGHT,
        face,
        "Bite",
        tracking,
    );
    ui.dummy([size[0] + 14.0, MENU_BAR_HEIGHT]);
    ui.same_line();
}

/// The current document title, shown at the right of the bar in muted monospace.
fn draw_title(ui: &mut Ui, title: &str) {
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
            origin[1] + (MENU_BAR_HEIGHT - size[1]) / 2.0 - 2.0,
        ],
        theme::TEXT.with_alpha(0.6),
        theme::face::SMALL_MONO,
        title,
    );
}

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
        item(ui, &mut command, "New", "Ctrl+N", Command::New);
        ui.separator();
        item(ui, &mut command, "Run Workflow", "Ctrl+R", Command::RunWorkflow);
        ui.separator();
        item(ui, &mut command, "Open Workflow", "Ctrl+O", Command::OpenWorkflow);
        item(ui, &mut command, "Save Workflow", "Ctrl+S", Command::SaveWorkflow);
        item(ui, &mut command, "Save Workflow As", "Ctrl+Shift+S", Command::SaveWorkflowAs);
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
        item(ui, &mut command, "Exit", "Alt+F4", Command::Exit);
    });

    ui.menu("Edit", |ui| {
        item(ui, &mut command, "Undo", "Ctrl+Z", Command::Undo);
        item(ui, &mut command, "Redo", "Ctrl+Y", Command::Redo);
        ui.separator();
        item(ui, &mut command, "Cut", "Ctrl+X", Command::Cut);
        item(ui, &mut command, "Copy", "Ctrl+C", Command::Copy);
        item(ui, &mut command, "Paste", "Ctrl+V", Command::Paste);
        ui.separator();
        item(ui, &mut command, "Duplicate", "Ctrl+D", Command::Duplicate);
        item(ui, &mut command, "Delete", "Delete", Command::Delete);
        ui.separator();
        item(ui, &mut command, "Select All", "Ctrl+A", Command::SelectAll);
    });

    ui.menu("View", |ui| {
        item(ui, &mut command, "Actual Size", "Ctrl+0", Command::ActualSize);
        item(ui, &mut command, "Zoom In", "Ctrl+Plus", Command::ZoomIn);
        item(ui, &mut command, "Zoom Out", "Ctrl+-", Command::ZoomOut);
        ui.separator();
        item(ui, &mut command, "Toggle Full Screen", "F11", Command::ToggleFullScreen);
    });

    ui.menu("Debug", |ui| {
        let label = if timers_enabled {
            "Disable Performance Timers"
        } else {
            "Enable Performance Timers"
        };
        item(ui, &mut command, label, "", Command::TogglePerformanceTimers);
        ui.separator();
        item(ui, &mut command, "View Log", "", Command::ViewLog);
        ui.separator();
        item(ui, &mut command, "Open Temp Folder", "", Command::OpenTempFolder);
        item(ui, &mut command, "Clear Cache", "", Command::ClearCache);
        if show_developer_items {
            ui.separator();
            item(ui, &mut command, "Show All UI Elements", "", Command::ShowAllUiElements);
        }
    });

    ui.menu("Help", |ui| {
        item(ui, &mut command, "About", "", Command::About);
        item(ui, &mut command, "Documentation", "", Command::Documentation);
        item(ui, &mut command, "Report a bug", "", Command::ReportBug);
        item(ui, &mut command, "Credits", "", Command::Credits);
        ui.separator();
        item(ui, &mut command, "Check for Updates", "", Command::CheckForUpdates);
    });

    command
}

/// The web addresses the Help menu opens, from the Electron menu definition.
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
