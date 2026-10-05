//! Debug > View Log, from `public/log-viewer.html`.
//!
//! The Electron viewer is a second browser window fed the entries as they are recorded. Here
//! it is a window inside the editor reading the log file itself, so it also shows what
//! earlier sessions wrote, and it re-reads only what has been appended since it last looked.
use crate::{controls, logging::Level, theme};
use bite_imgui::{Rounding, StyleVar, Ui, Vec2, WindowFlags};

/// One line of the file as the window shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub timestamp: String,
    pub level: Level,
    pub message: String,
    /// A session banner, which carries no level and rules off the lines before it.
    pub banner: bool,
}

/// Reads a line the way the logger writes one: `12:34:56.789 [INFO] message`.
///
/// Anything else is kept as it stands, so a stack trace or a wrapped line is still readable
/// rather than being dropped for not matching.
pub fn parse(line: &str) -> Line {
    if line.starts_with("--- Session") {
        return Line {
            timestamp: String::new(),
            level: Level::Info,
            message: line.trim_matches(['-', ' ']).to_string(),
            banner: true,
        };
    }
    let stamped = line.len() > 13
        && line.as_bytes()[2] == b':'
        && line.as_bytes()[5] == b':'
        && line.as_bytes()[8] == b'.'
        && line.as_bytes()[12] == b' ';
    if !stamped {
        return Line {
            timestamp: String::new(),
            level: Level::Info,
            message: line.to_string(),
            banner: false,
        };
    }
    let (timestamp, rest) = line.split_at(12);
    let rest = rest.trim_start();
    let (level, message) = match rest.split_once(']') {
        Some((tag, message)) => {
            let level = match tag.trim_start_matches('[') {
                "WARN" => Level::Warning,
                "ERROR" => Level::Error,
                _ => Level::Info,
            };
            (level, message.trim_start())
        }
        None => (Level::Info, rest),
    };
    Line {
        timestamp: timestamp.to_string(),
        level,
        message: message.to_string(),
        banner: false,
    }
}

/// Splits a message into its opening `[tag]` and the rest, which the viewer colors apart.
pub fn split_tag(message: &str) -> (Option<&str>, &str) {
    if !message.starts_with('[') {
        return (None, message);
    }
    match message.find(']') {
        Some(end) => (Some(&message[..=end]), &message[end + 1..]),
        None => (None, message),
    }
}

/// What the window remembers between frames.
pub struct LogWindow {
    pub open: bool,
    pub show_info: bool,
    pub show_warning: bool,
    pub show_error: bool,
    lines: Vec<Line>,
    /// Whether the window follows the file. A window given its lines directly, as the
    /// capture scene does, shows those instead.
    follows_file: bool,
    /// How many bytes of the file have been read, so only new text is parsed.
    read: u64,
    /// Lines before this are not shown. Clear moves the mark rather than emptying the file.
    hidden: usize,
}

impl Default for LogWindow {
    fn default() -> Self {
        Self {
            open: false,
            show_info: true,
            show_warning: true,
            show_error: true,
            lines: Vec::new(),
            follows_file: true,
            read: 0,
            hidden: 0,
        }
    }
}

impl LogWindow {
    /// Opens the window, reading the file from the beginning.
    pub fn show(&mut self) {
        self.open = true;
        self.lines.clear();
        self.read = 0;
        self.hidden = 0;
        self.refresh();
    }

    /// Whether a line of this level is shown.
    pub fn shows(&self, level: Level) -> bool {
        match level {
            Level::Info => self.show_info,
            Level::Warning => self.show_warning,
            Level::Error => self.show_error,
        }
    }

    /// Reads whatever has been appended since the last look.
    ///
    /// A file that has shrunk was replaced, so it is read again from the beginning.
    pub fn refresh(&mut self) {
        if !self.follows_file {
            return;
        }
        let Some(path) = crate::logging::log_path() else {
            return;
        };
        let length = std::fs::metadata(&path).map(|data| data.len()).unwrap_or(0);
        if length == self.read {
            return;
        }
        if length < self.read {
            self.lines.clear();
            self.hidden = 0;
            self.read = 0;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        // Only the tail is new; the part already parsed is measured in bytes.
        let fresh = text.get(self.read as usize..).unwrap_or(&text);
        for line in fresh.lines() {
            if !line.trim().is_empty() {
                self.lines.push(parse(line));
            }
        }
        self.read = text.len() as u64;
    }

    /// Opens the window on lines given directly, for a capture that must not depend on
    /// whatever the machine's log happens to hold.
    pub fn seed(&mut self, lines: Vec<Line>) {
        self.lines = lines;
        self.follows_file = false;
        self.hidden = 0;
        self.open = true;
    }

    /// Hides everything recorded so far, as the viewer's Clear button does.
    pub fn clear(&mut self) {
        self.hidden = self.lines.len();
    }

    /// The lines the filters let through.
    pub fn visible(&self) -> impl Iterator<Item = &Line> {
        self.lines[self.hidden.min(self.lines.len())..]
            .iter()
            .filter(|line| line.banner || self.shows(line.level))
    }
}

/// The color a line's message is drawn in.
fn message_color(line: &Line) -> bite_imgui::Color {
    match line.level {
        Level::Info => theme::log::INFO,
        Level::Warning => theme::log::WARNING,
        Level::Error => theme::log::ERROR,
    }
}

/// The color its level badge is drawn in, which is dimmer than the message for a notice.
fn badge_color(line: &Line) -> bite_imgui::Color {
    match line.level {
        Level::Info => theme::log::INFO_BADGE,
        Level::Warning => theme::log::WARNING,
        Level::Error => theme::log::ERROR,
    }
}

/// Draws the window. Returns true when the log file should be opened outside the editor.
pub fn draw(ui: &mut Ui, window: &mut LogWindow) -> bool {
    if !window.open {
        return false;
    }
    window.refresh();
    let mut reveal = false;
    let mut open = window.open;
    controls::debug_window(ui, "Log", DEFAULT_SIZE, &mut open, |ui| {
        toolbar(ui, window, &mut reveal);
        ui.separator();
        let lines = WindowFlags {
            horizontal_scrollbar: true,
            ..WindowFlags::default()
        };
        ui.child_with("log-lines", [0.0, 0.0], false, lines, |ui| {
            // Following the newest line is what a log window is for, but scrolling back
            // must stay put, so it only follows while the view is already at the bottom.
            let following = ui.scroll_y() >= ui.scroll_max_y() - theme::log::FOLLOW_MARGIN;
            let rows: Vec<Line> = window.visible().cloned().collect();
            // Each row carries its own line height, so spacing between items would double
            // the gap the stylesheet asks for.
            ui.with_style(&[StyleVar::ItemSpacing([0.0, 0.0])], |ui| {
                for line in &rows {
                    row(ui, line);
                }
            });
            if following {
                ui.set_scroll_y(ui.scroll_max_y());
            }
        });
    });
    window.open = open;
    reveal
}

/// The filter row across the top.
fn toolbar(ui: &mut Ui, window: &mut LogWindow, reveal: &mut bool) {
    ui.draw_list().text_with_face(
        ui.cursor_screen_position(),
        theme::log::LABEL,
        theme::face::SMALL_MONO,
        "Filter",
    );
    ui.dummy(controls::measure(ui, theme::face::SMALL_MONO, "Filter"));
    ui.same_line();
    ui.checkbox("Info", &mut window.show_info);
    ui.same_line();
    ui.checkbox("Warning", &mut window.show_warning);
    ui.same_line();
    ui.checkbox("Error", &mut window.show_error);
    ui.same_line();
    if ui.button("Clear") {
        window.clear();
    }
    ui.same_line();
    if ui.button("Open File") {
        *reveal = true;
    }
}

/// One line: its time, its level and its message, in three columns.
fn row(ui: &mut Ui, line: &Line) {
    let origin = ui.cursor_screen_position();
    // `line-height: 1.55` in the viewer's stylesheet, measured on the face the row uses
    // rather than whatever font happens to be current.
    let text = controls::measure(ui, theme::face::SMALL_MONO, "Ag")[1];
    let height = (text * theme::log::LINE_HEIGHT).round();
    let baseline = origin[1] + ((height - text) / 2.0).round();
    let width = ui.content_region_available()[0].max(1.0);
    if controls::point_in(
        ui.mouse_position(),
        [origin[0], origin[1]],
        [origin[0] + width, origin[1] + height],
    ) {
        ui.draw_list().rect(
            [origin[0] - theme::log::ROW_PADDING_X, origin[1]],
            [origin[0] + width, origin[1] + height],
            theme::log::ROW_HOVER,
            0.0,
            Rounding::None,
        );
    }
    let list = ui.draw_list();
    if line.banner {
        list.text_with_face(
            [origin[0], baseline],
            theme::log::LABEL,
            theme::face::SMALL_MONO,
            &line.message,
        );
        ui.dummy([width, height]);
        return;
    }
    let mut x = origin[0];
    list.text_with_face(
        [x, baseline],
        theme::log::TIMESTAMP,
        theme::face::SMALL_MONO,
        &line.timestamp,
    );
    // The columns line up across every row, so each is measured from a full stamp rather
    // than from the one on this line.
    x += list.measure(theme::face::SMALL_MONO, "00:00:00.000")[0] + theme::log::COLUMN_GAP;
    list.text_with_face(
        [x, baseline],
        badge_color(line),
        theme::face::SMALL_MONO,
        line.level.label(),
    );
    x += theme::log::BADGE_WIDTH + theme::log::COLUMN_GAP;
    let (tag, rest) = split_tag(&line.message);
    if let Some(tag) = tag {
        list.text_with_face([x, baseline], theme::log::TAG, theme::face::SMALL_MONO, tag);
        x += list.measure(theme::face::SMALL_MONO, tag)[0];
    }
    list.text_with_face(
        [x, baseline],
        message_color(line),
        theme::face::SMALL_MONO,
        rest,
    );
    ui.dummy([width, height]);
}

/// The size the window opens at, for the capture scene.
pub const DEFAULT_SIZE: Vec2 = [760.0, 460.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_line_splits_into_its_time_level_and_message() {
        let line = parse("12:34:56.789 [WARN] [magick] slow spawn");
        assert_eq!(line.timestamp, "12:34:56.789");
        assert_eq!(line.level, Level::Warning);
        assert_eq!(line.message, "[magick] slow spawn");
        assert!(!line.banner);
    }

    #[test]
    fn a_session_banner_is_its_own_kind_of_line() {
        let line = parse("--- Session 2026-09-20T10:11:12Z ---");
        assert!(line.banner);
        assert_eq!(line.message, "Session 2026-09-20T10:11:12Z");
    }

    #[test]
    fn a_line_in_no_particular_shape_is_kept_as_it_stands() {
        let line = parse("    at some::frame (src/lib.rs:12)");
        assert_eq!(line.message, "    at some::frame (src/lib.rs:12)");
        assert_eq!(line.level, Level::Info);
        assert!(line.timestamp.is_empty());
    }

    #[test]
    fn an_opening_tag_is_separated_so_it_can_be_colored() {
        assert_eq!(split_tag("[magick] done"), (Some("[magick]"), " done"));
        assert_eq!(split_tag("plain"), (None, "plain"));
        // An opening bracket that never closes is not a tag.
        assert_eq!(split_tag("[unclosed"), (None, "[unclosed"));
    }

    #[test]
    fn the_filters_decide_which_lines_show() {
        let mut window = LogWindow {
            lines: vec![
                parse("12:00:00.000 [INFO] one"),
                parse("12:00:01.000 [ERROR] two"),
            ],
            ..LogWindow::default()
        };
        assert_eq!(window.visible().count(), 2);
        window.show_info = false;
        let shown: Vec<&str> = window.visible().map(|line| line.message.as_str()).collect();
        assert_eq!(shown, ["two"]);
    }

    #[test]
    fn clearing_hides_what_is_there_without_losing_what_follows() {
        let mut window = LogWindow {
            lines: vec![parse("12:00:00.000 [INFO] before")],
            ..LogWindow::default()
        };
        window.clear();
        assert_eq!(window.visible().count(), 0);
        window.lines.push(parse("12:00:02.000 [INFO] after"));
        let shown: Vec<&str> = window.visible().map(|line| line.message.as_str()).collect();
        assert_eq!(shown, ["after"]);
    }
}
