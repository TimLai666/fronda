//! Local SKILL.md skill store (Swift: SkillStore). Upstream palmier-pro #199.
//!
//! Scans `~/.palmier/skills/<id>/SKILL.md`, parses each skill's YAML-ish
//! frontmatter (name/description), and builds a sorted skill list plus the
//! prompt index injected into the in-app agent. Pure std — no gpui.
//!
//! This is the local-store half of #199. Still to port: the GitHub `SkillCatalog`
//! (install/refresh), the `read_skill` agent tool + prompt injection wiring, and
//! the Settings > Skills pane UI.

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

/// Build a `Skill` from a folder id and its SKILL.md text. Returns `None` when
/// the frontmatter has no non-empty `name` (an unrecognized skill, skipped).
fn parse_skill(id: &str, path: &Path, text: &str) -> Option<Skill> {
    let (fields, _body) = parse_frontmatter(text);
    let name = fields.get("name").map(|s| s.trim()).unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    let description = fields
        .get("description")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Some(Skill {
        id: id.to_string(),
        name: name.to_string(),
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
        if let Some(skill) = parse_skill(&id, &md, &text) {
            found.push(skill);
        }
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
        let (fields, body) = parse_frontmatter(&text);
        let name = fields.get("name").map(|s| s.trim()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let description = fields
            .get("description")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        out.push(agent_contract::AgentSkill {
            id,
            name: name.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-skill-store-tests").join(name);
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

    #[test]
    fn load_skills_reads_valid_and_skips_invalid() {
        let root = temp_dir("load");
        write_skill(&root, "b-skill", "---\nname: Bravo\ndescription: second\n---\nbody");
        write_skill(&root, "a-skill", "---\nname: Alpha\ndescription: first\n---\nbody");
        // No name → skipped.
        write_skill(&root, "nameless", "---\ndescription: no name\n---\nbody");
        // No SKILL.md at all → skipped (folder created empty).
        std::fs::create_dir_all(root.join("empty-folder")).unwrap();

        let skills = load_skills(&root);
        assert_eq!(skills.len(), 2, "only the two named skills load");
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

        let skills = load_agent_skills(&root);
        assert_eq!(skills.len(), 1, "nameless skill skipped");
        assert_eq!(skills[0].id, "captions");
        assert_eq!(skills[0].description, "burn in");
        assert_eq!(skills[0].body, "1. Transcribe\n2. Style");
    }

    #[test]
    fn load_skills_missing_dir_is_empty() {
        let missing = std::env::temp_dir().join("fronda-skill-store-tests/does-not-exist-xyz");
        assert!(load_skills(&missing).is_empty());
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
