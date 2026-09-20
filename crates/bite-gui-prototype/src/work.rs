//! Background work: imports, previews and workflow runs.
//!
//! Every job runs off the interface thread and wakes the event loop when it has something
//! to report, so progress appears without the pointer having to move.
use bite_core::execution::BatchResult;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};

/// A message from a background job.
#[derive(Debug)]
pub enum Message {
    ImportProgress {
        node: String,
        completed: usize,
        total: usize,
    },
    /// Thumbnails for one branch, in the order they were requested.
    ImportFinished {
        node: String,
        paths: Vec<PathBuf>,
        thumbnails: Vec<Thumbnail>,
    },
    ImportFailed {
        node: String,
        error: String,
    },
    ScanFinished {
        node: String,
        count: usize,
    },
    PreviewFinished {
        node: String,
        index: usize,
        image: DecodedImage,
        /// The file the preview was made from, which the information overlay describes.
        source: SourceImage,
        resolved: std::collections::BTreeMap<String, bite_schema::Params>,
    },
    PreviewFailed(String),
    TextPreview {
        lines: Vec<String>,
    },
    RunProgress {
        completed: usize,
        total: usize,
        file: String,
    },
    RunFinished(Box<BatchResult>),
    RunFailed(String),
    UpdateChecked(Box<Result<crate::updates::UpdateInfo, String>>),
}

/// A decoded thumbnail ready for upload.
#[derive(Debug)]
pub struct Thumbnail {
    pub path: PathBuf,
    pub image: DecodedImage,
    /// The file the thumbnail was made from, for the preview overlay.
    pub source: SourceImage,
}

/// Straight eight-bit pixels with their dimensions.
#[derive(Debug, Default, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// The encoded size, which the preview overlay reports.
    pub bytes: u64,
}

/// What the overlay reports about the image the preview came from. The render itself runs
/// on a thumbnail, so its own dimensions are not the ones to show.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

/// One preview to render, handed to the worker that owns the ImageMagick session.
pub struct PreviewJob {
    pub graph: bite_schema::Graph,
    pub registry: Arc<bite_core::Registry>,
    /// The file the filmstrip has selected, which the parameters are measured against.
    pub path: PathBuf,
    /// The Input branch the result belongs to, and the position within it.
    pub node: String,
    pub index: usize,
    pub thumbnail_size: u32,
    /// The node the chain is rendered up to.
    pub target: String,
}

/// The channel the interface drains each frame, plus the flags jobs watch.
pub struct Jobs {
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    /// The worker previews are handed to, started with the first one asked for.
    preview_jobs: Option<Sender<PreviewJob>>,
    /// Raised when a preview needs recomputing once the current one finishes.
    preview_requested: bool,
    pub import_cancelled: Arc<AtomicBool>,
    /// Incremented per preview request so that a stale result can be discarded.
    pub preview_generation: u64,
    /// Set while a waker is available, so jobs can nudge the event loop.
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver,
            preview_jobs: None,
            preview_requested: false,
            import_cancelled: Arc::new(AtomicBool::new(false)),
            preview_generation: 0,
            waker: None,
        }
    }
}

impl Jobs {
    /// Installs the callback jobs use to wake the event loop.
    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.waker = Some(waker);
    }

    /// A handle a job can send through and wake with.
    pub fn handle(&self) -> JobHandle {
        JobHandle {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }

    pub fn try_recv(&self) -> Option<Message> {
        self.receiver.try_recv().ok()
    }

    pub fn request_preview(&mut self) {
        self.preview_requested = true;
    }

    /// Hands a preview to the worker, starting it on the first call.
    ///
    /// One long lived thread does every preview, because the ImageMagick session caches
    /// what it has measured and the thumbnail cache remembers what it has made. A thread
    /// per preview threw both away, so each switch between two images paid for the same
    /// two `identify` processes again.
    pub fn submit_preview(&mut self, job: PreviewJob) {
        let handle = self.handle();
        let sender = self.preview_jobs.get_or_insert_with(|| {
            let (sender, receiver) = channel::<PreviewJob>();
            std::thread::spawn(move || preview_worker(&receiver, &handle));
            sender
        });
        let _ = sender.send(job);
    }

    /// Takes the pending preview request, if there is one.
    pub fn take_preview_request(&mut self) -> bool {
        std::mem::take(&mut self.preview_requested)
    }

    pub fn cancel_import(&self) {
        self.import_cancelled.store(true, Ordering::Relaxed);
    }

    pub fn reset_import_cancel(&mut self) {
        self.import_cancelled = Arc::new(AtomicBool::new(false));
    }
}

/// The sending half handed to a background thread.
#[derive(Clone)]
pub struct JobHandle {
    sender: Sender<Message>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl JobHandle {
    /// Sends a message and wakes the event loop so it is drawn promptly.
    pub fn send(&self, message: Message) {
        if self.sender.send(message).is_ok() {
            if let Some(waker) = &self.waker {
                waker();
            }
        }
    }
}

/// Decodes a portable network graphics image into straight eight-bit pixels.
/// Decodes a set of cached thumbnails at once, across `jobs` threads, keeping the order.
///
/// Reading and decoding is all this does, so it scales with the cores. An entry that is
/// neither a portable network graphic nor a WebP comes back as an error for the caller to
/// put through ImageMagick, which the cache's own files never need.
pub fn decode_many(paths: &[PathBuf], jobs: usize) -> Vec<Result<DecodedImage, String>> {
    let one = |path: &PathBuf| -> Result<DecodedImage, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return decode_png(&bytes);
        }
        if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            return decode_webp(&bytes);
        }
        Err("not a thumbnail this can decode".into())
    };
    let workers = jobs.max(1).min(paths.len());
    if workers <= 1 {
        return paths.iter().map(one).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<Result<DecodedImage, String>>>> =
        paths.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let next = &next;
            let results = &results;
            let one = &one;
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
        .map(|slot| {
            slot.into_inner()
                .unwrap()
                .unwrap_or_else(|| Err("not decoded".into()))
        })
        .collect()
}

/// Decodes a WebP thumbnail without leaving the process.
///
/// The thumbnail cache stores WebP, which is a tenth the size of the same picture as a
/// portable network graphic. Electron hands those straight to the browser to decode; here
/// they went back through ImageMagick, one process per image, which is what made
/// re-importing an already cached folder take a second.
pub fn decode_webp(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let mut buffer = vec![0u8; decoder.output_buffer_size().ok_or("Image is too large")?];
    decoder
        .read_image(&mut buffer)
        .map_err(|error| error.to_string())?;
    let pixel_count = width as usize * height as usize;
    let pixels = if has_alpha {
        buffer
    } else {
        let mut pixels = vec![0u8; pixel_count * 4];
        for index in 0..pixel_count {
            pixels[index * 4..index * 4 + 3].copy_from_slice(&buffer[index * 3..index * 3 + 3]);
            pixels[index * 4 + 3] = 255;
        }
        pixels
    };
    Ok(DecodedImage {
        width,
        height,
        pixels,
        bytes: bytes.len() as u64,
    })
}

pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // Expanding palettes and narrowing sixteen-bit channels keeps one decode path.
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("Image is too large")?];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    let width = info.width;
    let height = info.height;
    let pixel_count = width as usize * height as usize;
    let mut pixels = vec![0u8; pixel_count * 4];
    match info.color_type {
        png::ColorType::Rgba => pixels.copy_from_slice(&buffer[..pixel_count * 4]),
        png::ColorType::Rgb => {
            for index in 0..pixel_count {
                pixels[index * 4..index * 4 + 3]
                    .copy_from_slice(&buffer[index * 3..index * 3 + 3]);
                pixels[index * 4 + 3] = 255;
            }
        }
        png::ColorType::Grayscale => {
            for index in 0..pixel_count {
                let value = buffer[index];
                pixels[index * 4..index * 4 + 3].copy_from_slice(&[value, value, value]);
                pixels[index * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for index in 0..pixel_count {
                let value = buffer[index * 2];
                pixels[index * 4..index * 4 + 3].copy_from_slice(&[value, value, value]);
                pixels[index * 4 + 3] = buffer[index * 2 + 1];
            }
        }
        png::ColorType::Indexed => {
            return Err("Indexed images must be expanded before decoding".into());
        }
    }
    Ok(DecodedImage {
        width,
        height,
        pixels,
        bytes: bytes.len() as u64,
    })
}

/// The directory the thumbnail cache uses, which matches the Electron temporary folder.
/// How many rendered previews the worker keeps. Each holds one thumbnail's pixels, so a
/// filmstrip's worth of switching stays in memory without it growing without bound.
const PREVIEW_CACHE_LIMIT: usize = 24;

/// Renders previews one after another, keeping its caches between them.
fn preview_worker(jobs: &Receiver<PreviewJob>, handle: &JobHandle) {
    let mut magick = bite_imagemagick::Magick::discover(Arc::new(AtomicBool::new(false)));
    let mut cache = bite_imagemagick::import::ThumbnailCache::new(cache_directory());
    let mut rendered: Vec<(String, RenderedPreview)> = Vec::new();
    while let Ok(job) = jobs.recv() {
        // Only the newest request matters. Rendering the ones behind it would show images
        // the filmstrip has already moved off, so they are dropped.
        let mut job = job;
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let key = preview_key(&job);
        if let Some(index) = rendered.iter().position(|(cached, _)| *cached == key) {
            // Going back to an image whose chain has not changed since costs nothing.
            let entry = rendered.remove(index);
            handle.send(entry.1.message(&job));
            rendered.push(entry);
            continue;
        }
        match render_preview(&mut magick, &mut cache, &job) {
            Ok(preview) => {
                handle.send(preview.message(&job));
                rendered.push((key, preview));
                if rendered.len() > PREVIEW_CACHE_LIMIT {
                    rendered.remove(0);
                }
            }
            Err(error) => handle.send(Message::PreviewFailed(error)),
        }
    }
}

/// What a rendered preview depends on: the file, the node it is rendered up to, and the
/// shape and parameters of the graph. Positions and selection are left out, as the Svelte
/// component leaves them out of the key that retriggers a render.
fn preview_key(job: &PreviewJob) -> String {
    use std::fmt::Write;
    let mut key = format!(
        "{}|{}|{}",
        job.path.to_string_lossy(),
        job.thumbnail_size,
        job.target
    );
    for node in &job.graph.nodes {
        let _ = write!(key, "|{}:{}:", node.id, node.data.definition_id);
        for (name, value) in &node.data.params {
            let _ = write!(key, "{name}={value:?},");
        }
    }
    for edge in &job.graph.edges {
        let _ = write!(
            key,
            "|{}:{}->{}:{}",
            edge.source, edge.source_handle, edge.target, edge.target_handle
        );
    }
    key
}

/// A finished preview, kept so that returning to it needs no work.
#[derive(Clone)]
struct RenderedPreview {
    image: DecodedImage,
    source: SourceImage,
    resolved: std::collections::BTreeMap<String, bite_schema::Params>,
}

impl RenderedPreview {
    /// Addresses the preview to the branch and position that asked for it.
    fn message(&self, job: &PreviewJob) -> Message {
        Message::PreviewFinished {
            node: job.node.clone(),
            index: job.index,
            image: self.image.clone(),
            source: self.source,
            resolved: self.resolved.clone(),
        }
    }
}

/// Runs one chain over the selected image's cached thumbnail.
fn render_preview(
    magick: &mut bite_imagemagick::Magick,
    cache: &mut bite_imagemagick::import::ThumbnailCache,
    job: &PreviewJob,
) -> Result<RenderedPreview, String> {
    let thumbnail = cache
        .load_batch(magick, std::slice::from_ref(&job.path), job.thumbnail_size)?
        .into_iter()
        .next()
        .ok_or("no thumbnail")?;
    let source = SourceImage {
        width: thumbnail.width,
        height: thumbnail.height,
        bytes: thumbnail.size_bytes,
    };
    // The chain runs over the thumbnail for speed while parameters written as a share of
    // the image measure against the original, as the Electron preview pipeline does.
    let result = bite_core::preview::render_from(
        &job.graph,
        &job.registry,
        magick,
        &thumbnail.thumbnail,
        &job.path,
        Some((job.target.as_str(), "out:output")),
    )?;
    Ok(RenderedPreview {
        image: decode_png(&result.png)?,
        source,
        resolved: result
            .resolved_values
            .into_iter()
            .map(|(id, context)| {
                (
                    id,
                    context
                        .into_iter()
                        .map(|(name, value)| (name, param_from_value(value)))
                        .collect(),
                )
            })
            .collect(),
    })
}

/// Converts a computed expression value back into a stored parameter.
fn param_from_value(value: bite_expr::Value) -> bite_schema::ParamValue {
    use bite_schema::ParamValue;
    match value {
        bite_expr::Value::Null => ParamValue::Null,
        bite_expr::Value::Bool(value) => ParamValue::Bool(value),
        bite_expr::Value::Int(value) => ParamValue::Int(value),
        bite_expr::Value::Float(value) => ParamValue::Number(value),
        bite_expr::Value::String(value) => ParamValue::String(value),
        bite_expr::Value::Vector(values) => ParamValue::Vector(values),
    }
}

pub fn cache_directory() -> PathBuf {
    std::env::temp_dir().join("bite-preview")
}

/// Removes cached thumbnails, as the Debug menu's cache action does.
pub fn clear_cache() -> Result<usize, String> {
    let directory = cache_directory();
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if (name.starts_with("thumb_") || name.starts_with("preview_"))
            && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
    }
    Ok(removed)
}

/// Deletes stale cache entries at startup, matching the Electron pruning rules.
pub fn prune_cache() {
    let directory = cache_directory();
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return;
    };
    let fortnight = std::time::Duration::from_secs(14 * 24 * 60 * 60);
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Interrupted batch scratch files are always removed.
        if name.starts_with("batch_ms_") {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        if !(name.starts_with("thumb_") || name.starts_with("preview_")) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map(|modified| {
                now.duration_since(modified)
                    .map(|age| age > fortnight)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A job over a one node graph, for the cache key checks.
    fn preview_job() -> PreviewJob {
        let mut graph = bite_schema::Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            viewport: bite_schema::Viewport {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        graph.nodes.push(bite_schema::GraphNode {
            id: "one".into(),
            kind: bite_schema::NodeKind::Processing(bite_schema::ProcessingNodeKind::Process),
            position: bite_schema::Position { x: 0.0, y: 0.0 },
            parent_id: None,
            extent: None,
            width: None,
            height: None,
            data: bite_schema::NodeData {
                label: "Blur".into(),
                definition_id: "blur".into(),
                params: std::collections::BTreeMap::from([(
                    "radius".to_string(),
                    bite_schema::ParamValue::Int(4),
                )]),
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
        });
        PreviewJob {
            graph,
            registry: Arc::new(bite_core::Registry::default()),
            path: PathBuf::from("/images/one.png"),
            node: "input".into(),
            index: 0,
            thumbnail_size: 256,
            target: "one".into(),
        }
    }

    #[test]
    fn a_cached_preview_is_keyed_by_the_file_and_what_processes_it() {
        let job = preview_job();
        let same = preview_key(&preview_job());
        assert_eq!(preview_key(&job), same);

        let mut moved = preview_job();
        moved.graph.nodes[0].position = bite_schema::Position { x: 400.0, y: 90.0 };
        // Moving a card does not change the picture it makes.
        assert_eq!(preview_key(&moved), same);

        let mut edited = preview_job();
        edited.graph.nodes[0]
            .data
            .params
            .insert("radius".into(), bite_schema::ParamValue::Int(9));
        assert_ne!(preview_key(&edited), same);

        let mut elsewhere = preview_job();
        elsewhere.path = PathBuf::from("/images/two.png");
        assert_ne!(preview_key(&elsewhere), same);

        let mut further = preview_job();
        further.target = "two".into();
        assert_ne!(preview_key(&further), same);
    }

    /// A two by two lossless WebP with an opaque red and green pixel on the top row and
    /// two transparent ones beneath, written by ImageMagick.
    const WEBP_FIXTURE: [u8; 44] = [
        0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x18, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x10, 0x17, 0x20, 0x10, 0x48, 0xda,
        0x1f, 0x7a, 0x8d, 0xf9, 0x8f, 0xf9, 0x0f, 0x6c, 0x61, 0x0c, 0x22, 0xfa, 0x1f, 0x01,
    ];

    #[test]
    fn a_webp_thumbnail_decodes_without_leaving_the_process() {
        let image = decode_webp(&WEBP_FIXTURE).expect("the fixture decodes");
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.pixels.len(), 2 * 2 * 4);
        assert_eq!(&image.pixels[0..4], &[0xff, 0x00, 0x00, 0xff]);
        assert_eq!(&image.pixels[4..8], &[0x00, 0xff, 0x00, 0xff]);
        // The bottom row is transparent, so every channel of it reads as nothing.
        assert!(image.pixels[8..].iter().all(|channel| *channel == 0));
    }

    #[test]
    fn a_preview_request_is_taken_exactly_once() {
        let mut jobs = Jobs::default();
        assert!(!jobs.take_preview_request());
        jobs.request_preview();
        assert!(jobs.take_preview_request());
        assert!(!jobs.take_preview_request());
    }

    #[test]
    fn messages_reach_the_interface_thread() {
        let jobs = Jobs::default();
        let handle = jobs.handle();
        handle.send(Message::PreviewFailed("boom".into()));
        match jobs.try_recv() {
            Some(Message::PreviewFailed(error)) => assert_eq!(error, "boom"),
            other => panic!("unexpected message: {other:?}"),
        }
        assert!(jobs.try_recv().is_none());
    }

    #[test]
    fn cancelling_an_import_raises_the_shared_flag() {
        let mut jobs = Jobs::default();
        assert!(!jobs.import_cancelled.load(Ordering::Relaxed));
        jobs.cancel_import();
        assert!(jobs.import_cancelled.load(Ordering::Relaxed));
        jobs.reset_import_cancel();
        assert!(!jobs.import_cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn a_grayscale_image_decodes_to_opaque_pixels() {
        // A one pixel grayscale image encoded as a portable network graphic.
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 1, 1);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[128]).unwrap();
        }
        let decoded = decode_png(&encoded).unwrap();
        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 1);
        assert_eq!(decoded.pixels, vec![128, 128, 128, 255]);
    }

    #[test]
    fn the_cache_directory_sits_under_the_temporary_folder() {
        let directory = cache_directory();
        assert!(directory.ends_with("bite-preview"));
    }
}
