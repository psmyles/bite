//! Times a cached re-import, the way the filmstrip does one.
use bite_gui_prototype::work;
use std::{path::PathBuf, sync::{Arc, atomic::AtomicBool}, time::Instant};

fn main() {
    let directory = std::env::args().nth(1).expect("a folder of images");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .expect("the folder opens")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    paths.sort();
    let mut magick = bite_imagemagick::Magick::discover(Arc::new(AtomicBool::new(false)));
    let mut cache = bite_imagemagick::import::ThumbnailCache::new(work::cache_directory());
    for pass in 0..2 {
        let started = Instant::now();
        let mut decoded = 0;
        for chunk in paths.chunks(16) {
            for info in cache.load_batch(&mut magick, chunk, 256).expect("thumbnails") {
                let bytes = std::fs::read(&info.thumbnail).expect("the thumbnail reads");
                let image = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                    work::decode_png(&bytes)
                } else {
                    work::decode_webp(&bytes)
                }
                .expect("the thumbnail decodes");
                decoded += usize::from(image.width > 0);
                if pass == 0 && decoded <= 2 {
                    println!(
                        "  {} measured {}x{} {}",
                        info.path.file_name().unwrap_or_default().to_string_lossy(),
                        info.width,
                        info.height,
                        info.format
                    );
                }
            }
        }
        println!("pass {pass}: {decoded} images in {:.2} sec", started.elapsed().as_secs_f32());
    }
}
