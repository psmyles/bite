//! Folder scanning and thumbnail services used by the native importer.
use super::Magick;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Condvar, Mutex,
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

/// The directories still to read, and how many are being read at this moment.
///
/// The two belong together: an empty queue only means the scan is over while nobody is
/// still reading a directory that might add more to it.
struct Pending {
    queue: VecDeque<PathBuf>,
    reading: usize,
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
    let pending = Mutex::new(Pending {
        queue: VecDeque::from([root.to_owned()]),
        reading: 0,
    });
    let ready = Condvar::new();
    let found = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            let pending = &pending;
            let ready = &ready;
            let found = &found;
            scope.spawn(move || {
                loop {
                    let mut held = pending.lock().unwrap();
                    let directory = loop {
                        if cancelled.load(Ordering::Relaxed) {
                            return;
                        }
                        if let Some(directory) = held.queue.pop_front() {
                            held.reading += 1;
                            break directory;
                        }
                        // Nothing to read and nobody reading it: whatever there was to
                        // find has been found. Waiting rather than spinning leaves the
                        // machine to the worker still reading a directory of its own,
                        // where an idle thread used to burn a core of its own.
                        if held.reading == 0 {
                            ready.notify_all();
                            return;
                        }
                        // Timed, so that a cancelled scan is noticed by a worker nobody
                        // is going to wake.
                        held = ready
                            .wait_timeout(held, Duration::from_millis(50))
                            .unwrap()
                            .0;
                    };
                    drop(held);

                    let mut directories = Vec::new();
                    let mut images = Vec::new();
                    if let Ok(entries) = fs::read_dir(directory) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                                if recursive {
                                    directories.push(path);
                                }
                            } else if entry.file_type().is_ok_and(|kind| kind.is_file())
                                && is_image(&path)
                            {
                                images.push(path);
                            }
                        }
                    }
                    if !images.is_empty() {
                        found.lock().unwrap().append(&mut images);
                    }
                    let mut held = pending.lock().unwrap();
                    held.reading -= 1;
                    held.queue.extend(directories);
                    drop(held);
                    // Either there is more to read, or this was the last reader and the
                    // others have nothing left to wait for. Both are worth waking for.
                    ready.notify_all();
                }
            });
        }
    });
    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut result = found.into_inner().unwrap();
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
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<Probe>>> = paths.iter().map(|_| Mutex::new(None)).collect();
    thread::scope(|scope| {
        for _ in 0..workers {
            let next = &next;
            let results = &results;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(index) else { break };
                *results[index].lock().unwrap() = Some(one(path));
            });
        }
    });
    results
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap()
                .unwrap_or(Err("not probed".into()))
        })
        .collect()
}

pub struct ThumbnailCache {
    pub directory: PathBuf,
    pub ttl: Duration,
    pub batch_size: usize,
    pub jobs: usize,
    pub stats: ThumbnailStats,
    /// The files the last [`ThumbnailCache::load_batch`] could not read, with the reason.
    ///
    /// One picture nobody can decode is not a reason to import nothing: it is left out of
    /// the returned thumbnails and recorded here, for the caller to report as it likes.
    /// Emptied at the start of every call, so it always describes the most recent one.
    pub failures: Vec<(PathBuf, String)>,
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
            failures: Vec::new(),
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
        self.failures.clear();
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
            // A file that vanished or cannot be stamped is left out rather than ending the
            // import: the rest of the folder is still perfectly readable.
            let (stamp, header) = match probe {
                Ok(probe) => probe,
                Err(error) => {
                    self.failures.push((path.clone(), error));
                    continue;
                }
            };
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

        // The files each command thumbnails. Which of them it prints measurements for comes
        // back with the arguments, since only an unmeasured one is asked to print.
        let batches: Vec<Vec<usize>> = misses
            .chunks(self.batch_size.max(1))
            .map(<[usize]>::to_vec)
            .collect();
        let prepared: Vec<(Vec<String>, Vec<usize>)> = batches
            .iter()
            .map(|members| Self::thumbnail_command(&items, members, size))
            .collect();
        let mut printing = Vec::with_capacity(prepared.len());
        let mut commands = Vec::with_capacity(prepared.len());
        for (args, printed) in prepared {
            commands.push(args);
            printing.push(printed);
        }
        let mut dropped = BTreeSet::new();
        for ((outcome, printed), members) in host
            .output_many(commands, self.jobs)
            .into_iter()
            .zip(printing)
            .zip(&batches)
        {
            match outcome {
                Ok(output) => Self::measure(&mut items, &output, &printed),
                // One picture ImageMagick cannot read fails every file batched into its
                // command, so each is tried again on its own. A folder with one truncated
                // photograph in it still imports the rest.
                Err(error) => {
                    if host.cancelled.load(Ordering::Relaxed) {
                        return Err("Cancelled".into());
                    }
                    for index in members {
                        let (args, printed) =
                            Self::thumbnail_command(&items, std::slice::from_ref(index), size);
                        // A command that held one file has already had its chance.
                        let attempt = if members.len() == 1 {
                            Err(error.clone())
                        } else {
                            host.output(&args)
                        };
                        match attempt {
                            Ok(output) => Self::measure(&mut items, &output, &printed),
                            Err(error) => {
                                self.failures.push((items[*index].path.clone(), error));
                                dropped.insert(*index);
                            }
                        }
                    }
                }
            }
        }

        for (index, item) in items.iter_mut().enumerate() {
            if dropped.contains(&index) {
                continue;
            }
            if item.width == 0 || item.height == 0 {
                match host.output(&[
                    "identify".into(),
                    "-format".into(),
                    "%w %h %m".into(),
                    format!("{}[0]", item.path.to_string_lossy()),
                ]) {
                    Ok(output) => {
                        let mut values = output.split_whitespace();
                        item.width = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        item.height = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        item.format = values.next().unwrap_or("UNKNOWN").to_owned();
                    }
                    Err(error) => {
                        if host.cancelled.load(Ordering::Relaxed) {
                            return Err("Cancelled".into());
                        }
                        self.failures.push((item.path.clone(), error));
                        dropped.insert(index);
                        continue;
                    }
                }
            }
            match Self::stamp(&item.path) {
                Ok(stamp) => {
                    self.metadata.insert(
                        item.path.clone(),
                        CachedInfo {
                            stamp,
                            inserted: Instant::now(),
                            width: item.width,
                            height: item.height,
                            format: item.format.clone(),
                        },
                    );
                }
                Err(error) => {
                    self.failures.push((item.path.clone(), error));
                    dropped.insert(index);
                }
            }
        }
        for index in &dropped {
            // A command that failed part way through may have left a truncated thumbnail
            // behind, which would read as a valid cache entry on the next import.
            let _ = fs::remove_file(&items[*index].thumbnail);
        }
        Ok(items
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !dropped.contains(index))
            .map(|(_, item)| item)
            .collect())
    }

    /// The arguments that write thumbnails for `members`, and which of them the command
    /// prints measurements for, in the order it prints them.
    fn thumbnail_command(
        items: &[ThumbnailInfo],
        members: &[usize],
        size: u32,
    ) -> (Vec<String>, Vec<usize>) {
        let mut args = Vec::new();
        let mut printed = Vec::new();
        for (position, index) in members.iter().enumerate() {
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
                args.extend(["-print".into(), "%w %h %m\n".into()]);
                printed.push(*index);
            }
            args.extend([
                "-thumbnail".into(),
                format!("{size}x{size}>"),
                "-quality".into(),
                "85".into(),
            ]);
            if position + 1 == members.len() {
                args.push(item.thumbnail.to_string_lossy().into_owned());
            } else {
                args.extend([
                    "-write".into(),
                    item.thumbnail.to_string_lossy().into_owned(),
                    "+delete".into(),
                ]);
            }
        }
        (args, printed)
    }

    /// Applies what one command printed to the files it printed for.
    fn measure(items: &mut [ThumbnailInfo], output: &str, printed: &[usize]) {
        for (line, index) in output.lines().zip(printed) {
            let mut values = line.split_whitespace();
            let item = &mut items[*index];
            item.width = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            item.height = values.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            item.format = values.next().unwrap_or("UNKNOWN").to_owned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The bundled ImageMagick, or whatever one is on the path, or nothing.
    ///
    /// Reading a picture is what these tests are about, so they need the real decoder. A
    /// machine without one reports it and passes rather than failing for the wrong reason.
    fn magick() -> Option<Magick> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut host = Magick::discover(Arc::new(AtomicBool::new(false)));
        for candidate in [
            root.join("resources/win/magick/magick.exe"),
            root.join("resources/mac/magick/bin/magick"),
        ] {
            if candidate.is_file() {
                host.binary = candidate;
                break;
            }
        }
        host.output(&["-version".into()])
            .is_ok()
            .then_some(host)
            .or_else(|| {
                println!("no ImageMagick to test against; skipping");
                None
            })
    }

    /// A one-by-one pixel, which any build of ImageMagick can read.
    const PIXEL: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R', 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0a, b'I', b'D',
        b'A', b'T', 0x78, 0x9c, 0x63, 0, 0x01, 0, 0, 0x05, 0, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0, 0,
        0, 0, b'I', b'E', b'N', b'D', 0xae, b'B', 0x60, 0x82,
    ];

    /// One picture nobody can decode used to fail the whole command it was batched into,
    /// and with it the entire import: a folder of five hundred photographs with one
    /// truncated file in it produced no thumbnails at all.
    #[test]
    fn one_unreadable_picture_does_not_lose_the_batch_it_was_read_with() {
        let Some(mut host) = magick() else { return };
        let root = std::env::temp_dir().join(format!("bite-batch-{}", std::process::id()));
        let sources = root.join("sources");
        fs::create_dir_all(&sources).unwrap();
        let good: Vec<PathBuf> = ["a.png", "b.png", "c.png"]
            .iter()
            .map(|name| {
                let path = sources.join(name);
                fs::write(&path, PIXEL).unwrap();
                path
            })
            .collect();
        // A file that claims to be a picture and is not, which is what a truncated
        // download or a half-copied photograph looks like.
        let broken = sources.join("broken.png");
        fs::write(&broken, b"this is not a picture").unwrap();

        let mut cache = ThumbnailCache::new(root.join("cache"));
        let mut paths = good.clone();
        paths.insert(2, broken.clone());
        // One command for the lot, so the unreadable file is batched with every good one.
        cache.batch_size = paths.len();
        let loaded = cache.load_batch(&mut host, &paths, 64).unwrap();

        assert_eq!(
            loaded.iter().map(|info| &info.path).collect::<Vec<_>>(),
            good.iter().collect::<Vec<_>>()
        );
        assert_eq!(cache.failures.len(), 1, "{:?}", cache.failures);
        assert_eq!(cache.failures[0].0, broken);
        for info in &loaded {
            assert!(info.thumbnail.is_file());
            assert!(info.width > 0 && info.height > 0);
        }
        // Nothing half-written is left behind to be mistaken for a valid cache entry.
        assert!(!cache.thumbnail_path(&broken, 64).exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// The command a retry builds writes one thumbnail and asks for one measurement.
    #[test]
    fn a_single_file_command_writes_its_thumbnail_without_a_write_and_delete() {
        let items = vec![
            ThumbnailInfo {
                path: PathBuf::from("first.png"),
                thumbnail: PathBuf::from("first.webp"),
                width: 0,
                height: 0,
                format: "PNG".into(),
                size_bytes: 0,
            },
            ThumbnailInfo {
                path: PathBuf::from("second.jpg"),
                thumbnail: PathBuf::from("second.webp"),
                width: 8,
                height: 8,
                format: "JPEG".into(),
                size_bytes: 0,
            },
        ];
        let (args, printed) = ThumbnailCache::thumbnail_command(&items, &[1], 64);
        assert!(
            !args.iter().any(|argument| argument == "+delete"),
            "{args:?}"
        );
        assert_eq!(args.last().unwrap(), "second.webp");
        // A measured file is not asked to print, so nothing is read back for it.
        assert!(printed.is_empty());
        // A JPEG is decoded at twice the thumbnail's size rather than in full.
        assert!(args.iter().any(|argument| argument == "jpeg:size=128x128"));

        let (args, printed) = ThumbnailCache::thumbnail_command(&items, &[0, 1], 64);
        assert_eq!(printed, vec![0]);
        assert!(
            args.iter().any(|argument| argument == "+delete"),
            "{args:?}"
        );
    }

    /// Every worker has to be woken by the one that found its work, and the last of them
    /// has to leave rather than wait for a directory nobody is going to queue. A tree
    /// several levels deep, scanned by more workers than it has directories, is where a
    /// missed wake-up or a worker that never gives up shows itself.
    #[test]
    fn a_deep_tree_is_scanned_by_more_workers_than_it_has_directories() {
        let root = std::env::temp_dir().join(format!("bite-deep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut directory = root.clone();
        let mut expected = Vec::new();
        for level in 0..6 {
            directory = directory.join(format!("level-{level}"));
            fs::create_dir_all(&directory).unwrap();
            for index in 0..3 {
                let path = directory.join(format!("image-{level}-{index}.png"));
                fs::write(&path, []).unwrap();
                expected.push(path);
            }
            fs::write(directory.join("notes.txt"), []).unwrap();
        }
        expected.sort();
        let cancelled = AtomicBool::new(false);
        assert_eq!(scan_folder(&root, true, 8, &cancelled).unwrap(), expected);
        // A scan told to stop reports it rather than returning half a folder.
        let stopped = AtomicBool::new(true);
        assert_eq!(
            scan_folder(&root, true, 8, &stopped).unwrap_err(),
            "Cancelled"
        );
        fs::remove_dir_all(root).unwrap();
    }

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
