//! Library / Event / Project hierarchy model (Issue #156).
//!
//! Maps production structure:
//! ```text
//! Library  (one per client or production)
//! └── Event  (one per shoot, campaign, or content type)
//!     └── ProjectRef  (one per deliverable — format, version, cut)
//! ```

use serde::{Deserialize, Serialize};

/// A top-level production library (Issue #156).
///
/// Contains one or more events, each of which contains project references.
/// Stored as a separate JSON file alongside (or parent of) the `.palmier` bundles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub events: Vec<Event>,
}

impl Library {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn find_event(&self, id: &str) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    pub fn total_projects(&self) -> usize {
        self.events.iter().map(|e| e.projects.len()).sum()
    }
}

/// An event within a library — typically one shoot or campaign (Issue #156).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub projects: Vec<ProjectRef>,
}

impl Event {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            projects: Vec::new(),
        }
    }

    pub fn add_project(&mut self, project: ProjectRef) {
        self.projects.push(project);
    }
}

/// A reference to a `.palmier` project within an event (Issue #156).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    /// Absolute or library-relative path to the `.palmier` bundle.
    pub path: String,
    /// Optional version label (e.g. "Final cut", "Version A").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_label: Option<String>,
}

impl ProjectRef {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            path: path.into(),
            version_label: None,
        }
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_new_starts_empty() {
        let lib = Library::new("Client Rossi");
        assert_eq!(lib.name, "Client Rossi");
        assert!(lib.events.is_empty());
        assert_eq!(lib.total_projects(), 0);
    }

    #[test]
    fn library_add_event() {
        let mut lib = Library::new("Test");
        let event = Event::new("Brand video");
        let event_id = event.id.clone();
        lib.add_event(event);
        assert_eq!(lib.events.len(), 1);
        assert!(lib.find_event(&event_id).is_some());
        assert!(lib.find_event("nonexistent").is_none());
    }

    #[test]
    fn library_total_projects() {
        let mut lib = Library::new("Test");
        let mut event1 = Event::new("E1");
        event1.add_project(ProjectRef::new("P1", "/path/p1.palmier"));
        event1.add_project(ProjectRef::new("P2", "/path/p2.palmier"));
        let mut event2 = Event::new("E2");
        event2.add_project(ProjectRef::new("P3", "/path/p3.palmier"));
        lib.add_event(event1);
        lib.add_event(event2);
        assert_eq!(lib.total_projects(), 3);
    }

    #[test]
    fn event_new_starts_empty() {
        let event = Event::new("Ad Campaign");
        assert_eq!(event.name, "Ad Campaign");
        assert!(event.projects.is_empty());
    }

    #[test]
    fn project_ref_new() {
        let p = ProjectRef::new("Instagram Reel", "/lib/ad/reel.palmier");
        assert_eq!(p.name, "Instagram Reel");
        assert_eq!(p.path, "/lib/ad/reel.palmier");
        assert!(p.version_label.is_none());
    }

    #[test]
    fn project_ref_with_version_label() {
        let mut p = ProjectRef::new("Cut", "/path.palmier");
        p.version_label = Some("Final cut — 16:9 4K".into());
        assert_eq!(p.version_label.as_deref(), Some("Final cut — 16:9 4K"));
    }

    #[test]
    fn library_serde_roundtrip() {
        let mut lib = Library::new("Test Library");
        let mut event = Event::new("Test Event");
        event.add_project(ProjectRef::new("Proj", "/p.palmier"));
        lib.add_event(event);

        let json = serde_json::to_string(&lib).unwrap();
        let restored: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(lib.name, restored.name);
        assert_eq!(restored.events.len(), 1);
        assert_eq!(restored.events[0].projects.len(), 1);
    }
}
