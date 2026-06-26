//! Mention picker model — tabbed @mention picker state for the chat view.
//!
//! Covers CHAT-004 through CHAT-007.

/// Categories of mentionable items in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionCategory {
    Tools,
    Media,
    Context,
}

impl MentionCategory {
    /// All categories in display order.
    pub const ALL: &'static [MentionCategory] = &[
        MentionCategory::Tools,
        MentionCategory::Media,
        MentionCategory::Context,
    ];

    /// Human-readable label for the category.
    pub fn label(&self) -> &'static str {
        match self {
            MentionCategory::Tools => "Tools",
            MentionCategory::Media => "Media",
            MentionCategory::Context => "Context",
        }
    }
}

/// A single candidate in the mention picker.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionCandidate {
    pub id: String,
    pub label: String,
    pub category: MentionCategory,
    pub subtitle: Option<String>,
}

/// CHAT-007: Maximum number of candidates shown at once.
pub const MAX_MENTION_CANDIDATES: usize = 20;

/// State for the @mention picker.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionPickerState {
    /// Whether the picker is visible.
    pub visible: bool,
    /// Current query string (without the @ prefix).
    pub query: String,
    /// Active category tab.
    pub active_category: MentionCategory,
    /// Currently highlighted candidate index.
    pub highlighted_index: usize,
    /// The filtered candidate list.
    pub candidates: Vec<MentionCandidate>,
    /// All available candidates (unfiltered).
    pub all_candidates: Vec<MentionCandidate>,
}

impl MentionPickerState {
    /// Create a new picker with the given candidates.
    pub fn new(candidates: Vec<MentionCandidate>) -> Self {
        Self {
            visible: false,
            query: String::new(),
            active_category: MentionCategory::Tools,
            highlighted_index: 0,
            candidates: Self::filter_candidates(&candidates, "", MentionCategory::Tools),
            all_candidates: candidates,
        }
    }

    /// Show the picker and start a query.
    pub fn open(&mut self, query: &str) {
        self.visible = true;
        self.query = query.to_string();
        self.highlighted_index = 0;
        self.refresh_filter();
    }

    /// Hide the picker.
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.highlighted_index = 0;
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open("");
        }
    }

    /// Switch to the next category tab.
    pub fn next_category(&mut self) {
        let all = MentionCategory::ALL;
        let idx = all
            .iter()
            .position(|c| *c == self.active_category)
            .unwrap_or(0);
        self.active_category = all[(idx + 1) % all.len()];
        self.highlighted_index = 0;
        self.refresh_filter();
    }

    /// Switch to the previous category tab.
    pub fn previous_category(&mut self) {
        let all = MentionCategory::ALL;
        let idx = all
            .iter()
            .position(|c| *c == self.active_category)
            .unwrap_or(0);
        self.active_category = all[(idx + all.len() - 1) % all.len()];
        self.highlighted_index = 0;
        self.refresh_filter();
    }

    /// Move highlight down (or to next tab if at end of current).
    pub fn highlight_next(&mut self) {
        let count = self.visible_count();
        if count == 0 {
            return;
        }
        if self.highlighted_index + 1 < count {
            self.highlighted_index += 1;
        } else {
            // Wrap: go to next tab
            self.next_category();
        }
    }

    /// Move highlight up (or to previous tab if at start of current).
    pub fn highlight_previous(&mut self) {
        if self.highlighted_index > 0 {
            self.highlighted_index -= 1;
        } else {
            // Wrap: go to previous tab
            self.previous_category();
        }
    }

    /// The current highlighted candidate, if any.
    pub fn selected_candidate(&self) -> Option<&MentionCandidate> {
        if self.highlighted_index < self.candidates.len() {
            Some(&self.candidates[self.highlighted_index])
        } else {
            None
        }
    }

    /// Number of visible candidates in the current filter.
    pub fn visible_count(&self) -> usize {
        self.candidates.len()
    }

    /// Rebuild the filtered candidate list from the query and active category.
    fn refresh_filter(&mut self) {
        self.candidates =
            Self::filter_candidates(&self.all_candidates, &self.query, self.active_category);
        if self.highlighted_index >= self.candidates.len() && !self.candidates.is_empty() {
            self.highlighted_index = self.candidates.len() - 1;
        }
    }

    /// Filter candidates by query and category, then cap at MAX_MENTION_CANDIDATES.
    fn filter_candidates(
        all: &[MentionCandidate],
        query: &str,
        category: MentionCategory,
    ) -> Vec<MentionCandidate> {
        let q = query.to_lowercase();
        let mut filtered: Vec<MentionCandidate> = all
            .iter()
            .filter(|c| c.category == category)
            .filter(|c| {
                q.is_empty()
                    || c.label.to_lowercase().contains(&q)
                    || c.subtitle
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect();
        filtered.truncate(MAX_MENTION_CANDIDATES);
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidates() -> Vec<MentionCandidate> {
        vec![
            MentionCandidate {
                id: "tool-1".into(),
                label: "Add Clips".into(),
                category: MentionCategory::Tools,
                subtitle: None,
            },
            MentionCandidate {
                id: "tool-2".into(),
                label: "Split Clip".into(),
                category: MentionCategory::Tools,
                subtitle: None,
            },
            MentionCandidate {
                id: "media-1".into(),
                label: "beach.mp4".into(),
                category: MentionCategory::Media,
                subtitle: Some("00:01:23".into()),
            },
            MentionCandidate {
                id: "media-2".into(),
                label: "sunset.mp4".into(),
                category: MentionCategory::Media,
                subtitle: Some("00:00:45".into()),
            },
            MentionCandidate {
                id: "ctx-1".into(),
                label: "Current Selection".into(),
                category: MentionCategory::Context,
                subtitle: None,
            },
        ]
    }

    #[test]
    fn mention_picker_initial_state() {
        let picker = MentionPickerState::new(make_candidates());
        assert!(!picker.visible);
        assert!(picker.query.is_empty());
        assert_eq!(picker.active_category, MentionCategory::Tools);
        assert_eq!(picker.highlighted_index, 0);
    }

    #[test]
    fn mention_picker_open_filters_by_category() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.open("@be");
        assert!(picker.visible);
        // Only Tools category is shown initially
        assert!(picker
            .candidates
            .iter()
            .all(|c| c.category == MentionCategory::Tools));
    }

    #[test]
    fn mention_picker_close_resets_state() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.open("@be");
        assert!(picker.visible);
        picker.close();
        assert!(!picker.visible);
        assert!(picker.query.is_empty());
    }

    #[test]
    fn mention_picker_next_category() {
        let mut picker = MentionPickerState::new(make_candidates());
        assert_eq!(picker.active_category, MentionCategory::Tools);
        picker.next_category();
        assert_eq!(picker.active_category, MentionCategory::Media);
        picker.next_category();
        assert_eq!(picker.active_category, MentionCategory::Context);
        picker.next_category();
        assert_eq!(picker.active_category, MentionCategory::Tools);
    }

    #[test]
    fn mention_picker_previous_category() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.previous_category();
        assert_eq!(picker.active_category, MentionCategory::Context);
    }

    #[test]
    fn mention_picker_highlight_next_wraps_to_next_tab() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.open("");
        let tools_count = picker.visible_count();
        // Navigate to the end of tools tab
        for _ in 0..tools_count {
            picker.highlight_next();
        }
        // Should have switched to Media tab
        assert_eq!(picker.active_category, MentionCategory::Media);
        assert_eq!(picker.highlighted_index, 0);
    }

    #[test]
    fn mention_picker_highlight_previous_wraps_to_previous_tab() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.open("");
        picker.highlight_previous();
        assert_eq!(picker.active_category, MentionCategory::Context);
    }

    #[test]
    fn mention_picker_selected_candidate() {
        let mut picker = MentionPickerState::new(make_candidates());
        picker.open("");
        assert!(picker.selected_candidate().is_some());
        assert_eq!(picker.selected_candidate().unwrap().id, "tool-1");
    }

    #[test]
    fn mention_picker_candidate_cap_applied() {
        let many_candidates: Vec<MentionCandidate> = (0..30)
            .map(|i| MentionCandidate {
                id: format!("tool-{i}"),
                label: format!("Tool {i}"),
                category: MentionCategory::Tools,
                subtitle: None,
            })
            .collect();
        let picker = MentionPickerState::new(many_candidates);
        // Open shows tools tab
        assert!(picker.candidates.len() <= MAX_MENTION_CANDIDATES);
    }
}
