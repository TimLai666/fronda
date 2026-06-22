use crate::ClipMathExt;
use core_model::Clip;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverwriteAction {
    Remove {
        clip_id: String,
    },
    TrimEnd {
        clip_id: String,
        new_duration: i64,
    },
    TrimStart {
        clip_id: String,
        new_start_frame: i64,
        new_trim_start: i64,
        new_duration: i64,
    },
    Split {
        clip_id: String,
        left_duration: i64,
        right_id: String,
        right_start_frame: i64,
        right_trim_start: i64,
        right_duration: i64,
    },
}

pub fn compute_overwrite(
    clips: &[Clip],
    region_start: i64,
    region_end: i64,
) -> Vec<OverwriteAction> {
    if region_end <= region_start {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for clip in clips {
        let clip_start = clip.start_frame;
        let clip_end = clip.end_frame();

        if clip_end <= region_start || clip_start >= region_end {
            continue;
        }

        if clip_start >= region_start && clip_end <= region_end {
            actions.push(OverwriteAction::Remove {
                clip_id: clip.id.clone(),
            });
        } else if clip_start < region_start && clip_end > region_end {
            actions.push(OverwriteAction::Split {
                clip_id: clip.id.clone(),
                left_duration: region_start - clip_start,
                right_id: Uuid::new_v4().to_string(),
                right_start_frame: region_end,
                right_trim_start: clip.trim_start_frame
                    + (((region_end - clip_start) as f64) * clip.speed).round() as i64,
                right_duration: clip_end - region_end,
            });
        } else if clip_start < region_start {
            actions.push(OverwriteAction::TrimEnd {
                clip_id: clip.id.clone(),
                new_duration: region_start - clip_start,
            });
        } else {
            let trim_amount = region_end - clip_start;
            actions.push(OverwriteAction::TrimStart {
                clip_id: clip.id.clone(),
                new_start_frame: region_end,
                new_trim_start: clip.trim_start_frame
                    + ((trim_amount as f64) * clip.speed).round() as i64,
                new_duration: clip_end - region_end,
            });
        }
    }

    actions
}
