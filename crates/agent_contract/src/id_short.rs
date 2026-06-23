//! ID shortening for agent/MCP tool outputs (AID-001 to AID-004).
//!
//! UUID-like ids are shortened to the shortest unique prefix with a floor
//! length of 8 characters, computed globally across all id namespaces.

use std::collections::HashMap;

/// Shortens each full id to the shortest unique prefix (min 8 chars) within
/// the given set of all ids.
///
/// Returns a map of full_id → short_prefix.
///
/// AID-001: shortest unique prefix, floor length 8.
/// AID-002: uniqueness computed globally across all id namespaces.
pub fn shorten_ids(all_ids: &[String]) -> HashMap<String, String> {
    let unique: Vec<&str> = {
        let mut s: Vec<&str> = all_ids.iter().map(String::as_str).collect();
        s.sort();
        s.dedup();
        s
    };

    let mut map = HashMap::new();

    for &full in &unique {
        let short = shortest_unique_prefix(full, &unique);
        map.insert(full.to_string(), short);
    }

    map
}

/// Given one full id and all unique full ids, returns the shortest prefix
/// (minimum 8 characters) that distinguishes it from every other id.
fn shortest_unique_prefix(full: &str, all: &[&str]) -> String {
    let max_len = full.len();
    let start = 8.min(max_len);

    for len in start..=max_len {
        let prefix = &full[..len];
        let mut count = 0;
        for other in all {
            if other.starts_with(prefix) {
                count += 1;
                if count > 1 {
                    break;
                }
            }
        }
        if count == 1 {
            return prefix.to_string();
        }
    }

    full.to_string()
}

/// Resolves a potentially short id back to a full id.
///
/// AID-003: accepts either short prefix or full id.
/// AID-004: ambiguous short prefixes hard-fail.
pub fn resolve_id(input: &str, all_ids: &[String]) -> Result<String, IdResolutionError> {
    // Exact match first
    if all_ids.iter().any(|id| id == input) {
        return Ok(input.to_string());
    }

    let matches: Vec<&String> = all_ids.iter().filter(|id| id.starts_with(input)).collect();

    match matches.len() {
        0 => Err(IdResolutionError::NotFound {
            input: input.to_string(),
        }),
        1 => Ok(matches[0].clone()),
        _ => Err(IdResolutionError::Ambiguous {
            input: input.to_string(),
            candidates: matches.into_iter().cloned().collect(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IdResolutionError {
    NotFound {
        input: String,
    },
    Ambiguous {
        input: String,
        candidates: Vec<String>,
    },
}

impl std::fmt::Display for IdResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdResolutionError::NotFound { input } => {
                write!(f, "id not found: {input}")
            }
            IdResolutionError::Ambiguous { input, candidates } => {
                write!(f, "ambiguous id '{input}': matches {candidates:?}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aid_001_shortest_unique_prefix_floor_8() {
        let ids = vec![
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(),
            "a1b2c3d4-e5f6-7890-abcd-ef1234567891".to_string(),
            "b2c3d4e5-f6a7-8901-bcde-f12345678901".to_string(),
        ];
        let map = shorten_ids(&ids);
        // First two share the first 8 chars, so need longer
        let a = &map[&ids[0]];
        let b = &map[&ids[1]];
        let c = &map[&ids[2]];
        assert_ne!(a, b, "two ids with same first 8 chars must differ");
        assert_eq!(c.len(), 8, "unique first 8 chars should stay at floor 8");
        assert!(
            a.len() >= 8 && b.len() >= 8,
            "floor is 8 but got {} / {}",
            a.len(),
            b.len()
        );
    }

    #[test]
    fn aid_002_global_uniqueness() {
        let ids = vec![
            // track id and clip id that share prefix
            "aaaaaaaa-0000-0000-0000-000000000001".to_string(),
            "aaaaaaaa-0000-0000-0000-000000000002".to_string(),
            "bbbbbbbb-0000-0000-0000-000000000001".to_string(),
        ];
        let map = shorten_ids(&ids);
        assert!(map[&ids[0]] != map[&ids[1]]);
        assert_eq!(map[&ids[2]].len(), 8);
    }

    #[test]
    fn aid_003_accepts_full_or_short() {
        let ids = vec![
            "aaaaaaaa-0000-0000-0000-000000000001".to_string(),
            "bbbbbbbb-0000-0000-0000-000000000002".to_string(),
        ];
        // Full id resolves
        assert_eq!(resolve_id(&ids[0], &ids).unwrap(), ids[0]);
        // Unique prefix resolves
        assert_eq!(resolve_id("aaaaaaaa", &ids).unwrap(), ids[0]);
        assert_eq!(resolve_id("bbbbbbbb", &ids).unwrap(), ids[1]);
    }

    #[test]
    fn aid_004_ambiguous_short_prefix_hard_fails() {
        let ids = vec![
            "aaaaaaaa-0000-0000-0000-000000000001".to_string(),
            "aaaaaaaa-0000-0000-0000-000000000002".to_string(),
        ];
        let result = resolve_id("aaaaaaaa", &ids);
        assert!(
            matches!(result, Err(IdResolutionError::Ambiguous { .. })),
            "expected Ambiguous error, got {result:?}"
        );
    }

    #[test]
    fn resolve_not_found() {
        let ids: Vec<String> = vec!["abcdef01-1234-5678-9abc-def012345678".to_string()];
        let result = resolve_id("zzzzzzzz", &ids);
        assert!(matches!(result, Err(IdResolutionError::NotFound { .. })));
    }
}
