use core_model::{Clip, Timeline};

pub trait ClipMathExt {
    fn end_frame(&self) -> i64;
    fn source_frames_consumed(&self) -> i64;
    fn source_duration_frames(&self) -> i64;
    fn contains_frame(&self, frame: i64) -> bool;
}

impl ClipMathExt for Clip {
    fn end_frame(&self) -> i64 {
        self.start_frame + self.duration_frames
    }

    fn source_frames_consumed(&self) -> i64 {
        ((self.duration_frames as f64) * self.speed).round() as i64
    }

    fn source_duration_frames(&self) -> i64 {
        self.source_frames_consumed() + self.trim_start_frame + self.trim_end_frame
    }

    fn contains_frame(&self, frame: i64) -> bool {
        frame >= self.start_frame && frame < self.end_frame()
    }
}

pub trait TimelineMathExt {
    fn total_frames(&self) -> i64;
    fn clamp_seek_frame(&self, frame: i64) -> i64;
}

impl TimelineMathExt for Timeline {
    fn total_frames(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.end_frame())
            .max()
            .unwrap_or(0)
    }

    fn clamp_seek_frame(&self, frame: i64) -> i64 {
        frame.clamp(0, self.total_frames())
    }
}

pub fn is_valid_half_open_range(start_frame: i64, end_frame: i64) -> bool {
    end_frame > start_frame
}
