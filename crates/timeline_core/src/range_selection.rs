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
