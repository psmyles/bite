//! Startup instrumentation: where the milliseconds before the first frame go.
//!
//! Inert unless `BITE_TIMING` is set. `BITE_TIMING=1` writes to stderr; anything else is a path
//! that each line is appended to, which is what a release build needs - it is a GUI-subsystem
//! binary with no console of its own, and a redirected pipe is itself slow enough to show up in
//! a measurement this small.
//!
//! Every stamp is measured from the **kernel's process-creation time**, not from an `Instant`
//! taken at the top of `main`. The loader, the CRT and the static initializers run before `main`
//! and the user pays for them; a breakdown that starts inside the process cannot see them and so
//! does not add up to what the user experiences.
//!
//! Each line also carries the working set and the commit charge at that moment, because the two
//! things this migration is measured on - time to first frame and footprint - move together, and
//! reading them from one place keeps them on the same clock.
//!
//! With `BITE_EXIT_AFTER_FIRST_FRAME` set, the first presented frame stamps itself, waits 1.5 s
//! for the allocator and the driver to settle, stamps again and exits. That is the whole harness
//! protocol: `scripts/ttfp.ps1` launches, waits, and parses those two lines out of the sink.

use std::sync::OnceLock;

/// Where timing lines go, decided once. `None` is the normal case: no sink, no cost.
enum Sink {
    Stderr,
    File(std::path::PathBuf),
}

fn sink() -> Option<&'static Sink> {
    static SINK: OnceLock<Option<Sink>> = OnceLock::new();
    SINK.get_or_init(|| {
        let value = std::env::var_os("BITE_TIMING")?;
        Some(if value == "1" {
            Sink::Stderr
        } else {
            Sink::File(value.into())
        })
    })
    .as_ref()
}

/// True when a harness asked the process to leave once it has shown one frame.
pub fn exit_after_first_frame() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("BITE_EXIT_AFTER_FIRST_FRAME").is_some())
}

/// Appends one stamped line for `step`: elapsed since process creation, working set, commit.
///
/// The format is fixed - the harness parses it - and reads as prose, so a developer watching
/// stderr gets the breakdown without a tool:
///
/// ```text
///    117.43 ms   ws    41.2 MB   commit    58.9 MB   Editor::new
/// ```
pub fn report(step: &str) {
    let Some(sink) = sink() else { return };
    let (working_set, commit) = memory();
    let line = format!(
        "{:>9.2} ms   ws {:>7.1} MB   commit {:>7.1} MB   {step}",
        ms_since_start(),
        working_set as f64 / (1024.0 * 1024.0),
        commit as f64 / (1024.0 * 1024.0),
    );
    match sink {
        Sink::Stderr => eprintln!("bite: {line}"),
        Sink::File(path) => {
            use std::io::Write as _;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

/// Stamps the frame that has just been handed to the display and, when a harness asked for it,
/// waits for the process to settle, stamps that too, and leaves.
///
/// The settled stamp is the one the footprint target is read from. Straight after the first frame
/// the driver still has upload staging in flight and the allocator has returned nothing, so the
/// working set at that instant flatters neither stack and compares neither honestly.
pub fn stamp_first_frame() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    report("FIRST FRAME");
    if exit_after_first_frame() {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        report("settled");
        std::process::exit(0);
    }
}

/// Milliseconds from the kernel's process-creation time to now.
#[cfg(windows)]
pub fn ms_since_start() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    // FILETIME is 100 ns ticks since 1601-01-01; the Unix epoch is this many ticks later.
    const UNIX_EPOCH_AS_FILETIME: u64 = 116_444_736_000_000_000;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetProcessTimes(
            process: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    let mut created = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    // SAFETY: the pseudo-handle from GetCurrentProcess is always valid, and the four out-params
    // are ours, correctly sized and initialized.
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exit,
            &mut kernel,
            &mut user,
        ) != 0
    };
    if !ok {
        return f64::NAN;
    }
    let created = (u64::from(created.high) << 32) | u64::from(created.low);
    // `SystemTime::now()` is GetSystemTimePreciseAsFileTime on Windows - the same clock.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let now = (now.as_nanos() / 100) as u64 + UNIX_EPOCH_AS_FILETIME;
    now.saturating_sub(created) as f64 / 10_000.0
}

/// Milliseconds from the kernel's process-creation time to now.
///
/// `proc_pidinfo(PROC_PIDTBSDINFO)` is the true equivalent of the Windows arm's
/// `GetProcessTimes`: the kernel's own record of when this process began, so - unlike an
/// `Instant` taken at the top of `main` - it includes dyld, which on a first, cold launch of a
/// bundle is a real share of the number. Both halves of the subtraction read the same wall
/// clock: `pbi_start_tv*` is the `gettimeofday` value at exec, and so is `SystemTime::now`.
#[cfg(target_os = "macos")]
pub fn ms_since_start() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    // SAFETY: the buffer is a `proc_bsdinfo` and the size passed is its own; `proc_pidinfo`
    // writes at most that many bytes and reports how many it wrote.
    let (info, wrote) = unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let wrote = libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::addr_of_mut!(info).cast(),
            std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int,
        );
        (info, wrote)
    };
    if wrote != std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int {
        // No origin means no measurement, and a plausible-looking number would be worse than
        // none - see the arm below.
        return f64::NAN;
    }
    let started = info.pbi_start_tvsec as f64 * 1e3 + info.pbi_start_tvusec as f64 / 1e3;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1e3;
    now - started
}

/// No process-creation clock here, and no honest way to fake one.
///
/// A `OnceLock<Instant>` initialised inside the measurement would report ~0 ms on every run - a
/// number that looks like a result. NaN is what "not measured on this platform" should look like,
/// and the harness rejects it. The editor ships on Windows and macOS, both of which have a real
/// arm above.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn ms_since_start() -> f64 {
    f64::NAN
}

/// This process's working set and commit charge (private bytes), in bytes.
#[cfg(windows)]
fn memory() -> (u64, u64) {
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCountersEx {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut MemoryCountersEx, cb: u32)
            -> i32;
    }

    let size = std::mem::size_of::<MemoryCountersEx>() as u32;
    let mut counters = MemoryCountersEx {
        cb: size,
        ..Default::default()
    };
    // SAFETY: `cb` names the size of the buffer being passed, which is the call's contract, and
    // the pseudo-handle is always valid.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) != 0 };
    if ok {
        (
            counters.working_set_size as u64,
            counters.private_usage as u64,
        )
    } else {
        (0, 0)
    }
}

/// This process's resident size and physical footprint, in bytes.
///
/// The pair the Windows arm reports is working set and commit charge. The nearest honest
/// equivalents here are `ri_resident_size` - the pages actually in RAM, as the working set is -
/// and `ri_phys_footprint`, which is what Activity Monitor calls Memory and what Apple counts a
/// process against: resident memory plus its compressed pages and IOKit mappings. Neither is the
/// same quantity as its Windows counterpart, so the two platforms' numbers are comparable in
/// shape rather than exactly.
#[cfg(target_os = "macos")]
fn memory() -> (u64, u64) {
    // `rusage_info_t` is a `void *` typedef and the parameter is declared `rusage_info_t *`, but
    // the pointer passed is the *destination*, not a pointer to one: the call writes the selected
    // struct at that address. So what goes in is the struct's own address cast through the
    // typedef, which is what the C callers do. Passing the address of a `rusage_info_t` variable
    // instead type-checks and smashes the stack.
    //
    // SAFETY: the buffer is a `rusage_info_v2`, which is the struct `RUSAGE_INFO_V2` selects, so
    // the call writes exactly its size into storage that large.
    let (info, ok) = unsafe {
        let mut info: libc::rusage_info_v2 = std::mem::zeroed();
        let ok = libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V2,
            std::ptr::addr_of_mut!(info).cast::<libc::rusage_info_t>(),
        ) == 0;
        (info, ok)
    };
    if ok {
        (info.ri_resident_size, info.ri_phys_footprint)
    } else {
        (0, 0)
    }
}

/// No per-process counters here; the lines still carry their timings.
#[cfg(not(any(windows, target_os = "macos")))]
fn memory() -> (u64, u64) {
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clock must be running and plausible: a test process is milliseconds to seconds old,
    /// never zero - which is what a broken arm reports - and never hours.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn the_process_clock_reports_a_plausible_age() {
        let ms = ms_since_start();
        assert!(ms.is_finite(), "the process-creation clock reported {ms}");
        assert!(
            ms > 0.0 && ms < 3_600_000.0,
            "a process age of {ms} ms is not plausible"
        );
    }

    /// Working set and commit must both read as something for a live process.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn the_memory_counters_report_something() {
        let (working_set, commit) = memory();
        assert!(working_set > 0, "working set read as zero");
        assert!(commit > 0, "commit read as zero");
    }

    /// With no sink set, reporting is a no-op that must not panic or write anywhere.
    #[test]
    fn reporting_without_a_sink_is_inert() {
        report("a step nobody asked to see");
    }
}
