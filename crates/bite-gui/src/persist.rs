//! Session state kept between launches: window bounds and panel sizes.
//!
//! The Electron build persists nothing, so this is a deliberate improvement recorded in the
//! parity notes rather than a behavior copied from it.
use crate::theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The outer window rectangle, in *physical* pixels - the same units the monitor list is in,
/// which is what [`Session::window_within`] measures it against.
///
/// When `maximized` is set, the rectangle is the one the window had *before* it was maximized:
/// what Windows calls the restored placement, and what unmaximizing after a restart should give
/// back. Storing the maximized rectangle here instead would grow the window to the monitor on
/// every launch and leave nothing to restore down to.
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

impl WindowBounds {
    /// The smallest and largest rectangle worth asking a window for, in physical pixels.
    const SIZE: std::ops::RangeInclusive<u32> = 320..=32_767;

    /// Replaces a size no window could be drawn at, keeping where it was and how it was
    /// shown. Saving guards against a zero size, but a file written by an older build or
    /// edited by hand can still hold one, and the window is created from it literally: a
    /// zero there asked for a surface nothing could be drawn on.
    fn sized(self) -> Self {
        if Self::SIZE.contains(&self.width) && Self::SIZE.contains(&self.height) {
            return self;
        }
        let default = Self::default();
        Self {
            width: default.width,
            height: default.height,
            ..self
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
        session.window = session.window.sized();
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
    ///
    /// The test is an *overlap* of the title area with a monitor, not containment of the window's
    /// top-left corner. A window sitting against the top or the left edge of its monitor has a
    /// negative outer position - Windows counts the invisible resize border in it - and a rule
    /// that insisted the corner be on the monitor threw those bounds away on every launch.
    ///
    /// Whatever the rectangle, `maximized` survives: a window that was maximized when it closed
    /// comes back maximized even if the restored rectangle underneath it has to be replaced.
    pub fn window_within(&self, monitors: &[(i32, i32, u32, u32)]) -> WindowBounds {
        if monitors.is_empty() {
            return self.window;
        }
        let bounds = self.window;
        let visible = monitors.iter().any(|(x, y, width, height)| {
            let right = x + *width as i32;
            let bottom = y + *height as i32;
            // At least 80px of the title bar must stay grabbable, horizontally and vertically.
            bounds.x + bounds.width as i32 - 80 >= *x
                && bounds.x + 80 <= right
                && bounds.y + 40 >= *y
                && bounds.y <= bottom - 40
        });
        if visible {
            bounds
        } else {
            WindowBounds {
                x: monitors[0].0 + 80,
                y: monitors[0].1 + 60,
                maximized: bounds.maximized,
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
    fn bounds_against_the_screen_edge_survive_the_visibility_test() {
        // What a maximized window reports on Windows: the resize border hangs off every side.
        let session = Session {
            window: WindowBounds {
                x: -11,
                y: -11,
                width: 2582,
                height: 1391,
                maximized: true,
            },
            panels: PanelSizes::default(),
        };
        let bounds = session.window_within(&[(0, 0, 2560, 1369)]);
        assert_eq!(bounds.x, -11);
        assert_eq!(bounds.y, -11);
        assert!(bounds.maximized);
    }

    #[test]
    fn a_maximized_window_stays_maximized_even_when_its_rectangle_is_replaced() {
        let session = Session {
            window: WindowBounds {
                x: -4000,
                y: -3000,
                width: 1280,
                height: 800,
                maximized: true,
            },
            panels: PanelSizes::default(),
        };
        let bounds = session.window_within(&[(0, 0, 1920, 1080)]);
        assert!(bounds.x >= 0 && bounds.y >= 0);
        assert!(bounds.maximized);
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

    /// A size no window can be made from is replaced with the default one, keeping where
    /// the window was and how it was shown. A zero here reached `with_inner_size` intact
    /// and the editor came up with a surface nothing could be drawn on.
    #[test]
    fn a_window_size_nothing_can_be_drawn_at_falls_back_to_the_default() {
        let unusable = |width, height| {
            WindowBounds {
                x: 240,
                y: 160,
                width,
                height,
                maximized: true,
            }
            .sized()
        };
        let default = WindowBounds::default();
        for bounds in [unusable(0, 0), unusable(1280, 0), unusable(99, 40_000)] {
            assert_eq!(
                (bounds.width, bounds.height),
                (default.width, default.height)
            );
            // Where it was and how it was shown are still the session's own.
            assert_eq!((bounds.x, bounds.y), (240, 160));
            assert!(bounds.maximized);
        }
        let kept = unusable(1400, 900);
        assert_eq!((kept.width, kept.height), (1400, 900));
    }
}
