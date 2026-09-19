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
        atomic::{AtomicBool, AtomicUsize, Ordering},
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

pub struct Magick {
    pub binary: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub cancelled: Arc<AtomicBool>,
    pub timeout: Duration,
    pub processes: usize,
    metadata_cache: BTreeMap<(PathBuf, bool), CachedMetadata>,
    pub metadata_cache_stats: CacheStats,
    pub metadata_cache_ttl: Duration,
}

impl Magick {
    pub fn discover(cancelled: Arc<AtomicBool>) -> Self {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("magick/magick.exe"));
                candidates.push(dir.join("ImageMagick/magick.exe"));
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
            processes: 0,
            metadata_cache: BTreeMap::new(),
            metadata_cache_stats: CacheStats::default(),
            metadata_cache_ttl: Duration::from_secs(10 * 60),
        }
    }

    fn file_stamp(path: &Path) -> Result<FileStamp, String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("Cannot read metadata for {}: {error}", path.display()))?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(FileStamp {
            size: metadata.len(),
            modified_ns,
        })
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
        self.processes += 1;
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
        let status = loop {
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

    fn output_many(
        &mut self,
        commands: Vec<Vec<String>>,
        jobs: usize,
    ) -> Vec<Result<String, String>> {
        if jobs <= 1 || commands.len() <= 1 {
            return commands.iter().map(|args| self.output(args)).collect();
        }
        let count = commands.len();
        let queue = Arc::new(Mutex::new(
            commands.into_iter().enumerate().collect::<VecDeque<_>>(),
        ));
        let results = Arc::new(Mutex::new(
            (0..count)
                .map(|_| None)
                .collect::<Vec<Option<Result<String, String>>>>(),
        ));
        let process_count = Arc::new(AtomicUsize::new(0));
        let binary = self.binary.clone();
        let environment = self.environment.clone();
        let cancelled = self.cancelled.clone();
        let timeout = self.timeout;
        thread::scope(|scope| {
            for _ in 0..jobs.min(count) {
                let queue = queue.clone();
                let results = results.clone();
                let process_count = process_count.clone();
                let binary = binary.clone();
                let environment = environment.clone();
                let cancelled = cancelled.clone();
                scope.spawn(move || {
                    let mut worker = Magick {
                        binary,
                        environment,
                        cancelled,
                        timeout,
                        processes: 0,
                        metadata_cache: BTreeMap::new(),
                        metadata_cache_stats: CacheStats::default(),
                        metadata_cache_ttl: Duration::from_secs(10 * 60),
                    };
                    loop {
                        let Some((index, args)) = queue.lock().unwrap().pop_front() else {
                            break;
                        };
                        let value = worker.output(&args);
                        results.lock().unwrap()[index] = Some(value);
                    }
                    process_count.fetch_add(worker.processes, Ordering::Relaxed);
                });
            }
        });
        self.processes += process_count.load(Ordering::Relaxed);
        Arc::try_unwrap(results)
            .unwrap()
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|result| result.expect("worker result"))
            .collect()
    }
}
impl ImageHost for Magick {
    fn run(&mut self, args: &[String]) -> Result<(), String> {
        self.output(args).map(|_| ())
    }
    fn capture(&mut self, args: &[String]) -> Result<String, String> {
        self.output(args)
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
        let queue = Arc::new(Mutex::new(
            operations.into_iter().enumerate().collect::<VecDeque<_>>(),
        ));
        let results = Arc::new(Mutex::new(
            (0..count)
                .map(|_| None)
                .collect::<Vec<Option<Result<(), String>>>>(),
        ));
        let process_count = Arc::new(AtomicUsize::new(0));
        let binary = self.binary.clone();
        let environment = self.environment.clone();
        let cancelled = self.cancelled.clone();
        let timeout = self.timeout;
        thread::scope(|scope| {
            for _ in 0..jobs.min(count) {
                let queue = queue.clone();
                let results = results.clone();
                let process_count = process_count.clone();
                let binary = binary.clone();
                let environment = environment.clone();
                let cancelled = cancelled.clone();
                scope.spawn(move || {
                    let mut worker = Magick {
                        binary,
                        environment,
                        cancelled,
                        timeout,
                        processes: 0,
                        metadata_cache: BTreeMap::new(),
                        metadata_cache_stats: CacheStats::default(),
                        metadata_cache_ttl: Duration::from_secs(10 * 60),
                    };
                    loop {
                        let Some((index, operation)) = queue.lock().unwrap().pop_front() else {
                            break;
                        };
                        let value = worker.emit(operation, options);
                        results.lock().unwrap()[index] = Some(value);
                    }
                    process_count.fetch_add(worker.processes, Ordering::Relaxed);
                });
            }
        });
        self.processes += process_count.load(Ordering::Relaxed);
        Arc::try_unwrap(results)
            .unwrap()
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|result| result.expect("worker result"))
            .collect()
    }
    fn metadata(&mut self, path: &Path, heavy: bool) -> Result<Context, String> {
        let key = (path.to_owned(), heavy);
        let stamp = Self::file_stamp(path)?;
        if let Some(cached) = self.metadata_cache.get(&key) {
            if cached.stamp == stamp && cached.inserted.elapsed() <= self.metadata_cache_ttl {
                self.metadata_cache_stats.hits += 1;
                return Ok(cached.value.clone());
            }
            self.metadata_cache_stats.invalidations += 1;
        }
        self.metadata_cache_stats.misses += 1;
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
        self.metadata_cache.insert(
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
            processes: 0,
            metadata_cache: BTreeMap::new(),
            metadata_cache_stats: CacheStats::default(),
            metadata_cache_ttl: Duration::from_secs(10 * 60),
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
        assert_eq!(host.metadata_cache_stats.hits, 1);
        assert_eq!(host.metadata_cache_stats.misses, 1);

        host.metadata_cache_ttl = Duration::ZERO;
        host.metadata(&path, false).unwrap();
        assert_eq!(host.metadata_cache_stats.invalidations, 1);
        host.metadata_cache_ttl = Duration::from_secs(60);
        thread::sleep(Duration::from_millis(10));
        png[16..20].copy_from_slice(&64u32.to_be_bytes());
        fs::write(&path, &png).unwrap();
        let changed = host.metadata(&path, false).unwrap();
        assert_eq!(changed["image.width"], Value::Int(64));
        assert_eq!(host.metadata_cache_stats.invalidations, 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
