//! The macOS menu bar.
//!
//! A Mac app without a menu bar reads as broken and, more concretely, cannot be quit from the
//! keyboard: Cmd+Q belongs to the menu, not to the window, so without one there is no way out
//! but Force Quit. This builds the minimum that makes the editor behave like a Mac app - the
//! application menu and a Window menu - and leaves everything else to the in-window menu bar the
//! editor draws itself, which is the same on both platforms.
//!
//! **Quit is ours, not AppKit's.** `PredefinedMenuItem::quit` sends `terminate:`, which ends the
//! process where it stands: no prompt about unsaved work, no `save_session`, so the window
//! position and panel sizes of the last session are lost and a dirty document goes with them.
//! The item below is an ordinary [`MenuItem`] carrying the same Cmd+Q accelerator, routed
//! through the event loop to [`Command::Exit`] - the identical path the File menu's Exit and the
//! red close button take. It is the one item here that does not map onto an AppKit selector, and
//! that is the point.
//!
//! **Accelerators here intercept keys before winit ever sees them**, which is why this menu
//! carries exactly one. A File menu with Cmd+N or an Edit menu with Cmd+C would shadow the
//! editor's own handling of those chords - Dear ImGui already implements them, and on macOS it
//! does so with Cmd rather than Ctrl because sokol_imgui sets `ConfigMacOSXBehaviors` - and the
//! keystroke would go to an AppKit responder chain that winit's view does not forward to ImGui.
//! The result would be shortcuts that look bound and do nothing.
//!
//! There is no Full Screen item for a related reason: the editor owns that state
//! (`Editor::fullscreen`, which `platform` syncs onto the window each frame), and AppKit's
//! `toggleFullScreen:` would change it behind winit's back and leave the flag stale. F11 and the
//! View menu keep the two in step.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use winit::event_loop::EventLoopProxy;

use crate::menu::Command;
use crate::platform::AppEvent;

/// The one item the editor performs itself; the rest are AppKit's own.
const ID_QUIT: &str = "bite.quit";

/// Builds the menu bar, installs it as the application's, and forwards its events into the event
/// loop.
///
/// Must run on the main thread, after the event loop is built (`NSApplication` has to be up) and
/// before it runs. The returned [`Menu`] has to be kept alive for the life of the process:
/// dropping it takes the menu bar with it.
///
/// `None` if the menu could not be built, which is not worth refusing to start over - the editor
/// merely becomes harder to quit, and the caller says so.
pub fn install(proxy: EventLoopProxy<AppEvent>) -> Option<Menu> {
    let product = env!("BITE_PRODUCT_NAME");
    let about = AboutMetadata {
        name: Some(product.to_string()),
        version: Some(env!("BITE_VERSION").to_string()),
        copyright: Some(env!("BITE_COPYRIGHT").to_string()),
        comments: Some(env!("BITE_DESCRIPTION").to_string()),
        website: Some(env!("BITE_HOMEPAGE").to_string()),
        ..Default::default()
    };

    let quit = MenuItem::with_id(
        ID_QUIT,
        format!("Quit {product}"),
        true,
        // Cmd+Q, the chord macOS users will try first and the only one this menu claims.
        Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
    );

    let app_menu = Submenu::with_items(
        product,
        true,
        &[
            &PredefinedMenuItem::about(Some(&format!("About {product}")), Some(about)),
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

    let window_menu = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
        ],
    )
    .ok()?;

    let menu = Menu::with_items(&[&app_menu, &window_menu]).ok()?;
    menu.init_for_nsapp();

    // muda delivers on a channel of its own; hand each event to the event loop so it is handled
    // on the main thread with everything else, in order, rather than racing the editor's state.
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(command) = command_for(&event.id) {
            // A closed event loop means the process is already leaving; the menu outliving it is
            // not an error.
            let _ = proxy.send_event(AppEvent::Menu(command));
        }
    }));

    Some(menu)
}

/// The editor command an item id stands for.
fn command_for(id: &MenuId) -> Option<Command> {
    match id.as_ref() {
        ID_QUIT => Some(Command::Exit),
        _ => None,
    }
}
