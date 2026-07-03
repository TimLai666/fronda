//! Video thumbnail extraction via the system ffmpeg executable.
//!
//! External-decoder adapter: no native decoding library is linked into
//! the app. When ffmpeg cannot be started, callers fall back to the
//! type-colored placeholder. Pure std — no gpui.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// ffmpeg executable: FRONDA_FFMPEG overrides, else PATH resolution.
fn ffmpeg_program() -> String {
    std::env::var("FRONDA_FFMPEG").unwrap_or_else(|_| "ffmpeg".into())
}

/// Default on-disk cache directory for extracted thumbnails.
pub fn thumbnail_cache_dir() -> PathBuf {
    crate::project_registry_store::fronda_config_dir().join("thumbnails")
}

/// Cache file for a source: keyed by absolute path + mtime so source
/// updates produce a new key. None when the source is unreadable.
pub fn cache_path_for(source: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let mtime = std::fs::metadata(source).ok()?.modified().ok()?;
    let stamp = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    // FNV-1a over the path bytes — stable, dependency-free.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Some(cache_dir.join(format!("{hash:016x}-{stamp}.png")))
}

/// Extract a ~160px-wide frame at 0.5s into the cache. Cache hits skip
/// ffmpeg entirely; every failure mode returns None.
pub fn extract(source: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let cache = cache_path_for(source, cache_dir)?;
    if cache.is_file() {
        return Some(cache);
    }
    std::fs::create_dir_all(cache_dir).ok()?;
    let status = std::process::Command::new(ffmpeg_program())
        .args(["-y", "-ss", "0.5", "-i"])
        .arg(source)
        .args(["-frames:v", "1", "-vf", "scale=160:-2"])
        .arg(&cache)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    (status.success() && cache.is_file()).then_some(cache)
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
    use std::process::Command;

    /// FRONDA_FFMPEG is process-global; serialize the tests that set it.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

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
    fn missing_ffmpeg_returns_none() {
        let dir = temp_dir("no-ffmpeg");
        let source = dir.join("clip.mp4");
        std::fs::write(&source, b"not a real video").unwrap();

        let _guard = env_lock().lock().unwrap();
        std::env::set_var("FRONDA_FFMPEG", dir.join("no-such-ffmpeg.exe"));
        let result = extract(&source, &dir.join("cache"));
        std::env::remove_var("FRONDA_FFMPEG");
        assert!(result.is_none());
    }

    #[test]
    fn cache_hit_skips_ffmpeg() {
        let dir = temp_dir("hit");
        let source = dir.join("clip.mp4");
        std::fs::write(&source, b"video").unwrap();
        let cache_dir = dir.join("cache");

        let cache = cache_path_for(&source, &cache_dir).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(&cache, b"png").unwrap();

        let _guard = env_lock().lock().unwrap();
        std::env::set_var("FRONDA_FFMPEG", dir.join("no-such-ffmpeg.exe"));
        let result = extract(&source, &cache_dir);
        std::env::remove_var("FRONDA_FFMPEG");
        assert_eq!(result, Some(cache), "cache hit must not need ffmpeg");
    }

    #[test]
    fn extracts_real_frame_when_ffmpeg_available() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("SKIP: ffmpeg not on PATH, real extraction not tested");
            return;
        }
        let dir = temp_dir("real");
        let source = dir.join("test.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=320x240:rate=30",
            ])
            .arg(&source)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "test video generation failed");

        let thumb = extract(&source, &dir.join("cache")).expect("extraction should succeed");
        let bytes = std::fs::read(&thumb).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[1..4], b"PNG", "output is a PNG");
    }
}
