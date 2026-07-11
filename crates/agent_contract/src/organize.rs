//! Path-addressed folder resolution for `organize_media` (tool-surface-v2).
//!
//! Folders are addressed by display path ('B-roll/Sunset'), never by id:
//! '/'-separated, segments trimmed, matched case-insensitively per level with
//! exact-case priority. Same-name siblings with no single exact-case winner
//! are ambiguous and refuse resolution.

use core_model::MediaFolder;
use uuid::Uuid;

/// Split a folder path into trimmed, non-empty segments.
pub fn split_path(path: &str) -> Vec<String> {
    path.split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Normalized (trimmed-segment) form of a path for comparisons.
pub fn normalize_path(path: &str) -> String {
    split_path(path).join("/")
}

/// Display path of a folder id ('A/B/C'). Cycle-safe.
pub fn folder_path(folders: &[MediaFolder], id: &str) -> String {
    let mut segs: Vec<String> = Vec::new();
    let mut cur = Some(id.to_string());
    let mut hops = 0usize;
    while let Some(c) = cur {
        let Some(f) = folders.iter().find(|f| f.id == c) else {
            break;
        };
        segs.push(f.name.trim().to_string());
        cur = f.parent_folder_id.clone();
        hops += 1;
        if hops > folders.len() {
            break;
        }
    }
    segs.reverse();
    segs.join("/")
}

/// Resolve one path segment among the children of `parent`.
/// Exact-case match wins; otherwise a single case-insensitive match; more
/// than one candidate without a single exact-case winner is ambiguous.
fn match_segment<'a>(
    folders: &'a [MediaFolder],
    parent: Option<&str>,
    seg: &str,
    full_path: &str,
) -> Result<Option<&'a MediaFolder>, String> {
    let ci: Vec<&MediaFolder> = folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == parent)
        .filter(|f| f.name.trim().to_lowercase() == seg.to_lowercase())
        .collect();
    let exact: Vec<&&MediaFolder> = ci.iter().filter(|f| f.name.trim() == seg).collect();
    match (exact.len(), ci.len()) {
        (1, _) => Ok(Some(*exact[0])),
        (0, 0) => Ok(None),
        (0, 1) => Ok(Some(ci[0])),
        _ => Err(format!(
            "Ambiguous folder path '{full_path}': more than one folder named '{seg}' at that level."
        )),
    }
}

/// Resolve a folder path to its id. `Ok(None)` = a segment doesn't exist;
/// `Err` = an ambiguous segment.
pub fn resolve_folder(folders: &[MediaFolder], path: &str) -> Result<Option<String>, String> {
    let segs = split_path(path);
    if segs.is_empty() {
        return Ok(None);
    }
    let mut parent: Option<String> = None;
    for seg in &segs {
        match match_segment(folders, parent.as_deref(), seg, path)? {
            Some(f) => parent = Some(f.id.clone()),
            None => return Ok(None),
        }
    }
    Ok(parent)
}

/// Resolve a folder path, creating every missing level. Returns the resolved
/// folder id plus the display paths of any folders created along the way.
pub fn resolve_or_create_folder(
    folders: &mut Vec<MediaFolder>,
    path: &str,
) -> Result<(String, Vec<String>), String> {
    let segs = split_path(path);
    if segs.is_empty() {
        return Err(format!("'{path}' is not a valid folder path."));
    }
    let mut parent: Option<String> = None;
    let mut walked: Vec<String> = Vec::new();
    let mut created: Vec<String> = Vec::new();
    for seg in &segs {
        let found = match_segment(folders, parent.as_deref(), seg, path)?
            .map(|f| (f.id.clone(), f.name.trim().to_string()));
        match found {
            Some((id, name)) => {
                walked.push(name);
                parent = Some(id);
            }
            None => {
                let folder = MediaFolder {
                    id: Uuid::new_v4().to_string(),
                    name: seg.clone(),
                    parent_folder_id: parent.clone(),
                };
                walked.push(seg.clone());
                created.push(walked.join("/"));
                parent = Some(folder.id.clone());
                folders.push(folder);
            }
        }
    }
    Ok((parent.expect("segs is non-empty"), created))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, name: &str, parent: Option<&str>) -> MediaFolder {
        MediaFolder {
            id: id.into(),
            name: name.into(),
            parent_folder_id: parent.map(String::from),
        }
    }

    #[test]
    fn split_trims_and_drops_empty_segments() {
        assert_eq!(split_path(" B-roll / Sunset "), vec!["B-roll", "Sunset"]);
        assert_eq!(split_path("//a//"), vec!["a"]);
        assert!(split_path("  /  ").is_empty());
        assert_eq!(normalize_path("a / b"), "a/b");
    }

    #[test]
    fn folder_path_walks_parents() {
        let folders = vec![
            folder("a", "A", None),
            folder("b", "B", Some("a")),
            folder("c", "C", Some("b")),
        ];
        assert_eq!(folder_path(&folders, "c"), "A/B/C");
        assert_eq!(folder_path(&folders, "a"), "A");
    }

    #[test]
    fn resolve_is_case_insensitive_per_segment() {
        let folders = vec![
            folder("a", "B-roll", None),
            folder("b", "Sunset", Some("a")),
        ];
        assert_eq!(
            resolve_folder(&folders, "b-roll/SUNSET").unwrap(),
            Some("b".to_string())
        );
        assert_eq!(resolve_folder(&folders, "b-roll/nope").unwrap(), None);
        assert_eq!(resolve_folder(&folders, "").unwrap(), None);
    }

    #[test]
    fn exact_case_beats_case_insensitive_siblings() {
        let folders = vec![folder("lo", "take", None), folder("hi", "Take", None)];
        assert_eq!(
            resolve_folder(&folders, "Take").unwrap(),
            Some("hi".to_string())
        );
        assert_eq!(
            resolve_folder(&folders, "take").unwrap(),
            Some("lo".to_string())
        );
        // No exact-case winner among multiple candidates → ambiguous.
        let err = resolve_folder(&folders, "TAKE").unwrap_err();
        assert!(err.contains("Ambiguous"), "{err}");
    }

    #[test]
    fn duplicate_exact_names_are_ambiguous() {
        let folders = vec![folder("x", "Take", None), folder("y", "Take", None)];
        let err = resolve_folder(&folders, "Take").unwrap_err();
        assert!(err.contains("Ambiguous"), "{err}");
    }

    #[test]
    fn resolve_or_create_builds_intermediate_levels() {
        let mut folders = vec![folder("a", "A", None)];
        let (id, created) = resolve_or_create_folder(&mut folders, "a/B/C").unwrap();
        assert_eq!(created, vec!["A/B".to_string(), "A/B/C".to_string()]);
        assert_eq!(folders.len(), 3);
        assert_eq!(folder_path(&folders, &id), "A/B/C");
        // Idempotent: nothing new the second time.
        let (id2, created2) = resolve_or_create_folder(&mut folders, "A/b/c").unwrap();
        assert_eq!(id2, id);
        assert!(created2.is_empty());
        assert_eq!(folders.len(), 3);
    }
}
