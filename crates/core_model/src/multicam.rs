//! Multicam source groups (upstream #283, Swift `MulticamSource.swift`).
//!
//! A group is metadata in `project.json`'s `multicamGroups`: members map media
//! assets onto one shared group clock via per-member sync offsets. Timeline
//! clips carry the group via `Clip.multicam_group_id` — nothing is nested.

use crate::timeline::Clip;
use serde::{Deserialize, Serialize};

/// What a member contributes: `Angle` = camera with scratch audio, `Mic` =
/// program audio, `Both` = camera whose audio plays in the mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MulticamMemberKind {
    Angle,
    Mic,
    Both,
}

impl MulticamMemberKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "angle" => Some(Self::Angle),
            "mic" => Some(Self::Mic),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Angle => "angle",
            Self::Mic => "mic",
            Self::Both => "both",
        }
    }
}

/// A member's position on the group clock: its source starts `offset_seconds`
/// after the group's zero. `locked` marks a user-pinned offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MulticamSyncMap {
    #[serde(default)]
    pub offset_seconds: f64,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub locked: bool,
}

impl Default for MulticamSyncMap {
    fn default() -> Self {
        Self {
            offset_seconds: 0.0,
            confidence: 0.0,
            locked: false,
        }
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MulticamMember {
    #[serde(default = "new_id")]
    pub id: String,
    pub media_ref: String,
    pub kind: MulticamMemberKind,
    #[serde(default)]
    pub angle_label: String,
    #[serde(default)]
    pub sync: MulticamSyncMap,
}

impl MulticamMember {
    pub fn provides_video(&self) -> bool {
        self.kind != MulticamMemberKind::Mic
    }

    pub fn provides_audio(&self) -> bool {
        self.kind != MulticamMemberKind::Angle
    }

    pub fn usable(&self) -> bool {
        self.sync.confidence > 0.0 || self.sync.locked
    }

    pub fn offset_frames(&self, fps: i64) -> i64 {
        (self.sync.offset_seconds * fps as f64).round() as i64
    }

    /// Project frame where this member's source frame 0 would sit, derived
    /// from a placed clip of this member.
    pub fn anchor_frame(&self, clip: &Clip, fps: i64) -> i64 {
        clip.start_frame - clip.trim_start_frame - self.offset_frames(fps)
    }

    /// Group-clock frames this member's source covers.
    pub fn coverage(&self, source_duration: f64, fps: i64) -> std::ops::Range<i64> {
        let start = (self.sync.offset_seconds * fps as f64).round() as i64;
        let end = ((self.sync.offset_seconds + source_duration) * fps as f64).round() as i64;
        start..end.max(start)
    }

    /// Source trim frame showing group-clock frame `group_frame`.
    pub fn trim_frame(&self, group_frame: i64, fps: i64) -> i64 {
        ((group_frame as f64 / fps as f64 - self.sync.offset_seconds) * fps as f64).round() as i64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MulticamSource {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub members: Vec<MulticamMember>,
    #[serde(default)]
    pub master_member_id: String,
}

impl MulticamSource {
    pub fn master(&self) -> Option<&MulticamMember> {
        self.members.iter().find(|m| m.id == self.master_member_id)
    }

    /// Usable video-providing members.
    pub fn angles(&self) -> Vec<&MulticamMember> {
        self.members
            .iter()
            .filter(|m| m.provides_video() && m.usable())
            .collect()
    }

    /// Usable audio-providing members.
    pub fn mics(&self) -> Vec<&MulticamMember> {
        self.members
            .iter()
            .filter(|m| m.provides_audio() && m.usable())
            .collect()
    }

    /// Case-insensitive angleLabel lookup (Swift `member(labeled:)`).
    pub fn member_labeled(&self, label: &str) -> Option<&MulticamMember> {
        let wanted = label.to_lowercase();
        self.members
            .iter()
            .find(|m| m.angle_label.to_lowercase() == wanted)
    }

    pub fn member_by_media_ref(&self, media_ref: &str) -> Option<&MulticamMember> {
        self.members.iter().find(|m| m.media_ref == media_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(offset: f64) -> MulticamMember {
        MulticamMember {
            id: "m-1".into(),
            media_ref: "camA".into(),
            kind: MulticamMemberKind::Angle,
            angle_label: "cam-a".into(),
            sync: MulticamSyncMap {
                offset_seconds: offset,
                confidence: 1.0,
                locked: false,
            },
        }
    }

    #[test]
    fn member_frame_math_mirrors_swift() {
        let m = member(5.0);
        assert_eq!(m.offset_frames(30), 150);
        assert_eq!(m.coverage(110.0, 30), 150..3450);
        // trimFrame: group frame 600 at 30fps, offset 5s → (20 - 5) * 30 = 450.
        assert_eq!(m.trim_frame(600, 30), 450);
        // Degenerate duration clamps to an empty range at start.
        assert_eq!(m.coverage(-1.0, 30), 150..150);
    }

    #[test]
    fn kind_provides_and_usable() {
        let mut m = member(0.0);
        assert!(m.provides_video() && !m.provides_audio());
        m.kind = MulticamMemberKind::Mic;
        assert!(!m.provides_video() && m.provides_audio());
        m.kind = MulticamMemberKind::Both;
        assert!(m.provides_video() && m.provides_audio());
        m.sync.confidence = 0.0;
        assert!(!m.usable());
        m.sync.locked = true;
        assert!(m.usable(), "locked counts as usable");
    }

    #[test]
    fn label_lookup_is_case_insensitive() {
        let group = MulticamSource {
            id: "g".into(),
            name: "G".into(),
            members: vec![member(0.0)],
            master_member_id: "m-1".into(),
        };
        assert!(group.member_labeled("CAM-A").is_some());
        assert!(group.member_labeled("cam-b").is_none());
        assert_eq!(group.master().unwrap().id, "m-1");
    }

    #[test]
    fn swift_shaped_json_round_trips_losslessly() {
        // Exactly what Swift 0.7 encodes (all keys always present).
        let raw = serde_json::json!({
            "id": "G-1", "name": "Pod",
            "members": [{
                "id": "M-1", "mediaRef": "a", "kind": "both", "angleLabel": "host",
                "sync": {"offsetSeconds": 1.5, "confidence": 0.91, "locked": false}
            }],
            "masterMemberId": "M-1"
        });
        let decoded: MulticamSource = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(decoded.members[0].kind, MulticamMemberKind::Both);
        assert_eq!(decoded.members[0].sync.offset_seconds, 1.5);
        let reencoded = serde_json::to_value(&decoded).unwrap();
        assert_eq!(reencoded, raw, "Swift-shaped JSON must round-trip losslessly");
    }

    #[test]
    fn clip_stamp_survives_coding_and_legacy_decodes_nil() {
        // Upstream `clipStampSurvivesCodingAndLegacyDecodesNil`.
        let base = serde_json::json!({
            "id": "c1", "mediaRef": "m", "startFrame": 0, "durationFrames": 10
        });
        let mut clip: Clip = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(clip.multicam_group_id, None, "legacy clip decodes nil");
        clip.multicam_group_id = Some("g1".into());
        let round: Clip =
            serde_json::from_value(serde_json::to_value(&clip).unwrap()).unwrap();
        assert_eq!(round.multicam_group_id.as_deref(), Some("g1"));
    }

    #[test]
    fn lenient_decode_fills_defaults() {
        // Older/foreign JSON without id/angleLabel/sync still decodes (the
        // whole project must not fall to the legacy path over group metadata).
        let raw = serde_json::json!({
            "members": [{"mediaRef": "m1", "kind": "angle"}]
        });
        let decoded: MulticamSource = serde_json::from_value(raw).unwrap();
        assert!(!decoded.members[0].id.is_empty(), "id generated");
        assert_eq!(decoded.members[0].angle_label, "");
        assert_eq!(decoded.members[0].sync, MulticamSyncMap::default());
        assert_eq!(decoded.master_member_id, "");
    }
}
