use crate::keyframes::{
    clamp_clip_fades_to_duration, clamp_clip_keyframes_to_duration, set_clip_duration,
    split_all_clip_keyframe_tracks,
};
use crate::{compute_overwrite, ClipMathExt, OverwriteAction};
use core_model::{Timeline, Track};
use std::collections::{BTreeSet, HashMap};
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
                if let Some(location) = find_clip(timeline, &clip_id) {
                    let original_track_index = location.track_index;
                    split_clip(timeline, &clip_id, start);
                    let right_clip_id = timeline.tracks[original_track_index]
                        .clips
                        .iter()
                        .find(|clip| clip.start_frame == start && clip.id != clip_id)
                        .map(|clip| clip.id.clone());

                    if let Some(right_clip_id) = right_clip_id {
                        if let Some(right_location) = find_clip(timeline, &right_clip_id) {
                            let right_end = timeline.tracks[right_location.track_index].clips
                                [right_location.clip_index]
                                .end_frame();
                            if right_end > end {
                                split_clip(timeline, &right_clip_id, end);
                            }
                        }
                        remove_clips(timeline, [right_clip_id], prune);
                    }
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

fn prune_empty_tracks(timeline: &mut Timeline) {
    timeline.tracks.retain(|track| !track.clips.is_empty());
}

fn remove_clips<I>(timeline: &mut Timeline, ids: I, prune: bool)
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

fn linked_partner_ids(timeline: &Timeline, clip_id: &str) -> Vec<String> {
    let mut link_index: HashMap<String, Vec<String>> = HashMap::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            if let Some(group_id) = &clip.link_group_id {
                link_index
                    .entry(group_id.clone())
                    .or_default()
                    .push(clip.id.clone());
            }
        }
    }

    for members in link_index.values() {
        if members.iter().any(|member| member == clip_id) {
            return members
                .iter()
                .filter(|member| member.as_str() != clip_id)
                .cloned()
                .collect();
        }
    }

    Vec::new()
}
