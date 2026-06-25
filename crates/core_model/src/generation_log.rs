use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

fn default_generation_log_version() -> i64 {
    1
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationLog {
    #[serde(default = "default_generation_log_version")]
    pub version: i64,
    #[serde(default)]
    pub entries: Vec<GenerationLogEntry>,
}

impl Default for GenerationLog {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl GenerationLog {
    /// Sort entries newest-first by `createdAt` (FMT-014).
    ///
    /// When timestamps are absent, falls back to deterministic ordering by
    /// entry id (which are UUIDs and globally unique). This ensures stable
    /// sort results even when timestamps are missing.
    pub fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| {
            match (&a.created_at, &b.created_at) {
                (Some(a_time), Some(b_time)) => b_time.cmp(a_time), // newest first
                (Some(_), None) => std::cmp::Ordering::Less,        // dated before undated
                (None, Some(_)) => std::cmp::Ordering::Greater,     // undated after dated
                (None, None) => b.id.cmp(&a.id),                    // deterministic fallback by id
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationLogEntry {
    pub id: String,
    pub model: String,
    pub cost_credits: Option<i64>,
    #[serde(default, with = "crate::date_serde::option_foundation_date")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationLogEntryRepr {
    #[serde(default = "new_id")]
    id: String,
    model: String,
    cost_credits: Option<i64>,
    cost: Option<f64>,
    #[serde(default, with = "crate::date_serde::option_foundation_date")]
    created_at: Option<DateTime<Utc>>,
}

impl<'de> Deserialize<'de> for GenerationLogEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = GenerationLogEntryRepr::deserialize(deserializer)?;
        let cost_credits = repr
            .cost_credits
            .or_else(|| repr.cost.map(|value| (value * 100.0).ceil() as i64));

        Ok(Self {
            id: repr.id,
            model: repr.model,
            cost_credits,
            created_at: repr.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn entry_with_time(id: &str, seconds: i64) -> GenerationLogEntry {
        GenerationLogEntry {
            id: id.to_string(),
            model: "test-model".to_string(),
            cost_credits: Some(100),
            created_at: Some(Utc.timestamp_opt(seconds, 0).unwrap()),
        }
    }

    fn entry_no_time(id: &str) -> GenerationLogEntry {
        GenerationLogEntry {
            id: id.to_string(),
            model: "test-model".to_string(),
            cost_credits: None,
            created_at: None,
        }
    }

    #[test]
    fn fmt_014_sort_newest_first() {
        let mut log = GenerationLog::default();
        log.entries = vec![
            entry_with_time("old", 1000),
            entry_with_time("middle", 2000),
            entry_with_time("newest", 3000),
        ];
        log.sort_entries();
        assert_eq!(log.entries[0].id, "newest", "FMT-014: newest first");
        assert_eq!(log.entries[1].id, "middle", "FMT-014: middle second");
        assert_eq!(log.entries[2].id, "old", "FMT-014: oldest last");
    }

    #[test]
    fn fmt_014_dated_before_undated() {
        let mut log = GenerationLog::default();
        log.entries = vec![entry_no_time("no-time"), entry_with_time("with-time", 1000)];
        log.sort_entries();
        assert_eq!(
            log.entries[0].id, "with-time",
            "FMT-014: dated before undated"
        );
        assert_eq!(log.entries[1].id, "no-time", "FMT-014: undated after dated");
    }

    #[test]
    fn fmt_014_deterministic_fallback_for_undated() {
        let mut log = GenerationLog::default();
        // Ids sort in reverse for deterministic ordering
        log.entries = vec![entry_no_time("b-entry"), entry_no_time("a-entry")];
        log.sort_entries();
        assert_eq!(
            log.entries[0].id, "b-entry",
            "FMT-014: deterministic fallback"
        );
        assert_eq!(log.entries[1].id, "a-entry", "FMT-014: second entry");
    }

    #[test]
    fn fmt_014_empty_log() {
        let mut log = GenerationLog::default();
        log.sort_entries();
        assert!(log.entries.is_empty(), "FMT-014: empty log stays empty");
    }

    #[test]
    fn fmt_014_single_entry() {
        let mut log = GenerationLog::default();
        log.entries = vec![entry_with_time("only", 1000)];
        log.sort_entries();
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries[0].id, "only");
    }
}
