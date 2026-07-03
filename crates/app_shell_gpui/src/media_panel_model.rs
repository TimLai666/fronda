//! Media panel model — pure state, no UI dependency.
//!
//! Covers UIX-011 (media panel width constants) and
//! the three-tab structure from MediaPanelView.

/// The three tabs in the media panel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaPanelTab {
    Media,
    Captions,
    Music,
}

impl MediaPanelTab {
    pub fn all() -> [MediaPanelTab; 3] {
        [
            MediaPanelTab::Media,
            MediaPanelTab::Captions,
            MediaPanelTab::Music,
        ]
    }

    /// SF Symbol icon name for the tab button.
    pub fn icon_name(&self) -> &'static str {
        match self {
            MediaPanelTab::Media => "folder",
            MediaPanelTab::Captions => "captions.bubble",
            MediaPanelTab::Music => "music.note",
        }
    }

    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            MediaPanelTab::Media => "Media",
            MediaPanelTab::Captions => "Captions",
            MediaPanelTab::Music => "Music",
        }
    }
}

/// Icon glyph for a media clip type.
pub fn tile_icon(kind: &core_model::ClipType) -> &'static str {
    match kind {
        core_model::ClipType::Audio => "\u{266a}",
        core_model::ClipType::Image => "\u{2b1c}",
        core_model::ClipType::Text => "T",
        _ => "\u{25b6}",
    }
}

/// Deterministic placeholder hue in [0.0, 1.0) derived from the media id.
pub fn tile_hue(id: &str) -> f32 {
    let sum: u32 = id.bytes().map(u32::from).sum();
    (sum % 100) as f32 / 100.0
}

/// One media entry as shown in the Library grid.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    pub kind: core_model::ClipType,
    /// Resolved on-disk source (existing files only) for real thumbnails.
    pub source_path: Option<std::path::PathBuf>,
}

/// Media panel state — pure model, testable without gpui.
#[derive(Debug, Clone)]
pub struct MediaPanelState {
    pub active_tab: MediaPanelTab,
    pub items: Vec<MediaItem>,
    /// (id, name) of manifest folders, display only.
    pub folders: Vec<(String, String)>,
}

impl MediaPanelState {
    pub fn new() -> Self {
        Self {
            active_tab: MediaPanelTab::Media,
            items: Vec::new(),
            folders: Vec::new(),
        }
    }

    /// Rebuild items and folders from the shared manifest, preserving
    /// view-only state such as the active tab.
    pub fn sync_from_manifest(
        &mut self,
        manifest: &core_model::MediaManifest,
        project_root: Option<&std::path::Path>,
    ) {
        self.items = manifest
            .entries
            .iter()
            .map(|e| {
                let source_path = match &e.source {
                    core_model::MediaSource::External { absolute_path } => {
                        Some(std::path::PathBuf::from(absolute_path))
                    }
                    core_model::MediaSource::Project { relative_path } => {
                        project_root.map(|root| root.join(relative_path))
                    }
                }
                .filter(|p| p.is_file());
                MediaItem {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    kind: e.r#type,
                    source_path,
                }
            })
            .collect();
        self.folders = manifest
            .folders
            .iter()
            .map(|f| (f.id.clone(), f.name.clone()))
            .collect();
    }

    pub fn select_tab(&mut self, tab: MediaPanelTab) {
        self.active_tab = tab;
    }

    pub fn is_active(&self, tab: &MediaPanelTab) -> bool {
        &self.active_tab == tab
    }
}

impl Default for MediaPanelState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_panel_default_tab_is_media() {
        let s = MediaPanelState::new();
        assert_eq!(s.active_tab, MediaPanelTab::Media);
    }

    #[test]
    fn media_panel_select_captions() {
        let mut s = MediaPanelState::new();
        s.select_tab(MediaPanelTab::Captions);
        assert_eq!(s.active_tab, MediaPanelTab::Captions);
    }

    #[test]
    fn media_panel_select_music() {
        let mut s = MediaPanelState::new();
        s.select_tab(MediaPanelTab::Music);
        assert_eq!(s.active_tab, MediaPanelTab::Music);
    }

    #[test]
    fn media_panel_all_tabs_count() {
        assert_eq!(MediaPanelTab::all().len(), 3);
    }

    #[test]
    fn media_panel_all_tabs_order() {
        let tabs = MediaPanelTab::all();
        assert_eq!(tabs[0], MediaPanelTab::Media);
        assert_eq!(tabs[1], MediaPanelTab::Captions);
        assert_eq!(tabs[2], MediaPanelTab::Music);
    }

    #[test]
    fn media_panel_is_active() {
        let mut s = MediaPanelState::new();
        assert!(s.is_active(&MediaPanelTab::Media));
        assert!(!s.is_active(&MediaPanelTab::Captions));
        s.select_tab(MediaPanelTab::Captions);
        assert!(!s.is_active(&MediaPanelTab::Media));
        assert!(s.is_active(&MediaPanelTab::Captions));
    }

    #[test]
    fn media_panel_tab_icons_defined() {
        for tab in MediaPanelTab::all() {
            assert!(!tab.icon_name().is_empty());
        }
    }

    #[test]
    fn media_panel_tab_labels_defined() {
        for tab in MediaPanelTab::all() {
            assert!(!tab.label().is_empty());
        }
    }

    // ── sync_from_manifest (media-panel-binding spec) ──────────────────

    fn manifest_with_two_entries_one_folder() -> core_model::MediaManifest {
        serde_json::from_str(
            r#"{"version":1,
                "entries":[
                    {"id":"m1","name":"Interview.mp4","type":"video","source":{"project":{"relativePath":"media/a.mp4"}},"duration":5.0},
                    {"id":"m2","name":"Music.wav","type":"audio","source":{"project":{"relativePath":"media/b.wav"}},"duration":9.0}
                ],
                "folders":[{"id":"f1","name":"B-roll"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn sync_maps_entries_and_folders() {
        let mut s = MediaPanelState::new();
        s.select_tab(MediaPanelTab::Music);
        s.sync_from_manifest(&manifest_with_two_entries_one_folder(), None);
        assert_eq!(s.items.len(), 2);
        assert_eq!(s.items[0].id, "m1");
        assert_eq!(s.items[0].name, "Interview.mp4");
        assert_eq!(s.items[1].kind, core_model::ClipType::Audio);
        assert_eq!(s.folders, vec![("f1".to_string(), "B-roll".to_string())]);
        assert_eq!(s.active_tab, MediaPanelTab::Music, "tab preserved");
    }

    #[test]
    fn tile_hue_is_stable_and_in_range() {
        let a = tile_hue("m1");
        let b = tile_hue("m1");
        assert_eq!(a, b, "same id same hue");
        assert!((0.0..1.0).contains(&a));
        for id in ["m2", "clip-long-identifier", ""] {
            let h = tile_hue(id);
            assert!((0.0..1.0).contains(&h), "hue out of range for {id:?}");
        }
    }

    #[test]
    fn tile_icons_by_kind() {
        assert_eq!(tile_icon(&core_model::ClipType::Audio), "\u{266a}");
        assert_eq!(tile_icon(&core_model::ClipType::Image), "\u{2b1c}");
        assert_eq!(tile_icon(&core_model::ClipType::Text), "T");
        assert_eq!(tile_icon(&core_model::ClipType::Video), "\u{25b6}");
    }

    #[test]
    fn source_path_resolution() {
        // External absolute path pointing at a real temp file resolves.
        let dir = std::env::temp_dir().join("fronda-media-panel-tests");
        let _ = std::fs::create_dir_all(&dir);
        let img = dir.join("photo.png");
        std::fs::write(&img, b"png").unwrap();

        let manifest: core_model::MediaManifest = serde_json::from_str(&format!(
            r#"{{"version":1,"entries":[
                {{"id":"m1","name":"photo.png","type":"image","source":{{"external":{{"absolutePath":{abs:?}}}}},"duration":0.0}},
                {{"id":"m2","name":"clip.mp4","type":"video","source":{{"project":{{"relativePath":"media/clip.mp4"}}}},"duration":1.0}}
            ]}}"#,
            abs = img.to_string_lossy()
        ))
        .unwrap();

        let mut s = MediaPanelState::new();
        s.sync_from_manifest(&manifest, None);
        assert_eq!(s.items[0].source_path.as_deref(), Some(img.as_path()));
        assert!(
            s.items[1].source_path.is_none(),
            "project-relative source without a root is unresolved"
        );

        // With a root, the relative path resolves only if the file exists.
        let root = dir.join("proj.palmier");
        let _ = std::fs::create_dir_all(root.join("media"));
        std::fs::write(root.join("media/clip.mp4"), b"mp4").unwrap();
        s.sync_from_manifest(&manifest, Some(&root));
        assert_eq!(
            s.items[1].source_path.as_deref(),
            Some(root.join("media/clip.mp4").as_path())
        );
    }

    #[test]
    fn sync_is_idempotent_and_replaces() {
        let mut s = MediaPanelState::new();
        let manifest = manifest_with_two_entries_one_folder();
        s.sync_from_manifest(&manifest, None);
        s.sync_from_manifest(&manifest, None);
        assert_eq!(s.items.len(), 2);
        assert_eq!(s.folders.len(), 1);

        let empty = core_model::MediaManifest::default();
        s.sync_from_manifest(&empty, None);
        assert!(s.items.is_empty(), "lists replaced, not appended");
        assert!(s.folders.is_empty());
    }
}
