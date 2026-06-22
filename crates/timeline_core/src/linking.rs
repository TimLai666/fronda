use core_model::Timeline;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedPartnerMove {
    pub clip_id: String,
    pub track_index: usize,
    pub to_frame: i64,
}

pub type LinkIndex = BTreeMap<String, BTreeSet<String>>;

pub fn build_link_index(timeline: &Timeline) -> LinkIndex {
    let mut index = LinkIndex::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            if let Some(group_id) = &clip.link_group_id {
                index
                    .entry(group_id.clone())
                    .or_default()
                    .insert(clip.id.clone());
            }
        }
    }
    index
}

pub fn expand_to_link_group(timeline: &Timeline, ids: &BTreeSet<String>) -> BTreeSet<String> {
    let index = build_link_index(timeline);
    let mut clip_to_group: BTreeMap<String, String> = BTreeMap::new();
    for (group_id, members) in &index {
        for member in members {
            clip_to_group.insert(member.clone(), group_id.clone());
        }
    }

    let mut groups = BTreeSet::new();
    for id in ids {
        if let Some(group_id) = clip_to_group.get(id) {
            groups.insert(group_id.clone());
        }
    }

    if groups.is_empty() {
        return ids.clone();
    }

    let mut expanded = ids.clone();
    for group_id in groups {
        if let Some(members) = index.get(&group_id) {
            expanded.extend(members.iter().cloned());
        }
    }
    expanded
}

pub fn linked_partner_ids(timeline: &Timeline, clip_id: &str) -> Vec<String> {
    build_link_index(timeline)
        .into_values()
        .find(|members| members.contains(clip_id))
        .map(|members| {
            members
                .into_iter()
                .filter(|member| member != clip_id)
                .collect()
        })
        .unwrap_or_default()
}

pub fn partner_moves_for_move_of(
    timeline: &Timeline,
    clip_id: &str,
    to_frame: i64,
) -> Vec<LinkedPartnerMove> {
    let Some((lead_track_index, lead_clip_index)) = locate_clip(timeline, clip_id) else {
        return Vec::new();
    };
    let current_frame = timeline.tracks[lead_track_index].clips[lead_clip_index].start_frame;
    let delta = to_frame - current_frame;
    if delta == 0 {
        return Vec::new();
    }

    linked_partner_ids(timeline, clip_id)
        .into_iter()
        .filter_map(|partner_id| {
            let (track_index, clip_index) = locate_clip(timeline, &partner_id)?;
            let partner = &timeline.tracks[track_index].clips[clip_index];
            Some(LinkedPartnerMove {
                clip_id: partner_id,
                track_index,
                to_frame: (partner.start_frame + delta).max(0),
            })
        })
        .collect()
}

pub fn link_group_offsets(timeline: &Timeline) -> BTreeMap<String, i64> {
    let mut by_group: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            let Some(group_id) = &clip.link_group_id else {
                continue;
            };
            by_group
                .entry(group_id.clone())
                .or_default()
                .push((clip.id.clone(), clip.start_frame - clip.trim_start_frame));
        }
    }

    let mut offsets = BTreeMap::new();
    for entries in by_group.into_values().filter(|entries| entries.len() > 1) {
        let reference = entries
            .iter()
            .map(|(_, start)| *start)
            .min()
            .expect("checked non-empty group");
        for (clip_id, start) in entries {
            let delta = start - reference;
            if delta != 0 {
                offsets.insert(clip_id, delta);
            }
        }
    }
    offsets
}

pub fn link_clips(timeline: &mut Timeline, ids: &BTreeSet<String>) -> Option<String> {
    if ids.len() < 2 {
        return None;
    }

    let new_group = Uuid::new_v4().to_string();
    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            if ids.contains(&clip.id) {
                clip.link_group_id = Some(new_group.clone());
            }
        }
    }
    Some(new_group)
}

pub fn unlink_clips(timeline: &mut Timeline, ids: &BTreeSet<String>) -> BTreeSet<String> {
    let expanded = expand_to_link_group(timeline, ids);
    let mut cleared = BTreeSet::new();

    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            if expanded.contains(&clip.id) && clip.link_group_id.is_some() {
                clip.link_group_id = None;
                cleared.insert(clip.id.clone());
            }
        }
    }

    cleared
}

fn locate_clip(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .clips
                .iter()
                .position(|clip| clip.id == clip_id)
                .map(|clip_index| (track_index, clip_index))
        })
}
