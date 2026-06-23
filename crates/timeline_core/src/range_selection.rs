#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineRange {
    pub start_frame: i64,
    pub end_frame: i64,
}

impl TimelineRange {
    pub fn normalized(&self) -> Self {
        if self.start_frame <= self.end_frame {
            *self
        } else {
            Self {
                start_frame: self.end_frame,
                end_frame: self.start_frame,
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.end_frame > self.start_frame
    }

    pub fn contains(&self, frame: i64) -> bool {
        frame >= self.start_frame && frame < self.end_frame
    }
}

/// RNG-002: The edge being dragged when editing an existing range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeEdge {
    Start,
    End,
}

/// RNG-002: Create or extend a timeline range via shift-drag.
pub fn shift_drag_range(
    anchor_frame: i64,
    current_frame: i64,
    existing_range: Option<TimelineRange>,
) -> TimelineRange {
    match existing_range {
        Some(range) => {
            let dist_to_start = (current_frame - range.start_frame).abs();
            let dist_to_end = (current_frame - range.end_frame).abs();
            if dist_to_start <= dist_to_end {
                TimelineRange {
                    start_frame: current_frame.min(range.end_frame - 1),
                    end_frame: range.end_frame,
                }
            } else {
                TimelineRange {
                    start_frame: range.start_frame,
                    end_frame: current_frame.max(range.start_frame + 1),
                }
            }
        }
        None => {
            let start = anchor_frame.min(current_frame);
            let end = anchor_frame.max(current_frame);
            TimelineRange {
                start_frame: start,
                end_frame: end.max(start + 1),
            }
        }
    }
    .normalized()
}

/// RNG-003: Drag a specific edge of an existing range to a new frame.
pub fn drag_range_edge(existing: TimelineRange, edge: RangeEdge, new_frame: i64) -> TimelineRange {
    match edge {
        RangeEdge::Start => {
            let end = existing.end_frame;
            TimelineRange {
                start_frame: new_frame.min(end - 1),
                end_frame: end,
            }
        }
        RangeEdge::End => {
            let start = existing.start_frame;
            TimelineRange {
                start_frame: start,
                end_frame: new_frame.max(start + 1),
            }
        }
    }
}

/// RNG-004: A gap is the empty interval between the previous clip's end
/// and the next clip's start on one track.
pub fn find_gap_at_frame(track: &core_model::Track, frame: i64) -> Option<TimelineRange> {
    if track.clips.is_empty() {
        return None;
    }

    let mut sorted = track.clips.clone();
    sorted.sort_by_key(|c| c.start_frame);

    for i in 0..sorted.len() - 1 {
        let gap_start = sorted[i].start_frame + sorted[i].duration_frames;
        let gap_end = sorted[i + 1].start_frame;
        if gap_start < gap_end && frame >= gap_start && frame < gap_end {
            return Some(TimelineRange {
                start_frame: gap_start,
                end_frame: gap_end,
            });
        }
    }
    None
}

/// Find all gaps on a track.
pub fn find_all_gaps(track: &core_model::Track) -> Vec<TimelineRange> {
    if track.clips.len() < 2 {
        return Vec::new();
    }

    let mut sorted = track.clips.clone();
    sorted.sort_by_key(|c| c.start_frame);

    let mut gaps = Vec::new();
    for i in 0..sorted.len() - 1 {
        let gap_start = sorted[i].start_frame + sorted[i].duration_frames;
        let gap_end = sorted[i + 1].start_frame;
        if gap_start < gap_end {
            gaps.push(TimelineRange {
                start_frame: gap_start,
                end_frame: gap_end,
            });
        }
    }
    gaps
}
