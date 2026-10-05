pub mod header;
pub mod import;
use bite_core::execution::ImageHost;
use bite_expr::{definition::metadata_types, Context, Type, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub invalidations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    size: u64,
    modified_ns: u128,
}

#[derive(Clone)]
struct CachedMetadata {
    stamp: FileStamp,
    inserted: Instant,
    value: Context,
}

/// What a command measured, and the state of every file it measured it from.
#[derive(Clone)]
struct CachedCapture {
    sources: Vec<(PathBuf, FileStamp)>,
    inserted: Instant,
    value: String,
}

pub struct Magick {
    pub binary: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub cancelled: Arc<AtomicBool>,
    pub timeout: Duration,
    pub cache_ttl: Duration,
    shared: Arc<Shared>,
}

/// What every host cloned from one `discover` has in common.
///
/// A batch is split across workers and a workflow may have several output nodes, so the
/// measurements and the counts have to outlive any one host: a file read for the first
/// output is still read for the second, whichever worker happened to reach it.
#[derive(Default)]
struct Shared {
    processes: AtomicUsize,
    metadata: Mutex<BTreeMap<(PathBuf, bool), CachedMetadata>>,
    metadata_stats: Mutex<CacheStats>,
    /// What each analysis command reported. A workflow with an image output and a report
    /// runs its chain once per output, and asking the same question of the same file
    /// twice costs as much as asking it the first time.
    captures: Mutex<BTreeMap<Vec<String>, CachedCapture>>,
    capture_stats: Mutex<CacheStats>,
    current_child_memory: AtomicU64,
    peak_child_memory: AtomicU64,
}

/// How many measurements are kept before the expired ones are cleared out. A run measures
/// once per file, so this holds several large batches; the interface keeps one host for
/// the whole session and would otherwise grow without bound.
const CAPTURE_CACHE_LIMIT: usize = 8192;

#[cfg(windows)]
fn child_working_set(child: &std::process::Child) -> u64 {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[repr(C)]
    struct ProcessMemoryCounters {
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
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    // SAFETY: the child handle is valid for the duration of this call and the
    // structure matches Windows PROCESS_MEMORY_COUNTERS.
    let ok =
        unsafe { GetProcessMemoryInfo(child.as_raw_handle().cast(), &mut counters, counters.cb) };
    if ok == 0 {
        0
    } else {
        counters.working_set_size as u64
    }
}

#[cfg(not(windows))]
fn child_working_set(_child: &std::process::Child) -> u64 {
    0
}

/// How many threads one of `workers` concurrent ImageMagick pipelines should take.
pub fn thread_share(workers: usize) -> usize {
    let cores = thread::available_parallelism().map_or(1, |count| count.get());
    (cores / workers.max(1)).max(1)
}

impl Magick {
    pub fn discover(cancelled: Arc<AtomicBool>) -> Self {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("magick/magick.exe"));
                candidates.push(dir.join("ImageMagick/magick.exe"));
                // The macOS bundle: `Contents/MacOS` holds the executables and everything else
                // lives beside them in `Contents/Resources`, which is where the packaging script
                // puts the tree `scripts/bundle-magick-mac.sh` built.
                candidates.push(dir.join("../Resources/magick/bin/magick"));
                candidates.push(dir.join("../Resources/ImageMagick/bin/magick"));
                candidates.push(dir.join("magick/bin/magick"));
            }
        }
        candidates.push(PathBuf::from("resources/win/magick/magick.exe"));
        candidates.push(PathBuf::from("resources/mac/magick/bin/magick"));
        let binary = std::env::var_os("BITE_MAGICK")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                candidates
                    .into_iter()
                    .find(|p| p.is_file())
                    .unwrap_or_else(|| "magick".into())
            });
        #[allow(unused_mut)] // macOS bundle relocation adds environment entries.
        let mut environment = BTreeMap::from([("MAGICK_THREAD_LIMIT".into(), "1".into())]);
        #[cfg(target_os = "macos")]
        if let Some(root) = binary.parent().and_then(Path::parent) {
            if root.join("lib/ImageMagick").is_dir() {
                environment.insert("MAGICK_HOME".into(), root.to_string_lossy().into());
                let mut config = Vec::new();
                for parent in [
                    root.join("lib/ImageMagick"),
                    root.join("etc"),
                    root.join("share"),
                ] {
                    if let Ok(entries) = fs::read_dir(parent) {
                        for e in entries.flatten() {
                            let name = e.file_name().to_string_lossy().into_owned();
                            let p = e.path();
                            if name.starts_with("modules-") {
                                environment.insert(
                                    "MAGICK_CODER_MODULE_PATH".into(),
                                    p.join("coders").to_string_lossy().into(),
                                );
                                environment.insert(
                                    "MAGICK_FILTER_MODULE_PATH".into(),
                                    p.join("filters").to_string_lossy().into(),
                                );
                            }
                            if name.starts_with("config-") || name.starts_with("ImageMagick-") {
                                config.push(p);
                            }
                        }
                    }
                }
                if let Ok(paths) = std::env::join_paths(config) {
                    environment.insert(
                        "MAGICK_CONFIGURE_PATH".into(),
                        paths.to_string_lossy().into(),
                    );
                }
            }
        }
        Self {
            binary,
            environment,
            cancelled,
            timeout: Duration::from_secs(120),
            cache_ttl: Duration::from_secs(10 * 60),
            shared: Arc::default(),
        }
    }

    /// Another host onto the same ImageMagick, sharing this one's measurements and counts.
    fn worker(&self, threads: usize) -> Self {
        let mut environment = self.environment.clone();
        environment.insert("MAGICK_THREAD_LIMIT".into(), threads.to_string());
        Self {
            binary: self.binary.clone(),
            environment,
            cancelled: self.cancelled.clone(),
            timeout: self.timeout,
            cache_ttl: self.cache_ttl,
            shared: self.shared.clone(),
        }
    }

    /// How many ImageMagick processes have been started through this host and its workers.
    pub fn processes(&self) -> usize {
        self.shared.processes.load(Ordering::Relaxed)
    }

    pub fn metadata_cache_stats(&self) -> CacheStats {
        *self.shared.metadata_stats.lock().unwrap()
    }

    pub fn capture_cache_stats(&self) -> CacheStats {
        *self.shared.capture_stats.lock().unwrap()
    }

    pub fn peak_child_memory_bytes(&self) -> u64 {
        self.shared.peak_child_memory.load(Ordering::Relaxed)
    }

    pub fn reset_peak_child_memory(&self) {
        self.shared.peak_child_memory.store(
            self.shared.current_child_memory.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }

    fn stamp_of(metadata: &fs::Metadata) -> FileStamp {
        FileStamp {
            size: metadata.len(),
            modified_ns: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |duration| duration.as_nanos()),
        }
    }

    /// The files a command reads, as they stand now.
    ///
    /// A command is options and image paths mixed together and only the host knows which
    /// is which, so anything that is a file on disk counts. The rest cannot go stale.
    fn source_stamps(args: &[String]) -> Vec<(PathBuf, FileStamp)> {
        args.iter()
            .filter_map(|arg| {
                let metadata = fs::metadata(arg).ok().filter(fs::Metadata::is_file)?;
                Some((PathBuf::from(arg), Self::stamp_of(&metadata)))
            })
            .collect()
    }

    fn file_stamp(path: &Path) -> Result<FileStamp, String> {
        fs::metadata(path)
            .map(|metadata| Self::stamp_of(&metadata))
            .map_err(|error| format!("Cannot read metadata for {}: {error}", path.display()))
    }
    pub fn analysis_identity(&self) -> Result<String, String> {
        let binary = if self.binary.is_file() {
            self.binary.clone()
        } else {
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
                .flat_map(|directory| {
                    [
                        directory.join(&self.binary),
                        directory.join(format!("{}.exe", self.binary.to_string_lossy())),
                    ]
                })
                .find(|path| path.is_file())
                .ok_or_else(|| {
                    format!(
                        "Cannot locate ImageMagick binary {} for planning provenance",
                        self.binary.display()
                    )
                })?
        };
        let bytes = fs::read(&binary)
            .map_err(|error| format!("Cannot fingerprint {}: {error}", binary.display()))?;
        let mut a = 0xcbf29ce484222325u64;
        let mut b = 0x84222325cbf29ce4u64;
        for byte in bytes {
            a = (a ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            b = (b ^ u64::from(byte)).wrapping_mul(0x100000001b3 ^ 0x9e37);
        }
        Ok(format!("image-magick:{a:016x}{b:016x}"))
    }
    fn output(&mut self, args: &[String]) -> Result<String, String> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        self.shared.processes.fetch_add(1, Ordering::Relaxed);
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .envs(&self.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn magick: {e}. Is ImageMagick v7 in PATH?"))?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        fn drain(mut reader: impl Read) -> Result<Vec<u8>, String> {
            let mut result = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                if result.len() < 16 * 1024 * 1024 {
                    let keep = n.min(16 * 1024 * 1024 - result.len());
                    result.extend_from_slice(&buffer[..keep]);
                }
            }
            Ok(result)
        }
        let out = thread::spawn(move || drain(stdout));
        let error = thread::spawn(move || drain(stderr));
        let start = Instant::now();
        let mut stopped = None;
        let mut tracked_memory = 0;
        let status = loop {
            let memory = child_working_set(&child);
            if memory > tracked_memory {
                let delta = memory - tracked_memory;
                let current = self
                    .shared
                    .current_child_memory
                    .fetch_add(delta, Ordering::Relaxed)
                    + delta;
                self.shared
                    .peak_child_memory
                    .fetch_max(current, Ordering::Relaxed);
            } else if tracked_memory > memory {
                self.shared
                    .current_child_memory
                    .fetch_sub(tracked_memory - memory, Ordering::Relaxed);
            }
            tracked_memory = memory;
            if self.cancelled.load(Ordering::Relaxed) || start.elapsed() > self.timeout {
                stopped = Some(if self.cancelled.load(Ordering::Relaxed) {
                    "Cancelled"
                } else {
                    "ImageMagick timed out"
                });
                let _ = child.kill();
                break child.wait().map_err(|e| e.to_string())?;
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            thread::sleep(Duration::from_millis(5));
        };
        self.shared
            .current_child_memory
            .fetch_sub(tracked_memory, Ordering::Relaxed);
        let stdout = out.join().map_err(|_| "stdout reader failed")??;
        let stderr = error.join().map_err(|_| "stderr reader failed")??;
        if let Some(message) = stopped {
            return Err(message.into());
        }
        if !status.success() {
            return Err(format!(
                "magick exited {status}: {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }

    /// Runs each command over `jobs` workers, keeping the order of the results.
    fn output_many(
        &mut self,
        commands: Vec<Vec<String>>,
        jobs: usize,
    ) -> Vec<Result<String, String>> {
        if jobs <= 1 || commands.len() <= 1 {
            return commands.iter().map(|args| self.output(args)).collect();
        }
        let count = commands.len();
        let queue = Mutex::new(commands.into_iter().enumerate().collect::<VecDeque<_>>());
        let results = Mutex::new((0..count).map(|_| None).collect::<Vec<_>>());
        let workers = jobs.min(count);
        thread::scope(|scope| {
            for _ in 0..workers {
                let queue = &queue;
                let results = &results;
                let mut worker = self.worker(thread_share(workers));
                scope.spawn(move || loop {
                    if worker.cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let Some((index, args)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let value = worker.output(&args);
                    results.lock().unwrap()[index] = Some(value);
                });
            }
        });
        results
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|result| result.unwrap_or_else(|| Err("Cancelled".into())))
            .collect()
    }
}
impl ImageHost for Magick {
    fn run(&mut self, args: &[String]) -> Result<(), String> {
        self.output(args).map(|_| ())
    }
    /// Measures an image, reusing what the same command reported about the same files.
    ///
    /// A workflow runs its chain once per output node, so a channel gate feeding both an
    /// image output and a report asks for every mean twice over. The answer only depends
    /// on the command and the files it reads, and both are known here.
    fn capture(&mut self, args: &[String]) -> Result<String, String> {
        let sources = Self::source_stamps(args);
        // A command that reads no file has nothing to notice a change in, so it is
        // measured afresh every time rather than remembered forever.
        if sources.is_empty() {
            return self.output(args);
        }
        if let Some(cached) = self.shared.captures.lock().unwrap().get(args) {
            if cached.sources == sources && cached.inserted.elapsed() <= self.cache_ttl {
                self.shared.capture_stats.lock().unwrap().hits += 1;
                return Ok(cached.value.clone());
            }
            self.shared.capture_stats.lock().unwrap().invalidations += 1;
        }
        self.shared.capture_stats.lock().unwrap().misses += 1;
        let value = self.output(args)?;
        let mut captures = self.shared.captures.lock().unwrap();
        if captures.len() >= CAPTURE_CACHE_LIMIT {
            captures.retain(|_, cached| cached.inserted.elapsed() <= self.cache_ttl);
            if captures.len() >= CAPTURE_CACHE_LIMIT {
                captures.clear();
            }
        }
        captures.insert(
            args.to_vec(),
            CachedCapture {
                sources,
                inserted: Instant::now(),
                value: value.clone(),
            },
        );
        Ok(value)
    }
    fn emit_many(
        &mut self,
        operations: Vec<bite_core::execution::OutputOperation>,
        options: &bite_core::execution::RunOptions,
        jobs: usize,
    ) -> Vec<Result<(), String>> {
        if jobs <= 1 || operations.len() <= 1 {
            return operations
                .into_iter()
                .map(|operation| self.emit(operation, options))
                .collect();
        }
        let count = operations.len();
        let queue = Mutex::new(operations.into_iter().enumerate().collect::<VecDeque<_>>());
        let results = Mutex::new((0..count).map(|_| None).collect::<Vec<_>>());
        let workers = jobs.min(count).max(1);
        thread::scope(|scope| {
            for _ in 0..workers {
                let queue = &queue;
                let results = &results;
                // Each pipeline gets an equal share of the hardware threads. Pinning every
                // one of them to a single thread leaves most of a machine idle when there
                // are only a few images to process, and letting each take the whole machine
                // would have them fighting over it. The total stays at about one thread per
                // core either way.
                let mut worker = self.worker(thread_share(workers));
                scope.spawn(move || loop {
                    if worker.cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let Some((index, operation)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let value = worker.emit(operation, options);
                    results.lock().unwrap()[index] = Some(value);
                });
            }
        });
        results
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|result| result.unwrap_or_else(|| Err("Cancelled".into())))
            .collect()
    }
    /// One host per job, each with an equal share of the hardware threads, so that the
    /// files of a batch can be resolved at the same time without their ImageMagick thread
    /// pools fighting over the machine.
    fn workers(&self, jobs: usize) -> Vec<Box<dyn ImageHost + Send>> {
        let jobs = jobs.max(1);
        if jobs == 1 {
            return Vec::new();
        }
        (0..jobs)
            .map(|_| Box::new(self.worker(thread_share(jobs))) as Box<dyn ImageHost + Send>)
            .collect()
    }
    fn metadata(&mut self, path: &Path, heavy: bool) -> Result<Context, String> {
        let key = (path.to_owned(), heavy);
        let stamp = Self::file_stamp(path)?;
        if let Some(cached) = self.shared.metadata.lock().unwrap().get(&key) {
            if cached.stamp == stamp && cached.inserted.elapsed() <= self.cache_ttl {
                self.shared.metadata_stats.lock().unwrap().hits += 1;
                return Ok(cached.value.clone());
            }
            self.shared.metadata_stats.lock().unwrap().invalidations += 1;
        }
        self.shared.metadata_stats.lock().unwrap().misses += 1;
        let mut ctx: Context = metadata_types()
            .into_iter()
            .map(|(k, t)| {
                (
                    k,
                    if t == Type::String {
                        Value::String(String::new())
                    } else {
                        Value::Int(0)
                    },
                )
            })
            .collect();
        ctx.insert(
            "image.path".into(),
            Value::String(path.to_string_lossy().into()),
        );
        ctx.insert(
            "image.name".into(),
            Value::String(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
            ),
        );
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        ctx.insert("image.extension".into(), Value::String(ext.clone()));
        ctx.insert("image.format".into(), Value::String(ext));
        ctx.insert(
            "image.size".into(),
            Value::Int(fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0)),
        );
        if let Some((w, h)) = header::dimensions(path) {
            ctx.insert("image.width".into(), Value::Int(w.into()));
            ctx.insert("image.height".into(), Value::Int(h.into()));
        }
        if heavy {
            let info = self.output(&[
                "identify".into(),
                "-format".into(),
                "%B\n%w\n%h\n%z\n%e\n%x\n%y".into(),
                path.to_string_lossy().into(),
            ])?;
            let lines: Vec<_> = info.lines().collect();
            for (i, key) in [
                "image.size",
                "image.width",
                "image.height",
                "image.bit_depth",
                "image.extension",
                "image.dpi_x",
                "image.dpi_y",
            ]
            .iter()
            .enumerate()
            {
                let value = lines.get(i).copied().unwrap_or("");
                ctx.insert(
                    (*key).into(),
                    if i == 4 {
                        Value::String(value.to_lowercase())
                    } else {
                        Value::Float(
                            value
                                .trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.')
                                .parse()
                                .unwrap_or(0.0),
                        )
                    },
                );
            }
            if let Ok(exif) = self.output(&[
                "identify".into(),
                "-format".into(),
                "%[EXIF:*]".into(),
                path.to_string_lossy().into(),
            ]) {
                for line in exif.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        ctx.insert(
                            format!("image.exif.{}", k.trim().trim_start_matches("exif:")),
                            Value::String(v.trim().into()),
                        );
                    }
                }
            }
        }
        self.shared.metadata.lock().unwrap().insert(
            key,
            CachedMetadata {
                stamp,
                inserted: Instant::now(),
                value: ctx.clone(),
            },
        );
        Ok(ctx)
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn child_fixture() {
        match std::env::var("BITE_PROCESS_FIXTURE").as_deref() {
            Ok("sleep") => thread::sleep(Duration::from_secs(20)),
            Ok("failure") => panic!("intentional encoder failure"),
            Ok("tokens") => println!("{}", std::env::var("BITE_FIXTURE_VALUE").unwrap()),
            _ => {}
        }
    }

    fn fixture(mode: &str) -> Magick {
        Magick {
            binary: std::env::current_exe().unwrap(),
            environment: BTreeMap::from([("BITE_PROCESS_FIXTURE".into(), mode.into())]),
            cancelled: Arc::new(AtomicBool::new(false)),
            timeout: Duration::from_secs(5),
            cache_ttl: Duration::from_secs(10 * 60),
            shared: Arc::default(),
        }
    }

    #[test]
    fn child_failure_timeout_and_cancellation_are_reaped() {
        let args = ["--exact", "process_tests::child_fixture", "--nocapture"].map(String::from);
        let mut failed = fixture("failure");
        assert!(failed
            .output(&args)
            .unwrap_err()
            .contains("intentional encoder failure"));
        let mut timed = fixture("sleep");
        timed.timeout = Duration::from_millis(100);
        let start = Instant::now();
        assert_eq!(timed.output(&args).unwrap_err(), "ImageMagick timed out");
        assert!(start.elapsed() < Duration::from_secs(5));
        let mut cancelled = fixture("sleep");
        let flag = cancelled.cancelled.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            flag.store(true, Ordering::Relaxed);
        });
        assert_eq!(cancelled.output(&args).unwrap_err(), "Cancelled");
        trigger.join().unwrap();
        let mut literal = fixture("tokens");
        literal
            .environment
            .insert("BITE_FIXTURE_VALUE".into(), "space & quote ' ; $()".into());
        assert!(literal
            .output(&args)
            .unwrap()
            .contains("space & quote ' ; $()"));
    }

    #[test]
    fn metadata_cache_hits_expires_and_invalidates_on_source_change() {
        let dir = std::env::temp_dir().join(format!(
            "bite-metadata-cache-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.png");
        let mut png = vec![0; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[16..20].copy_from_slice(&32u32.to_be_bytes());
        png[20..24].copy_from_slice(&16u32.to_be_bytes());
        fs::write(&path, &png).unwrap();
        let mut host = fixture("tokens");

        let first = host.metadata(&path, false).unwrap();
        let second = host.metadata(&path, false).unwrap();
        assert_eq!(first, second);
        assert_eq!(host.metadata_cache_stats().hits, 1);
        assert_eq!(host.metadata_cache_stats().misses, 1);

        host.cache_ttl = Duration::ZERO;
        host.metadata(&path, false).unwrap();
        assert_eq!(host.metadata_cache_stats().invalidations, 1);
        host.cache_ttl = Duration::from_secs(60);
        thread::sleep(Duration::from_millis(10));
        png[16..20].copy_from_slice(&64u32.to_be_bytes());
        fs::write(&path, &png).unwrap();
        let changed = host.metadata(&path, false).unwrap();
        assert_eq!(changed["image.width"], Value::Int(64));
        assert_eq!(host.metadata_cache_stats().invalidations, 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn captures_are_reused_until_the_files_they_measured_change() {
        let dir = std::env::temp_dir().join(format!(
            "bite-capture-cache-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.png");
        fs::write(&path, b"first").unwrap();
        let mut host = fixture("tokens");
        host.environment
            .insert("BITE_FIXTURE_VALUE".into(), "0.25".into());
        let args = [
            path.to_string_lossy().into_owned(),
            "--exact".into(),
            "process_tests::child_fixture".into(),
            "--nocapture".into(),
        ];

        assert!(host.capture(&args).unwrap().contains("0.25"));
        // The second output node asks the same question of the same file, and is answered
        // without starting a process: the host now reports a different number, and the
        // remembered one comes back anyway.
        host.environment
            .insert("BITE_FIXTURE_VALUE".into(), "0.75".into());
        assert!(host.capture(&args).unwrap().contains("0.25"));
        assert_eq!(host.capture_cache_stats().hits, 1);
        assert_eq!(host.capture_cache_stats().misses, 1);
        assert_eq!(host.processes(), 1);

        // A file that has changed underneath is measured again.
        thread::sleep(Duration::from_millis(10));
        fs::write(&path, b"second").unwrap();
        assert!(host.capture(&args).unwrap().contains("0.75"));
        assert_eq!(host.capture_cache_stats().invalidations, 1);
        assert_eq!(host.processes(), 2);

        // A command that reads no file has nothing to go stale, so it is never kept.
        let transient = ["--exact".into(), "process_tests::child_fixture".into()];
        host.capture(&transient).unwrap();
        host.capture(&transient).unwrap();
        assert_eq!(host.capture_cache_stats().hits, 1, "still only the one hit");
        assert_eq!(host.processes(), 4);
        fs::remove_dir_all(dir).unwrap();
    }
}
