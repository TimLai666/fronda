//! Local SKILL.md skill store (Swift: SkillStore). Upstream palmier-pro #199 + #319.
//!
//! Scans `~/.palmier/skills/<id>/SKILL.md`, parses each skill's YAML-ish
//! frontmatter (name/description), and builds a sorted skill list plus the
//! prompt index injected into the in-app agent. [`SkillStore`] adds the #319
//! editing surface: validated atomic `save`, `delete`, and `new_skill`, all
//! path-confined to the skills directory. Pure std — no gpui.
//!
//! Still to port from #199/#319: the GitHub `SkillCatalog` (community
//! install/refresh) and the external-agent copy menu (platform adapter).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A single installed skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Folder name under the skills directory.
    pub id: String,
    pub name: String,
    pub description: String,
    /// Path to the skill's `SKILL.md`.
    pub path: PathBuf,
}

/// Default local skills directory: `~/.palmier/skills`.
///
/// The `.palmier` identifier is a compatibility identifier (see the identifier
/// migration plan); it is intentionally not renamed here.
pub fn default_skills_dir() -> PathBuf {
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".palmier").join("skills")
}

/// Parse SKILL.md frontmatter. Mirrors Swift `SkillFrontmatter.parse`: a leading
/// `---` line opens the block, `key: value` pairs run until the closing `---`,
/// and a value wrapped in double quotes has them stripped. Returns the fields
/// and the remaining body (trimmed). With no frontmatter, fields are empty and
/// the body is the whole text.
pub fn parse_frontmatter(text: &str) -> (BTreeMap<String, String>, String) {
    let mut fields = BTreeMap::new();
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return (fields, text.to_string());
    }
    let mut i = 1;
    while i < lines.len() && lines[i].trim() != "---" {
        if let Some(colon) = lines[i].find(':') {
            let key = lines[i][..colon].trim().to_string();
            let mut value = lines[i][colon + 1..].trim().to_string();
            if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                value = value[1..value.len() - 1].to_string();
            }
            if !key.is_empty() {
                fields.insert(key, value);
            }
        }
        i += 1;
    }
    let body = if i + 1 < lines.len() {
        lines[i + 1..].join("\n").trim().to_string()
    } else {
        String::new()
    };
    (fields, body)
}

/// Mirrors Swift `SkillFrontmatter.requiredFields` (#319): both `name` and
/// `description` must be non-blank after trimming. Returns
/// `(name, description, body)`, or `None` for an unrecognized skill.
pub fn required_fields(text: &str) -> Option<(String, String, String)> {
    let (fields, body) = parse_frontmatter(text);
    let name = fields.get("name").map(|s| s.trim()).unwrap_or_default();
    let description = fields
        .get("description")
        .map(|s| s.trim())
        .unwrap_or_default();
    if name.is_empty() || description.is_empty() {
        return None;
    }
    Some((name.to_string(), description.to_string(), body))
}

/// Build a `Skill` from a folder id and its SKILL.md text. Returns `None` when
/// the frontmatter lacks a non-blank `name` or `description` (#319).
fn parse_skill(id: &str, path: &Path, text: &str) -> Option<Skill> {
    let (name, description, _body) = required_fields(text)?;
    Some(Skill {
        id: id.to_string(),
        name,
        description,
        path: path.to_path_buf(),
    })
}

/// Scan a skills directory, returning valid skills sorted by id. A missing
/// directory or an unreadable/invalid `SKILL.md` yields no error — that skill is
/// simply skipped.
pub fn load_skills(dir: &Path) -> Vec<Skill> {
    let mut found: Vec<Skill> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        let Some(id) = sub.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        let md = sub.join("SKILL.md");
        let Ok(text) = std::fs::read_to_string(&md) else {
            continue;
        };
        let Some(skill) = parse_skill(&id, &md, &text) else {
            eprintln!("Skill skipped {id}: frontmatter needs a non-empty name and description");
            continue;
        };
        found.push(skill);
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// Scan a skills directory into `agent_contract::AgentSkill`s (with bodies) for
/// the in-app agent's `read_skill` tool. Like [`load_skills`] but also carries
/// each skill's body (the SKILL.md content after its frontmatter).
pub fn load_agent_skills(dir: &Path) -> Vec<agent_contract::AgentSkill> {
    let mut out: Vec<agent_contract::AgentSkill> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let sub = entry.path();
        if !sub.is_dir() {
            continue;
        }
        let Some(id) = sub.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        let md = sub.join("SKILL.md");
        let Ok(text) = std::fs::read_to_string(&md) else {
            continue;
        };
        let Some((name, description, body)) = required_fields(&text) else {
            eprintln!("Skill skipped {id}: frontmatter needs a non-empty name and description");
            continue;
        };
        out.push(agent_contract::AgentSkill {
            id,
            name,
            description,
            body,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Load the local skills and hand them to the in-app agent's executor
/// (upstream #199). Call at boot / after skills change.
pub fn load_skills_into_executor(exec: &mut agent_contract::ToolExecutor) {
    exec.set_skills(load_agent_skills(&default_skills_dir()));
}

/// The always-on skill index injected into the in-app agent's system prompt.
/// Mirrors Swift `SkillStore.promptIndex`. Empty when there are no skills.
pub fn prompt_index(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let lines = skills
        .iter()
        .map(|s| format!("- {}: {}", s.id, s.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\n# Skills\nPlaybooks for specific tasks. Before a task that matches one, \
call read_skill(id) to load its full procedure, then follow it.\n{lines}"
    )
}

// ── SkillStore (#319 editing surface) ────────────────────────────────────────

/// Save-validation failure message — mirrors the Swift #319 alert copy.
pub const SAVE_VALIDATION_ERROR: &str =
    "Add nonempty name and description fields to the skill frontmatter.";

/// New-skill SKILL.md template — mirrors Swift `SkillStore.newSkill`.
pub const SKILL_TEMPLATE: &str = "---\n\
name: New skill\n\
description: Describe in one line when the assistant should use this skill.\n\
---\n\
\n\
## Workflow\n\
1. First step.\n\
2. Second step.";

/// Mirrors Swift `SkillStore.isValidSkillId`: a single safe path component.
pub fn is_valid_skill_id(id: &str) -> bool {
    !id.is_empty() && id != "." && id != ".." && !id.contains('/') && !id.contains('\\')
}

/// Rebuild a SKILL.md from edited name/description/body, preserving any other
/// frontmatter fields (and their order) from `original`. Without an original
/// frontmatter block a fresh one is created.
pub fn update_skill_md(original: &str, name: &str, description: &str, body: &str) -> String {
    let mut front: Vec<String> = Vec::new();
    let mut saw_name = false;
    let mut saw_description = false;
    let lines: Vec<&str> = original.split('\n').collect();
    if lines.first().map(|l| l.trim()) == Some("---") {
        let mut i = 1;
        while i < lines.len() && lines[i].trim() != "---" {
            let line = lines[i];
            let key = line.split(':').next().map(str::trim).unwrap_or_default();
            match key {
                "name" if !saw_name => {
                    front.push(format!("name: {name}"));
                    saw_name = true;
                }
                "description" if !saw_description => {
                    front.push(format!("description: {description}"));
                    saw_description = true;
                }
                _ => front.push(line.to_string()),
            }
            i += 1;
        }
    }
    if !saw_description {
        front.insert(0, format!("description: {description}"));
    }
    if !saw_name {
        front.insert(0, format!("name: {name}"));
    }
    format!("---\n{}\n---\n\n{}", front.join("\n"), body.trim_end())
}

/// Mutable skill store rooted at one skills directory (Swift `SkillStore`).
///
/// Every write path re-validates the id (single path component, resolved
/// location stays inside the root) and `save` refuses content whose
/// frontmatter fails [`required_fields`] before touching disk.
#[derive(Debug)]
pub struct SkillStore {
    dir: PathBuf,
    skills: Vec<Skill>,
}

impl SkillStore {
    pub fn new(dir: PathBuf) -> Self {
        let mut store = Self {
            dir,
            skills: Vec::new(),
        };
        store.reload();
        store
    }

    /// Store over the default `~/.palmier/skills` directory.
    pub fn default_location() -> Self {
        Self::new(default_skills_dir())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    pub fn skill(&self, id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.id == id)
    }

    pub fn reload(&mut self) {
        self.skills = load_skills(&self.dir);
    }

    /// Resolve `<root>/<id>` only when `id` is a safe single component and the
    /// joined path stays directly under the root (Swift `skillDirectory(for:)`).
    fn skill_dir(&self, id: &str) -> Result<PathBuf, String> {
        if !is_valid_skill_id(id) {
            return Err(format!("Invalid skill id \u{201C}{id}\u{201D}."));
        }
        let dir = self.dir.join(id);
        if dir.parent() != Some(self.dir.as_path()) {
            return Err(format!("Invalid skill id \u{201C}{id}\u{201D}."));
        }
        Ok(dir)
    }

    /// Raw SKILL.md contents for the editor (path-confined read).
    pub fn raw(&self, id: &str) -> Option<String> {
        let dir = self.skill_dir(id).ok()?;
        std::fs::read_to_string(dir.join("SKILL.md")).ok()
    }

    /// Validate then atomically write a skill's SKILL.md (temp + rename).
    /// Nothing is written when validation fails.
    pub fn save(&mut self, id: &str, content: &str) -> Result<(), String> {
        let dir = self.skill_dir(id)?;
        if required_fields(content).is_none() {
            return Err(SAVE_VALIDATION_ERROR.to_string());
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("Unable to save skill: {e}"))?;
        let target = dir.join("SKILL.md");
        let temp = dir.join(".SKILL.md.tmp");
        std::fs::write(&temp, content).map_err(|e| format!("Unable to save skill: {e}"))?;
        if let Err(e) = std::fs::rename(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("Unable to save skill: {e}"));
        }
        self.reload();
        Ok(())
    }

    /// Delete a skill's folder (Swift `SkillStore.delete`).
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let dir = self.skill_dir(id)?;
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Unable to delete skill: {e}"))?;
        self.reload();
        Ok(())
    }

    /// Create a fresh skill folder from the template, returning its id
    /// ("new-skill", then "new-skill-2", … — Swift `SkillStore.newSkill`).
    pub fn new_skill(&mut self) -> Result<String, String> {
        let mut id = String::from("new-skill");
        let mut n = 2;
        while self.dir.join(&id).exists() {
            id = format!("new-skill-{n}");
            n += 1;
        }
        let dir = self.dir.join(&id);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Unable to create skill: {e}"))?;
        std::fs::write(dir.join("SKILL.md"), SKILL_TEMPLATE)
            .map_err(|e| format!("Unable to create skill: {e}"))?;
        self.reload();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("fronda-skill-store-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_skill(root: &Path, id: &str, contents: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), contents).unwrap();
    }

    #[test]
    fn parse_frontmatter_extracts_fields_and_body() {
        let text = "---\nname: My Skill\ndescription: \"Does a thing\"\n---\n\nBody here.";
        let (fields, body) = parse_frontmatter(text);
        assert_eq!(fields.get("name").map(String::as_str), Some("My Skill"));
        assert_eq!(
            fields.get("description").map(String::as_str),
            Some("Does a thing"),
            "surrounding quotes stripped"
        );
        assert_eq!(body, "Body here.");
    }

    #[test]
    fn parse_frontmatter_no_block_returns_whole_text() {
        let text = "no frontmatter here";
        let (fields, body) = parse_frontmatter(text);
        assert!(fields.is_empty());
        assert_eq!(body, text);
    }

    // Swift SkillFrontmatterTests.requiresNonemptyNameAndDescription (#319).
    #[test]
    fn required_fields_requires_nonempty_name_and_description() {
        let valid = "---\nname: Editing\ndescription: Edit clips.\n---\n\nInstructions";
        let missing_name = "---\ndescription: Edit clips.\n---\n\nInstructions";
        let empty_description = "---\nname: Editing\ndescription:   \n---\n\nInstructions";
        let missing_description = "---\nname: Editing\n---\n\nInstructions";

        assert_eq!(
            required_fields(valid),
            Some((
                "Editing".to_string(),
                "Edit clips.".to_string(),
                "Instructions".to_string()
            ))
        );
        assert_eq!(required_fields(missing_name), None);
        assert_eq!(required_fields(empty_description), None);
        assert_eq!(required_fields(missing_description), None);
    }

    #[test]
    fn load_skills_reads_valid_and_skips_invalid() {
        let root = temp_dir("load");
        write_skill(
            &root,
            "b-skill",
            "---\nname: Bravo\ndescription: second\n---\nbody",
        );
        write_skill(
            &root,
            "a-skill",
            "---\nname: Alpha\ndescription: first\n---\nbody",
        );
        // No name → skipped.
        write_skill(&root, "nameless", "---\ndescription: no name\n---\nbody");
        // Blank description → skipped (#319).
        write_skill(
            &root,
            "descriptionless",
            "---\nname: NoDesc\ndescription:   \n---\nbody",
        );
        // No SKILL.md at all → skipped (folder created empty).
        std::fs::create_dir_all(root.join("empty-folder")).unwrap();

        let skills = load_skills(&root);
        assert_eq!(skills.len(), 2, "only the two fully-described skills load");
        // Sorted by id.
        assert_eq!(skills[0].id, "a-skill");
        assert_eq!(skills[0].name, "Alpha");
        assert_eq!(skills[1].id, "b-skill");
    }

    #[test]
    fn load_agent_skills_carries_body_after_frontmatter() {
        let root = temp_dir("agent");
        write_skill(
            &root,
            "captions",
            "---\nname: Captions\ndescription: burn in\n---\n\n1. Transcribe\n2. Style",
        );
        write_skill(&root, "nameless", "---\ndescription: no name\n---\nbody");
        write_skill(&root, "descriptionless", "---\nname: NoDesc\n---\nbody");

        let skills = load_agent_skills(&root);
        assert_eq!(
            skills.len(),
            1,
            "nameless and descriptionless skills skipped"
        );
        assert_eq!(skills[0].id, "captions");
        assert_eq!(skills[0].description, "burn in");
        assert_eq!(skills[0].body, "1. Transcribe\n2. Style");
    }

    #[test]
    fn load_skills_missing_dir_is_empty() {
        let missing = std::env::temp_dir().join("fronda-skill-store-tests/does-not-exist-xyz");
        assert!(load_skills(&missing).is_empty());
    }

    // ── SkillStore (#319) ────────────────────────────────────────────────────

    const VALID_MD: &str = "---\nname: Editing\ndescription: Edit clips.\n---\n\nSteps here.";

    #[test]
    fn store_save_round_trip_updates_skill_and_raw() {
        let root = temp_dir("store-roundtrip");
        write_skill(&root, "editing", VALID_MD);
        let mut store = SkillStore::new(root);
        assert_eq!(store.skills().len(), 1);

        let updated = "---\nname: Editing v2\ndescription: Edit clips faster.\n---\n\nNew steps.";
        store.save("editing", updated).expect("save must succeed");

        let skill = store.skill("editing").expect("skill still present");
        assert_eq!(skill.name, "Editing v2");
        assert_eq!(skill.description, "Edit clips faster.");
        assert_eq!(store.raw("editing").as_deref(), Some(updated));
    }

    #[test]
    fn store_save_validation_failure_writes_nothing() {
        let root = temp_dir("store-invalid");
        write_skill(&root, "editing", VALID_MD);
        let mut store = SkillStore::new(root.clone());

        let blank_name = "---\nname:   \ndescription: Edit clips.\n---\n\nBody";
        let err = store.save("editing", blank_name).unwrap_err();
        assert_eq!(err, SAVE_VALIDATION_ERROR);
        // On-disk content untouched.
        assert_eq!(store.raw("editing").as_deref(), Some(VALID_MD));
        // A validation failure must not create folders for unknown ids either.
        assert!(store.save("brand-new", blank_name).is_err());
        assert!(!root.join("brand-new").exists());
    }

    #[test]
    fn store_save_rejects_path_escape_ids() {
        let root = temp_dir("store-escape");
        let mut store = SkillStore::new(root.clone());
        for bad in ["../evil", "a/b", "a\\b", "..", ".", ""] {
            let err = store
                .save(bad, VALID_MD)
                .expect_err("path-escaping id must be refused");
            assert!(err.contains("Invalid skill id"), "unexpected error: {err}");
        }
        // Nothing escaped the root: parent of the root gained no "evil" entry.
        assert!(!root.parent().unwrap().join("evil").exists());
        assert!(store.raw("../evil").is_none());
        assert!(store.delete("../evil").is_err());
    }

    #[test]
    fn store_save_is_atomic_no_temp_left_behind() {
        let root = temp_dir("store-atomic");
        write_skill(&root, "editing", VALID_MD);
        let mut store = SkillStore::new(root.clone());
        store.save("editing", VALID_MD).unwrap();
        let names: Vec<String> = std::fs::read_dir(root.join("editing"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec!["SKILL.md"], "temp file must not survive a save");
    }

    #[test]
    fn store_new_skill_uses_template_and_unique_ids() {
        let root = temp_dir("store-new");
        let mut store = SkillStore::new(root);
        let first = store.new_skill().unwrap();
        let second = store.new_skill().unwrap();
        assert_eq!(first, "new-skill");
        assert_eq!(second, "new-skill-2");
        assert!(required_fields(SKILL_TEMPLATE).is_some(), "template must validate");
        assert_eq!(store.skill(&first).unwrap().name, "New skill");
        assert_eq!(store.raw(&second).as_deref(), Some(SKILL_TEMPLATE));
    }

    #[test]
    fn store_delete_removes_folder() {
        let root = temp_dir("store-delete");
        write_skill(&root, "editing", VALID_MD);
        let mut store = SkillStore::new(root.clone());
        store.delete("editing").unwrap();
        assert!(store.skills().is_empty());
        assert!(!root.join("editing").exists());
        assert!(store.delete("editing").is_err(), "double delete reports an error");
    }

    #[test]
    fn update_skill_md_preserves_extra_frontmatter_fields() {
        let original = "---\nlicense: MIT\nname: Old\ndescription: Old desc\n---\n\nOld body";
        let updated = update_skill_md(original, "New", "New desc", "New body");
        assert_eq!(
            updated,
            "---\nlicense: MIT\nname: New\ndescription: New desc\n---\n\nNew body"
        );
        let (fields, body) = parse_frontmatter(&updated);
        assert_eq!(fields.get("license").map(String::as_str), Some("MIT"));
        assert_eq!(fields.get("name").map(String::as_str), Some("New"));
        assert_eq!(body, "New body");
    }

    #[test]
    fn update_skill_md_without_frontmatter_creates_block() {
        let updated = update_skill_md("just a body", "Name", "Desc", "Body text");
        assert_eq!(updated, "---\nname: Name\ndescription: Desc\n---\n\nBody text");
        assert!(required_fields(&updated).is_some());
    }

    #[test]
    fn prompt_index_lists_skills_or_is_empty() {
        assert_eq!(prompt_index(&[]), "");
        let skills = vec![Skill {
            id: "captions".into(),
            name: "Captions".into(),
            description: "burn in captions".into(),
            path: PathBuf::from("/x/SKILL.md"),
        }];
        let idx = prompt_index(&skills);
        assert!(idx.contains("# Skills"));
        assert!(idx.contains("read_skill(id)"));
        assert!(idx.contains("- captions: burn in captions"));
    }
}
