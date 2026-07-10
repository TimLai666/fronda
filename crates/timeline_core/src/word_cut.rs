//! Word-aligned cut planning for the `remove_words` agent tool (upstream #160, #245).
//!
//! Pure frame math: given transcript words mapped onto the timeline (which word is
//! selected for removal, in project frames), plan the ripple-delete ranges that cut
//! the selected words plus their surrounding pause, merge adjacent removals, and pick
//! the single primary track to cut (the ripple carries its linked A/V partners).
//!
//! The transcription itself (source audio → words) is a host/platform concern and is
//! NOT ported here — callers supply the already-mapped [`TimelineWord`] list, mirroring
//! how `get_transcript`/`inspect_media` take caller-supplied words.

use crate::edit::find_clip;
use crate::ripple::{merge_ranges, FrameRange};
use core_model::{Clip, Timeline};
use std::collections::{BTreeMap, BTreeSet};

/// How much silence to leave between the words on either side of a cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutAggressiveness {
    Tight,
    Balanced,
    Loose,
}

impl CutAggressiveness {
    /// Kept gap in milliseconds (split half to each side of a cut).
    pub fn kept_gap_ms(self) -> f64 {
        match self {
            CutAggressiveness::Tight => 60.0,
            CutAggressiveness::Balanced => 150.0,
            CutAggressiveness::Loose => 320.0,
        }
    }

    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "tight" => Some(CutAggressiveness::Tight),
            "balanced" => Some(CutAggressiveness::Balanced),
            "loose" => Some(CutAggressiveness::Loose),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CutAggressiveness::Tight => "tight",
            CutAggressiveness::Balanced => "balanced",
            CutAggressiveness::Loose => "loose",
        }
    }

    pub const ALL: [CutAggressiveness; 3] = [
        CutAggressiveness::Tight,
        CutAggressiveness::Balanced,
        CutAggressiveness::Loose,
    ];
}

/// Round a millisecond duration to whole project frames.
pub fn ms_to_frames(ms: f64, fps: i64) -> i64 {
    (ms / 1000.0 * fps as f64).round() as i64
}

/// One word inside a single clip, in project frames, flagged for removal or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerWord {
    pub start_frame: i64,
    pub end_frame: i64,
    pub selected: bool,
}

/// Plan the cut ranges for one clip: cut each run of selected words plus up to half the
/// adjacent gap on each side, then merge adjacent ranges. Zero-length words are dropped
/// first so neighbour lookups use only real words. Mirrors Swift `WordCutPlanner.cutRanges`.
pub fn cut_ranges(
    words: &[PlannerWord],
    clip_start: i64,
    clip_end: i64,
    keep_gap_frames: i64,
) -> Vec<FrameRange> {
    let words: Vec<&PlannerWord> = words.iter().filter(|w| w.end_frame > w.start_frame).collect();
    if clip_end <= clip_start || words.is_empty() {
        return Vec::new();
    }
    let half = (keep_gap_frames / 2).max(0);
    let mut ranges: Vec<FrameRange> = Vec::new();
    let mut k = 0;
    while k < words.len() {
        if !words[k].selected {
            k += 1;
            continue;
        }
        let mut l = k;
        while l + 1 < words.len() && words[l + 1].selected {
            l += 1;
        }
        let left = if k > 0 { words[k - 1].end_frame } else { clip_start };
        let right = if l + 1 < words.len() {
            words[l + 1].start_frame
        } else {
            clip_end
        };
        let run_start = words[k].start_frame;
        let run_end = words[l].end_frame;
        let keep_before = (run_start - left).max(0).min(half);
        let keep_after = (right - run_end).max(0).min(half);
        let start = clip_start.max((left + keep_before).min(run_start));
        let end = clip_end.min(run_end.max(right - keep_after));
        if end > start {
            ranges.push(FrameRange { start, end });
        }
        k = l + 1;
    }
    merge_ranges(&ranges)
}

/// Map a source-seconds span to project frames for a clip, clamped to the clip's visible
/// window first so a boundary-straddler yields its real sliver, not a fabricated full-clip
/// span. Returns `None` when the span is not visible. Mirrors Swift `spanFrames`.
pub fn span_frames(start_sec: f64, end_sec: f64, clip: &Clip, fps: i64) -> Option<(i64, i64)> {
    let fps_d = fps as f64;
    let speed = clip.speed.max(0.0001);
    let vis_start = clip.trim_start_frame as f64;
    let vis_end = vis_start + clip.duration_frames as f64 * speed;
    let s = (start_sec * fps_d).max(vis_start);
    let e = (end_sec * fps_d).min(vis_end);
    if e <= s {
        return None;
    }
    let to_timeline =
        |source_frame: f64| (clip.start_frame as f64 + (source_frame - vis_start) / speed).round() as i64;
    let a = to_timeline(s);
    Some((a, a.max(to_timeline(e))))
}

/// A transcript word already mapped onto one timeline clip. `index` is the stable global
/// 0-based position in timeline order (what `get_transcript` emits and `remove_words` takes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineWord {
    pub index: usize,
    pub clip_id: String,
    pub track_index: usize,
    pub clip_start_frame: i64,
    pub clip_end_frame: i64,
    pub text: String,
    pub start_frame: i64,
    pub end_frame: i64,
}

/// Map source-second word stamps `(text, start_seconds, end_seconds)` onto one clip as
/// [`TimelineWord`]s in project frames, via [`span_frames`] (source_offset_seconds =
/// trim_start_frame / fps, divided by clip speed — the silence-detector placement
/// convention). Invisible words are dropped without consuming an index; straddlers keep
/// their visible sliver. Global indices run from `first_index` so per-clip results
/// concatenate into one timeline-ordered list.
pub fn map_word_stamps(
    stamps: &[(&str, f64, f64)],
    clip: &Clip,
    track_index: usize,
    first_index: usize,
    fps: i64,
) -> Vec<TimelineWord> {
    let mut out = Vec::new();
    for &(text, start_sec, end_sec) in stamps {
        let Some((start_frame, end_frame)) = span_frames(start_sec, end_sec, clip, fps) else {
            continue;
        };
        out.push(TimelineWord {
            index: first_index + out.len(),
            clip_id: clip.id.clone(),
            track_index,
            clip_start_frame: clip.start_frame,
            clip_end_frame: clip.start_frame + clip.duration_frames,
            text: text.to_string(),
            start_frame,
            end_frame,
        });
    }
    out
}

/// The resolved cut plan: cut `ranges` on `primary_track`; the ripple carries linked
/// partners across the same span. `removed_texts` is the removed words in timeline order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRemovalPlan {
    pub primary_track: usize,
    pub ranges: Vec<FrameRange>,
    pub removed_texts: Vec<String>,
}

/// Plan a `remove_words` edit from the mapped word list + the selected global indices.
///
/// Groups words by clip (contiguous runs of the same `clip_id`, as the flat list is
/// clip-ordered), plans cut ranges per clip, and collects them per track. Cutting is
/// done on ONE primary track: if the selection touches several tracks they must all be
/// one link group (e.g. camera + mic) — otherwise the call is refused, because cutting
/// unlinked tracks together would break their alignment. Only the primary track's own
/// ranges are used; the ripple removes the same span from linked partners, so flattening
/// foreign-track frames onto the primary track would over-cut it. Mirrors Swift `removeWords`.
pub fn plan_word_removal(
    timeline: &Timeline,
    all_words: &[TimelineWord],
    selected: &BTreeSet<usize>,
    keep_gap_frames: i64,
) -> Result<WordRemovalPlan, String> {
    let mut removed_texts: Vec<String> = Vec::new();
    let mut ranges_by_track: BTreeMap<usize, Vec<FrameRange>> = BTreeMap::new();
    let mut involved_clips: Vec<String> = Vec::new();

    let mut i = 0;
    while i < all_words.len() {
        let clip_id = &all_words[i].clip_id;
        let mut j = i + 1;
        while j < all_words.len() && &all_words[j].clip_id == clip_id {
            j += 1;
        }
        let group = &all_words[i..j];
        i = j;

        if !group.iter().any(|w| selected.contains(&w.index)) {
            continue;
        }
        let track_index = group[0].track_index;
        let clip_start = group[0].clip_start_frame;
        let clip_end = group[0].clip_end_frame;

        for w in group {
            if selected.contains(&w.index) && w.end_frame > w.start_frame {
                removed_texts.push(w.text.clone());
            }
        }
        let plan: Vec<PlannerWord> = group
            .iter()
            .map(|w| PlannerWord {
                start_frame: w.start_frame,
                end_frame: w.end_frame,
                selected: selected.contains(&w.index),
            })
            .collect();
        let ranges = cut_ranges(&plan, clip_start, clip_end, keep_gap_frames);
        if !ranges.is_empty() {
            ranges_by_track
                .entry(track_index)
                .or_default()
                .extend(ranges);
            involved_clips.push(group[0].clip_id.clone());
        }
    }

    if ranges_by_track.is_empty() {
        return Err("The selected words resolved to no removable frames. Re-read get_transcript.".into());
    }

    let primary_track = if ranges_by_track.len() == 1 {
        *ranges_by_track.keys().next().unwrap()
    } else {
        let group_ids: Vec<String> = involved_clips
            .iter()
            .filter_map(|id| {
                find_clip(timeline, id).and_then(|loc| {
                    timeline.tracks[loc.track_index].clips[loc.clip_index]
                        .link_group_id
                        .clone()
                })
            })
            .collect();
        let distinct: BTreeSet<&String> = group_ids.iter().collect();
        if group_ids.len() != involved_clips.len() || distinct.len() != 1 {
            let tracks = ranges_by_track
                .keys()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Selected words span multiple unlinked tracks ({tracks}). Remove words one track at a time — linked video/audio is cut automatically. If these tracks are the same source (e.g. camera + mic), link them into one unit first."
            ));
        }
        *ranges_by_track.keys().min().unwrap()
    };

    let ranges = merge_ranges(&ranges_by_track[&primary_track]);
    Ok(WordRemovalPlan {
        primary_track,
        ranges,
        removed_texts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{ClipType, Track};

    // ── WordCutPlanner (mirrors Swift WordCutPlannerTests) ──────────────────

    fn words(selected: &[usize]) -> Vec<PlannerWord> {
        let spans = [(0, 10), (11, 20), (21, 30), (31, 40)];
        spans
            .iter()
            .enumerate()
            .map(|(i, &(s, e))| PlannerWord {
                start_frame: s,
                end_frame: e,
                selected: selected.contains(&i),
            })
            .collect()
    }

    #[test]
    fn cut_single_word() {
        assert_eq!(
            cut_ranges(&words(&[1]), 0, 100, 6),
            vec![FrameRange { start: 11, end: 20 }]
        );
    }

    #[test]
    fn cut_contiguous_run() {
        assert_eq!(
            cut_ranges(&words(&[1, 2]), 0, 100, 6),
            vec![FrameRange { start: 11, end: 30 }]
        );
    }

    #[test]
    fn cut_non_adjacent_yields_two_ranges() {
        assert_eq!(cut_ranges(&words(&[0, 2]), 0, 100, 0).len(), 2);
    }

    #[test]
    fn cut_overlapping_timestamps() {
        let ws = vec![
            PlannerWord { start_frame: 0, end_frame: 10, selected: false },
            PlannerWord { start_frame: 9, end_frame: 20, selected: true },
            PlannerWord { start_frame: 19, end_frame: 30, selected: false },
        ];
        assert_eq!(
            cut_ranges(&ws, 0, 100, 6),
            vec![FrameRange { start: 9, end: 20 }]
        );
    }

    #[test]
    fn cut_ignores_zero_length_words() {
        // A zero-length word is dropped before planning, so a lone zero-length
        // selection yields no removable range (nothing real is selected).
        let ws = vec![PlannerWord { start_frame: 5, end_frame: 5, selected: true }];
        assert!(cut_ranges(&ws, 0, 100, 0).is_empty());
        // With a real neighbour, the dropped zero-length word doesn't shift the cut:
        // [word0 kept][word1 zero-len selected, dropped][word2 selected] → only word2 cut.
        let ws2 = vec![
            PlannerWord { start_frame: 0, end_frame: 10, selected: false },
            PlannerWord { start_frame: 12, end_frame: 12, selected: true }, // dropped
            PlannerWord { start_frame: 20, end_frame: 30, selected: true },
        ];
        // filtered: [(0,10,false),(20,30,true)]; lone selected survivor with no right
        // neighbour extends to clip_end. left=10, half=0 → start=10; end=100.
        assert_eq!(cut_ranges(&ws2, 0, 100, 0), vec![FrameRange { start: 10, end: 100 }]);
    }

    #[test]
    fn cut_empty_when_nothing_selected() {
        assert!(cut_ranges(&words(&[]), 0, 100, 6).is_empty());
    }

    // ── ms_to_frames ────────────────────────────────────────────────────────

    #[test]
    fn ms_to_frames_rounds() {
        assert_eq!(ms_to_frames(150.0, 30), 5); // 0.15 * 30 = 4.5 → 5 (half away from zero)
        assert_eq!(ms_to_frames(60.0, 30), 2); // 0.06 * 30 = 1.8 → 2
        assert_eq!(ms_to_frames(320.0, 24), 8); // 0.32 * 24 = 7.68 → 8
        assert_eq!(ms_to_frames(0.0, 30), 0);
    }

    #[test]
    fn aggressiveness_gaps() {
        assert_eq!(CutAggressiveness::Tight.kept_gap_ms(), 60.0);
        assert_eq!(CutAggressiveness::Balanced.kept_gap_ms(), 150.0);
        assert_eq!(CutAggressiveness::Loose.kept_gap_ms(), 320.0);
        assert_eq!(CutAggressiveness::from_raw("loose"), Some(CutAggressiveness::Loose));
        assert_eq!(CutAggressiveness::from_raw("nope"), None);
    }

    // ── span_frames ─────────────────────────────────────────────────────────

    fn clip(start: i64, dur: i64, trim: i64, speed: f64) -> Clip {
        Clip {
            id: "c".into(),
            media_ref: "m".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: trim,
            trim_end_frame: 0,
            speed,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
            opacity: 1.0,
            transform: Default::default(),
            crop: Default::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            text_animation: None,
            word_timings: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: Default::default(),
            chroma_key: None,
            multicam_group_id: None,
        }
    }

    #[test]
    fn span_frames_identity_at_1x_no_trim() {
        // clip at timeline 0, trim 0, speed 1, fps 30: source second 1.0..2.0 → frames 30..60.
        let c = clip(0, 300, 0, 1.0);
        assert_eq!(span_frames(1.0, 2.0, &c, 30), Some((30, 60)));
    }

    #[test]
    fn span_frames_offset_by_clip_start_and_trim() {
        // clip starts at timeline 100, trims first 30 source frames; source 1.5s (=45f) → 45-30=15 past clip start → 115.
        let c = clip(100, 300, 30, 1.0);
        assert_eq!(span_frames(1.5, 1.5 + 1.0, &c, 30), Some((115, 145)));
    }

    #[test]
    fn span_frames_none_when_outside_visible() {
        let c = clip(0, 30, 0, 1.0); // visible 0..30 source frames = 0..1s
        assert_eq!(span_frames(5.0, 6.0, &c, 30), None);
    }

    #[test]
    fn span_frames_speed_compresses() {
        // 2x speed: 60 source frames of visible content occupy 30 timeline frames.
        let c = clip(0, 30, 0, 2.0);
        // source 0..1s (0..30 src frames) → timeline 0..15.
        assert_eq!(span_frames(0.0, 1.0, &c, 30), Some((0, 15)));
    }

    // ── map_word_stamps (transcription-provider-seam) ───────────────────────

    #[test]
    fn map_word_stamps_spec_table_trimmed_clip() {
        // Spec row 1: trim 60 @ 30fps (2.0s offset), word at source 3.0s,
        // clip start 100 → project frame 130.
        let c = clip(100, 300, 60, 1.0);
        let words = map_word_stamps(&[("hello", 3.0, 3.5)], &c, 0, 0, 30);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].start_frame, 130);
        assert_eq!(words[0].end_frame, 145);
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[0].index, 0);
        assert_eq!(words[0].clip_id, "c");
        assert_eq!(words[0].track_index, 0);
        assert_eq!(words[0].clip_start_frame, 100);
        assert_eq!(words[0].clip_end_frame, 400);
    }

    #[test]
    fn map_word_stamps_spec_table_untrimmed_clip() {
        // Spec row 2: trim 0 @ 30fps, word at source 1.0s, clip start 0 → frame 30.
        let c = clip(0, 300, 0, 1.0);
        let words = map_word_stamps(&[("one", 1.0, 1.5)], &c, 2, 5, 30);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].start_frame, 30);
        assert_eq!(words[0].end_frame, 45);
        assert_eq!(words[0].index, 5, "global index starts at first_index");
        assert_eq!(words[0].track_index, 2);
    }

    #[test]
    fn map_word_stamps_speed_scales() {
        // 2x speed, trim 60: source 3.0s = frame 90 → (90-60)/2 = 15 past clip start.
        let c = clip(100, 300, 60, 2.0);
        let words = map_word_stamps(&[("fast", 3.0, 3.5)], &c, 0, 0, 30);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].start_frame, 115);
        assert_eq!(words[0].end_frame, 123); // 100 + round(45/2)
    }

    #[test]
    fn map_word_stamps_multi_clip_chaining() {
        // One source split across two clips: A shows 0..2s, B shows 2..4s.
        // Each word lands in exactly one clip; global indices stay contiguous.
        let a = clip(0, 60, 0, 1.0);
        let mut b = clip(60, 60, 60, 1.0);
        b.id = "b".into();
        let stamps = [("one", 1.0, 1.5), ("two", 3.0, 3.5)];
        let mut all = map_word_stamps(&stamps, &a, 0, 0, 30);
        all.extend(map_word_stamps(&stamps, &b, 0, all.len(), 30));
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].index, all[0].start_frame), (0, 30));
        assert_eq!(all[0].clip_id, "c");
        assert_eq!((all[1].index, all[1].start_frame, all[1].end_frame), (1, 90, 105));
        assert_eq!(all[1].clip_id, "b");
    }

    #[test]
    fn map_word_stamps_drops_invisible_and_clamps_straddler() {
        // trim 60 @ 30fps: visible source window is 2.0s.. — a word before it is
        // dropped without consuming an index; a straddler keeps its visible sliver.
        let c = clip(100, 300, 60, 1.0);
        let stamps = [("gone", 0.5, 1.0), ("edge", 1.5, 2.5), ("in", 3.0, 3.5)];
        let words = map_word_stamps(&stamps, &c, 0, 0, 30);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "edge");
        assert_eq!((words[0].start_frame, words[0].end_frame), (100, 115));
        assert_eq!(words[0].index, 0, "dropped word does not consume an index");
        assert_eq!(words[1].text, "in");
        assert_eq!(words[1].index, 1);
    }

    #[test]
    fn map_word_stamps_empty_input() {
        let c = clip(0, 300, 0, 1.0);
        assert!(map_word_stamps(&[], &c, 0, 0, 30).is_empty());
    }

    // ── plan_word_removal ─────────────────────────────────────────────────

    fn tl(tracks: Vec<Track>) -> Timeline {
        let mut t = Timeline::default();
        t.fps = 30;
        t.tracks = tracks;
        t
    }

    fn track(clips: Vec<Clip>) -> Track {
        Track {
            id: "t".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            display_height: 50.0,
            clips,
        }
    }

    fn tw(index: usize, clip_id: &str, track_index: usize, start: i64, end: i64) -> TimelineWord {
        TimelineWord {
            index,
            clip_id: clip_id.into(),
            track_index,
            clip_start_frame: 0,
            clip_end_frame: 100,
            text: format!("w{index}"),
            start_frame: start,
            end_frame: end,
        }
    }

    #[test]
    fn plan_single_track_one_word() {
        let timeline = tl(vec![track(vec![clip(0, 100, 0, 1.0)])]);
        let all = vec![
            tw(0, "c", 0, 0, 10),
            tw(1, "c", 0, 11, 20),
            tw(2, "c", 0, 21, 30),
        ];
        let selected: BTreeSet<usize> = [1].into_iter().collect();
        let plan = plan_word_removal(&timeline, &all, &selected, 6).unwrap();
        assert_eq!(plan.primary_track, 0);
        assert_eq!(plan.ranges, vec![FrameRange { start: 11, end: 20 }]);
        assert_eq!(plan.removed_texts, vec!["w1"]);
    }

    #[test]
    fn plan_errors_when_no_removable_frames() {
        let timeline = tl(vec![track(vec![clip(0, 100, 0, 1.0)])]);
        let all = vec![tw(0, "c", 0, 0, 10)];
        let selected: BTreeSet<usize> = [5].into_iter().collect(); // no such word selected → empty
        assert!(plan_word_removal(&timeline, &all, &selected, 6).is_err());
    }

    #[test]
    fn plan_refuses_unlinked_multi_track() {
        // words on two DIFFERENT tracks, clips not linked → refuse.
        let mut ca = clip(0, 100, 0, 1.0);
        ca.id = "a".into();
        let mut cb = clip(0, 100, 0, 1.0);
        cb.id = "b".into();
        let timeline = tl(vec![track(vec![ca]), track(vec![cb])]);
        let all = vec![tw(0, "a", 0, 0, 10), tw(1, "b", 1, 0, 10)];
        let selected: BTreeSet<usize> = [0, 1].into_iter().collect();
        let err = plan_word_removal(&timeline, &all, &selected, 0).unwrap_err();
        assert!(err.contains("multiple unlinked tracks"), "{err}");
    }

    #[test]
    fn plan_allows_linked_multi_track_picks_min() {
        // same two tracks but clips share a link group → allowed, primary = min track index.
        let mut ca = clip(0, 100, 0, 1.0);
        ca.id = "a".into();
        ca.link_group_id = Some("g1".into());
        let mut cb = clip(0, 100, 0, 1.0);
        cb.id = "b".into();
        cb.link_group_id = Some("g1".into());
        let timeline = tl(vec![track(vec![ca]), track(vec![cb])]);
        let all = vec![tw(0, "a", 0, 0, 10), tw(1, "b", 1, 0, 10)];
        let selected: BTreeSet<usize> = [0, 1].into_iter().collect();
        let plan = plan_word_removal(&timeline, &all, &selected, 0).unwrap();
        assert_eq!(plan.primary_track, 0);
    }
}
