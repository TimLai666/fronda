use core_model::{ClipType, Timeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackInsertionError {
    IndexOutOfBounds,
}

/// Inserts a new track at the given index.
/// Visual tracks (Video/Image/Text) stay before audio tracks.
/// Returns the index the track was inserted at.
pub fn insert_track_at(
    timeline: &mut Timeline,
    index: usize,
    track_type: ClipType,
) -> Result<usize, TrackInsertionError> {
    if index > timeline.tracks.len() {
        return Err(TrackInsertionError::IndexOutOfBounds);
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let new_track = core_model::Track {
        id: new_id,
        r#type: track_type,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips: Vec::new(),
    };

    // Find the partition point: visual tracks (Video, Image, Text, Lottie) before audio tracks
    let audio_start = timeline
        .tracks
        .iter()
        .position(|t| matches!(t.r#type, ClipType::Audio))
        .unwrap_or(timeline.tracks.len());

    let actual_index = if matches!(track_type, ClipType::Audio) {
        // Audio tracks go after all visual tracks, at the end
        timeline.tracks.len()
    } else {
        // Visual tracks go at the given index, but not past audio_start
        index.min(audio_start)
    };

    timeline.tracks.insert(actual_index, new_track);
    Ok(actual_index)
}

/// Removes a track at the given index.
pub fn remove_track(timeline: &mut Timeline, index: usize) -> bool {
    if index >= timeline.tracks.len() {
        return false;
    }
    timeline.tracks.remove(index);
    true
}

/// Sorts clips on a track by their start_frame.
pub fn sort_clips_on_track(timeline: &mut Timeline, track_index: usize) -> bool {
    if track_index >= timeline.tracks.len() {
        return false;
    }
    let track = &mut timeline.tracks[track_index];
    track.clips.sort_by_key(|c| c.start_frame);
    true
}

/// Computes the display label for a track, matching Swift UI semantics:
/// - Video/Image/Lottie tracks: V1, V2, ... counting from bottom (lowest frame index)
/// - Audio tracks: A1, A2, ... counting from top
/// - Text tracks: T1, T2, ... counting from bottom
pub fn display_label_for_track(timeline: &Timeline, track_index: usize) -> String {
    if track_index >= timeline.tracks.len() {
        return String::new();
    }

    let track = &timeline.tracks[track_index];
    let track_type = track.r#type;

    // Collect all tracks of the same type in order
    let same_type_indices: Vec<usize> = timeline
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.r#type == track_type)
        .map(|(i, _)| i)
        .collect();

    let pos = same_type_indices
        .iter()
        .position(|i| *i == track_index)
        .unwrap_or(0);

    let count = pos + 1;

    match track_type {
        ClipType::Audio => format!("A{count}"),
        ClipType::Text => format!("T{count}"),
        ClipType::Video | ClipType::Image | ClipType::Lottie => format!("V{count}"),
    }
}

/// TRK-007: Toggle track mute. Returns the new state.
pub fn toggle_track_mute(timeline: &mut Timeline, track_index: usize) -> Option<bool> {
    let track = timeline.tracks.get_mut(track_index)?;
    track.muted = !track.muted;
    Some(track.muted)
}

/// TRK-007: Toggle track hidden. Returns the new state.
pub fn toggle_track_hidden(timeline: &mut Timeline, track_index: usize) -> Option<bool> {
    let track = timeline.tracks.get_mut(track_index)?;
    track.hidden = !track.hidden;
    Some(track.hidden)
}

/// TRK-007: Toggle track sync-lock. Returns the new state.
pub fn toggle_track_sync_lock(timeline: &mut Timeline, track_index: usize) -> Option<bool> {
    let track = timeline.tracks.get_mut(track_index)?;
    track.sync_locked = !track.sync_locked;
    Some(track.sync_locked)
}

/// TRK-008: Minimum and maximum track display heights.
pub const MIN_TRACK_HEIGHT: f64 = 30.0;
pub const MAX_TRACK_HEIGHT: f64 = 300.0;

/// TRK-008: Clamp a track display height to valid range.
pub fn clamp_track_height(height: f64) -> f64 {
    height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT)
}
