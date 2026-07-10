//! Import local files into the shared manifest via the import_media tool.

use std::path::{Path, PathBuf};

use core_model::ClipType;

/// Import each file into the shared executor. Unrecognized extensions are
/// skipped (logged) without aborting the remaining files.
pub fn import_files_into_shared_state(paths: &[PathBuf]) {
    let executor = crate::editor_state_hub::EditorStateHub::global().executor();
    let guard = executor.lock();
    let Ok(mut exec) = guard else {
        return;
    };
    for path in paths {
        if let Err(reason) = import_one(&mut exec, path) {
            eprintln!("Import skipped {}: {reason}", path.display());
        }
    }
}

/// Import a single file; pure logic over an executor, unit-testable.
pub fn import_one(exec: &mut agent_contract::ToolExecutor, path: &Path) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| "no file extension".to_string())?;
    // Early skip keeps the log message meaningful; the executor re-checks.
    ClipType::from_extension(ext).ok_or_else(|| format!("unknown extension .{ext}"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "unreadable file name".to_string())?;
    exec.execute(
        "import_media",
        &serde_json::json!({
            "source": { "path": path.to_string_lossy() },
            "name": name,
        }),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_contract::ToolExecutor;
    use core_model::{MediaManifest, Timeline};

    #[test]
    fn imports_image_and_video_with_correct_types() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        import_one(&mut exec, Path::new("C:/media/photo.png")).unwrap();
        import_one(&mut exec, Path::new("C:/media/take1.mp4")).unwrap();

        let entries = &exec.media_manifest().entries;
        assert_eq!(entries.len(), 2);
        let photo = entries.iter().find(|e| e.name == "photo.png").unwrap();
        assert_eq!(photo.r#type, ClipType::Image);
        let take = entries.iter().find(|e| e.name == "take1.mp4").unwrap();
        assert_eq!(take.r#type, ClipType::Video);
    }

    #[test]
    fn unknown_extension_is_skipped_with_error() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        let err = import_one(&mut exec, Path::new("C:/media/notes.txt")).unwrap_err();
        assert!(err.contains("unknown extension"), "err={err}");
        assert!(exec.media_manifest().entries.is_empty());
    }
}
