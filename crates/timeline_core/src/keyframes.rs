use core_model::{AnimPair, Clip, Crop, Interpolation, Keyframe, KeyframeTrack};

pub trait KeyframeValue: Clone + PartialEq {
    fn interpolate(a: &Self, b: &Self, t: f64) -> Self;
}

impl KeyframeValue for f64 {
    fn interpolate(a: &Self, b: &Self, t: f64) -> Self {
        a + (b - a) * t
    }
}

impl KeyframeValue for AnimPair {
    fn interpolate(a: &Self, b: &Self, t: f64) -> Self {
        Self {
            a: f64::interpolate(&a.a, &b.a, t),
            b: f64::interpolate(&a.b, &b.b, t),
        }
    }
}

impl KeyframeValue for Crop {
    fn interpolate(a: &Self, b: &Self, t: f64) -> Self {
        Self {
            left: f64::interpolate(&a.left, &b.left, t),
            top: f64::interpolate(&a.top, &b.top, t),
            right: f64::interpolate(&a.right, &b.right, t),
            bottom: f64::interpolate(&a.bottom, &b.bottom, t),
        }
    }
}

pub fn clamp_clip_keyframes_to_duration(clip: &mut Clip) {
    clamp_keyframe_track(&mut clip.opacity_track, clip.duration_frames);
    clamp_keyframe_track(&mut clip.position_track, clip.duration_frames);
    clamp_keyframe_track(&mut clip.scale_track, clip.duration_frames);
    clamp_keyframe_track(&mut clip.rotation_track, clip.duration_frames);
    clamp_keyframe_track(&mut clip.crop_track, clip.duration_frames);
    clamp_keyframe_track(&mut clip.volume_track, clip.duration_frames);
}

pub fn clamp_clip_fades_to_duration(clip: &mut Clip) {
    clip.fade_in_frames = clip.fade_in_frames.clamp(0, clip.duration_frames);
    clip.fade_out_frames = clip
        .fade_out_frames
        .clamp(0, clip.duration_frames - clip.fade_in_frames);
}

pub fn set_clip_duration(clip: &mut Clip, new_duration: i64) {
    clip.duration_frames = new_duration;
    clamp_clip_keyframes_to_duration(clip);
    clamp_clip_fades_to_duration(clip);
}

pub fn split_all_clip_keyframe_tracks(clip: &Clip, split_offset: i64) -> (Clip, Clip) {
    let mut left = clip.clone();
    let mut right = clip.clone();

    let (left_opacity, right_opacity) =
        split_keyframe_track(clip.opacity_track.clone(), split_offset, clip.opacity);
    let (left_volume, right_volume) =
        split_keyframe_track(clip.volume_track.clone(), split_offset, clip.volume);
    let (left_position, right_position) = split_keyframe_track(
        clip.position_track.clone(),
        split_offset,
        AnimPair { a: 0.0, b: 0.0 },
    );
    let (left_scale, right_scale) = split_keyframe_track(
        clip.scale_track.clone(),
        split_offset,
        AnimPair { a: 1.0, b: 1.0 },
    );
    let (left_rotation, right_rotation) =
        split_keyframe_track(clip.rotation_track.clone(), split_offset, 0.0);
    let (left_crop, right_crop) =
        split_keyframe_track(clip.crop_track.clone(), split_offset, clip.crop);

    left.opacity_track = left_opacity;
    right.opacity_track = right_opacity;
    left.volume_track = left_volume;
    right.volume_track = right_volume;
    left.position_track = left_position;
    right.position_track = right_position;
    left.scale_track = left_scale;
    right.scale_track = right_scale;
    left.rotation_track = left_rotation;
    right.rotation_track = right_rotation;
    left.crop_track = left_crop;
    right.crop_track = right_crop;

    (left, right)
}

fn clamp_keyframe_track<V>(track: &mut Option<KeyframeTrack<V>>, duration_frames: i64)
where
    V: Clone + PartialEq,
{
    let Some(existing) = track.take() else {
        return;
    };

    let mut normalized = KeyframeTrack::default();
    for keyframe in existing
        .keyframes
        .into_iter()
        .filter(|keyframe| keyframe.frame >= 0 && keyframe.frame <= duration_frames)
    {
        upsert_keyframe(&mut normalized, keyframe);
    }

    *track = if normalized.keyframes.is_empty() {
        None
    } else {
        Some(normalized)
    };
}

pub fn split_keyframe_track<V>(
    track: Option<KeyframeTrack<V>>,
    split_offset: i64,
    fallback: V,
) -> (Option<KeyframeTrack<V>>, Option<KeyframeTrack<V>>)
where
    V: KeyframeValue,
{
    let Some(track) = track else {
        return (None, None);
    };
    if track.keyframes.is_empty() {
        return (Some(track.clone()), Some(track));
    }

    let boundary = sample_keyframe_track(&track, split_offset, fallback);

    let mut left_keyframes: Vec<Keyframe<V>> = track
        .keyframes
        .iter()
        .filter(|keyframe| keyframe.frame <= split_offset)
        .cloned()
        .collect();
    if left_keyframes.last().map(|keyframe| keyframe.frame) != Some(split_offset) {
        left_keyframes.push(Keyframe {
            frame: split_offset,
            value: boundary.clone(),
            interpolation_out: Interpolation::Smooth,
        });
    }

    let mut right_keyframes: Vec<Keyframe<V>> = track
        .keyframes
        .iter()
        .filter(|keyframe| keyframe.frame >= split_offset)
        .map(|keyframe| Keyframe {
            frame: keyframe.frame - split_offset,
            value: keyframe.value.clone(),
            interpolation_out: keyframe.interpolation_out,
        })
        .collect();
    if right_keyframes.first().map(|keyframe| keyframe.frame) != Some(0) {
        right_keyframes.insert(
            0,
            Keyframe {
                frame: 0,
                value: boundary,
                interpolation_out: Interpolation::Smooth,
            },
        );
    }

    (
        if left_keyframes.is_empty() {
            None
        } else {
            Some(KeyframeTrack {
                keyframes: left_keyframes,
            })
        },
        if right_keyframes.is_empty() {
            None
        } else {
            Some(KeyframeTrack {
                keyframes: right_keyframes,
            })
        },
    )
}

pub fn sample_keyframe_track<V>(track: &KeyframeTrack<V>, frame: i64, fallback: V) -> V
where
    V: KeyframeValue,
{
    if track.keyframes.is_empty() {
        return fallback;
    }
    if track.keyframes.len() == 1 {
        return track.keyframes[0].value.clone();
    }
    if frame <= track.keyframes[0].frame {
        return track.keyframes[0].value.clone();
    }
    let last = track.keyframes.last().expect("checked non-empty");
    if frame >= last.frame {
        return last.value.clone();
    }

    let Some(b_index) = track
        .keyframes
        .iter()
        .position(|keyframe| keyframe.frame > frame)
    else {
        return last.value.clone();
    };
    let a = &track.keyframes[b_index - 1];
    let b = &track.keyframes[b_index];
    let raw = (frame - a.frame) as f64 / (b.frame - a.frame) as f64;

    match a.interpolation_out {
        Interpolation::Hold => a.value.clone(),
        Interpolation::Linear => V::interpolate(&a.value, &b.value, raw),
        Interpolation::Smooth => V::interpolate(&a.value, &b.value, smoothstep(raw)),
    }
}

fn upsert_keyframe<V>(track: &mut KeyframeTrack<V>, keyframe: Keyframe<V>)
where
    V: Clone + PartialEq,
{
    if let Some(index) = track
        .keyframes
        .iter()
        .position(|existing| existing.frame == keyframe.frame)
    {
        track.keyframes[index] = keyframe;
        return;
    }

    let insert_at = track
        .keyframes
        .iter()
        .position(|existing| existing.frame > keyframe.frame)
        .unwrap_or(track.keyframes.len());
    track.keyframes.insert(insert_at, keyframe);
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}
