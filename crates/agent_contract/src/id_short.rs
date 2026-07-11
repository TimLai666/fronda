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

/// Input arg keys holding a single id (tool-surface-v2 C-3).
pub const SCALAR_ID_KEYS: &[&str] = &[
    "clipId",
    "sourceClipId",
    "referenceClipId",
    "targetClipId",
    "mediaRef",
    "startFrameMediaRef",
    "endFrameMediaRef",
    "sourceVideoMediaRef",
    "videoSourceMediaRef",
    "captionGroupId",
    "timelineId",
    "item",
    "from",
    "reference",
    "groupId",
    "memberId",
];

/// Input arg keys holding arrays of ids (tool-surface-v2 C-3).
pub const ARRAY_ID_KEYS: &[&str] = &[
    "clipIds",
    "targetClipIds",
    "items",
    "ids",
    "deletes",
    "referenceMediaRefs",
    "referenceImageMediaRefs",
    "referenceVideoMediaRefs",
    "referenceAudioMediaRefs",
];

/// C-3 input expansion: a value under a known id key that is a >= 8-char
/// prefix of exactly one universe id expands to the full id. Ambiguous
/// prefixes hard-fail; unknown values pass through so the tool reports
/// not-found itself.
fn expand_one(value: &str, all_ids: &[String]) -> Result<Option<String>, String> {
    if value.len() < 8 || all_ids.iter().any(|id| id == value) {
        return Ok(None);
    }
    let matches: Vec<&String> = all_ids.iter().filter(|id| id.starts_with(value)).collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0].clone())),
        n => Err(format!(
            "Ambiguous id '{value}' matches {n} items; re-read with get_timeline or get_media for current ids."
        )),
    }
}

/// Recursively expand short id prefixes in tool arguments (C-3): known scalar
/// keys and known array-of-string keys at any depth.
pub fn expand_input_ids(
    args: &serde_json::Value,
    all_ids: &[String],
) -> Result<serde_json::Value, String> {
    match args {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                let expanded = if SCALAR_ID_KEYS.contains(&k.as_str()) {
                    match v.as_str() {
                        Some(s) => match expand_one(s, all_ids)? {
                            Some(full) => serde_json::Value::String(full),
                            None => v.clone(),
                        },
                        None => expand_input_ids(v, all_ids)?,
                    }
                } else if ARRAY_ID_KEYS.contains(&k.as_str()) {
                    match v.as_array() {
                        Some(arr) => {
                            let mut items = Vec::with_capacity(arr.len());
                            for item in arr {
                                match item.as_str() {
                                    Some(s) => match expand_one(s, all_ids)? {
                                        Some(full) => items.push(serde_json::Value::String(full)),
                                        None => items.push(item.clone()),
                                    },
                                    None => items.push(expand_input_ids(item, all_ids)?),
                                }
                            }
                            serde_json::Value::Array(items)
                        }
                        None => expand_input_ids(v, all_ids)?,
                    }
                } else {
                    expand_input_ids(v, all_ids)?
                };
                out.insert(k.clone(), expanded);
            }
            Ok(serde_json::Value::Object(out))
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(expand_input_ids(item, all_ids)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        other => Ok(other.clone()),
    }
}

fn is_uuid_shaped(s: &[u8]) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Replace every known full UUID embedded in `text` with its short prefix.
pub fn shorten_uuids_in_text(text: &str, map: &HashMap<String, String>) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 36 <= bytes.len() && is_uuid_shaped(&bytes[i..i + 36]) {
            let candidate = &text[i..i + 36];
            if let Some(short) = map.get(candidate) {
                out.push_str(short);
                i += 36;
                continue;
            }
        }
        // Advance one full UTF-8 character.
        let ch_len = text[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Recursively shorten every known full id in a tool output value (C-3):
/// applies to whole-string ids and to ids embedded in longer strings
/// (e.g. JSON serialized into MCP text content).
pub fn shorten_output_ids(
    value: &serde_json::Value,
    map: &HashMap<String, String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(shorten_uuids_in_text(s, map)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| shorten_output_ids(v, map)).collect())
        }
        serde_json::Value::Object(obj) => serde_json::Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), shorten_output_ids(v, map)))
                .collect(),
        ),
        other => other.clone(),
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

    fn uni() -> Vec<String> {
        vec![
            "aaaa1111-0000-4000-8000-000000000001".to_string(),
            "bbbb2222-0000-4000-8000-000000000002".to_string(),
            "bbbb2222-0000-4000-8000-000000000003".to_string(),
        ]
    }

    #[test]
    fn expand_scalar_key_prefix_to_full_id() {
        let args = serde_json::json!({"clipId": "aaaa1111", "other": "aaaa1111"});
        let out = expand_input_ids(&args, &uni()).unwrap();
        assert_eq!(out["clipId"], "aaaa1111-0000-4000-8000-000000000001");
        assert_eq!(out["other"], "aaaa1111", "non-id keys untouched");
    }

    #[test]
    fn expand_nested_array_keys() {
        let args = serde_json::json!({
            "moves": [{"items": ["aaaa1111"], "into": "B-roll"}],
        });
        let out = expand_input_ids(&args, &uni()).unwrap();
        assert_eq!(
            out["moves"][0]["items"][0],
            "aaaa1111-0000-4000-8000-000000000001"
        );
        assert_eq!(out["moves"][0]["into"], "B-roll");
    }

    #[test]
    fn expand_ambiguous_prefix_hard_fails_with_contract_message() {
        let args = serde_json::json!({"clipId": "bbbb2222"});
        let err = expand_input_ids(&args, &uni()).unwrap_err();
        assert_eq!(
            err,
            "Ambiguous id 'bbbb2222' matches 2 items; re-read with get_timeline or get_media for current ids."
        );
    }

    #[test]
    fn expand_short_or_unknown_values_pass_through() {
        let args = serde_json::json!({"clipId": "aaaa", "mediaRef": "zzzz9999"});
        let out = expand_input_ids(&args, &uni()).unwrap();
        assert_eq!(out["clipId"], "aaaa", "< 8 chars pass through");
        assert_eq!(out["mediaRef"], "zzzz9999", "no-match passes through");
    }

    #[test]
    fn shorten_uuids_embedded_in_text() {
        let map: HashMap<String, String> = [(
            "aaaa1111-0000-4000-8000-000000000001".to_string(),
            "aaaa1111".to_string(),
        )]
        .into_iter()
        .collect();
        let text = r#"{"id": "aaaa1111-0000-4000-8000-000000000001", "n": 1}"#;
        assert_eq!(
            shorten_uuids_in_text(text, &map),
            r#"{"id": "aaaa1111", "n": 1}"#
        );
        // Unknown uuids stay intact.
        let other = "cccc3333-0000-4000-8000-000000000009";
        assert_eq!(shorten_uuids_in_text(other, &map), other);
    }

    #[test]
    fn shorten_output_walks_the_value_tree() {
        let map: HashMap<String, String> = [(
            "aaaa1111-0000-4000-8000-000000000001".to_string(),
            "aaaa1111".to_string(),
        )]
        .into_iter()
        .collect();
        let v = serde_json::json!({
            "content": [{"type": "text", "text": "clip aaaa1111-0000-4000-8000-000000000001 moved"}]
        });
        let out = shorten_output_ids(&v, &map);
        assert_eq!(out["content"][0]["text"], "clip aaaa1111 moved");
    }
}
