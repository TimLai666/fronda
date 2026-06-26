//! Data types for the Home view — pure logic, no gpui dependency.
//!
//! Covers APP-002 (reopening shows Home), PRJ-014 (close → Home).

/// Constants for the home view layout.
pub struct HomeLayout;
impl HomeLayout {
    pub const HEADING_FONT_SIZE: f64 = 28.0;
    pub const SUBTITLE_FONT_SIZE: f64 = 14.0;
    pub const CARD_WIDTH: f64 = 150.0;
    pub const CARD_HEIGHT: f64 = 120.0;
    pub const CARD_GAP: f64 = 12.0;
    pub const SECTION_TOP: f64 = 40.0;
    pub const HEADING_TOP: f64 = 48.0;
}

/// A recent project entry displayed on the Home view.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectCard {
    pub name: String,
    pub path: String,
    pub last_opened_label: String,
}

/// Navigation action from the Home view.
#[derive(Debug, Clone, PartialEq)]
pub enum HomeAction {
    NewProject,
    OpenProject,
    OpenProjectAt(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_layout_constants() {
        assert!((HomeLayout::CARD_WIDTH - 150.0).abs() < 1e-10);
        assert!((HomeLayout::CARD_HEIGHT - 120.0).abs() < 1e-10);
        assert!((HomeLayout::CARD_GAP - 12.0).abs() < 1e-10);
        assert!((HomeLayout::HEADING_TOP - 48.0).abs() < 1e-10);
        assert!((HomeLayout::SECTION_TOP - 40.0).abs() < 1e-10);
    }

    #[test]
    fn project_card_struct() {
        let card = ProjectCard {
            name: "Test Project".into(),
            path: "/tmp/test.palmier".into(),
            last_opened_label: "Today".into(),
        };
        assert_eq!(card.name, "Test Project");
        assert_eq!(card.path, "/tmp/test.palmier");
        assert_eq!(card.last_opened_label, "Today");
    }

    #[test]
    fn home_action_variants() {
        match HomeAction::NewProject {
            HomeAction::NewProject => {}
            _ => panic!("expected NewProject"),
        }
        match HomeAction::OpenProject {
            HomeAction::OpenProject => {}
            _ => panic!("expected OpenProject"),
        }
        match HomeAction::OpenProjectAt(3) {
            HomeAction::OpenProjectAt(idx) => assert_eq!(idx, 3),
            _ => panic!("expected OpenProjectAt"),
        }
    }
}
