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
