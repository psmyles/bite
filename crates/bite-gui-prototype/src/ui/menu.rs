use bite_imgui::Ui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportShell {
    PowerShell,
    Bash,
    Cmd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuCommand {
    New,
    RunWorkflow,
    OpenWorkflow,
    SaveWorkflow,
    SaveWorkflowAs,
    ExportCli(ExportShell),
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
    ToggleWorkflowSettings,
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

pub struct MenuState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub performance_timers_enabled: bool,
    pub can_check_for_updates: bool,
    pub show_dev_items: bool,
}

pub fn draw(ui: &mut Ui<'_>, state: MenuState) -> Option<MenuCommand> {
    let mut command = None;
    ui.main_menu_bar(|ui| {
        ui.text("Bite");
        ui.menu("File", |ui| {
            item(ui, &mut command, "New", "Ctrl+N", true, MenuCommand::New);
            ui.separator();
            item(
                ui,
                &mut command,
                "Run Workflow",
                "Ctrl+R",
                true,
                MenuCommand::RunWorkflow,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                "Open Workflow",
                "Ctrl+O",
                true,
                MenuCommand::OpenWorkflow,
            );
            item(
                ui,
                &mut command,
                "Save Workflow",
                "Ctrl+S",
                true,
                MenuCommand::SaveWorkflow,
            );
            item(
                ui,
                &mut command,
                "Save Workflow As",
                "Ctrl+Shift+S",
                true,
                MenuCommand::SaveWorkflowAs,
            );
            ui.separator();
            ui.menu("Export CLI Script", |ui| {
                item(
                    ui,
                    &mut command,
                    "PowerShell",
                    "",
                    true,
                    MenuCommand::ExportCli(ExportShell::PowerShell),
                );
                item(
                    ui,
                    &mut command,
                    "Bash",
                    "",
                    true,
                    MenuCommand::ExportCli(ExportShell::Bash),
                );
                item(
                    ui,
                    &mut command,
                    "Windows Command Prompt",
                    "",
                    true,
                    MenuCommand::ExportCli(ExportShell::Cmd),
                );
            });
            ui.separator();
            item(ui, &mut command, "Exit", "Alt+F4", true, MenuCommand::Exit);
        });
        ui.menu("Edit", |ui| {
            item(
                ui,
                &mut command,
                "Undo",
                "Ctrl+Z",
                state.can_undo,
                MenuCommand::Undo,
            );
            item(
                ui,
                &mut command,
                "Redo",
                "Ctrl+Y",
                state.can_redo,
                MenuCommand::Redo,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                "Cut",
                "Ctrl+X",
                state.has_selection,
                MenuCommand::Cut,
            );
            item(
                ui,
                &mut command,
                "Copy",
                "Ctrl+C",
                state.has_selection,
                MenuCommand::Copy,
            );
            item(
                ui,
                &mut command,
                "Paste",
                "Ctrl+V",
                true,
                MenuCommand::Paste,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                "Duplicate",
                "Ctrl+D",
                state.has_selection,
                MenuCommand::Duplicate,
            );
            item(
                ui,
                &mut command,
                "Delete",
                "Delete",
                state.has_selection,
                MenuCommand::Delete,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                "Select All",
                "Ctrl+A",
                true,
                MenuCommand::SelectAll,
            );
        });
        ui.menu("View", |ui| {
            item(
                ui,
                &mut command,
                "Actual Size",
                "Ctrl+0",
                true,
                MenuCommand::ActualSize,
            );
            item(
                ui,
                &mut command,
                "Zoom In",
                "Ctrl++",
                true,
                MenuCommand::ZoomIn,
            );
            item(
                ui,
                &mut command,
                "Zoom Out",
                "Ctrl+-",
                true,
                MenuCommand::ZoomOut,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                "Toggle Full Screen",
                "F11",
                true,
                MenuCommand::ToggleFullScreen,
            );
        });
        ui.menu("Debug", |ui| {
            item(
                ui,
                &mut command,
                "Workflow settings...",
                "",
                true,
                MenuCommand::ToggleWorkflowSettings,
            );
            ui.separator();
            item(
                ui,
                &mut command,
                if state.performance_timers_enabled {
                    "Disable Performance Timers"
                } else {
                    "Enable Performance Timers"
                },
                "",
                true,
                MenuCommand::TogglePerformanceTimers,
            );
            ui.separator();
            item(ui, &mut command, "View Log", "", true, MenuCommand::ViewLog);
            ui.separator();
            item(
                ui,
                &mut command,
                "Open Temp Folder",
                "",
                true,
                MenuCommand::OpenTempFolder,
            );
            item(
                ui,
                &mut command,
                "Clear Cache",
                "",
                true,
                MenuCommand::ClearCache,
            );
            if state.show_dev_items {
                ui.separator();
                item(
                    ui,
                    &mut command,
                    "Show All UI Elements",
                    "",
                    true,
                    MenuCommand::ShowAllUiElements,
                );
            }
        });
        ui.menu("Help", |ui| {
            item(ui, &mut command, "About", "", true, MenuCommand::About);
            item(
                ui,
                &mut command,
                "Documentation",
                "",
                true,
                MenuCommand::Documentation,
            );
            item(
                ui,
                &mut command,
                "Report a bug",
                "",
                true,
                MenuCommand::ReportBug,
            );
            item(ui, &mut command, "Credits", "", true, MenuCommand::Credits);
            ui.separator();
            item(
                ui,
                &mut command,
                "Check for Updates",
                "",
                state.can_check_for_updates,
                MenuCommand::CheckForUpdates,
            );
        });
    });
    command
}

fn item(
    ui: &mut Ui<'_>,
    command: &mut Option<MenuCommand>,
    label: &str,
    shortcut: &str,
    enabled: bool,
    value: MenuCommand,
) {
    if ui.menu_item(label, shortcut, false, enabled) {
        *command = Some(value);
    }
}
