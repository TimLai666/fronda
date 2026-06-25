/// Detected pasteboard content type for media import.
///
/// PST-001: The media panel reports importable clipboard content only
/// when the pasteboard contains file URLs or PNG/TIFF image data.
#[derive(Debug, Clone, PartialEq)]
pub enum PasteboardContent {
    /// PST-002(1): One or more file URLs (highest priority)
    FileUrls(Vec<String>),
    /// PST-002(2): PNG image data
    Png,
    /// PST-002(3): TIFF image data
    Tiff,
    /// No importable content
    Empty,
}

impl PasteboardContent {
    /// Priority value: lower = higher priority
    fn priority(&self) -> u8 {
        match self {
            PasteboardContent::FileUrls(_) => 0,
            PasteboardContent::Png => 1,
            PasteboardContent::Tiff => 2,
            PasteboardContent::Empty => 3,
        }
    }
}

/// PST-002/PST-003: Detect importable clipboard content from flags.
///
/// Priority: file URLs > PNG > TIFF > Empty.
/// PST-003: If both file URLs and image bytes are present, file URLs win.
pub fn detect_pasteboard_content(
    has_file_urls: bool,
    has_png: bool,
    has_tiff: bool,
) -> PasteboardContent {
    if has_file_urls {
        PasteboardContent::FileUrls(Vec::new())
    } else if has_png {
        PasteboardContent::Png
    } else if has_tiff {
        PasteboardContent::Tiff
    } else {
        PasteboardContent::Empty
    }
}

/// PST-004: Output file extension for pasteboard image data.
///
/// File URLs return None (extension is derived from the actual file).
pub fn output_extension_for(content: &PasteboardContent) -> Option<&'static str> {
    match content {
        PasteboardContent::FileUrls(_) => None,
        PasteboardContent::Png => Some("png"),
        PasteboardContent::Tiff => Some("tiff"),
        PasteboardContent::Empty => None,
    }
}

/// PST-002/PST-003: Select the highest-priority content from a slice.
pub fn highest_priority_content<'a>(
    contents: &'a [PasteboardContent],
) -> Option<&'a PasteboardContent> {
    contents
        .iter()
        .min_by_key(|c| c.priority())
        .filter(|c| !matches!(c, PasteboardContent::Empty))
}

/// A resolved pasteboard import request.
#[derive(Debug, Clone, PartialEq)]
pub struct PasteboardImportRequest {
    /// The resolved content to import
    pub content: PasteboardContent,
    /// PST-005: Target folder (None = library root)
    pub target_folder_id: Option<String>,
}

impl PasteboardImportRequest {
    /// Create an import request from clipboard state.
    /// PST-005: If `current_folder_id` is provided, imports into that folder.
    pub fn resolve(
        has_file_urls: bool,
        has_png: bool,
        has_tiff: bool,
        current_folder_id: Option<String>,
    ) -> Self {
        Self {
            content: detect_pasteboard_content(has_file_urls, has_png, has_tiff),
            target_folder_id: current_folder_id,
        }
    }

    /// Returns true if the clipboard has importable content.
    pub fn is_importable(&self) -> bool {
        !matches!(self.content, PasteboardContent::Empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PST-001: Empty clipboard is not importable ──
    #[test]
    fn pst_001_empty_not_importable() {
        let result = detect_pasteboard_content(false, false, false);
        assert_eq!(result, PasteboardContent::Empty);
    }

    // ── PST-001: File URLs are importable ──
    #[test]
    fn pst_001_file_urls_importable() {
        let result = detect_pasteboard_content(true, false, false);
        assert_eq!(result, PasteboardContent::FileUrls(vec![]));
    }

    // ── PST-001: PNG is importable ──
    #[test]
    fn pst_001_png_importable() {
        let result = detect_pasteboard_content(false, true, false);
        assert_eq!(result, PasteboardContent::Png);
    }

    // ── PST-001: TIFF is importable ──
    #[test]
    fn pst_001_tiff_importable() {
        let result = detect_pasteboard_content(false, false, true);
        assert_eq!(result, PasteboardContent::Tiff);
    }

    // ── PST-002: Priority ordering: file URLs > PNG > TIFF ──
    #[test]
    fn pst_002_priority_file_urls_over_png() {
        let result = detect_pasteboard_content(true, true, false);
        assert_eq!(result, PasteboardContent::FileUrls(vec![]));
    }

    #[test]
    fn pst_002_priority_png_over_tiff() {
        let result = detect_pasteboard_content(false, true, true);
        assert_eq!(result, PasteboardContent::Png);
    }

    // ── PST-003: File URLs win over image data ──
    #[test]
    fn pst_003_file_urls_win_over_all() {
        let result = detect_pasteboard_content(true, true, true);
        assert_eq!(result, PasteboardContent::FileUrls(vec![]));
    }

    // ── PST-004: Output extension for PNG ──
    #[test]
    fn pst_004_extension_png() {
        assert_eq!(output_extension_for(&PasteboardContent::Png), Some("png"));
    }

    // ── PST-004: Output extension for TIFF ──
    #[test]
    fn pst_004_extension_tiff() {
        assert_eq!(output_extension_for(&PasteboardContent::Tiff), Some("tiff"));
    }

    // ── PST-004: File URLs have no fixed extension ──
    #[test]
    fn pst_004_extension_file_urls() {
        assert_eq!(
            output_extension_for(&PasteboardContent::FileUrls(vec![])),
            None
        );
    }

    // ── PST-004: Empty has no extension ──
    #[test]
    fn pst_004_extension_empty() {
        assert_eq!(output_extension_for(&PasteboardContent::Empty), None);
    }

    // ── PST-005: Import request with folder ──
    #[test]
    fn pst_005_import_request_with_folder() {
        let req = PasteboardImportRequest::resolve(true, false, false, Some("folder1".into()));
        assert!(req.is_importable());
        assert_eq!(req.target_folder_id, Some("folder1".into()));
        assert_eq!(req.content, PasteboardContent::FileUrls(vec![]));
    }

    // ── PST-005: Import request without folder (root) ──
    #[test]
    fn pst_005_import_request_root() {
        let req = PasteboardImportRequest::resolve(false, true, false, None);
        assert!(req.is_importable());
        assert_eq!(req.target_folder_id, None);
    }

    // ── PST-005: Empty clipboard not importable ──
    #[test]
    fn pst_005_empty_not_importable() {
        let req = PasteboardImportRequest::resolve(false, false, false, None);
        assert!(!req.is_importable());
    }

    // ── highest_priority_content: picks the best ──
    #[test]
    fn highest_priority_file_urls() {
        let contents = vec![
            PasteboardContent::Png,
            PasteboardContent::FileUrls(vec![]),
            PasteboardContent::Tiff,
        ];
        let best = highest_priority_content(&contents);
        assert_eq!(best, Some(&PasteboardContent::FileUrls(vec![])));
    }

    // ── highest_priority_content: only empty returns None ──
    #[test]
    fn highest_priority_empty_only() {
        let contents = vec![PasteboardContent::Empty];
        let best = highest_priority_content(&contents);
        assert!(best.is_none());
    }

    // ── highest_priority_content: empty slice returns None ──
    #[test]
    fn highest_priority_empty_slice() {
        let contents: Vec<PasteboardContent> = vec![];
        let best = highest_priority_content(&contents);
        assert!(best.is_none());
    }
}
