use crate::keyframes::{
    clamp_clip_fades_to_duration, clamp_clip_keyframes_to_duration, rescale_clip_keyframes,
    set_clip_duration, split_all_clip_keyframe_tracks,
};
use crate::{
    compute_overwrite, expand_to_link_group, linked_partner_ids, partner_moves_for_move_of,
    ClipMathExt, OverwriteAction,
};
use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipLocation {
    pub track_index: usize,
    pub clip_index: usize,
}

pub fn find_clip(timeline: &Timeline, clip_id: &str) -> Option<ClipLocation> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .clips
                .iter()
                .position(|clip| clip.id == clip_id)
                .map(|clip_index| ClipLocation {
                    track_index,
                    clip_index,
                })
        })
}

pub fn split_clip(timeline: &mut Timeline, clip_id: &str, at_frame: i64) -> Vec<String> {
    let Some(location) = find_clip(timeline, clip_id) else {
        return Vec::new();
    };
    let clip = timeline.tracks[location.track_index].clips[location.clip_index].clone();

    let group_ids: BTreeSet<String> = if clip.link_group_id.is_some() {
        let mut ids = BTreeSet::from([clip_id.to_string()]);
        ids.extend(linked_partner_ids(timeline, clip_id));
        ids
    } else {
        BTreeSet::from([clip_id.to_string()])
    };

    let mut right_ids = Vec::new();
    for group_id in &group_ids {
        if let Some(right_id) = split_single_clip(timeline, group_id, at_frame) {
            right_ids.push(right_id);
        }
    }

    if group_ids.len() > 1 && !right_ids.is_empty() {
        let new_group = Uuid::new_v4().to_string();
        for right_id in &right_ids {
            if let Some(location) = find_clip(timeline, right_id) {
                timeline.tracks[location.track_index].clips[location.clip_index].link_group_id =
                    Some(new_group.clone());
            }
        }
    }

    right_ids
}

pub fn apply_clip_speed(timeline: &mut Timeline, clip_id: &str, new_speed: f64) -> bool {
    if !new_speed.is_finite() || new_speed <= 0.0 {
        return false;
    }

    let Some(location) = find_clip(timeline, clip_id) else {
        return false;
    };

    let track_index = location.track_index;
    let basis = timeline.tracks[track_index].clips[location.clip_index].clone();
    let clip = timeline.tracks[track_index].clips[location.clip_index].clone();
    let source_frames = (basis.duration_frames as f64) * basis.speed;
    let new_duration = ((source_frames / new_speed).round() as i64).max(1);
    let old_end = clip.end_frame();

    timeline.tracks[track_index].clips[location.clip_index].speed = new_speed;
    timeline.tracks[track_index].clips[location.clip_index].duration_frames = new_duration;
    // PR #129: rescale keyframes before clamp to preserve keyframe positions
    let old_duration = basis.duration_frames.max(1) as f64;
    let rescale_ratio = new_duration as f64 / old_duration;
    rescale_clip_keyframes(
        &mut timeline.tracks[track_index].clips[location.clip_index],
        rescale_ratio,
    );
    clamp_clip_keyframes_to_duration(&mut timeline.tracks[track_index].clips[location.clip_index]);
    clamp_clip_fades_to_duration(&mut timeline.tracks[track_index].clips[location.clip_index]);

    let ripple_delta = (clip.start_frame + new_duration) - old_end;
    if ripple_delta != 0 {
        let chain_ids = contiguous_clip_ids(&timeline.tracks[track_index], old_end, &clip.id);
        for follower in &mut timeline.tracks[track_index].clips {
            if chain_ids.contains(&follower.id) {
                follower.start_frame += ripple_delta;
            }
        }
    }

    sort_clips(&mut timeline.tracks[track_index]);
    true
}

pub fn clear_region(
    timeline: &mut Timeline,
    track_index: usize,
    start: i64,
    end: i64,
    prune: bool,
) {
    if timeline.tracks.get(track_index).is_none() {
        return;
    }

    let actions = compute_overwrite(&timeline.tracks[track_index].clips, start, end);
    for action in actions {
        match action {
            OverwriteAction::Remove { clip_id } => remove_clips(timeline, [clip_id], prune),
            OverwriteAction::TrimEnd {
                clip_id,
                new_duration,
            } => {
                if let Some(location) = find_clip(timeline, &clip_id) {
                    let clip =
                        timeline.tracks[location.track_index].clips[location.clip_index].clone();
                    let source_delta = (((clip.duration_frames - new_duration) as f64) * clip.speed)
                        .round() as i64;
                    let new_trim_end = clip.trim_end_frame + source_delta;
                    let target =
                        &mut timeline.tracks[location.track_index].clips[location.clip_index];
                    target.trim_end_frame = new_trim_end;
                    set_clip_duration(target, new_duration);
                }
            }
            OverwriteAction::TrimStart {
                clip_id,
                new_start_frame,
                new_trim_start,
                new_duration,
            } => {
                if let Some(location) = find_clip(timeline, &clip_id) {
                    let target =
                        &mut timeline.tracks[location.track_index].clips[location.clip_index];
                    target.start_frame = new_start_frame;
                    target.trim_start_frame = new_trim_start;
                    set_clip_duration(target, new_duration);
                }
            }
            OverwriteAction::Split { clip_id, .. } => {
                // Track-LOCAL split: clearing a region on one track must not touch a
                // link-grouped partner on another track. `split_clip` is link-aware
                // and would split (and orphan) every group member across all tracks;
                // the other overwrite branches all operate on the single found clip,
                // so this one must too.
                if let Some(right_clip_id) = split_single_clip(timeline, &clip_id, start) {
                    if let Some(right_location) = find_clip(timeline, &right_clip_id) {
                        let right_end = timeline.tracks[right_location.track_index].clips
                            [right_location.clip_index]
                            .end_frame();
                        if right_end > end {
                            split_single_clip(timeline, &right_clip_id, end);
                        }
                    }
                    remove_clips(timeline, [right_clip_id], prune);
                }
            }
        }
    }
}

fn split_single_clip(timeline: &mut Timeline, clip_id: &str, at_frame: i64) -> Option<String> {
    let location = find_clip(timeline, clip_id)?;
    let clip = timeline.tracks[location.track_index].clips[location.clip_index].clone();
    if at_frame <= clip.start_frame || at_frame >= clip.end_frame() {
        return None;
    }

    let split_offset = at_frame - clip.start_frame;
    let left_source = ((split_offset as f64) * clip.speed).round() as i64;
    let right_source = (((clip.duration_frames - split_offset) as f64) * clip.speed).round() as i64;

    let (mut left, mut right) = split_all_clip_keyframe_tracks(&clip, split_offset);
    left.duration_frames = split_offset;
    left.trim_end_frame = clip.trim_end_frame + right_source;
    left.fade_out_frames = 0;
    clamp_clip_fades_to_duration(&mut left);

    right.id = Uuid::new_v4().to_string();
    right.start_frame = at_frame;
    right.duration_frames = clip.duration_frames - split_offset;
    right.trim_start_frame = clip.trim_start_frame + left_source;
    right.fade_in_frames = 0;
    clamp_clip_fades_to_duration(&mut right);

    timeline.tracks[location.track_index].clips[location.clip_index] = left;
    timeline.tracks[location.track_index]
        .clips
        .push(right.clone());
    sort_clips(&mut timeline.tracks[location.track_index]);

    Some(right.id)
}

fn sort_clips(track: &mut Track) {
    track.clips.sort_by_key(|clip| clip.start_frame);
}

pub fn place_clips(
    timeline: &mut Timeline,
    track_index: usize,
    start_frame: i64,
    clips: &[Clip],
) -> Vec<String> {
    if track_index >= timeline.tracks.len() || clips.is_empty() {
        return Vec::new();
    }
    let total_duration: i64 = clips.iter().map(|c| c.duration_frames).sum();
    if total_duration <= 0 {
        return Vec::new();
    }
    // CLP-002: Overwrite placement clears conflicting destination regions before inserting
    clear_region(
        timeline,
        track_index,
        start_frame,
        start_frame + total_duration,
        false,
    );
    // Place clips sequentially at the target
    let mut offset = 0i64;
    let mut placed_ids = Vec::new();
    for clip in clips {
        let mut new_clip = clip.clone();
        new_clip.id = Uuid::new_v4().to_string();
        new_clip.start_frame = start_frame + offset;
        timeline.tracks[track_index].clips.push(new_clip.clone());
        placed_ids.push(new_clip.id);
        offset += clip.duration_frames;
    }
    sort_clips(&mut timeline.tracks[track_index]);
    placed_ids
}

pub fn move_clips(
    timeline: &mut Timeline,
    clip_ids: &[String],
    dest_track_index: usize,
    dest_start_frame: i64,
) -> Vec<String> {
    if clip_ids.is_empty() || dest_track_index >= timeline.tracks.len() {
        return Vec::new();
    }

    let id_set: BTreeSet<String> = clip_ids.iter().cloned().collect();
    let expanded = expand_to_link_group(timeline, &id_set);
    let dest_type = timeline.tracks[dest_track_index].r#type;

    // Phase 1: Collect all data before any mutation
    let mut primary_clips: Vec<Clip> = Vec::new();
    for clip_id in clip_ids {
        let Some(loc) = find_clip(timeline, clip_id) else {
            return Vec::new();
        };
        let clip = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
        // CLP-005: destination track compatibility
        if !clip_types_compatible(&clip.media_type, &dest_type) {
            return Vec::new();
        }
        primary_clips.push(clip);
    }

    if primary_clips.is_empty() {
        return Vec::new();
    }

    // Clamp the destination to the frame-0 floor ONCE (Swift moveClips uses
    // max(0, toFrame)) and derive BOTH the primary placement and each linked-partner
    // delta from it. Deriving the partner delta from the UNCLAMPED dest while the
    // primary lands at the clamped one desyncs an A/V link on a negative dest.
    let clamped_dest = dest_start_frame.max(0);

    // Compute each primary clip's new frame for linked-partner delta propagation
    let mut primary_new_frames: Vec<(String, i64)> = Vec::new();
    {
        let mut offset = 0i64;
        for clip in &primary_clips {
            primary_new_frames.push((clip.id.clone(), clamped_dest + offset));
            offset += clip.duration_frames;
        }
    }

    // Collect linked partner data (partners NOT in original selection)
    let mut linked_partner_clips: Vec<(Clip, usize, i64)> = Vec::new();
    for (clip_id, new_frame) in &primary_new_frames {
        for pm in partner_moves_for_move_of(timeline, clip_id, *new_frame) {
            if id_set.contains(&pm.clip_id) {
                continue;
            }
            // Avoid duplicates (partner linked to multiple selected clips)
            if linked_partner_clips
                .iter()
                .any(|(c, _, _)| c.id == pm.clip_id)
            {
                continue;
            }
            let Some(loc) = find_clip(timeline, &pm.clip_id) else {
                continue;
            };
            let partner = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
            linked_partner_clips.push((partner, pm.track_index, pm.to_frame));
        }
    }

    let total_duration: i64 = primary_clips.iter().map(|c| c.duration_frames).sum();

    // Phase 2: CLP-003 — remove moved clips (primary + partners, via `expanded`)
    // from source BEFORE clearing destinations.
    let remove_ids: Vec<String> = expanded.into_iter().collect();
    remove_clips(timeline, remove_ids, false);

    // Phase 3: CLP-004/006 — clear ALL destination regions (primary + every
    // partner) before placing anything, so a clear never runs against an
    // already-placed moved clip (which it would Remove, losing it).
    if total_duration > 0 {
        clear_region(
            timeline,
            dest_track_index,
            clamped_dest,
            clamped_dest + total_duration,
            false,
        );
    }
    for (clip, track_index, new_start_frame) in &linked_partner_clips {
        let start = (*new_start_frame).max(0);
        if clip.duration_frames > 0 && *track_index < timeline.tracks.len() {
            clear_region(timeline, *track_index, start, start + clip.duration_frames, false);
        }
    }

    // Phase 4: insert primary clips at the clamped target.
    let mut offset = 0i64;
    let mut placed_ids = Vec::new();
    for mut clip in primary_clips {
        let new_id = Uuid::new_v4().to_string();
        clip.id = new_id.clone();
        clip.start_frame = clamped_dest + offset;
        let duration = clip.duration_frames;
        timeline.tracks[dest_track_index].clips.push(clip);
        placed_ids.push(new_id);
        offset += duration;
    }
    sort_clips(&mut timeline.tracks[dest_track_index]);

    // Phase 5: place linked partners (destinations already cleared in Phase 3).
    let mut partner_tracks: BTreeSet<usize> = BTreeSet::new();
    for (mut clip, track_index, new_start_frame) in linked_partner_clips {
        clip.start_frame = new_start_frame.max(0);
        timeline.tracks[track_index].clips.push(clip);
        partner_tracks.insert(track_index);
    }
    for ti in partner_tracks {
        if ti < timeline.tracks.len() {
            sort_clips(&mut timeline.tracks[ti]);
        }
    }

    placed_ids
}

fn clip_types_compatible(clip_type: &ClipType, track_type: &ClipType) -> bool {
    match (clip_type, track_type) {
        (ClipType::Audio, ClipType::Audio) => true,
        (ClipType::Audio, _) => false,
        (_, ClipType::Audio) => false,
        _ => true,
    }
}

/// Auto-create linked audio clips for placed video clips (CLP-007/008).
///
/// For each video clip in `video_clip_ids`, creates an audio clip on
/// `audio_track_index` with the same position, duration, trims, and speed.
/// Each video–audio pair shares a new `link_group_id`.
/// Returns the IDs of the created audio clips.
pub fn link_audio_for_placed_clips(
    timeline: &mut Timeline,
    video_clip_ids: &[String],
    audio_track_index: usize,
) -> Vec<String> {
    if audio_track_index >= timeline.tracks.len() || video_clip_ids.is_empty() {
        return Vec::new();
    }
    if timeline.tracks[audio_track_index].r#type != ClipType::Audio {
        return Vec::new();
    }

    // Phase 1: collect immutable data from video clips (clone before mutation)
    struct VideoInfo {
        track_index: usize,
        clip_index: usize,
        link_group: String,
        audio_id: String,
        start_frame: i64,
        duration_frames: i64,
        media_ref: String,
        trim_start_frame: i64,
        trim_end_frame: i64,
        speed: f64,
    }

    let mut video_info_list: Vec<VideoInfo> = Vec::new();
    let mut audio_ids: Vec<String> = Vec::new();
    let mut total_duration: i64 = 0;

    for video_id in video_clip_ids {
        let Some(loc) = find_clip(timeline, video_id) else {
            continue;
        };
        let video = &timeline.tracks[loc.track_index].clips[loc.clip_index];
        let link_group = Uuid::new_v4().to_string();
        let audio_id = Uuid::new_v4().to_string();

        audio_ids.push(audio_id.clone());
        video_info_list.push(VideoInfo {
            track_index: loc.track_index,
            clip_index: loc.clip_index,
            link_group,
            audio_id,
            start_frame: video.start_frame,
            duration_frames: video.duration_frames,
            media_ref: video.media_ref.clone(),
            trim_start_frame: video.trim_start_frame,
            trim_end_frame: video.trim_end_frame,
            speed: video.speed,
        });
        total_duration += video.duration_frames;
    }

    if video_info_list.is_empty() {
        return Vec::new();
    }

    let first_start = video_info_list[0].start_frame;

    // Phase 2: stamp link_group_ids onto video clips (mutable)
    for info in &video_info_list {
        timeline.tracks[info.track_index].clips[info.clip_index].link_group_id =
            Some(info.link_group.clone());
    }

    // Phase 3: clear destination region on audio track
    clear_region(
        timeline,
        audio_track_index,
        first_start,
        first_start + total_duration,
        false,
    );

    // Phase 4: create and place audio clips
    for info in &video_info_list {
        timeline.tracks[audio_track_index].clips.push(Clip {
            id: info.audio_id.clone(),
            media_ref: info.media_ref.clone(),
            media_type: ClipType::Audio,
            source_clip_type: ClipType::Audio,
            start_frame: info.start_frame,
            duration_frames: info.duration_frames,
            trim_start_frame: info.trim_start_frame,
            trim_end_frame: info.trim_end_frame,
            speed: info.speed,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: Some(info.link_group.clone()),
            caption_group_id: None,
            text_content: None,
            text_style: None,
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
            text_animation: None,
            word_timings: None,
        });
    }
    sort_clips(&mut timeline.tracks[audio_track_index]);

    audio_ids
}

pub fn prune_empty_tracks(timeline: &mut Timeline) {
    timeline.tracks.retain(|track| !track.clips.is_empty());
}

pub fn remove_clips<I>(timeline: &mut Timeline, ids: I, prune: bool)
where
    I: IntoIterator<Item = String>,
{
    let ids: BTreeSet<String> = ids.into_iter().collect();
    if ids.is_empty() {
        return;
    }

    for track in &mut timeline.tracks {
        track.clips.retain(|clip| !ids.contains(&clip.id));
    }
    timeline.selected_clip_ids.retain(|id| !ids.contains(id));
    if prune {
        prune_empty_tracks(timeline);
    }
}

fn contiguous_clip_ids(track: &Track, from_end: i64, exclude_id: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let mut chain_end = from_end;

    let mut ordered = track.clips.clone();
    ordered.sort_by_key(|clip| clip.start_frame);

    for clip in ordered
        .into_iter()
        .filter(|clip| clip.id != exclude_id && clip.start_frame >= from_end)
    {
        if clip.start_frame != chain_end {
            break;
        }
        chain_end = clip.end_frame();
        ids.insert(clip.id);
    }

    ids
}
