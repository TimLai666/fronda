//! Mutation envelope (tool-surface-v2 design C-4): the uniform timeline diff
//! every clip-mutation tool returns, in `get_timeline` vocabulary.
//!
//! Keys appear only when non-empty: `clips` (changed/new clips as v2 clip
//! shapes + `track`, capped at 30), `captionGroups` (≥3 changed clips sharing
//! a captionGroupId fold into a summary), `shifted` (pure-shift rules grouped
//! by (track, delta) at ≥3), `removedClipIds`, `createdTracks`, `notes`.
//! Tool-specific extra keys merge at the top level.

use crate::timeline_v2::{
    caption_groups_v2, clip_v2, clip_v2_folded, folded_audio_partners, CAPTION_FOLD_MIN,
    CHANGED_CLIPS_CAP, SHIFT_GROUP_MIN,
};
use crate::tool_exec::track_label;
use core_model::{Clip, Timeline};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

/// C-4 note text when the track list changed and the payload carries no
/// `tracks` key of its own.
pub const TRACKS_SHIFTED_NOTE: &str =
    "Track indices shifted — re-read get_timeline before the next index-based call.";

fn index_clips(timeline: &Timeline) -> HashMap<&str, (usize, &Clip)> {
    let mut map = HashMap::new();
    for (ti, track) in timeline.tracks.iter().enumerate() {
        for clip in &track.clips {
            map.insert(clip.id.as_str(), (ti, clip));
        }
    }
    map
}

/// Whether `after` is `before` moved to a different start frame on the same
/// track with nothing else changed.
fn is_pure_shift(before: &Clip, after: &Clip) -> bool {
    if before.start_frame == after.start_frame {
        return false;
    }
    let mut normalized = before.clone();
    normalized.start_frame = after.start_frame;
    normalized == *after
}

/// Build the C-4 mutation envelope from the timelines around a mutation.
/// `extras` must be a JSON object; its keys merge into the top level
/// (`notes` arrays concatenate).
pub fn build_envelope(before: &Timeline, after: &Timeline, extras: Value) -> Value {
    let before_map = index_clips(before);
    let after_map = index_clips(after);

    let mut removed: Vec<String> = before_map
        .keys()
        .filter(|id| !after_map.contains_key(*id))
        .map(|id| id.to_string())
        .collect();
    removed.sort();

    // Classify changes: pure shifts vs. everything else.
    // Shift groups key on (track, delta); members carry the ORIGINAL start.
    let mut shift_groups: HashMap<(usize, i64), Vec<(i64, &Clip)>> = HashMap::new();
    let mut changed: Vec<(usize, &Clip)> = Vec::new();
    for (id, (ti, clip)) in &after_map {
        match before_map.get(id) {
            None => changed.push((*ti, clip)),
            Some((bti, bclip)) => {
                if bti == ti && is_pure_shift(bclip, clip) {
                    let delta = clip.start_frame - bclip.start_frame;
                    shift_groups
                        .entry((*ti, delta))
                        .or_default()
                        .push((bclip.start_frame, clip));
                } else if bti != ti || *bclip != *clip {
                    changed.push((*ti, clip));
                }
            }
        }
    }

    let mut shifted: Vec<Value> = Vec::new();
    let mut shift_rules: Vec<(usize, i64, i64, usize)> = Vec::new();
    for ((ti, delta), members) in shift_groups {
        if members.len() >= SHIFT_GROUP_MIN {
            let from = members.iter().map(|(s, _)| *s).min().unwrap_or(0);
            shift_rules.push((ti, from, delta, members.len()));
        } else {
            for (_, clip) in members {
                changed.push((ti, clip));
            }
        }
    }
    shift_rules.sort();
    for (track, from, by, count) in shift_rules {
        shifted.push(json!({
            "track": track,
            "fromFrame": from,
            "by": by,
            "count": count,
        }));
    }

    // Caption fold: ≥3 changed clips sharing a captionGroupId become a group
    // summary (get_timeline captionGroups shape + track).
    let mut caption_members: HashMap<String, Vec<(usize, &Clip)>> = HashMap::new();
    for (ti, clip) in &changed {
        if let Some(cg) = &clip.caption_group_id {
            caption_members
                .entry(cg.clone())
                .or_default()
                .push((*ti, clip));
        }
    }
    let mut folded_caption_ids: HashSet<String> = HashSet::new();
    let mut caption_groups: Vec<Value> = Vec::new();
    let mut folded_group_ids: Vec<String> = caption_members
        .iter()
        .filter(|(_, m)| m.len() >= CAPTION_FOLD_MIN)
        .map(|(cg, _)| cg.clone())
        .collect();
    folded_group_ids.sort();
    for cg in &folded_group_ids {
        let members = &caption_members[cg];
        let track = members.iter().map(|(ti, _)| *ti).min().unwrap_or(0);
        let clips: Vec<&Clip> = members.iter().map(|(_, c)| *c).collect();
        for c in &clips {
            folded_caption_ids.insert(c.id.clone());
        }
        for (view, _) in caption_groups_v2(&clips, false) {
            let mut summary = match view.summary {
                Value::Object(m) => m,
                other => {
                    let mut m = Map::new();
                    m.insert("summary".into(), other);
                    m
                }
            };
            summary.insert("track".into(), json!(track));
            caption_groups.push(Value::Object(summary));
        }
    }

    // Render changed clips with A/V folding against the post-mutation
    // timeline: when both partners changed, the audio row folds into the
    // visual one.
    let fold = folded_audio_partners(after);
    let changed_ids: HashSet<&str> = changed.iter().map(|(_, c)| c.id.as_str()).collect();
    let mut folded_audio_ids: HashSet<String> = HashSet::new();
    for (ti, clip) in &changed {
        let _ = ti;
        if let Some((_, audio_id)) = fold.get(&clip.id) {
            if changed_ids.contains(audio_id.as_str()) {
                folded_audio_ids.insert(audio_id.clone());
            }
        }
    }

    let mut rows: Vec<(usize, i64, Value)> = Vec::new();
    for (ti, clip) in &changed {
        if folded_caption_ids.contains(&clip.id) || folded_audio_ids.contains(&clip.id) {
            continue;
        }
        let mut obj = match fold.get(&clip.id) {
            Some((ati, audio_id)) if folded_audio_ids.contains(audio_id) => {
                let audio = after_map[audio_id.as_str()].1;
                clip_v2_folded(clip, *ati, audio)
            }
            _ => clip_v2(clip),
        };
        obj.insert("track".into(), json!(ti));
        rows.push((*ti, clip.start_frame, Value::Object(obj)));
    }
    rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let total_changed = rows.len();
    let clips: Vec<Value> = rows
        .into_iter()
        .take(CHANGED_CLIPS_CAP)
        .map(|(_, _, v)| v)
        .collect();

    // Created tracks + track-list drift note.
    let before_track_ids: Vec<&str> = before.tracks.iter().map(|t| t.id.as_str()).collect();
    let after_track_ids: Vec<&str> = after.tracks.iter().map(|t| t.id.as_str()).collect();
    let before_set: HashSet<&str> = before_track_ids.iter().copied().collect();
    let mut created_tracks: Vec<Value> = Vec::new();
    for (i, t) in after.tracks.iter().enumerate() {
        if !before_set.contains(t.id.as_str()) {
            created_tracks.push(json!({
                "index": i,
                "label": track_label(after, i),
                "type": t.r#type.name(),
            }));
        }
    }
    let track_list_changed = before_track_ids != after_track_ids;

    let mut out = Map::new();
    if !clips.is_empty() {
        out.insert("clips".into(), json!(clips));
    }
    if total_changed > CHANGED_CLIPS_CAP {
        out.insert(
            "clipsNote".into(),
            json!(format!(
                "Showing {CHANGED_CLIPS_CAP} of {total_changed} changed clips — re-read get_timeline for the rest."
            )),
        );
    }
    if !caption_groups.is_empty() {
        out.insert("captionGroups".into(), json!(caption_groups));
    }
    if !shifted.is_empty() {
        out.insert("shifted".into(), json!(shifted));
    }
    if !removed.is_empty() {
        out.insert("removedClipIds".into(), json!(removed));
    }
    if !created_tracks.is_empty() {
        out.insert("createdTracks".into(), json!(created_tracks));
    }

    let mut notes: Vec<Value> = Vec::new();
    let mut extras_map = match extras {
        Value::Object(m) => m,
        Value::Null => Map::new(),
        other => {
            let mut m = Map::new();
            m.insert("result".into(), other);
            m
        }
    };
    if let Some(Value::Array(extra_notes)) = extras_map.remove("notes") {
        notes.extend(extra_notes);
    }
    if track_list_changed && !extras_map.contains_key("tracks") {
        notes.push(json!(TRACKS_SHIFTED_NOTE));
    }
    for (k, v) in extras_map {
        out.insert(k, v);
    }
    if !notes.is_empty() {
        out.insert("notes".into(), json!(notes));
    }

    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::make_clip;
    use core_model::{ClipType, Track};

    fn track(kind: ClipType, clips: Vec<Clip>) -> Track {
        Track {
            id: uuid::Uuid::new_v4().to_string(),
            r#type: kind,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips,
        }
    }

    fn named_clip(id: &str, start: i64, dur: i64) -> Clip {
        let mut c = make_clip(start, dur);
        c.id = id.into();
        c
    }

    fn timeline(tracks: Vec<Track>) -> Timeline {
        let mut t = Timeline::default();
        t.tracks = tracks;
        t
    }

    #[test]
    fn empty_diff_yields_only_extras() {
        let t = timeline(vec![track(ClipType::Video, vec![named_clip("c1", 0, 100)])]);
        let env = build_envelope(&t, &t, json!({"synced": 2}));
        assert_eq!(env, json!({"synced": 2}));
    }

    #[test]
    fn added_clip_listed_with_track_index() {
        let before = timeline(vec![track(ClipType::Video, vec![])]);
        let mut after = before.clone();
        after.tracks[0].clips.push(named_clip("new1", 60, 90));
        let env = build_envelope(&before, &after, json!({}));
        let clips = env["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0]["id"], json!("new1"));
        assert_eq!(clips[0]["track"], json!(0));
        assert_eq!(clips[0]["frames"], json!([60, 150]));
        assert!(env.get("shifted").is_none());
        assert!(env.get("removedClipIds").is_none());
    }

    #[test]
    fn removed_clips_sorted() {
        let before = timeline(vec![track(
            ClipType::Video,
            vec![named_clip("zz", 0, 10), named_clip("aa", 20, 10)],
        )]);
        let mut after = before.clone();
        after.tracks[0].clips.clear();
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(env["removedClipIds"], json!(["aa", "zz"]));
    }

    #[test]
    fn three_pure_shifts_compress_into_rule() {
        let before = timeline(vec![track(
            ClipType::Video,
            vec![
                named_clip("a", 100, 50),
                named_clip("b", 200, 50),
                named_clip("c", 300, 50),
            ],
        )]);
        let mut after = before.clone();
        for c in &mut after.tracks[0].clips {
            c.start_frame -= 40;
        }
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(
            env["shifted"],
            json!([{"track": 0, "fromFrame": 100, "by": -40, "count": 3}])
        );
        assert!(env.get("clips").is_none());
    }

    #[test]
    fn two_pure_shifts_stay_as_clips() {
        let before = timeline(vec![track(
            ClipType::Video,
            vec![named_clip("a", 100, 50), named_clip("b", 200, 50)],
        )]);
        let mut after = before.clone();
        for c in &mut after.tracks[0].clips {
            c.start_frame += 10;
        }
        let env = build_envelope(&before, &after, json!({}));
        assert!(env.get("shifted").is_none());
        assert_eq!(env["clips"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn property_change_lists_resulting_clip_state() {
        let before = timeline(vec![track(ClipType::Video, vec![named_clip("a", 0, 100)])]);
        let mut after = before.clone();
        after.tracks[0].clips[0].opacity = 0.5;
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(env["clips"][0]["opacity"], json!(0.5));
    }

    #[test]
    fn clips_cap_at_30_with_note() {
        let before = timeline(vec![track(ClipType::Video, vec![])]);
        let mut after = before.clone();
        for i in 0..35 {
            after.tracks[0]
                .clips
                .push(named_clip(&format!("c{i:02}"), i * 100, 50));
        }
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(env["clips"].as_array().unwrap().len(), 30);
        assert_eq!(
            env["clipsNote"],
            json!("Showing 30 of 35 changed clips — re-read get_timeline for the rest.")
        );
    }

    #[test]
    fn created_track_reported_with_label() {
        let before = timeline(vec![track(ClipType::Video, vec![])]);
        let mut after = before.clone();
        after.tracks.push(track(ClipType::Audio, vec![]));
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(
            env["createdTracks"],
            json!([{"index": 1, "label": "A1", "type": "audio"}])
        );
    }

    #[test]
    fn track_reorder_adds_note_unless_payload_has_tracks() {
        let before = timeline(vec![
            track(ClipType::Video, vec![]),
            track(ClipType::Video, vec![]),
        ]);
        let mut after = before.clone();
        after.tracks.swap(0, 1);
        let env = build_envelope(&before, &after, json!({}));
        assert_eq!(env["notes"], json!([TRACKS_SHIFTED_NOTE]));

        let env2 = build_envelope(&before, &after, json!({"tracks": []}));
        assert!(env2.get("notes").is_none());
    }

    #[test]
    fn caption_fold_at_three_changed_clips() {
        let before = timeline(vec![track(ClipType::Video, vec![])]);
        let mut after = before.clone();
        for i in 0..3 {
            let mut c = named_clip(&format!("cap{i}"), i * 60, 60);
            c.media_type = ClipType::Text;
            c.caption_group_id = Some("cg1".into());
            c.text_content = Some(format!("word {i}"));
            after.tracks[0].clips.push(c);
        }
        let env = build_envelope(&before, &after, json!({}));
        assert!(env.get("clips").is_none(), "caption clips folded: {env}");
        let group = &env["captionGroups"][0];
        assert_eq!(group["captionGroupId"], json!("cg1"));
        assert_eq!(group["clipCount"], json!(3));
        assert_eq!(group["track"], json!(0));
    }

    #[test]
    fn two_caption_clips_stay_in_clips() {
        let before = timeline(vec![track(ClipType::Video, vec![])]);
        let mut after = before.clone();
        for i in 0..2 {
            let mut c = named_clip(&format!("cap{i}"), i * 60, 60);
            c.media_type = ClipType::Text;
            c.caption_group_id = Some("cg1".into());
            after.tracks[0].clips.push(c);
        }
        let env = build_envelope(&before, &after, json!({}));
        assert!(env.get("captionGroups").is_none());
        assert_eq!(env["clips"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn av_pair_folds_audio_into_visual_row() {
        let before = timeline(vec![
            track(ClipType::Video, vec![]),
            track(ClipType::Audio, vec![]),
        ]);
        let mut after = before.clone();
        let mut v = named_clip("v1", 0, 100);
        v.link_group_id = Some("lg1".into());
        let mut a = named_clip("a1", 0, 100);
        a.media_type = ClipType::Audio;
        a.link_group_id = Some("lg1".into());
        after.tracks[0].clips.push(v);
        after.tracks[1].clips.push(a);
        let env = build_envelope(&before, &after, json!({}));
        let clips = env["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 1, "audio partner folded: {env}");
        assert_eq!(clips[0]["id"], json!("v1"));
        assert_eq!(clips[0]["audio"]["id"], json!("a1"));
        assert_eq!(clips[0]["audio"]["track"], json!(1));
    }

    #[test]
    fn extras_merge_and_notes_concatenate() {
        let before = timeline(vec![
            track(ClipType::Video, vec![]),
            track(ClipType::Video, vec![]),
        ]);
        let mut after = before.clone();
        after.tracks.swap(0, 1);
        let env = build_envelope(
            &before,
            &after,
            json!({"moved": 4, "notes": ["tool note"]}),
        );
        assert_eq!(env["moved"], json!(4));
        assert_eq!(env["notes"], json!(["tool note", TRACKS_SHIFTED_NOTE]));
    }
}
