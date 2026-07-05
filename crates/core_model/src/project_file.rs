//! Root of `project.json` (upstream #255): a project holds multiple timelines.
//!
//! Legacy (pre-0.6.1) files stored a bare `Timeline`; [`ProjectFile::decode`]
//! falls back and wraps, mirroring Swift `ProjectFile.decode` exactly.

use crate::Timeline;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-timeline UI state, written on tab switch and save (upstream #255).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineViewState {
    #[serde(default)]
    pub playhead_frame: i64,
    /// Pixels per frame; Swift's default is `Defaults.pixelsPerFrame`.
    #[serde(default = "default_zoom_scale")]
    pub zoom_scale: f64,
    #[serde(default)]
    pub scroll_offset_x: f64,
}

fn default_zoom_scale() -> f64 {
    1.0
}

/// Root of project.json (upstream #255).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    // Intentionally NOT `#[serde(default)]`: a legacy bare-Timeline JSON must
    // FAIL to parse as ProjectFile so `decode` falls back to the legacy path.
    pub timelines: Vec<Timeline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_timeline_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_timeline_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_states: Option<HashMap<String, TimelineViewState>>,
    /// Per-project speaker identities (upstream #261: id/name/color/centroid).
    /// Round-tripped opaquely - Fronda doesn't run speaker identification yet,
    /// and dropping the field on save would erase Swift-computed registries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<serde_json::Value>,
}

impl ProjectFile {
    /// Wrap a single (legacy) timeline the way Swift's fallback does.
    pub fn wrapping(timeline: Timeline) -> Self {
        let id = timeline.id.clone();
        Self {
            timelines: vec![timeline],
            active_timeline_id: Some(id.clone()),
            open_timeline_ids: Some(vec![id]),
            view_states: None,
            speakers: None,
        }
    }

    /// Decode project.json, falling back to a legacy bare `Timeline`.
    ///
    /// Mirrors Swift `ProjectFile.decode`: try ProjectFile first; a parsed file
    /// with zero timelines is an error (no legacy fallback for it — Swift's
    /// legacy attempt also fails since a bare Timeline decode can't consume it
    /// meaningfully); a parse failure retries as a bare Timeline and wraps it;
    /// if both fail, the ORIGINAL ProjectFile error is returned.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        match serde_json::from_slice::<ProjectFile>(bytes) {
            Ok(file) if !file.timelines.is_empty() => Ok(file),
            Ok(_) => Err(serde::de::Error::custom("project has no timelines")),
            Err(original) => match serde_json::from_slice::<Timeline>(bytes) {
                Ok(legacy) => Ok(Self::wrapping(legacy)),
                Err(_) => Err(original),
            },
        }
    }

    /// Index of the active timeline: `activeTimelineId` when it resolves,
    /// else the first timeline.
    pub fn active_index(&self) -> usize {
        self.active_timeline_id
            .as_deref()
            .and_then(|id| self.timelines.iter().position(|t| t.id == id))
            .unwrap_or(0)
    }

    pub fn active_timeline(&self) -> Option<&Timeline> {
        self.timelines.get(self.active_index())
    }

    pub fn timeline_for(&self, id: &str) -> Option<&Timeline> {
        self.timelines.iter().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline_json_legacy() -> String {
        // A pre-0.6.1 bare-Timeline project.json.
        r#"{"fps": 24, "width": 1280, "height": 720, "settingsConfigured": true, "tracks": []}"#
            .to_string()
    }

    #[test]
    fn decode_new_format_round_trips() {
        let mut a = Timeline {
            name: "Main".into(),
            ..Default::default()
        };
        a.id = "tl-a".into();
        let mut b = Timeline {
            name: "B-roll".into(),
            ..Default::default()
        };
        b.id = "tl-b".into();
        let file = ProjectFile {
            timelines: vec![a, b],
            active_timeline_id: Some("tl-b".into()),
            open_timeline_ids: Some(vec!["tl-a".into(), "tl-b".into()]),
            view_states: Some(HashMap::from([(
                "tl-a".into(),
                TimelineViewState {
                    playhead_frame: 42,
                    zoom_scale: 0.5,
                    scroll_offset_x: 250.0,
                },
            )])),
            speakers: Some(serde_json::json!([{"id": 1, "name": "Alex",
                "color": [0.1, 0.2, 0.3], "centroid": [0.5]}])),
        };
        let bytes = serde_json::to_vec(&file).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.get("timelines").is_some());
        assert_eq!(json["activeTimelineId"], "tl-b");
        assert_eq!(json["viewStates"]["tl-a"]["playheadFrame"], 42);
        assert_eq!(json["viewStates"]["tl-a"]["scrollOffsetX"], 250.0);

        let decoded = ProjectFile::decode(&bytes).unwrap();
        assert_eq!(decoded, file, "incl. the opaque speakers registry (#261)");
        assert_eq!(decoded.active_index(), 1);
        assert_eq!(decoded.active_timeline().unwrap().name, "B-roll");
    }

    #[test]
    fn decode_legacy_bare_timeline_wraps() {
        let decoded = ProjectFile::decode(timeline_json_legacy().as_bytes()).unwrap();
        assert_eq!(decoded.timelines.len(), 1);
        let t = &decoded.timelines[0];
        assert_eq!(t.fps, 24);
        assert_eq!(t.width, 1280);
        assert_eq!(
            decoded.active_timeline_id.as_deref(),
            Some(t.id.as_str()),
            "active id = the wrapped timeline's id"
        );
        assert_eq!(
            decoded.open_timeline_ids.as_deref(),
            Some(&[t.id.clone()][..])
        );
    }

    #[test]
    fn decode_empty_timelines_errors() {
        let err = ProjectFile::decode(br#"{"timelines": []}"#).unwrap_err();
        assert!(err.to_string().contains("no timelines"), "{err}");
    }

    #[test]
    fn decode_garbage_reports_projectfile_error() {
        assert!(ProjectFile::decode(b"not json").is_err());
        // Valid JSON that is neither shape: object without timelines AND
        // without any Timeline-shaped content still wraps as an all-default
        // legacy timeline? No: Timeline's serde defaults accept any object,
        // so this DOES wrap (same tolerance Swift lacks — Swift requires fps).
        // Pin the behaviour so a change is deliberate.
        let decoded = ProjectFile::decode(br#"{"unrelated": true}"#).unwrap();
        assert_eq!(decoded.timelines.len(), 1);
        assert_eq!(decoded.timelines[0].fps, 30, "all-default legacy wrap");
    }

    #[test]
    fn swift_v061_fixture_decodes() {
        // Shape mirrors what Swift 0.6.1 writes (auditor-verified key set).
        let fixture = r#"{
            "timelines": [
                {
                    "id": "A0FBDE", "name": "Timeline 1", "fps": 30,
                    "width": 1920, "height": 1080, "settingsConfigured": true,
                    "folderId": "folder-9",
                    "tracks": [
                        {"id": "t1", "type": "video", "muted": false, "hidden": false,
                         "syncLocked": true, "displayHeight": 500, "clips": []}
                    ]
                }
            ],
            "activeTimelineId": "A0FBDE",
            "openTimelineIds": ["A0FBDE"],
            "viewStates": {"A0FBDE": {"playheadFrame": 7, "zoomScale": 2.0, "scrollOffsetX": 12.5}}
        }"#;
        let decoded = ProjectFile::decode(fixture.as_bytes()).unwrap();
        let t = &decoded.timelines[0];
        assert_eq!(t.id, "A0FBDE");
        assert_eq!(t.folder_id.as_deref(), Some("folder-9"));
        assert_eq!(
            t.tracks[0].display_height, 200.0,
            "displayHeight clamps to TrackSize.maxHeight like Swift"
        );
        assert_eq!(
            decoded.view_states.as_ref().unwrap()["A0FBDE"].playhead_frame,
            7
        );
    }
}
