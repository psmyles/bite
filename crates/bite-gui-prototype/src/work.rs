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

/// The channel the interface drains each frame, plus the flags jobs watch.
pub struct Jobs {
    sender: Sender<Message>,
    receiver: Receiver<Message>,
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
