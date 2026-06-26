use std::fmt;

/// A parsed internal drag payload item
#[derive(Debug, Clone, PartialEq)]
pub enum DragItem {
    Asset(String),
    Folder(String),
}

/// Parse result for drag payload strings
#[derive(Debug, Clone, PartialEq)]
pub struct DragPayload {
    pub items: Vec<DragItem>,
}

impl fmt::Display for DragItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DragItem::Asset(id) => write!(f, "asset({id})"),
            DragItem::Folder(id) => write!(f, "folder({id})"),
        }
    }
}

impl fmt::Display for DragPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.items.iter().map(|i| i.to_string()).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// Errors that can occur during drag payload parsing.
/// Not currently returned — reserved for stricter validation.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum DragParseError {
    /// Empty input
    Empty,
}

/// DRAG-001: Internal media drag payloads use `palmier-asset://<id>` strings
/// DRAG-002: Internal folder drag payloads use `palmier-folder://<id>` strings
/// DRAG-003: Finder file URLs must never be mistaken for internal asset/folder payloads
///   (file:// URLs will not match our scheme, so they'll be recognized as external)
/// DRAG-004: Internal payloads may contain multiple newline-separated items
/// DRAG-005: Mixed asset-and-folder internal payloads are valid
/// DRAG-006: Unknown ids and malformed lines in internal payloads are ignored rather than crashing
pub fn parse_drag_payload(input: &str) -> DragPayload {
    let items = input
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            if let Some(id) = line.strip_prefix("palmier-asset://") {
                if !id.is_empty() {
                    return Some(DragItem::Asset(id.to_string()));
                }
            }
            if let Some(id) = line.strip_prefix("palmier-folder://") {
                if !id.is_empty() {
                    return Some(DragItem::Folder(id.to_string()));
                }
            }
            None
        })
        .collect();

    DragPayload { items }
}

/// Check if a URL string is an internal drag payload (not a Finder file URL)
pub fn is_internal_drag_payload(input: &str) -> bool {
    input.starts_with("palmier-asset://") || input.starts_with("palmier-folder://")
}

#[cfg(test)]
mod tests {
    use super::*;

    // DRAG-001: palmier-asset://abc123 parses as Asset("abc123")
    #[test]
    fn drag_001_single_asset() {
        let result = parse_drag_payload("palmier-asset://abc123");
        assert_eq!(
            result,
            DragPayload {
                items: vec![DragItem::Asset("abc123".into())]
            }
        );
    }

    // DRAG-002: palmier-folder://folder1 parses as Folder("folder1")
    #[test]
    fn drag_002_single_folder() {
        let result = parse_drag_payload("palmier-folder://folder1");
        assert_eq!(
            result,
            DragPayload {
                items: vec![DragItem::Folder("folder1".into())]
            }
        );
    }

    // DRAG-003: file:// URLs are NOT internal; https:// URLs are NOT internal
    #[test]
    fn drag_003_file_url_not_internal() {
        assert!(!is_internal_drag_payload("file:///Users/me/video.mp4"));
        assert!(!is_internal_drag_payload("https://example.com"));
        // Internal ones still return true
        assert!(is_internal_drag_payload("palmier-asset://abc"));
        assert!(is_internal_drag_payload("palmier-folder://xyz"));
    }

    #[test]
    fn drag_003_file_url_not_parsed_as_asset() {
        let result = parse_drag_payload("file:///Users/me/video.mp4");
        assert_eq!(result, DragPayload { items: vec![] });
    }

    // DRAG-004: Multiple newline-separated items parse correctly
    #[test]
    fn drag_004_multiple_assets() {
        let result = parse_drag_payload(
            "palmier-asset://abc123\npalmier-asset://def456\npalmier-asset://ghi789",
        );
        assert_eq!(
            result,
            DragPayload {
                items: vec![
                    DragItem::Asset("abc123".into()),
                    DragItem::Asset("def456".into()),
                    DragItem::Asset("ghi789".into()),
                ]
            }
        );
    }

    // DRAG-005: Mixed asset+folder payloads work
    #[test]
    fn drag_005_mixed_asset_and_folder() {
        let result = parse_drag_payload(
            "palmier-asset://vid1\npalmier-folder://myfolder\npalmier-asset://vid2",
        );
        assert_eq!(
            result,
            DragPayload {
                items: vec![
                    DragItem::Asset("vid1".into()),
                    DragItem::Folder("myfolder".into()),
                    DragItem::Asset("vid2".into()),
                ]
            }
        );
    }

    // DRAG-006: Unknown lines silently ignored, not crashing
    #[test]
    fn drag_006_unknown_lines_ignored() {
        let result = parse_drag_payload(
            "palmier-asset://good\n  garbage  \nunknown://bad\npalmier-folder://also_good",
        );
        assert_eq!(
            result,
            DragPayload {
                items: vec![
                    DragItem::Asset("good".into()),
                    DragItem::Folder("also_good".into()),
                ]
            }
        );
    }

    // Empty input returns empty items list
    #[test]
    fn empty_input() {
        let result = parse_drag_payload("");
        assert_eq!(result, DragPayload { items: vec![] });
    }

    // Whitespace around lines is trimmed
    #[test]
    fn whitespace_trimmed() {
        let result = parse_drag_payload("  palmier-asset://abc  \n  \n  palmier-folder://xyz  ");
        assert_eq!(
            result,
            DragPayload {
                items: vec![
                    DragItem::Asset("abc".into()),
                    DragItem::Folder("xyz".into()),
                ]
            }
        );
    }

    // Empty scheme IDs are ignored
    #[test]
    fn empty_id_ignored() {
        let result = parse_drag_payload("palmier-asset://\npalmier-folder://");
        assert_eq!(result, DragPayload { items: vec![] });
    }

    // is_internal_drag_payload works for single-line inputs
    #[test]
    fn is_internal_drag_payload_on_asset() {
        assert!(is_internal_drag_payload("palmier-asset://abc123"));
    }

    #[test]
    fn is_internal_drag_payload_on_folder() {
        assert!(is_internal_drag_payload("palmier-folder://xyz"));
    }

    #[test]
    fn is_internal_drag_payload_false_for_file() {
        assert!(!is_internal_drag_payload("file:///users/test/file.mov"));
    }

    #[test]
    fn is_internal_drag_payload_false_for_http() {
        assert!(!is_internal_drag_payload("https://example.com"));
    }
}
