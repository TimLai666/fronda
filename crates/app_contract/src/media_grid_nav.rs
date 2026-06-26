//! Media panel keyboard grid navigation (DRAG-011 through DRAG-015).
//!
//! Pure logic for keyboard-driven navigation of media panel items,
//! without any gpui dependency.

/// State for the media panel keyboard navigation.
#[derive(Debug, Clone)]
pub struct MediaGridNav {
    /// Ordered item IDs in the current view.
    pub ordered_ids: Vec<String>,
    /// Number of columns in the grid.
    pub column_count: usize,
    /// Currently selected item index (None = no selection).
    pub selected_index: Option<usize>,
    /// Whether the selected item is a folder.
    pub selected_is_folder: bool,
    /// IDs of selected assets (folders excluded).
    pub selected_asset_ids: Vec<String>,
    /// IDs of selected folders (assets excluded).
    pub selected_folder_ids: Vec<String>,
}

impl MediaGridNav {
    pub fn new(ordered_ids: Vec<String>, column_count: usize) -> Self {
        Self {
            ordered_ids,
            column_count,
            selected_index: None,
            selected_is_folder: false,
            selected_asset_ids: Vec::new(),
            selected_folder_ids: Vec::new(),
        }
    }

    /// DRAG-011: Navigation is driven by ordered item ids and column count.
    pub fn row_count(&self) -> usize {
        if self.ordered_ids.is_empty() || self.column_count == 0 {
            return 0;
        }
        (self.ordered_ids.len() + self.column_count - 1) / self.column_count
    }

    /// DRAG-012: Right/down starts at first item when no selection.
    pub fn move_right(&mut self) {
        let idx = match self.selected_index {
            Some(i) => {
                let next = i + 1;
                if next >= self.ordered_ids.len() {
                    return; // Clamp at end
                }
                next
            }
            None => 0,
        };
        self.select_index(idx);
    }

    /// DRAG-012: Left/up starts at last item when no selection.
    pub fn move_left(&mut self) {
        let idx = match self.selected_index {
            Some(i) => {
                if i == 0 {
                    return; // Clamp at start
                }
                i - 1
            }
            None => {
                if self.ordered_ids.is_empty() {
                    return;
                }
                self.ordered_ids.len() - 1
            }
        };
        self.select_index(idx);
    }

    /// Move down by one row.
    pub fn move_down(&mut self) {
        let idx = match self.selected_index {
            Some(i) => {
                let next = i + self.column_count;
                if next >= self.ordered_ids.len() {
                    return; // Clamp
                }
                next
            }
            None => {
                if self.ordered_ids.is_empty() {
                    return;
                }
                0
            }
        };
        self.select_index(idx);
    }

    /// Move up by one row.
    pub fn move_up(&mut self) {
        let idx = match self.selected_index {
            Some(i) => {
                if i < self.column_count {
                    return; // Already at top row, clamp
                }
                i - self.column_count
            }
            None => {
                if self.ordered_ids.is_empty() {
                    return;
                }
                // Start from bottom row center column
                let last_row_start = ((self.row_count() - 1) * self.column_count)
                    .min(self.ordered_ids.len().saturating_sub(1));
                last_row_start
            }
        };
        self.select_index(idx);
    }

    /// Select a given index.
    fn select_index(&mut self, index: usize) {
        if index >= self.ordered_ids.len() {
            return;
        }
        self.selected_index = Some(index);

        if self.selected_is_folder {
            self.selected_folder_ids.clear();
        } else {
            self.selected_asset_ids.clear();
        }
    }

    /// DRAG-014: Selecting a folder clears selected assets.
    pub fn select_folder(&mut self, index: usize) {
        if index >= self.ordered_ids.len() {
            return;
        }
        self.selected_index = Some(index);
        self.selected_is_folder = true;
        self.selected_asset_ids.clear();
        self.selected_folder_ids = vec![self.ordered_ids[index].clone()];
    }

    /// DRAG-015: Selecting an asset clears selected folders.
    pub fn select_asset(&mut self, index: usize) {
        if index >= self.ordered_ids.len() {
            return;
        }
        self.selected_index = Some(index);
        self.selected_is_folder = false;
        self.selected_folder_ids.clear();
        self.selected_asset_ids = vec![self.ordered_ids[index].clone()];
    }

    /// Current selection's item ID, if any.
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_index.map(|i| self.ordered_ids[i].as_str())
    }

    /// DRAG-013: Check whether moving in the given direction would wrap.
    pub fn would_wrap_right(&self) -> bool {
        match self.selected_index {
            Some(i) => i + 1 >= self.ordered_ids.len(),
            None => self.ordered_ids.is_empty(),
        }
    }

    pub fn would_wrap_left(&self) -> bool {
        match self.selected_index {
            Some(0) => true,
            Some(_) => false,
            None => self.ordered_ids.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nav(items: usize, cols: usize) -> MediaGridNav {
        let ids: Vec<String> = (0..items).map(|i| format!("item-{i}")).collect();
        MediaGridNav::new(ids, cols)
    }

    #[test]
    fn grid_nav_initial_state() {
        let nav = make_nav(10, 4);
        assert_eq!(nav.ordered_ids.len(), 10);
        assert_eq!(nav.column_count, 4);
        assert!(nav.selected_index.is_none());
    }

    #[test]
    fn grid_nav_row_count() {
        let nav = make_nav(10, 4);
        assert_eq!(nav.row_count(), 3); // 10 items in 4 cols = 3 rows
    }

    #[test]
    fn grid_nav_row_count_empty() {
        let nav = make_nav(0, 4);
        assert_eq!(nav.row_count(), 0);
    }

    #[test]
    fn grid_nav_move_right_no_selection_starts_at_first() {
        let mut nav = make_nav(10, 4);
        nav.move_right();
        assert_eq!(nav.selected_index, Some(0));
    }

    #[test]
    fn grid_nav_move_left_no_selection_starts_at_last() {
        let mut nav = make_nav(10, 4);
        nav.move_left();
        assert_eq!(nav.selected_index, Some(9));
    }

    #[test]
    fn grid_nav_move_right_clamps_at_end() {
        let mut nav = make_nav(3, 4);
        nav.select_asset(2);
        nav.move_right(); // would go to 3, but only 3 items (0,1,2)
        assert_eq!(nav.selected_index, Some(2));
    }

    #[test]
    fn grid_nav_move_left_clamps_at_start() {
        let mut nav = make_nav(3, 4);
        nav.select_asset(0);
        nav.move_left();
        assert_eq!(nav.selected_index, Some(0));
    }

    #[test]
    fn grid_nav_move_down() {
        let mut nav = make_nav(10, 4);
        nav.select_asset(0);
        nav.move_down();
        assert_eq!(nav.selected_index, Some(4));
    }

    #[test]
    fn grid_nav_move_down_clamps() {
        let mut nav = make_nav(6, 4);
        nav.select_asset(5);
        nav.move_down(); // 5 + 4 = 9 >= 6, clamp
        assert_eq!(nav.selected_index, Some(5));
    }

    #[test]
    fn grid_nav_move_up() {
        let mut nav = make_nav(10, 4);
        nav.select_asset(4);
        nav.move_up();
        assert_eq!(nav.selected_index, Some(0));
    }

    #[test]
    fn grid_nav_move_up_clamps_at_top_row() {
        let mut nav = make_nav(10, 4);
        nav.select_asset(2);
        nav.move_up(); // already in top row (index < col_count)
        assert_eq!(nav.selected_index, Some(2));
    }

    #[test]
    fn grid_nav_select_folder_clears_assets() {
        let mut nav = make_nav(10, 4);
        nav.select_asset(3);
        assert_eq!(nav.selected_asset_ids.len(), 1);
        nav.select_folder(0);
        assert!(nav.selected_asset_ids.is_empty());
        assert_eq!(nav.selected_folder_ids.len(), 1);
        assert!(nav.selected_is_folder);
    }

    #[test]
    fn grid_nav_select_asset_clears_folders() {
        let mut nav = make_nav(10, 4);
        nav.select_folder(0);
        assert_eq!(nav.selected_folder_ids.len(), 1);
        nav.select_asset(3);
        assert!(nav.selected_folder_ids.is_empty());
        assert_eq!(nav.selected_asset_ids.len(), 1);
        assert!(!nav.selected_is_folder);
    }

    #[test]
    fn grid_nav_would_wrap_right() {
        let mut nav = make_nav(3, 4);
        nav.select_asset(2);
        assert!(nav.would_wrap_right());
        nav.select_asset(0);
        assert!(!nav.would_wrap_right());
    }

    #[test]
    fn grid_nav_would_wrap_left() {
        let mut nav = make_nav(3, 4);
        nav.select_asset(0);
        assert!(nav.would_wrap_left());
        nav.select_asset(1);
        assert!(!nav.would_wrap_left());
    }
}
