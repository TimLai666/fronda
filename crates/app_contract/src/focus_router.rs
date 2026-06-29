//! Focus-based action routing (CCB-014).
//!
//! Determines which paste/action handler to invoke based on the current
//! focus target within the application.

/// Named focus targets that can receive keyboard actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Timeline,
    MediaPanel,
    Chat,
    Settings,
    Home,
}

/// Paste routing result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteAction {
    /// Paste from the app-local clip clipboard (timeline clips).
    ClipClipboard,
    /// Paste from the OS pasteboard (file URLs, images).
    OsPasteboard,
    /// No paste action available for this focus target.
    Noop,
}

/// Route a paste action based on the currently focused target.
///
/// CCB-014:
/// - timeline-focused paste → clip clipboard
/// - media-panel-focused paste → OS pasteboard
/// - chat-focused paste → OS pasteboard (for pasting media into chat)
/// - settings/home → noop
pub fn route_paste(focus: FocusTarget) -> PasteAction {
    match focus {
        FocusTarget::Timeline => PasteAction::ClipClipboard,
        FocusTarget::MediaPanel => PasteAction::OsPasteboard,
        FocusTarget::Chat => PasteAction::OsPasteboard,
        FocusTarget::Settings => PasteAction::Noop,
        FocusTarget::Home => PasteAction::Noop,
    }
}

/// Route a copy action based on focus.
pub fn route_copy(focus: FocusTarget) -> PasteAction {
    match focus {
        FocusTarget::Timeline => PasteAction::ClipClipboard,
        _ => PasteAction::OsPasteboard,
    }
}

// ── DRAG-007/008/009: drop routing ──────────────────────────────────────────

/// Where a drag-and-drop payload was dropped.
///
/// Passed by the gpui-ce drop handler to the routing layer.
/// DRAG-007/008/009.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropTarget {
    /// Dropped onto the media panel root / empty area (DRAG-008).
    MediaPanelRoot,
    /// Dropped onto a specific folder or breadcrumb in the media panel (DRAG-009).
    MediaPanelFolder { folder_id: String },
    /// Internal drag dropped onto the library root — reparents items (DRAG-007).
    LibraryRoot,
}

/// The resolved action for a drop event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAction {
    /// DRAG-007: Reparent dragged library items to the library root.
    ReparentToRoot { item_ids: Vec<String> },
    /// DRAG-008: Import file URLs into the currently active folder.
    ImportIntoCurrentFolder { file_paths: Vec<String> },
    /// DRAG-009: Import file URLs into a specific folder.
    ImportIntoFolder {
        file_paths: Vec<String>,
        folder_id: String,
    },
    /// The drop should be ignored (unsupported files, no-op targets, etc.).
    Noop,
}

/// Route a drop event to the appropriate action.
///
/// - `target`: where the payload landed (DRAG-007/008/009)
/// - `internal_item_ids`: IDs from an internal drag payload (may be empty)
/// - `file_paths`: file paths from a Finder/OS drop (may be empty, filtered
///   by `is_supported_extension`)
pub fn route_drop(
    target: &DropTarget,
    internal_item_ids: &[String],
    file_paths: &[String],
) -> DropAction {
    match target {
        DropTarget::LibraryRoot => {
            if internal_item_ids.is_empty() {
                return DropAction::Noop;
            }
            // DRAG-007: reparent internal items to root
            DropAction::ReparentToRoot {
                item_ids: internal_item_ids.to_vec(),
            }
        }
        DropTarget::MediaPanelRoot => {
            // DRAG-008: Finder drop onto media panel → import into current folder
            let supported: Vec<String> = file_paths
                .iter()
                .filter(|p| {
                    let ext = p.rsplit('.').next().unwrap_or("");
                    is_supported_extension(ext)
                })
                .cloned()
                .collect();
            if supported.is_empty() {
                DropAction::Noop
            } else {
                DropAction::ImportIntoCurrentFolder {
                    file_paths: supported,
                }
            }
        }
        DropTarget::MediaPanelFolder { folder_id } => {
            // DRAG-009: Finder drop onto folder/breadcrumb → import into that folder
            let supported: Vec<String> = file_paths
                .iter()
                .filter(|p| {
                    let ext = p.rsplit('.').next().unwrap_or("");
                    is_supported_extension(ext)
                })
                .cloned()
                .collect();
            if supported.is_empty() {
                DropAction::Noop
            } else {
                DropAction::ImportIntoFolder {
                    file_paths: supported,
                    folder_id: folder_id.clone(),
                }
            }
        }
    }
}

/// Supported file extensions for media import.
pub const SUPPORTED_MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "avi", "mkv", "webm", "mp3", "wav", "aac", "m4a", "flac", "ogg", "png",
    "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "lottie", "json",
];

/// Check whether a file extension is supported for media import.
///
/// DRAG-010: Unsupported file extensions in Finder drops are ignored.
pub fn is_supported_extension(ext: &str) -> bool {
    let ext = ext.trim_start_matches('.').to_lowercase();
    SUPPORTED_MEDIA_EXTENSIONS.contains(&ext.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_paste_timeline_uses_clip_clipboard() {
        assert_eq!(
            route_paste(FocusTarget::Timeline),
            PasteAction::ClipClipboard
        );
    }

    #[test]
    fn route_paste_media_panel_uses_os_pasteboard() {
        assert_eq!(
            route_paste(FocusTarget::MediaPanel),
            PasteAction::OsPasteboard
        );
    }

    #[test]
    fn route_paste_chat_uses_os_pasteboard() {
        assert_eq!(route_paste(FocusTarget::Chat), PasteAction::OsPasteboard);
    }

    #[test]
    fn route_paste_settings_is_noop() {
        assert_eq!(route_paste(FocusTarget::Settings), PasteAction::Noop);
    }

    #[test]
    fn route_paste_home_is_noop() {
        assert_eq!(route_paste(FocusTarget::Home), PasteAction::Noop);
    }

    #[test]
    fn route_copy_timeline_uses_clip_clipboard() {
        assert_eq!(
            route_copy(FocusTarget::Timeline),
            PasteAction::ClipClipboard
        );
    }

    #[test]
    fn route_copy_media_panel_uses_os_pasteboard() {
        assert_eq!(
            route_copy(FocusTarget::MediaPanel),
            PasteAction::OsPasteboard
        );
    }

    #[test]
    fn is_supported_extension_common_video() {
        assert!(is_supported_extension("mp4"));
        assert!(is_supported_extension(".mov"));
        assert!(is_supported_extension("mkv"));
    }

    #[test]
    fn is_supported_extension_common_audio() {
        assert!(is_supported_extension("mp3"));
        assert!(is_supported_extension(".wav"));
        assert!(is_supported_extension("flac"));
    }

    #[test]
    fn is_supported_extension_common_images() {
        assert!(is_supported_extension("png"));
        assert!(is_supported_extension(".jpg"));
        assert!(is_supported_extension("webp"));
    }

    #[test]
    fn is_supported_extension_unsupported_returns_false() {
        assert!(!is_supported_extension("exe"));
        assert!(!is_supported_extension(".dll"));
        assert!(!is_supported_extension("zip"));
    }

    #[test]
    fn is_supported_extension_case_insensitive() {
        assert!(is_supported_extension("MP4"));
        assert!(is_supported_extension(".PNG"));
    }

    #[test]
    fn is_supported_lottie_json() {
        assert!(is_supported_extension("lottie"));
        assert!(is_supported_extension("json"));
    }

    // ── DRAG-007/008/009: drop routing tests ─────────────────────────────────

    #[test]
    fn drag_007_internal_drop_to_root_reparents() {
        let result = route_drop(
            &DropTarget::LibraryRoot,
            &["asset-1".to_string(), "folder-2".to_string()],
            &[],
        );
        match result {
            DropAction::ReparentToRoot { item_ids } => {
                assert_eq!(item_ids.len(), 2);
                assert!(item_ids.contains(&"asset-1".to_string()));
            }
            other => panic!("Expected ReparentToRoot, got {other:?}"),
        }
    }

    #[test]
    fn drag_007_empty_internal_drop_to_root_is_noop() {
        let result = route_drop(&DropTarget::LibraryRoot, &[], &["/file.mp4".to_string()]);
        assert_eq!(result, DropAction::Noop);
    }

    #[test]
    fn drag_008_finder_drop_to_panel_root_imports() {
        let result = route_drop(
            &DropTarget::MediaPanelRoot,
            &[],
            &["/clip.mp4".to_string(), "/thumb.png".to_string()],
        );
        match result {
            DropAction::ImportIntoCurrentFolder { file_paths } => {
                assert_eq!(file_paths.len(), 2);
            }
            other => panic!("Expected ImportIntoCurrentFolder, got {other:?}"),
        }
    }

    #[test]
    fn drag_008_unsupported_files_produce_noop() {
        let result = route_drop(
            &DropTarget::MediaPanelRoot,
            &[],
            &["/malware.exe".to_string()],
        );
        assert_eq!(result, DropAction::Noop);
    }

    #[test]
    fn drag_009_finder_drop_to_folder_imports_into_folder() {
        let result = route_drop(
            &DropTarget::MediaPanelFolder {
                folder_id: "folder-abc".to_string(),
            },
            &[],
            &["/video.mov".to_string()],
        );
        match result {
            DropAction::ImportIntoFolder {
                file_paths,
                folder_id,
            } => {
                assert_eq!(folder_id, "folder-abc");
                assert_eq!(file_paths.len(), 1);
            }
            other => panic!("Expected ImportIntoFolder, got {other:?}"),
        }
    }

    #[test]
    fn drag_009_mixed_files_filters_unsupported() {
        let result = route_drop(
            &DropTarget::MediaPanelFolder {
                folder_id: "f1".to_string(),
            },
            &[],
            &[
                "/clip.mp4".to_string(),
                "/doc.pdf".to_string(),
                "/thumb.png".to_string(),
            ],
        );
        match result {
            DropAction::ImportIntoFolder { file_paths, .. } => {
                assert_eq!(file_paths.len(), 2); // mp4 and png, not pdf
            }
            other => panic!("Expected ImportIntoFolder, got {other:?}"),
        }
    }
}
