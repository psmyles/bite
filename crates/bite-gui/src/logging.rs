//! The session log, matching the Electron logger's file and format.
use std::{
    collections::VecDeque,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

/// One recorded line.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub timestamp: String,
    pub level: Level,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// The ring buffer the log window reads, capped as the Electron logger caps it.
const MAX_ENTRIES: usize = 1000;

fn buffer() -> &'static Mutex<VecDeque<Entry>> {
    static BUFFER: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// The open log file, kept between lines, with the path it was opened for.
///
/// Every line used to open the file, append to it and close it again. A run that logs once
/// per picture paid that for each one; the path travels with the handle so that anything
/// repointing `BITE_LOG_PATH`, such as a test, still lands in the file it asked for.
fn writer() -> &'static Mutex<Option<(PathBuf, File)>> {
    static WRITER: OnceLock<Mutex<Option<(PathBuf, File)>>> = OnceLock::new();
    WRITER.get_or_init(|| Mutex::new(None))
}

/// Appends through the held handle, opening it the first time and after a path change.
///
/// The file is written unbuffered, so the log window tailing it sees each line as it
/// lands. Nothing here reports a failure: a log that cannot be written is not worth
/// interrupting the editor over, and the window still reads the buffer.
fn append(path: &Path, line: &str) {
    let Ok(mut held) = writer().lock() else {
        return;
    };
    if held.as_ref().is_none_or(|(open, _)| open != path) {
        *held = File::options()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(|file| (path.to_owned(), file));
    }
    if let Some((_, file)) = held.as_mut() {
        let _ = writeln!(file, "{line}");
    }
}

/// Where the log file lives on this platform.
pub fn log_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("BITE_LOG_PATH") {
        return Some(PathBuf::from(explicit));
    }
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Logs"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local/state"))
        });
    root.map(|root| root.join("BITE").join("bite.log"))
}

/// Opens the log for this session, writing the banner the Electron logger writes.
pub fn start_session() {
    let Some(path) = log_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    append(&path, &format!("--- Session {} ---", timestamp_iso()));
}

/// Records a line, both in the buffer and in the file.
pub fn log(level: Level, message: impl Into<String>) {
    let entry = Entry {
        timestamp: timestamp_clock(),
        level,
        message: message.into(),
    };
    if let Ok(mut entries) = buffer().lock() {
        while entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry.clone());
    }
    if let Some(path) = log_path() {
        append(
            &path,
            &format!(
                "{} [{}] {}",
                entry.timestamp,
                entry.level.label(),
                entry.message
            ),
        );
    }
}

pub fn info(message: impl Into<String>) {
    log(Level::Info, message);
}

pub fn warn(message: impl Into<String>) {
    log(Level::Warning, message);
}

pub fn error(message: impl Into<String>) {
    log(Level::Error, message);
}

/// Every recorded line, newest last.
pub fn entries() -> Vec<Entry> {
    buffer()
        .lock()
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut entries) = buffer().lock() {
        entries.clear();
    }
}

/// Seconds since the epoch, split into a wall clock time.
fn parts() -> (u64, u64, u64, u64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total = now.as_secs();
    let milliseconds = u64::from(now.subsec_millis());
    (total / 3600 % 24, total / 60 % 60, total % 60, milliseconds)
}

/// The clock stamp each line carries, as hours, minutes, seconds and milliseconds.
pub fn timestamp_clock() -> String {
    let (hours, minutes, seconds, milliseconds) = parts();
    format!("{hours:02}:{minutes:02}:{seconds:02}.{milliseconds:03}")
}

/// A full stamp for the session banner.
pub fn timestamp_iso() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Days since the epoch converted to a civil date.
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    let (hours, minutes, secs, _) = parts();
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{secs:02}Z")
}

/// Converts a day count since the epoch into a calendar date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_carry_the_labels_the_log_window_shows() {
        assert_eq!(Level::Info.label(), "INFO");
        assert_eq!(Level::Warning.label(), "WARN");
        assert_eq!(Level::Error.label(), "ERROR");
    }

    #[test]
    fn the_clock_stamp_has_hours_minutes_seconds_and_milliseconds() {
        let stamp = timestamp_clock();
        assert_eq!(stamp.len(), 12);
        assert_eq!(stamp.as_bytes()[2], b':');
        assert_eq!(stamp.as_bytes()[5], b':');
        assert_eq!(stamp.as_bytes()[8], b'.');
    }

    #[test]
    fn the_epoch_converts_to_the_first_of_january_nineteen_seventy() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(31), (1970, 2, 1));
    }

    #[test]
    fn entries_are_recorded_in_order() {
        // The test writes to a scratch file so the real session log is left alone.
        let scratch = std::env::temp_dir().join("bite-logging-test.log");
        unsafe { std::env::set_var("BITE_LOG_PATH", &scratch) };
        clear();
        log(Level::Info, "first");
        log(Level::Error, "second");
        let entries = entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "first");
        assert_eq!(entries[1].level, Level::Error);
        clear();
        let _ = std::fs::remove_file(&scratch);
        unsafe { std::env::remove_var("BITE_LOG_PATH") };
    }
}
