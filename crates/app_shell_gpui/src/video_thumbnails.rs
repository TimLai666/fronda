//! Video thumbnail extraction via statically-linked ffmpeg.
//!
//! ffmpeg is compiled into the binary, so decoding needs no ffmpeg
//! executable at runtime. Any decode failure falls back to the
//! type-colored placeholder. No gpui dependency.

use ffmpeg_the_third as ffmpeg;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Default on-disk cache directory for extracted thumbnails.
pub fn thumbnail_cache_dir() -> PathBuf {
    crate::project_registry_store::fronda_config_dir().join("thumbnails")
}

/// Upper bound on the on-disk thumbnail cache before startup pruning.
pub const THUMBNAIL_CACHE_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// FNV-1a over the source path — stable per source, dependency-free.
fn source_hash(source: &Path) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Stable filename prefix identifying every cached version of a source.
pub fn cache_prefix_for(source: &Path) -> String {
    format!("{}-", source_hash(source))
}

/// Cache file for a source: keyed by path hash + mtime so source
/// updates produce a new key. None when the source is unreadable.
pub fn cache_path_for(source: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let mtime = std::fs::metadata(source).ok()?.modified().ok()?;
    let stamp = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(cache_dir.join(format!("{}{stamp}.png", cache_prefix_for(source))))
}

/// Remove other cached versions of the same source (same prefix, not the
/// kept file). Failures are ignored — the cache is non-critical.
pub fn evict_stale_versions(cache_dir: &Path, prefix: &str, kept: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == kept {
            continue;
        }
        let is_sibling = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(prefix));
        if is_sibling {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Prune the cache directory to `max_bytes`, deleting oldest files first.
/// Returns the number of bytes freed. No-op when already under the cap.
pub fn prune_by_size(cache_dir: &Path, max_bytes: u64) -> u64 {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return 0;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((e.path(), meta.len(), meta.modified().ok()?))
        })
        .collect();
    let total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    if total <= max_bytes {
        return 0;
    }
    // Oldest first.
    files.sort_by_key(|(_, _, modified)| *modified);
    let mut remaining = total;
    let mut freed = 0;
    for (path, size, _) in files {
        if remaining <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            remaining -= size;
            freed += size;
        }
    }
    freed
}

fn init_ffmpeg() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = ffmpeg::init();
    });
}

/// Decode a frame near 0.5s, scale to 160px wide RGB, write PNG to `out`.
fn decode_first_frame_png(source: &Path, out: &Path) -> Option<()> {
    init_ffmpeg();
    let mut ictx = ffmpeg::format::input(source).ok()?;
    let stream = ictx.streams().best(ffmpeg::media::Type::Video)?;
    let video_index = stream.index();
    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters()).ok()?;
    let mut decoder = decoder_ctx.decoder().video().ok()?;

    let (w, h) = (decoder.width(), decoder.height());
    if w == 0 || h == 0 {
        return None;
    }
    let tw = 160u32;
    let th = ((160u64 * h as u64 / w as u64) as u32).max(2) & !1;

    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        w,
        h,
        ffmpeg::format::Pixel::RGB24,
        tw,
        th,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .ok()?;

    // Best-effort seek to 0.5s (AV_TIME_BASE microseconds) to skip black leads.
    let _ = ictx.seek(500_000, ..500_000);

    let mut frame = ffmpeg::frame::Video::empty();
    let mut rgb = ffmpeg::frame::Video::empty();
    let mut got = false;

    'outer: for res in ictx.packets() {
        let Ok((packet_stream, packet)) = res else {
            break;
        };
        if packet_stream.index() != video_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut frame).is_ok() {
            if scaler.run(&frame, &mut rgb).is_ok() {
                got = true;
                break 'outer;
            }
        }
    }
    if !got {
        let _ = decoder.send_eof();
        if decoder.receive_frame(&mut frame).is_ok() {
            got = scaler.run(&frame, &mut rgb).is_ok();
        }
    }
    if !got {
        return None;
    }

    let data = rgb.data(0);
    let stride = rgb.stride(0);
    let mut img = image::RgbImage::new(tw, th);
    for y in 0..th as usize {
        let row = &data[y * stride..];
        for x in 0..tw as usize {
            let p = &row[x * 3..x * 3 + 3];
            img.put_pixel(x as u32, y as u32, image::Rgb([p[0], p[1], p[2]]));
        }
    }
    let tmp = out.with_extension("tmp.png");
    img.save_with_format(&tmp, image::ImageFormat::Png).ok()?;
    std::fs::rename(&tmp, out).ok()?;
    Some(())
}

/// Extract a ~160px-wide frame into the cache. Cache hits skip decoding;
/// every failure mode returns None (caller shows the placeholder).
pub fn extract(source: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let cache = cache_path_for(source, cache_dir)?;
    if cache.is_file() {
        return Some(cache);
    }
    std::fs::create_dir_all(cache_dir).ok()?;
    decode_first_frame_png(source, &cache)?;
    evict_stale_versions(cache_dir, &cache_prefix_for(source), &cache);
    Some(cache)
}

type ResultMap = Mutex<(HashMap<PathBuf, Option<PathBuf>>, HashSet<PathBuf>)>;

fn results() -> &'static ResultMap {
    static INSTANCE: OnceLock<ResultMap> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new((HashMap::new(), HashSet::new())))
}

/// Non-blocking request: returns a cached result immediately, or kicks a
/// background extraction and returns None until it completes. Failures
/// are recorded so a broken source is not retried every frame.
pub fn request_thumbnail(source: &Path) -> Option<PathBuf> {
    let mut guard = results().lock().ok()?;
    let (done, in_flight) = &mut *guard;
    if let Some(result) = done.get(source) {
        return result.clone();
    }
    if in_flight.insert(source.to_path_buf()) {
        let source = source.to_path_buf();
        std::thread::spawn(move || {
            let result = extract(&source, &thumbnail_cache_dir());
            if let Ok(mut guard) = results().lock() {
                let (done, in_flight) = &mut *guard;
                in_flight.remove(&source);
                done.insert(source, result);
            }
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("fronda-video-thumbnails-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cache_key_stable_and_mtime_sensitive() {
        let dir = temp_dir("keys");
        let source = dir.join("clip.mp4");
        std::fs::write(&source, b"a").unwrap();

        let cache = dir.join("cache");
        let first = cache_path_for(&source, &cache).unwrap();
        let again = cache_path_for(&source, &cache).unwrap();
        assert_eq!(first, again, "same source + mtime => same key");

        // Bump mtime well past filesystem resolution.
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(&source, b"ab").unwrap();
        let bumped = cache_path_for(&source, &cache).unwrap();
        assert_ne!(first, bumped, "mtime change produces a new key");
    }

    #[test]
    fn decode_failure_returns_none() {
        let dir = temp_dir("bad-video");
        let source = dir.join("clip.mp4");
        std::fs::write(&source, b"not a real video").unwrap();
        assert!(extract(&source, &dir.join("cache")).is_none());
    }

    #[test]
    fn cache_hit_short_circuits_decode() {
        let dir = temp_dir("hit");
        let source = dir.join("clip.mp4");
        std::fs::write(&source, b"video").unwrap();
        let cache_dir = dir.join("cache");

        // Pre-place the cache file; extract must return it without decoding
        // (the source is not a real video, so decoding would fail).
        let cache = cache_path_for(&source, &cache_dir).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(&cache, b"png").unwrap();

        assert_eq!(extract(&source, &cache_dir), Some(cache));
    }

    /// Write a file then wait long enough for a distinct mtime, so
    /// creation order equals modified-time order without a filetime dep.
    fn write_aged(path: &Path, bytes: usize) {
        std::fs::write(path, vec![0u8; bytes]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    #[test]
    fn evict_removes_same_source_versions_only() {
        let dir = temp_dir("evict");
        let kept = dir.join("aaaa-2000.png");
        std::fs::write(&kept, b"new").unwrap();
        let stale = dir.join("aaaa-1000.png");
        std::fs::write(&stale, b"old").unwrap();
        let other = dir.join("bbbb-1000.png");
        std::fs::write(&other, b"other").unwrap();

        evict_stale_versions(&dir, "aaaa-", &kept);

        assert!(kept.is_file(), "current version kept");
        assert!(!stale.is_file(), "old version of same source removed");
        assert!(other.is_file(), "different source untouched");
    }

    #[test]
    fn prune_deletes_oldest_until_under_cap() {
        let dir = temp_dir("prune");
        // Three 100-byte files, a oldest → c newest; cap 250 bytes.
        write_aged(&dir.join("a.png"), 100);
        write_aged(&dir.join("b.png"), 100);
        write_aged(&dir.join("c.png"), 100);

        let freed = prune_by_size(&dir, 250);
        assert_eq!(freed, 100, "one oldest file freed");
        assert!(!dir.join("a.png").is_file(), "oldest deleted first");
        assert!(dir.join("b.png").is_file());
        assert!(dir.join("c.png").is_file());
    }

    #[test]
    fn prune_under_cap_is_noop() {
        let dir = temp_dir("prune-noop");
        std::fs::write(dir.join("a.png"), vec![0u8; 100]).unwrap();
        assert_eq!(prune_by_size(&dir, 1000), 0);
        assert!(dir.join("a.png").is_file());
    }

    #[test]
    fn prune_empty_dir_is_safe() {
        let dir = temp_dir("prune-empty");
        assert_eq!(prune_by_size(&dir, 1000), 0);
    }

    #[test]
    fn cache_prefix_stable_per_source() {
        let a = cache_prefix_for(Path::new("C:/media/clip.mp4"));
        let b = cache_prefix_for(Path::new("C:/media/clip.mp4"));
        assert_eq!(a, b);
        assert!(a.ends_with('-'));
        assert_ne!(a, cache_prefix_for(Path::new("C:/media/other.mp4")));
    }

    #[test]
    fn decodes_fixture_to_png() {
        // Statically-linked ffmpeg decodes a committed H.264 fixture — no
        // system ffmpeg needed, so this runs everywhere including CI.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
        assert!(fixture.is_file(), "fixture missing: {}", fixture.display());

        let dir = temp_dir("decode");
        let thumb = extract(&fixture, &dir).expect("fixture should decode");
        let bytes = std::fs::read(&thumb).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[1..4], b"PNG", "output is a PNG");

        // A 160-wide thumbnail from a 160x120 source (4:3 → 120 tall).
        let decoded = image::open(&thumb).unwrap();
        assert_eq!(decoded.width(), 160);
        assert_eq!(decoded.height(), 120);
    }
}
