//! Session state kept between launches: window bounds and panel sizes.
//!
//! The Electron build persists nothing, so this is a deliberate improvement recorded in the
//! parity notes rather than a behavior copied from it.
use crate::theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowBounds {
    fn default() -> Self {
        Self {
            x: 80,
            y: 60,
            width: 1280,
            height: 800,
            maximized: false,
        }
    }
}

/// Panel sizes in the same units the shell uses: pixels, except the inspector fraction.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PanelSizes {
    pub left: f32,
    pub right: f32,
    pub filmstrip: f32,
    pub inspector_fraction: f32,
}

impl Default for PanelSizes {
    fn default() -> Self {
        Self {
            left: theme::LEFT_PANEL_DEFAULT,
            right: theme::RIGHT_PANEL_DEFAULT,
            filmstrip: theme::FILMSTRIP_DEFAULT,
            inspector_fraction: theme::INSPECTOR_DEFAULT,
        }
    }
}

impl PanelSizes {
    /// Clamps every size into the range the stylesheet allows.
    pub fn clamped(self) -> Self {
        Self {
            left: self
                .left
                .clamp(theme::LEFT_PANEL_MIN, theme::LEFT_PANEL_MAX),
            right: self
                .right
                .clamp(theme::RIGHT_PANEL_MIN, theme::RIGHT_PANEL_MAX),
            filmstrip: self
                .filmstrip
                .clamp(theme::FILMSTRIP_MIN, theme::FILMSTRIP_MAX),
            inspector_fraction: self
                .inspector_fraction
                .clamp(theme::INSPECTOR_MIN, theme::INSPECTOR_MAX),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Session {
    pub window: WindowBounds,
    pub panels: PanelSizes,
}

/// Where the session file lives on this platform.
pub fn session_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("BITE_SESSION_PATH") {
        return Some(PathBuf::from(explicit));
    }
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
        });
    root.map(|root| root.join("BITE").join("session.json"))
}

impl Session {
    pub fn load() -> Self {
        let Some(path) = session_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut session: Self = serde_json::from_str(&text).unwrap_or_default();
        session.panels = session.panels.clamped();
        session
    }

    pub fn save(&self) {
        let Some(path) = session_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Discards remembered bounds that no longer land on a visible monitor.
    pub fn window_within(&self, monitors: &[(i32, i32, u32, u32)]) -> WindowBounds {
        if monitors.is_empty() {
            return self.window;
        }
        let bounds = self.window;
        let visible = monitors.iter().any(|(x, y, width, height)| {
            let right = x + *width as i32;
            let bottom = y + *height as i32;
            // At least part of the title area must remain reachable.
            bounds.x + 80 >= *x
                && bounds.x <= right - 80
                && bounds.y >= *y - 8
                && bounds.y <= bottom - 40
        });
        if visible {
            bounds
        } else {
            WindowBounds {
                x: monitors[0].0 + 80,
                y: monitors[0].1 + 60,
                ..WindowBounds::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_sizes_clamp_to_the_stylesheet_limits() {
        let sizes = PanelSizes {
            left: 10.0,
            right: 5000.0,
            filmstrip: 0.0,
            inspector_fraction: 0.99,
        }
        .clamped();
        assert_eq!(sizes.left, theme::LEFT_PANEL_MIN);
        assert_eq!(sizes.right, theme::RIGHT_PANEL_MAX);
        assert_eq!(sizes.filmstrip, theme::FILMSTRIP_MIN);
        assert_eq!(sizes.inspector_fraction, theme::INSPECTOR_MAX);
    }

    #[test]
    fn defaults_match_the_stylesheet_tokens() {
        let sizes = PanelSizes::default();
        assert_eq!(sizes.left, 220.0);
        assert_eq!(sizes.right, 280.0);
        assert_eq!(sizes.filmstrip, 120.0);
        assert_eq!(sizes.inspector_fraction, 0.65);
    }

    #[test]
    fn offscreen_window_bounds_are_moved_onto_a_monitor() {
        let session = Session {
            window: WindowBounds {
                x: -4000,
                y: -3000,
                width: 1280,
                height: 800,
                maximized: false,
            },
            panels: PanelSizes::default(),
        };
        let bounds = session.window_within(&[(0, 0, 1920, 1080)]);
        assert!(bounds.x >= 0 && bounds.y >= 0);
    }

    #[test]
    fn remembered_bounds_on_a_live_monitor_are_kept() {
        let session = Session {
            window: WindowBounds {
                x: 100,
                y: 100,
                width: 1400,
                height: 900,
                maximized: false,
            },
            panels: PanelSizes::default(),
        };
        let bounds = session.window_within(&[(0, 0, 1920, 1080)]);
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.width, 1400);
    }
}
