//! What Debug > Performance Timers records, from `src/main/pipeline/timing.ts`.
//!
//! Electron times every image of an import and every image of a batch, because it spawns a
//! process for each one. The native pipeline composes a whole chain into one command, so the
//! number worth watching is how many processes a piece of work took at all, and that is
//! reported beside the times.
//!
//! The toggle is read from background threads, so the flag lives here rather than on the
//! editor, and every report goes to the session log where the log window shows it.
use crate::logging;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    logging::info(format!(
        "[timings] {}",
        if enabled { "enabled" } else { "disabled" }
    ));
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// How an import went, in the terms the thumbnail cache counts it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Import {
    pub images: usize,
    /// Thumbnails that were already on disk and still current.
    pub cached: usize,
    /// Thumbnails that had to be made, or remade because the file had changed.
    pub generated: usize,
    pub processes: usize,
    pub milliseconds: u128,
}

/// How one preview render went.
#[derive(Clone, Copy, Debug, Default)]
pub struct Preview {
    pub nodes: usize,
    /// Whether the worker had this exact result already.
    pub reused: bool,
    pub processes: usize,
    pub milliseconds: u128,
}

/// How a batch run went.
#[derive(Clone, Copy, Debug, Default)]
pub struct Run {
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub processes: usize,
    pub milliseconds: u128,
}

/// The average of `total` over `count`, or nothing when there was nothing to average.
fn per(total: u128, count: usize) -> String {
    if count == 0 {
        return "-".into();
    }
    format!("{}ms", total / count as u128)
}

/// The two lines an import reports.
pub fn import_lines(import: Import) -> [String; 2] {
    [
        format!(
            "[timings] Import - {} image(s) in {}ms, {} each",
            import.images,
            import.milliseconds,
            per(import.milliseconds, import.images)
        ),
        format!(
            "[timings]   cached: {}  generated: {}  magick processes: {}",
            import.cached, import.generated, import.processes
        ),
    ]
}

/// The line a preview reports.
pub fn preview_line(preview: Preview) -> String {
    format!(
        "[timings] Preview - {} node(s) in {}ms, {} magick process(es){}",
        preview.nodes,
        preview.milliseconds,
        preview.processes,
        if preview.reused { ", reused" } else { "" }
    )
}

/// The two lines a run reports.
pub fn run_lines(run: Run) -> [String; 2] {
    [
        format!(
            "[timings] Run - {} processed, {} skipped, {} failed in {}ms, {} each",
            run.processed,
            run.skipped,
            run.failed,
            run.milliseconds,
            per(run.milliseconds, run.processed)
        ),
        format!("[timings]   magick processes: {}", run.processes),
    ]
}

pub fn report_import(import: Import) {
    if enabled() {
        for line in import_lines(import) {
            logging::info(line);
        }
    }
}

pub fn report_preview(preview: Preview) {
    if enabled() {
        logging::info(preview_line(preview));
    }
}

pub fn report_run(run: Run) {
    if enabled() {
        for line in run_lines(run) {
            logging::info(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_average_needs_something_to_average() {
        assert_eq!(per(900, 3), "300ms");
        assert_eq!(per(900, 0), "-");
    }

    #[test]
    fn an_import_reports_its_time_and_what_it_cost() {
        let lines = import_lines(Import {
            images: 26,
            cached: 24,
            generated: 2,
            processes: 1,
            milliseconds: 520,
        });
        assert_eq!(lines[0], "[timings] Import - 26 image(s) in 520ms, 20ms each");
        assert_eq!(
            lines[1],
            "[timings]   cached: 24  generated: 2  magick processes: 1"
        );
    }

    #[test]
    fn a_reused_preview_says_so_and_charges_nothing() {
        let line = preview_line(Preview {
            nodes: 3,
            reused: true,
            processes: 0,
            milliseconds: 0,
        });
        assert_eq!(
            line,
            "[timings] Preview - 3 node(s) in 0ms, 0 magick process(es), reused"
        );
    }

    #[test]
    fn a_run_reports_what_it_did_and_how_many_processes_it_took() {
        let lines = run_lines(Run {
            processed: 8,
            skipped: 1,
            failed: 0,
            processes: 8,
            milliseconds: 2400,
        });
        assert_eq!(
            lines[0],
            "[timings] Run - 8 processed, 1 skipped, 0 failed in 2400ms, 300ms each"
        );
        assert_eq!(lines[1], "[timings]   magick processes: 8");
    }

    #[test]
    fn nothing_is_recorded_until_the_menu_turns_it_on() {
        // The flag is global, so it is put back the way it was found.
        let before = enabled();
        set_enabled(false);
        assert!(!enabled());
        set_enabled(true);
        assert!(enabled());
        set_enabled(before);
    }
}
