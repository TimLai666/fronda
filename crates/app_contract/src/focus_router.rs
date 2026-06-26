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
}
