//! The editor executable.
//!
//! A release build on Windows is a GUI-subsystem binary, so launching it - from the Start menu,
//! a shortcut or a `.bite` file - opens no console. A console-subsystem build gets one handed to
//! it by Windows, which both shows a terminal beside the editor and takes the editor down with
//! it when that terminal is closed. A debug build stays on the console subsystem, where the
//! terminal is where the developer is already looking.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use bite_gui_prototype::smoke;
use std::path::{Path, PathBuf};

/// Borrows the launching terminal's console, when the process was started from one.
///
/// A GUI-subsystem process starts with no console and no standard handles, so `--capture` and
/// the error paths below would otherwise write into nowhere. `AttachConsole` adopts the parent's
/// console and, because the handles are unset, points the standard ones at it; run from Explorer
/// or a shortcut there is no parent console, the call fails, and nothing is created - which is
/// exactly the no-terminal launch this subsystem is for. Redirected output (`> log.txt`) already
/// has its handles set, and they are left alone.
///
/// The shell does not wait for a GUI-subsystem process, so it prints its next prompt straight
/// away and this output arrives under it; a script that needs the exit code has to wait for the
/// process itself (`Start-Process -Wait`), or use a debug build.
///
/// Returns whether there is now a console to write to, which decides how a fatal error is
/// reported.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() -> bool {
    // (DWORD)-1 - ATTACH_PARENT_PROCESS.
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    // A failure means there was no console to attach to, which is not an error here.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

/// Every other build keeps the console it was given.
#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() -> bool {
    true
}

fn main() {
    let console = attach_parent_console();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(position) = arguments.iter().position(|value| value == "--capture") {
        if let Err(error) = capture(&arguments[position + 1..]) {
            fail(&error, console);
        }
        return;
    }
    // A bare first argument is a workflow to open, as the file association passes one.
    let initial = arguments
        .iter()
        .find(|argument| !argument.starts_with("--"))
        .map(PathBuf::from);
    if let Err(error) = bite_gui_prototype::app::run(initial) {
        fail(&error, console);
    }
}

/// Reports a fatal startup error and leaves.
///
/// Opened from Explorer, a shortcut or a `.bite` file there is no console for the message to
/// land in, and a GUI build that failed before its window exists would otherwise vanish without
/// a word. A dialog is the only place the user would see it.
fn fail(error: &str, console: bool) -> ! {
    if console {
        eprintln!("{error}");
    } else {
        bite_gui_prototype::dialogs::message("Bite", error);
    }
    std::process::exit(1);
}

/// Renders the comparison scenes without opening a window.
///
/// `--capture <directory> [workflow] [scale]` writes one image per scene.
fn capture(arguments: &[String]) -> Result<(), String> {
    let directory = arguments
        .first()
        .ok_or("--capture needs an output directory")?;
    let workflow = arguments
        .get(1)
        .filter(|value| value.ends_with(".bite"))
        .map(PathBuf::from);
    let scale = arguments
        .iter()
        .skip(1)
        .find_map(|value| value.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(1.0, 4.0);
    let written = smoke::capture_all(
        Path::new(directory),
        workflow.as_deref(),
        (1600, 1000),
        scale,
    )?;
    println!("Captured {} scenes", written.len());
    Ok(())
}
