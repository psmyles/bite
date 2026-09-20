//! Folder scanning and thumbnail services used by the native importer.
use super::Magick;
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "svgz", "ico", "bmp", "tif", "tiff",
    "heic", "heif", "jp2", "j2k", "jpf", "jpx", "jxl", "psd", "psb", "exr", "hdr", "dpx", "cin",
    "cr2", "cr3", "nef", "nrw", "arw", "dng", "orf", "raf", "rw2", "pef", "srw", "x3f", "3fr",
    "kdc", "mrw", "erf", "rwl", "tga", "pcx", "ppm", "pgm", "pbm", "pnm", "sgi", "rgb", "rgba",
    "miff", "mng", "jng", "xbm", "xpm", "xwd", "sun", "iff", "lbm", "wbmp", "pict", "pct", "dds",
    "fits", "fts",
];

pub fn default_jobs() -> usize {
    thread::available_parallelism()
        .map(|count| (count.get() / 2).clamp(1, 8))
        .unwrap_or(1)
}

fn is_image(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        IMAGE_EXTENSIONS.contains(&extension.to_string_lossy().to_ascii_lowercase().as_str())
    })
}

/// Scan directories concurrently while keeping deterministic result ordering.
pub fn scan_folder(
    root: &Path,
    recursive: bool,
    jobs: usize,
    cancelled: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err(format!("Cannot read input directory: {}", root.display()));
    }
    let queue = Arc::new(Mutex::new(VecDeque::from([root.to_owned()])));
    let pending = Arc::new(AtomicUsize::new(1));
    let found = Arc::new(Mutex::new(Vec::new()));
    thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            let queue = queue.clone();
            let pending = pending.clone();
            let found = found.clone();
            scope.spawn(move || loop {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                let directory = queue.lock().unwrap().pop_front();
                let Some(directory) = directory else {
                    if pending.load(Ordering::Acquire) == 0 {
                        break;
                    }
                    thread::yield_now();
                    continue;
                };
                if let Ok(entries) = fs::read_dir(directory) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if entry.file_type().is_ok_and(|kind| kind.is_dir()) && recursive {
                            pending.fetch_add(1, Ordering::Release);
                            queue.lock().unwrap().push_back(path);
                        } else if entry.file_type().is_ok_and(|kind| kind.is_file())
                            && is_image(&path)
                        {
                            found.lock().unwrap().push(path);
                        }
                    }
                }
                pending.fetch_sub(1, Ordering::Release);
            });
        }
    });
    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut result = Arc::try_unwrap(found).unwrap().into_inner().unwrap();
    result.sort();
    Ok(result)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ThumbnailInfo {
    pub path: PathBuf,
    pub thumbnail: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct ThumbnailStats {
    pub memory_hits: usize,
    pub memory_misses: usize,
    pub disk_hits: usize,
    pub disk_misses: usize,
    pub invalidations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified_ns: u128,
}

#[derive(Debug, Clone)]
struct CachedInfo {
    stamp: Stamp,
    inserted: Instant,
    width: u32,
    height: u32,
    format: String,
}

/// Stamps and measures every file at once, across `jobs` threads, keeping the order.
///
/// Each entry is the file's stamp and what its header gave, or the error that stopped it.
type Probe = Result<(Stamp, Option<(u32, u32, &'static str)>), String>;

fn probe_all(paths: &[PathBuf], jobs: usize) -> Vec<Probe> {
    let one = |path: &PathBuf| -> Probe {
        Ok((ThumbnailCache::stamp(path)?, super::header::probe(path)))
    };
    let workers = jobs.max(1).min(paths.len());
    if workers <= 1 {
        return paths.iter().map(one).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Mutex<Option<Probe>>> = paths.iter().map(|_| Mutex::new(None)).collect();
    thread::scope(|scope| {
        for _ in 0..workers {
            let next = &next;
            let results = &results;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(index) else { break };
                    *results[index].lock().unwrap() = Some(one(path));
                }
            });
        }
    });
    results
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().unwrap_or(Err("not probed".into())))
        .collect()
}

pub struct ThumbnailCache {
    pub directory: PathBuf,
    pub ttl: Duration,
    pub batch_size: usize,
    pub jobs: usize,
    pub stats: ThumbnailStats,
    metadata: BTreeMap<PathBuf, CachedInfo>,
}

impl ThumbnailCache {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            ttl: Duration::from_secs(10 * 60),
            batch_size: 8,
            jobs: default_jobs(),
            stats: ThumbnailStats::default(),
            metadata: BTreeMap::new(),
        }
    }

    fn stamp(path: &Path) -> Result<Stamp, String> {
        let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(Stamp {
            size: metadata.len(),
            modified_ns,
        })
    }

    fn thumbnail_path(&self, source: &Path, size: u32) -> PathBuf {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in source.to_string_lossy().as_bytes() {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        self.directory
            .join(format!("thumb_{hash:016x}_{size}.webp"))
    }

    pub fn load_batch(
        &mut self,
        host: &mut Magick,
        paths: &[PathBuf],
        size: u32,
    ) -> Result<Vec<ThumbnailInfo>, String> {
        if size == 0 || size > 16384 {
            return Err("thumbnail size must be between 1 and 16384".into());
        }
        fs::create_dir_all(&self.directory).map_err(|error| error.to_string())?;
        let mut items = Vec::with_capacity(paths.len());
        let mut misses = Vec::new();
        // Reading a header means reading the front of the file, so the whole set is read
        // at once rather than one after another. The Electron import does the same with a
        // `Promise.all`, and a folder is mostly waiting on the disk.
        let probes = probe_all(paths, self.jobs);
        for (path, probe) in paths.iter().zip(probes) {
            if host.cancelled.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let (stamp, header) = probe?;
            // The signature names the format, so a `.jpg` reads as `JPEG` here just as it
            // does when ImageMagick is the one that measured it.
            let format = header.map_or_else(
                || {
                    path.extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_uppercase()
                },
                |(_, _, name)| name.to_owned(),
            );
            let cached = self
                .metadata
                .get(path)
                .filter(|cached| cached.stamp == stamp && cached.inserted.elapsed() <= self.ttl);
            let (width, height, format) = if let Some(cached) = cached {
                self.stats.memory_hits += 1;
                (cached.width, cached.height, cached.format.clone())
            } else {
                if self.metadata.contains_key(path) {
                    self.stats.invalidations += 1;
                }
                self.stats.memory_misses += 1;
                let (width, height) = header.map_or((0, 0), |(w, h, _)| (w, h));
                (width, height, format)
            };
            let thumbnail = self.thumbnail_path(path, size);
            let disk_valid = fs::metadata(&thumbnail)
                .ok()
                .filter(|metadata| metadata.len() > 0)
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .is_some_and(|modified| modified.as_nanos() >= stamp.modified_ns);
            if disk_valid {
                self.stats.disk_hits += 1;
            } else {
                if thumbnail.exists() {
                    self.stats.invalidations += 1;
                }
                self.stats.disk_misses += 1;
                misses.push(items.len());
            }
            items.push(ThumbnailInfo {
                path: path.clone(),
                thumbnail,
                width,
                height,
                format,
                size_bytes: stamp.size,
            });
        }

        let mut commands = Vec::new();
        // Which items each command measures, in the order the command prints them.
        let mut measured: Vec<Vec<usize>> = Vec::new();
        for chunk in misses.chunks(self.batch_size.max(1)) {
            let mut args = Vec::new();
            let mut printed = Vec::new();
            for (position, index) in chunk.iter().enumerate() {
                let item = &items[*index];
                if matches!(
                    item.path.extension().and_then(|value| value.to_str()),
                    Some(extension) if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
                ) {
                    args.extend([
                        "-define".into(),
                        format!("jpeg:size={}x{}", size * 2, size * 2),
                    ]);
                }
                args.push(format!("{}[0]", item.path.to_string_lossy()));
                // A format the header parser does not read is measured here rather than in
                // a second process of its own, while the picture is already open.
                if item.width == 0 || item.height == 0 {
                    args.extend(["-print".into(), "%w %h %m
".into()]);
                    printed.push(*index);
                }
                args.extend([
                    "-thumbnail".into(),
                    format!("{size}x{size}>"),
                    "-quality".into(),
                    "85".into(),
                ]);
                if position + 1 == chunk.len() {
                    args.push(item.thumbnail.to_string_lossy().into_owned());
                } else {
                    args.extend([
                        "-write".into(),
                        item.thumbnail.to_string_lossy().into_owned(),
                        "+delete".into(),
                    ]);
                }
            }
            commands.push(args);
            measured.push(printed);
        }
        for (command, printed) in host.output_many(commands, self.jobs).into_iter().zip(measured) {
            let output = command?;
            for (line, index) in output.lines().zip(printed) {
                let mut values = line.split_whitespace();
                let item = &mut items[index];
                item.width = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                item.height = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                item.format = values.next().unwrap_or("UNKNOWN").to_owned();
            }
        }

        for item in &mut items {
            if item.width == 0 || item.height == 0 {
                let output = host.output(&[
                    "identify".into(),
                    "-format".into(),
                    "%w %h %m".into(),
                    format!("{}[0]", item.path.to_string_lossy()),
                ])?;
                let mut values = output.split_whitespace();
                item.width = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                item.height = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                item.format = values.next().unwrap_or("UNKNOWN").to_owned();
            }
            self.metadata.insert(
                item.path.clone(),
                CachedInfo {
                    stamp: Self::stamp(&item.path)?,
                    inserted: Instant::now(),
                    width: item.width,
                    height: item.height,
                    format: item.format.clone(),
                },
            );
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_is_recursive_filtered_and_sorted() {
        let root = std::env::temp_dir().join(format!("bite-scan-{}", std::process::id()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("z.png"), []).unwrap();
        fs::write(root.join("ignore.txt"), []).unwrap();
        fs::write(nested.join("a.JPEG"), []).unwrap();
        let cancelled = AtomicBool::new(false);
        assert_eq!(scan_folder(&root, false, 2, &cancelled).unwrap().len(), 1);
        let recursive = scan_folder(&root, true, 2, &cancelled).unwrap();
        assert_eq!(recursive, [nested.join("a.JPEG"), root.join("z.png")]);
        fs::remove_dir_all(root).unwrap();
    }
}
