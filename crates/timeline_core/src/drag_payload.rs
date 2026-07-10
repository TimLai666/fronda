use core_model::ClipType;
use std::fmt;

/// A media-library asset dragged across panels (timeline tracks, generation
/// reference tiles). Used as the typed in-app drag payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDrag {
    pub asset_id: String,
    pub media_type: ClipType,
}

impl AssetDrag {
    /// DRAG-001 string form (`palmier-asset://<id>`), for pasteboard bridges.
    pub fn payload_string(&self) -> String {
        format!("palmier-asset://{}", self.asset_id)
    }
}

/// Pick the track a dragged asset lands on (drag-drop spec: asset→timeline):
/// the hovered track when type-compatible, else the first compatible track,
/// else None (caller falls back to auto-create placement).
pub fn asset_drop_track(
    track_types: &[ClipType],
    hovered: Option<usize>,
    media_type: ClipType,
) -> Option<usize> {
    if let Some(idx) = hovered {
        if track_types
            .get(idx)
            .is_some_and(|t| crate::is_track_compatible(*t, media_type))
        {
            return Some(idx);
        }
    }
    track_types
        .iter()
        .position(|t| crate::is_track_compatible(*t, media_type))
}

/// A parsed internal drag payload item
#[derive(Debug, Clone, PartialEq)]
pub enum DragItem {
    Asset(String),
    Folder(String),
    /// A moment/segment drag: an asset id plus a source-time range in seconds
    /// (`palmier-asset://<id>#<start>-<end>`, from search hits / moment thumbnails).
    AssetSegment { id: String, start: f64, end: f64 },
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
            DragItem::AssetSegment { id, start, end } => {
                write!(f, "asset({id}#{start}-{end})")
            }
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
            if let Some(rest) = line.strip_prefix("palmier-asset://") {
                if rest.is_empty() {
                    return None;
                }
                // `<id>#<start>-<end>` → moment segment; a malformed segment is
                // rejected (dropped) rather than folded into the id.
                if let Some((id, seg)) = rest.split_once('#') {
                    let (start, end) = parse_segment(seg)?;
                    if id.is_empty() {
                        return None;
                    }
                    return Some(DragItem::AssetSegment {
                        id: id.to_string(),
                        start,
                        end,
                    });
                }
                return Some(DragItem::Asset(rest.to_string()));
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

/// Parse the `start-end` part of a moment drag segment (seconds). Requires two
/// finite floats with `0 <= start < end`.
fn parse_segment(seg: &str) -> Option<(f64, f64)> {
    let (s, e) = seg.split_once('-')?;
    let start: f64 = s.trim().parse().ok()?;
    let end: f64 = e.trim().parse().ok()?;
    if start.is_finite() && end.is_finite() && start >= 0.0 && end > start {
        Some((start, end))
    } else {
        None
    }
}

/// Parse a single `palmier-asset://<id>#<start>-<end>` moment drag string into
/// `(id, start, end)`. Returns `None` for a non-asset scheme, empty id, or a
/// missing/malformed segment.
pub fn parse_asset_segment(input: &str) -> Option<(String, f64, f64)> {
    let rest = input.trim().strip_prefix("palmier-asset://")?;
    let (id, seg) = rest.split_once('#')?;
    if id.is_empty() {
        return None;
    }
    let (start, end) = parse_segment(seg)?;
    Some((id.to_string(), start, end))
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

    #[test]
    fn moment_segment_parses_start_end() {
        let result = parse_drag_payload("palmier-asset://a#2.5-5.75");
        assert_eq!(
            result,
            DragPayload {
                items: vec![DragItem::AssetSegment {
                    id: "a".into(),
                    start: 2.5,
                    end: 5.75,
                }]
            }
        );
        assert_eq!(
            parse_asset_segment("palmier-asset://a#2.5-5.75"),
            Some(("a".to_string(), 2.5, 5.75))
        );
    }

    #[test]
    fn moment_segment_rejects_reversed_or_malformed() {
        // start >= end → whole item dropped.
        assert_eq!(parse_drag_payload("palmier-asset://a#5-2").items, vec![]);
        assert_eq!(parse_asset_segment("palmier-asset://a#5-2"), None);
        // non-numeric segment dropped.
        assert_eq!(parse_drag_payload("palmier-asset://a#x-y").items, vec![]);
        // empty id with segment dropped.
        assert_eq!(parse_drag_payload("palmier-asset://#1-2").items, vec![]);
        // no '#' → still a plain asset (unchanged behavior).
        assert_eq!(
            parse_drag_payload("palmier-asset://a").items,
            vec![DragItem::Asset("a".into())]
        );
        // parse_asset_segment on a plain asset (no segment) → None.
        assert_eq!(parse_asset_segment("palmier-asset://a"), None);
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

    // AssetDrag string form round-trips through the DRAG-001 parser.
    #[test]
    fn asset_drag_payload_string_round_trips() {
        let drag = AssetDrag {
            asset_id: "abc123".into(),
            media_type: ClipType::Video,
        };
        assert_eq!(drag.payload_string(), "palmier-asset://abc123");
        assert_eq!(
            parse_drag_payload(&drag.payload_string()).items,
            vec![DragItem::Asset("abc123".into())]
        );
        assert!(is_internal_drag_payload(&drag.payload_string()));
    }

    // Asset drop targeting: hovered track wins when compatible.
    #[test]
    fn asset_drop_track_prefers_hovered_when_compatible() {
        let tracks = [ClipType::Video, ClipType::Audio];
        assert_eq!(asset_drop_track(&tracks, Some(0), ClipType::Video), Some(0));
        assert_eq!(asset_drop_track(&tracks, Some(1), ClipType::Audio), Some(1));
        // Image is visual → compatible with the video track.
        assert_eq!(asset_drop_track(&tracks, Some(0), ClipType::Image), Some(0));
    }

    // Asset drop targeting: incompatible hover falls back to the first compatible track.
    #[test]
    fn asset_drop_track_falls_back_on_type_mismatch() {
        let tracks = [ClipType::Video, ClipType::Audio];
        assert_eq!(asset_drop_track(&tracks, Some(0), ClipType::Audio), Some(1));
        assert_eq!(asset_drop_track(&tracks, Some(1), ClipType::Video), Some(0));
        // No hover at all → first compatible.
        assert_eq!(asset_drop_track(&tracks, None, ClipType::Image), Some(0));
    }

    // Asset drop targeting: no compatible track → None (caller auto-creates).
    #[test]
    fn asset_drop_track_none_without_compatible_track() {
        assert_eq!(asset_drop_track(&[], Some(0), ClipType::Video), None);
        assert_eq!(
            asset_drop_track(&[ClipType::Video], Some(0), ClipType::Audio),
            None
        );
        // Out-of-range hover index is ignored, not a panic.
        assert_eq!(
            asset_drop_track(&[ClipType::Audio], Some(9), ClipType::Audio),
            Some(0)
        );
    }
}
