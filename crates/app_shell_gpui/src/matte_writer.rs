//! Host writer for `import_media` matte imports (#242, formerly the
//! create_matte tool): renders a solid-colour PNG and writes it
//! into the open project's `media/` directory, returning a project-relative [`MediaSource`].
//! Implements [`agent_contract::MatteWriter`] so the pure executor stays FS-free.

use agent_contract::MatteWriter;
use core_model::MediaSource;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Writes mattes into a specific project package's `media/` directory.
pub struct ProjectMatteWriter {
    project_root: PathBuf,
}

impl ProjectMatteWriter {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl MatteWriter for ProjectMatteWriter {
    fn write_matte(
        &self,
        rgba: [u8; 4],
        width: i64,
        height: i64,
        base_name: &str,
    ) -> Result<MediaSource, String> {
        let w = width.clamp(1, 16_384) as u32;
        let h = height.clamp(1, 16_384) as u32;
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));

        let media_dir = self.project_root.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|e| format!("create media dir: {e}"))?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let file = format!("matte-{}-{stamp}.png", slugify(base_name));
        let path = media_dir.join(&file);
        img.save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| format!("write matte png: {e}"))?;

        Ok(MediaSource::Project {
            relative_path: format!("media/{file}"),
        })
    }
}

/// Filesystem-safe slug from a display name; non-alphanumerics collapse to `-`, capped at 40 chars.
fn slugify(name: &str) -> String {
    let raw: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = raw.trim_matches('-');
    if trimmed.is_empty() {
        "matte".to_string()
    } else {
        trimmed.chars().take(40).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fronda-matte-test-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_valid_solid_png_into_media_dir() {
        let root = temp_project();
        let writer = ProjectMatteWriter::new(root.clone());
        let src = writer
            .write_matte([255, 0, 0, 255], 4, 6, "Red BG!")
            .unwrap();
        let MediaSource::Project { relative_path } = src else {
            panic!("expected a project-relative source");
        };
        assert!(
            relative_path.starts_with("media/matte-red-bg-"),
            "slugged name: {relative_path}"
        );
        let path = root.join(&relative_path);
        assert!(path.is_file(), "png written to disk");
        // Decodes as a 4x6 solid-red PNG.
        let img = image::open(&path).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (4, 6));
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(3, 5).0, [255, 0, 0, 255]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn slugify_handles_empty_and_symbols() {
        assert_eq!(slugify("  "), "matte");
        assert_eq!(slugify("!!!"), "matte");
        assert_eq!(slugify("Lower Third"), "lower-third");
    }
}
