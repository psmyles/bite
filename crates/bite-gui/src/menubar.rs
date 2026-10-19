//! The macOS menu bar.
//!
//! On macOS the editor's menu lives here, in the system menu bar at the top of the screen, rather
//! than in the window: the application menu, then the same File / Edit / View / Debug / Help
//! menus `crate::menu` draws in-window on the other platforms, then a Window menu. `menu::draw`
//! is not called on macOS, so the panels start at the top of the window.
//!
//! **Quit is ours, not AppKit's.** `PredefinedMenuItem::quit` sends `terminate:`, which ends the
//! process where it stands: no prompt about unsaved work, no `save_session`, so the window
//! position and panel sizes of the last session are lost and a dirty document goes with them.
//! The item below is an ordinary [`MenuItem`] carrying the same Cmd+Q accelerator, routed
//! through the event loop to [`Command::Exit`] - the identical path the red close button takes.
//!
//! **Accelerators here intercept keys before winit ever sees them.** A key equivalent is matched
//! by AppKit before the keystroke reaches winit's view, so Cmd+C bound here would never reach a
//! focused Dear ImGui text field, and the editor's own shortcut handling (`app::handle_shortcuts`,
//! which stands down while a field has focus or a modal is open) would be bypassed. So an item
//! chosen *by its key equivalent* does not run its command: it reports [`Trigger::Key`], and the
//! event loop replays the chord into Dear ImGui as if the menu were not there (see
//! [`Chord::replay`]). Only an item chosen with the pointer runs its command directly. The one
//! exception is Quit, which has no ImGui binding to replay into and always runs.
//!
//! Delete is listed without an accelerator for the same reason: a bare Backspace key equivalent
//! would be taken before the input method sees it and break composition in text fields, and the
//! key already works through `handle_shortcuts`.
//!
//! There is no AppKit Full Screen item: the editor owns that state (`Editor::fullscreen`, which
//! `platform` syncs onto the window each frame), and `toggleFullScreen:` would change it behind
//! winit's back and leave the flag stale. The View menu's item is the editor's own command.

use std::cell::Cell;

use bite_imgui::{Context, Key};
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use objc2_app_kit::{NSApplication, NSEventType};
use objc2_foundation::MainThreadMarker;
use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;

use crate::menu::Command;
use crate::platform::AppEvent;

/// How a menu item was chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    /// Clicked, or picked from an open menu.
    Pointer,
    /// By its key equivalent, which AppKit took before the window saw the keystroke.
    Key,
}

/// A key equivalent, both as AppKit binds it and as Dear ImGui would have received it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Chord {
    /// Whether Cmd is part of the chord. Every chord but F11's has it.
    cmd: bool,
    shift: bool,
    code: Code,
    key: Key,
}

impl Chord {
    const fn cmd(code: Code, key: Key) -> Self {
        Self {
            cmd: true,
            shift: false,
            code,
            key,
        }
    }

    const fn cmd_shift(code: Code, key: Key) -> Self {
        Self {
            cmd: true,
            shift: true,
            code,
            key,
        }
    }

    const fn bare(code: Code, key: Key) -> Self {
        Self {
            cmd: false,
            shift: false,
            code,
            key,
        }
    }

    fn accelerator(self) -> Accelerator {
        let mut modifiers = Modifiers::empty();
        if self.cmd {
            modifiers |= Modifiers::META;
        }
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        Accelerator::new((!modifiers.is_empty()).then_some(modifiers), self.code)
    }

    /// Feeds the chord to Dear ImGui as one press and release, the way winit would have had the
    /// menu not taken it.
    ///
    /// `held` is the modifier state winit last reported. A modifier the user is already holding
    /// is left alone, because pressing and releasing it again here would drop it while it is
    /// still down; only the ones the chord needs and the keyboard does not supply are pressed,
    /// and released again afterwards. Dear ImGui trickles queued input, so the press and the release
    /// land on separate frames and `key_pressed` sees the press.
    pub fn replay(self, context: &mut Context, held: ModifiersState) {
        let press_super = self.cmd && !held.super_key();
        let press_shift = self.shift && !held.shift_key();
        if press_super {
            context.key(Key::Super, true);
        }
        if press_shift {
            context.key(Key::Shift, true);
        }
        context.key(self.key, true);
        context.key(self.key, false);
        if press_shift {
            context.key(Key::Shift, false);
        }
        if press_super {
            context.key(Key::Super, false);
        }
    }
}

/// Every editor item the menu bar carries, by menu, in order. `None` is a separator.
///
/// The labels, order and chords are `menu::menus`'s, spelled the Mac way: Redo is Shift+Cmd+Z
/// (Cmd+Y still works, through `handle_shortcuts`), and Zoom In is Cmd+=, the key the plus sign
/// shares.
type Row = Option<(Command, &'static str, Option<Chord>)>;

const FILE: &[Row] = &[
    Some((Command::New, "New", Some(Chord::cmd(Code::KeyN, Key::N)))),
    None,
    Some((
        Command::RunWorkflow,
        "Run Workflow",
        Some(Chord::cmd(Code::KeyR, Key::R)),
    )),
    None,
    Some((
        Command::OpenWorkflow,
        "Open Workflow…",
        Some(Chord::cmd(Code::KeyO, Key::O)),
    )),
    Some((
        Command::SaveWorkflow,
        "Save Workflow",
        Some(Chord::cmd(Code::KeyS, Key::S)),
    )),
    Some((
        Command::SaveWorkflowAs,
        "Save Workflow As…",
        Some(Chord::cmd_shift(Code::KeyS, Key::S)),
    )),
];

const EXPORT: &[Row] = &[
    Some((Command::ExportPowerShell, "PowerShell", None)),
    Some((Command::ExportBash, "Bash", None)),
    Some((Command::ExportCmd, "Windows Command Prompt", None)),
];

const EDIT: &[Row] = &[
    Some((Command::Undo, "Undo", Some(Chord::cmd(Code::KeyZ, Key::Z)))),
    Some((
        Command::Redo,
        "Redo",
        Some(Chord::cmd_shift(Code::KeyZ, Key::Z)),
    )),
    None,
    Some((Command::Cut, "Cut", Some(Chord::cmd(Code::KeyX, Key::X)))),
    Some((Command::Copy, "Copy", Some(Chord::cmd(Code::KeyC, Key::C)))),
    Some((
        Command::Paste,
        "Paste",
        Some(Chord::cmd(Code::KeyV, Key::V)),
    )),
    None,
    Some((
        Command::Duplicate,
        "Duplicate",
        Some(Chord::cmd(Code::KeyD, Key::D)),
    )),
    Some((Command::Delete, "Delete", None)),
    None,
    Some((
        Command::SelectAll,
        "Select All",
        Some(Chord::cmd(Code::KeyA, Key::A)),
    )),
];

const VIEW: &[Row] = &[
    Some((
        Command::ActualSize,
        "Actual Size",
        Some(Chord::cmd(Code::Digit0, Key::Digit0)),
    )),
    Some((
        Command::ZoomIn,
        "Zoom In",
        Some(Chord::cmd(Code::Equal, Key::Equal)),
    )),
    Some((
        Command::ZoomOut,
        "Zoom Out",
        Some(Chord::cmd(Code::Minus, Key::Minus)),
    )),
    None,
    Some((
        Command::ToggleFullScreen,
        "Toggle Full Screen",
        Some(Chord::bare(Code::F11, Key::F11)),
    )),
];

/// The Debug menu below the timer toggle, which is built on its own because its label changes.
const DEBUG: &[Row] = &[
    None,
    Some((Command::ViewLog, "View Log", None)),
    None,
    Some((Command::OpenTempFolder, "Open Temp Folder", None)),
    Some((Command::ClearCache, "Clear Cache", None)),
];

const DEVELOPER: &[Row] = &[
    None,
    Some((Command::ShowAllUiElements, "Show All UI Elements", None)),
];

const HELP: &[Row] = &[
    Some((Command::Documentation, "Documentation", None)),
    Some((Command::ReportBug, "Report a Bug", None)),
    Some((Command::Credits, "Credits", None)),
    None,
    Some((Command::CheckForUpdates, "Check for Updates…", None)),
];

/// The item ids, which are the commands' names, so one lookup serves every row.
const ID_QUIT: &str = "Exit";
const ID_ABOUT: &str = "About";

fn id(command: Command) -> String {
    format!("{command:?}")
}

/// The menu bar, kept alive for the life of the process: dropping it takes the menu bar with it.
pub struct MenuBar {
    _menu: Menu,
    /// The one item whose label follows editor state.
    timers: MenuItem,
    timers_shown: Cell<bool>,
}

impl MenuBar {
    /// Brings the labels that follow editor state up to date. Cheap enough to call every frame:
    /// AppKit is only touched when something changed.
    pub fn sync(&self, timers_enabled: bool) {
        if self.timers_shown.replace(timers_enabled) != timers_enabled {
            self.timers.set_text(timers_label(timers_enabled));
        }
    }
}

fn timers_label(enabled: bool) -> &'static str {
    if enabled {
        "Disable Performance Timers"
    } else {
        "Enable Performance Timers"
    }
}

/// Builds the menu bar, installs it as the application's, and forwards its events into the event
/// loop.
///
/// Must run on the main thread, after the event loop is built (`NSApplication` has to be up) and
/// before it runs. `show_developer_items` adds the Debug menu's developer-only rows, as it does
/// in-window.
///
/// `None` if the menu could not be built, which is not worth refusing to start over - the editor
/// merely becomes harder to use, and the caller says so.
pub fn install(proxy: EventLoopProxy<AppEvent>, show_developer_items: bool) -> Option<MenuBar> {
    let product = env!("BITE_PRODUCT_NAME");

    // About opens the editor's own dialog, as it does in-window, rather than AppKit's panel: the
    // editor's lists the ImageMagick and library versions too.
    let about = MenuItem::with_id(ID_ABOUT, format!("About {product}"), true, None);
    let quit = MenuItem::with_id(
        ID_QUIT,
        format!("Quit {product}"),
        true,
        Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
    );
    let app_menu = Submenu::with_items(
        product,
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some(&format!("Hide {product}"))),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &quit,
        ],
    )
    .ok()?;

    let file = submenu("File", FILE)?;
    let export = submenu("Export CLI Script", EXPORT)?;
    file.append_items(&[&PredefinedMenuItem::separator(), &export])
        .ok()?;

    let timers_enabled = false;
    let timers = MenuItem::with_id(
        id(Command::TogglePerformanceTimers),
        timers_label(timers_enabled),
        true,
        None,
    );
    let debug = Submenu::with_items("Debug", true, &[&timers]).ok()?;
    append(&debug, DEBUG)?;
    if show_developer_items {
        append(&debug, DEVELOPER)?;
    }

    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
        ],
    )
    .ok()?;

    let help = submenu("Help", HELP)?;

    let menu = Menu::with_items(&[
        &app_menu,
        &file,
        &submenu("Edit", EDIT)?,
        &submenu("View", VIEW)?,
        &debug,
        &window,
        &help,
    ])
    .ok()?;
    menu.init_for_nsapp();
    // The Window menu AppKit keeps its window list in, and the Help menu it puts its search
    // field on.
    window.set_as_windows_menu_for_nsapp();
    help.set_as_help_menu_for_nsapp();

    // muda calls this synchronously from the item's action, on the main thread and inside the
    // AppKit event that chose it - which is what lets `trigger` read that event. The command
    // itself is handed to the event loop, so it is handled with everything else, in order,
    // rather than racing the editor's state.
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(command) = command_for(&event.id) {
            // A closed event loop means the process is already leaving; the menu outliving it is
            // not an error.
            let _ = proxy.send_event(AppEvent::Menu(command, trigger()));
        }
    }));

    Some(MenuBar {
        _menu: menu,
        timers,
        timers_shown: Cell::new(timers_enabled),
    })
}

/// A submenu holding `rows`.
fn submenu(title: &str, rows: &[Row]) -> Option<Submenu> {
    let submenu = Submenu::new(title, true);
    append(&submenu, rows)?;
    Some(submenu)
}

fn append(submenu: &Submenu, rows: &[Row]) -> Option<()> {
    for row in rows {
        let item: Box<dyn IsMenuItem> = match row {
            Some((command, label, chord)) => Box::new(MenuItem::with_id(
                id(*command),
                *label,
                true,
                chord.map(Chord::accelerator),
            )),
            None => Box::new(PredefinedMenuItem::separator()),
        };
        submenu.append(item.as_ref()).ok()?;
    }
    Some(())
}

/// Whether the item being chosen right now was chosen by its key equivalent.
fn trigger() -> Trigger {
    let Some(mtm) = MainThreadMarker::new() else {
        return Trigger::Pointer;
    };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: reading the event being dispatched, on the main thread that dispatches it.
    match app.currentEvent().map(|event| unsafe { event.r#type() }) {
        Some(NSEventType::KeyDown) => Trigger::Key,
        _ => Trigger::Pointer,
    }
}

/// The chord to replay into Dear ImGui in place of running `command`, if there is one.
///
/// A key equivalent replays its own chord, so the keystroke goes where it would have gone without
/// the menu. A pointer pick runs the command - unless a text field has focus (`typing`) and the
/// command is one a text field has its own version of, which is the field's to act on, as a
/// focused text view takes Edit > Copy in any Mac app.
pub fn replay_for(command: Command, trigger: Trigger, typing: bool) -> Option<Chord> {
    match trigger {
        Trigger::Key => chord(command),
        Trigger::Pointer if typing => match command {
            Command::Undo
            | Command::Redo
            | Command::Cut
            | Command::Copy
            | Command::Paste
            | Command::SelectAll => chord(command),
            Command::Delete => Some(Chord::bare(Code::Delete, Key::Delete)),
            _ => None,
        },
        Trigger::Pointer => None,
    }
}

/// The chord an item's key equivalent replays into Dear ImGui, if it has one. Quit has none: it
/// is not an ImGui binding, so it always runs.
fn chord(command: Command) -> Option<Chord> {
    [FILE, EDIT, VIEW]
        .into_iter()
        .flatten()
        .flatten()
        .find(|(candidate, _, _)| *candidate == command)
        .and_then(|(_, _, chord)| *chord)
}

/// The editor command an item id stands for.
fn command_for(id: &MenuId) -> Option<Command> {
    match id.as_ref() {
        ID_QUIT => return Some(Command::Exit),
        ID_ABOUT => return Some(Command::About),
        _ => {}
    }
    [FILE, EXPORT, EDIT, VIEW, DEBUG, DEVELOPER, HELP]
        .into_iter()
        .flatten()
        .flatten()
        .map(|(command, _, _)| *command)
        .chain([Command::TogglePerformanceTimers])
        .find(|command| self::id(*command) == id.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chord_is_the_one_the_editor_already_binds() {
        // A key equivalent is replayed into Dear ImGui and resolved by `menu::shortcut`, so a
        // chord that resolves to a different command - or none - would be a menu row that lies.
        for (command, _, chord) in [FILE, EDIT, VIEW].into_iter().flatten().flatten() {
            let Some(chord) = chord else { continue };
            let name = match chord.key {
                Key::F11 => {
                    assert_eq!(*command, Command::ToggleFullScreen);
                    continue;
                }
                Key::Digit0 => "0".to_string(),
                Key::Equal => "=".to_string(),
                Key::Minus => "-".to_string(),
                other => format!("{other:?}").to_lowercase(),
            };
            let modifiers = crate::menu::Modifiers {
                primary: chord.cmd,
                shift: chord.shift,
            };
            assert_eq!(
                crate::menu::shortcut(&name, modifiers),
                Some(*command),
                "{command:?}"
            );
        }
    }

    #[test]
    fn a_key_equivalent_is_replayed_and_a_click_runs() {
        let save = Command::SaveWorkflow;
        assert!(replay_for(save, Trigger::Key, false).is_some());
        assert!(replay_for(save, Trigger::Key, true).is_some());
        assert!(replay_for(save, Trigger::Pointer, false).is_none());
        // Saving is not a text field's to do, so a click saves even while one has focus.
        assert!(replay_for(save, Trigger::Pointer, true).is_none());
        // Quit has no ImGui binding, so it runs however it was chosen.
        assert!(replay_for(Command::Exit, Trigger::Key, false).is_none());
    }

    #[test]
    fn edit_commands_go_to_a_focused_text_field() {
        for command in [
            Command::Undo,
            Command::Redo,
            Command::Cut,
            Command::Copy,
            Command::Paste,
        ]
        .into_iter()
        .chain([Command::SelectAll, Command::Delete])
        {
            assert!(
                replay_for(command, Trigger::Pointer, true).is_some(),
                "{command:?}"
            );
            assert!(
                replay_for(command, Trigger::Pointer, false).is_none(),
                "{command:?}"
            );
        }
    }

    #[test]
    fn every_item_id_maps_back_to_its_command() {
        for (command, _, _) in [FILE, EXPORT, EDIT, VIEW, DEBUG, DEVELOPER, HELP]
            .into_iter()
            .flatten()
            .flatten()
        {
            assert_eq!(command_for(&MenuId::new(id(*command))), Some(*command));
        }
        assert_eq!(
            command_for(&MenuId::new(id(Command::TogglePerformanceTimers))),
            Some(Command::TogglePerformanceTimers)
        );
        assert_eq!(command_for(&MenuId::new(ID_QUIT)), Some(Command::Exit));
        assert_eq!(command_for(&MenuId::new(ID_ABOUT)), Some(Command::About));
    }
}
