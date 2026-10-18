//! Debug > View Log, from `public/log-viewer.html`.
//!
//! The Electron viewer is a second browser window fed the entries as they are recorded. Here
//! it is a window inside the editor reading the log file itself, so it also shows what
//! earlier sessions wrote, and it re-reads only what has been appended since it last looked.
use crate::{clock, controls, logging::Level, theme};
use bite_imgui::{Rounding, StyleVar, Ui, Vec2};

/// One line of the file as the window shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// When the line was written, ready to show, or empty when the line does not say.
    pub timestamp: String,
    pub level: Level,
    pub message: String,
    /// A session banner, which carries no level and rules off the lines before it.
    pub banner: bool,
}

/// Reads a line the way the logger writes one, `2026-09-20T09:00:00.120Z [INFO] message`,
/// or a session banner, `--- Session 2026-09-20T09:00:00.000Z ---`.
///
/// The file is stamped in UTC; `local` shows the stamps in the local zone instead, which a
/// capture turns off so it reads the same anywhere. A banner shows its date and time, and a
/// line only its time, since the banner above it already says which day it is.
///
/// Anything else is kept as it stands, so a stack trace or a wrapped line is still readable
/// rather than being dropped for not matching.
pub fn parse(line: &str, local: bool) -> Line {
    if let Some(rest) = line.strip_prefix("--- Session ") {
        let stamp = rest.trim_end_matches(['-', ' ']);
        let message = match clock::parse_iso(stamp) {
            Some(ms) => {
                let (date, mut time) = clock::readable(ms, local);
                // A banner reads to the second; the milliseconds would only be noise.
                time.truncate(time.len() - 4);
                format!("Session {date} {time}")
            }
            None => format!("Session {stamp}"),
        };
        return Line {
            timestamp: String::new(),
            level: Level::Info,
            message,
            banner: true,
        };
    }
    let stamped = line
        .split_once(' ')
        .and_then(|(stamp, rest)| Some((clock::parse_iso(stamp)?, rest)));
    let Some((ms, rest)) = stamped else {
        return Line {
            timestamp: String::new(),
            level: Level::Info,
            message: line.to_string(),
            banner: false,
        };
    };
    let (level, message) = match rest
        .strip_prefix('[')
        .and_then(|tagged| tagged.split_once(']'))
    {
        Some(("INFO", message)) => (Level::Info, message.trim_start()),
        Some(("WARN", message)) => (Level::Warning, message.trim_start()),
        Some(("ERROR", message)) => (Level::Error, message.trim_start()),
        _ => (Level::Info, rest),
    };
    Line {
        timestamp: clock::readable(ms, local).1,
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

/// Breaks `text` into lines of at most `columns` characters, as `white-space: pre-wrap` and
/// `word-break: break-word` do: at the last space that fits, or mid-word when none does.
///
/// Each line is a slice of `text`, so its offset into the message is known. A space that a
/// line breaks at stays at the end of that line, where it cannot be seen.
pub fn wrap(text: &str, columns: usize) -> Vec<&str> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    let mut rest = text;
    while let Some((cut, _)) = rest.char_indices().nth(columns) {
        let end = if rest[cut..].starts_with(' ') {
            cut + 1
        } else {
            match rest[..cut].rfind(' ') {
                Some(space) if space > 0 => space + 1,
                _ => cut,
            }
        };
        lines.push(&rest[..end]);
        rest = &rest[end..];
    }
    lines.push(rest);
    lines
}

/// What the window remembers between frames.
pub struct LogWindow {
    pub open: bool,
    pub show_info: bool,
    pub show_warning: bool,
    pub show_error: bool,
    lines: Vec<Line>,
    /// Shows stamps in the local zone rather than in UTC.
    local: bool,
    /// Whether the window follows the file. A window given its lines directly, as the
    /// capture scene does, shows those instead.
    follows_file: bool,
    /// How many bytes of the file have been read, so only new text is parsed.
    read: u64,
}

impl Default for LogWindow {
    fn default() -> Self {
        Self {
            open: false,
            show_info: true,
            show_warning: true,
            show_error: true,
            lines: Vec::new(),
            local: true,
            follows_file: true,
            read: 0,
        }
    }
}

impl LogWindow {
    /// Opens the window, reading the file from the beginning.
    pub fn show(&mut self) {
        self.open = true;
        self.restart();
        self.refresh();
    }

    /// Forgets what has been read, so the file is read again from its start.
    fn restart(&mut self) {
        self.lines.clear();
        self.read = 0;
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
            self.restart();
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        // Only the tail is new; the part already parsed is measured in bytes.
        let fresh = text.get(self.read as usize..).unwrap_or(&text);
        for line in fresh.lines() {
            if !line.trim().is_empty() {
                self.lines.push(parse(line, self.local));
            }
        }
        self.read = text.len() as u64;
    }

    /// Opens the window on lines given directly, for a capture that must not depend on
    /// whatever the machine's log happens to hold, or on the zone it is in.
    pub fn seed(&mut self, lines: &[&str]) {
        self.local = false;
        self.lines = lines.iter().map(|line| parse(line, false)).collect();
        self.follows_file = false;
        self.open = true;
    }

    /// Empties the log, on disk as well as in the window, as the Clear button asks.
    pub fn clear(&mut self) {
        if self.follows_file {
            crate::logging::clear_file();
        }
        self.restart();
    }

    /// The lines the filters let through.
    pub fn visible(&self) -> impl Iterator<Item = &Line> {
        self.lines
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
        // Long lines wrap, so the list only ever scrolls up and down.
        ui.child("log-lines", [0.0, 0.0], false, |ui| {
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

/// The filter row across the top: a band of its own, ruled off from the lines below.
fn toolbar(ui: &mut Ui, window: &mut LogWindow, reveal: &mut bool) {
    use theme::log::{TOOLBAR_GAP, TOOLBAR_PADDING_Y, TOOLBAR_ROW_HEIGHT};
    let origin = ui.cursor_screen_position();
    let left = ui.window_position()[0];
    let right = left + ui.window_size()[0];
    // The band runs edge to edge, so it is placed from the window rather than the cursor,
    // which the window's own padding has already moved in.
    let top = origin[1] - TOOLBAR_PADDING_Y;
    let bottom = origin[1] + TOOLBAR_ROW_HEIGHT + TOOLBAR_PADDING_Y;
    let list = ui.draw_list();
    list.rect(
        [left, top],
        [right, bottom],
        theme::log::TOOLBAR_BG,
        0.0,
        Rounding::None,
    );
    list.line(
        [left, bottom - 0.5],
        [right, bottom - 0.5],
        theme::log::TOOLBAR_BORDER,
        1.0,
    );

    // The items keep to the window's padding, so they line up with the lines below.
    let mut x = origin[0];
    // `text-transform: uppercase; letter-spacing: 0.06em`.
    let tracking = 0.06 * f32::from(theme::face::SMALL_MONO.size);
    let label_size = controls::measure_tracked(ui, theme::face::SMALL_MONO, "FILTER", tracking);
    controls::draw_tracked_text(
        ui,
        [x, origin[1] + (TOOLBAR_ROW_HEIGHT - label_size[1]) / 2.0],
        theme::log::LABEL,
        theme::face::SMALL_MONO,
        "FILTER",
        tracking,
    );
    x += label_size[0] + TOOLBAR_GAP;
    let filters = [
        ("Info", theme::log::INFO_FILTER, &mut window.show_info),
        ("Warning", theme::log::WARNING, &mut window.show_warning),
        ("Error", theme::log::ERROR, &mut window.show_error),
    ];
    for (label, color, value) in filters {
        ui.set_cursor_screen_position([x, origin[1]]);
        x += filter(ui, label, color, value) + TOOLBAR_GAP;
    }

    // `margin-left: auto` pushes the buttons to the far end of the row.
    let mut x = origin[0] + ui.content_region_available()[0];
    for (label, action) in [("Clear", 0), ("Open File", 1)] {
        x -= button_width(ui, label);
        ui.set_cursor_screen_position([x, origin[1]]);
        if button(ui, label) {
            match action {
                0 => window.clear(),
                _ => *reveal = true,
            }
        }
        x -= 8.0;
    }
    ui.set_cursor_screen_position([origin[0], bottom]);
    // `#log-list` has four pixels of padding above its first line.
    ui.dummy([1.0, 4.0]);
}

/// One level filter: a checkbox and its label, clickable as one, as a `<label>` is.
/// Returns the width it took.
fn filter(ui: &mut Ui, label: &str, color: bite_imgui::Color, value: &mut bool) -> f32 {
    use theme::log::{CHECK, CHECK_GAP, CHECK_SIZE, TOOLBAR_ROW_HEIGHT};
    let origin = ui.cursor_screen_position();
    let text = controls::measure(ui, theme::face::SMALL_MONO, label);
    let width = CHECK_SIZE + CHECK_GAP + text[0];
    if ui.invisible_button(
        &format!("##log-filter-{label}"),
        [width, TOOLBAR_ROW_HEIGHT],
    ) {
        *value = !*value;
    }
    if ui.item_hovered() {
        ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
    }
    let list = ui.draw_list();
    let box_min = [
        origin[0],
        origin[1] + ((TOOLBAR_ROW_HEIGHT - CHECK_SIZE) / 2.0).round(),
    ];
    let box_max = [box_min[0] + CHECK_SIZE, box_min[1] + CHECK_SIZE];
    if *value {
        list.rect(box_min, box_max, CHECK, 2.0, Rounding::All);
        let at = |u: f32, v: f32| [box_min[0] + CHECK_SIZE * u, box_min[1] + CHECK_SIZE * v];
        list.line(at(0.24, 0.52), at(0.42, 0.70), theme::TEXT_BRIGHT, 1.5);
        list.line(at(0.42, 0.70), at(0.76, 0.30), theme::TEXT_BRIGHT, 1.5);
    } else {
        list.rect_outline(box_min, box_max, CHECK, 2.0, Rounding::All, 1.0);
    }
    list.text_with_face(
        [
            box_max[0] + CHECK_GAP,
            origin[1] + (TOOLBAR_ROW_HEIGHT - text[1]) / 2.0,
        ],
        color,
        theme::face::SMALL_MONO,
        label,
    );
    width
}

/// `letter-spacing: 0.04em` on the toolbar buttons.
fn button_tracking() -> f32 {
    0.04 * f32::from(theme::face::SMALL_MONO.size)
}

fn button_width(ui: &Ui, label: &str) -> f32 {
    controls::measure_tracked(ui, theme::face::SMALL_MONO, label, button_tracking())[0]
        + theme::log::BUTTON_PADDING_X * 2.0
}

/// The viewer's Clear button: outlined, with no fill, brightening on hover.
fn button(ui: &mut Ui, label: &str) -> bool {
    use theme::log::{BUTTON_RADIUS, TOOLBAR_ROW_HEIGHT};
    let origin = ui.cursor_screen_position();
    let width = button_width(ui, label);
    let pressed = ui.invisible_button(&format!("##log-{label}"), [width, TOOLBAR_ROW_HEIGHT]);
    let hovered = ui.item_hovered();
    if hovered {
        ui.set_mouse_cursor(bite_imgui::MouseCursor::Hand);
    }
    let (border, text_color) = if hovered {
        (
            theme::log::BUTTON_BORDER_HOVER,
            theme::log::BUTTON_TEXT_HOVER,
        )
    } else {
        (theme::log::BUTTON_BORDER, theme::log::BUTTON_TEXT)
    };
    ui.draw_list().rect_outline(
        origin,
        [origin[0] + width, origin[1] + TOOLBAR_ROW_HEIGHT],
        border,
        BUTTON_RADIUS,
        Rounding::All,
        1.0,
    );
    let text = controls::measure_tracked(ui, theme::face::SMALL_MONO, label, button_tracking());
    controls::draw_tracked_text(
        ui,
        [
            origin[0] + theme::log::BUTTON_PADDING_X,
            origin[1] + (TOOLBAR_ROW_HEIGHT - text[1]) / 2.0,
        ],
        text_color,
        theme::face::SMALL_MONO,
        label,
        button_tracking(),
    );
    pressed
}

/// One line: its time, its level and its message, in three columns. A message too long for
/// the window wraps onto further lines inside its own column.
fn row(ui: &mut Ui, line: &Line) {
    let origin = ui.cursor_screen_position();
    // `line-height: 1.55` in the viewer's stylesheet, measured on the face the row uses
    // rather than whatever font happens to be current.
    let text = controls::measure(ui, theme::face::SMALL_MONO, "Ag")[1];
    let height = (text * theme::log::LINE_HEIGHT).round();
    let baseline = origin[1] + ((height - text) / 2.0).round();
    let width = ui.content_region_available()[0].max(1.0);
    let list = ui.draw_list();
    if line.banner {
        hover(ui, origin, width, height);
        list.text_with_face(
            [origin[0], baseline],
            theme::log::LABEL,
            theme::face::SMALL_MONO,
            &line.message,
        );
        ui.dummy([width, height]);
        return;
    }
    // The columns line up across every row, so each is measured from a full stamp rather
    // than from the one on this line.
    let stamp_width = list.measure(theme::face::SMALL_MONO, "00:00:00.000")[0];
    let message_x = origin[0]
        + stamp_width
        + theme::log::COLUMN_GAP
        + theme::log::BADGE_WIDTH
        + theme::log::COLUMN_GAP;
    // The face is monospaced, so a column count is all the wrapping needs.
    let advance = list.measure(theme::face::SMALL_MONO, "0")[0].max(1.0);
    let columns = ((origin[0] + width - message_x) / advance).floor().max(1.0) as usize;
    let wrapped = wrap(&line.message, columns);
    let total = height * wrapped.len() as f32;
    hover(ui, origin, width, total);

    let list = ui.draw_list();
    list.text_with_face(
        [origin[0], baseline],
        theme::log::TIMESTAMP,
        theme::face::SMALL_MONO,
        &line.timestamp,
    );
    list.text_with_face(
        [origin[0] + stamp_width + theme::log::COLUMN_GAP, baseline],
        badge_color(line),
        theme::face::SMALL_MONO,
        line.level.label(),
    );
    // The tag is coloured by where it falls in the message, so it stays coloured even in the
    // unlikely case that a wrap splits it.
    let tag_end = split_tag(&line.message).0.map_or(0, str::len);
    let start = line.message.as_ptr() as usize;
    for (index, piece) in wrapped.iter().enumerate() {
        let y = baseline + height * index as f32;
        let offset = piece.as_ptr() as usize - start;
        let split = tag_end.saturating_sub(offset).min(piece.len());
        let (tag, rest) = piece.split_at(split);
        let mut x = message_x;
        if !tag.is_empty() {
            list.text_with_face([x, y], theme::log::TAG, theme::face::SMALL_MONO, tag);
            x += list.measure(theme::face::SMALL_MONO, tag)[0];
        }
        list.text_with_face([x, y], message_color(line), theme::face::SMALL_MONO, rest);
    }
    ui.dummy([width, total]);
}

/// The hover highlight behind a row, drawn before its text.
fn hover(ui: &Ui, origin: Vec2, width: f32, height: f32) {
    let max = [origin[0] + width, origin[1] + height];
    if controls::point_in(ui.mouse_position(), origin, max) {
        ui.draw_list().rect(
            [origin[0] - theme::log::ROW_PADDING_X, origin[1]],
            max,
            theme::log::ROW_HOVER,
            0.0,
            Rounding::None,
        );
    }
}

/// The size the window opens at, for the capture scene.
pub const DEFAULT_SIZE: Vec2 = [760.0, 460.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_line_splits_into_its_time_level_and_message() {
        let line = parse("2026-09-20T12:34:56.789Z [WARN] [magick] slow spawn", false);
        assert_eq!(line.timestamp, "12:34:56.789");
        assert_eq!(line.level, Level::Warning);
        assert_eq!(line.message, "[magick] slow spawn");
        assert!(!line.banner);
    }

    #[test]
    fn a_session_banner_shows_its_date_and_time() {
        let banner = parse("--- Session 2026-09-20T23:59:00.250Z ---", false);
        assert!(banner.banner);
        assert_eq!(banner.message, "Session 2026-09-20 23:59:00");
    }

    #[test]
    fn a_line_in_no_particular_shape_is_kept_as_it_stands() {
        let line = parse("    at some::frame (src/lib.rs:12)", true);
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
    fn a_long_message_wraps_at_the_last_space_that_fits() {
        assert_eq!(wrap("one two three", 8), ["one two ", "three"]);
        assert_eq!(wrap("one two three", 7), ["one two ", "three"]);
        assert_eq!(wrap("short", 8), ["short"]);
        assert_eq!(wrap("", 8), [""]);
    }

    #[test]
    fn a_word_longer_than_the_line_is_broken_where_it_runs_out() {
        assert_eq!(
            wrap("/Applications/Bite.app/Contents", 12),
            ["/Application", "s/Bite.app/C", "ontents"]
        );
        // Columns count characters, not bytes.
        assert_eq!(
            wrap("caf\u{e9}s caf\u{e9}s", 5),
            ["caf\u{e9}s ", "caf\u{e9}s"]
        );
    }

    #[test]
    fn the_filters_decide_which_lines_show() {
        let mut window = LogWindow::default();
        window.seed(&[
            "2026-09-20T12:00:00.000Z [INFO] one",
            "2026-09-20T12:00:01.000Z [ERROR] two",
        ]);
        assert_eq!(window.visible().count(), 2);
        window.show_info = false;
        let shown: Vec<&str> = window.visible().map(|line| line.message.as_str()).collect();
        assert_eq!(shown, ["two"]);
    }

    #[test]
    fn clearing_empties_the_window() {
        // A seeded window does not follow the file, so this leaves the real log alone.
        let mut window = LogWindow::default();
        window.seed(&["2026-09-20T12:00:00.000Z [INFO] before"]);
        window.clear();
        assert_eq!(window.visible().count(), 0);
    }
}
