use chrono::{DateTime, SecondsFormat, Utc};
use serde::{de, Deserialize, Deserializer, Serializer};

const COCOA_REFERENCE_UNIX_SECONDS: f64 = 978_307_200.0;

#[derive(Deserialize)]
#[serde(untagged)]
enum NumericDate {
    Integer(i64),
    Float(f64),
}

fn cocoa_seconds_to_datetime<E: de::Error>(seconds: f64) -> Result<DateTime<Utc>, E> {
    let unix_seconds = seconds + COCOA_REFERENCE_UNIX_SECONDS;
    let mut whole_seconds = unix_seconds.floor() as i64;
    let mut nanos = ((unix_seconds - whole_seconds as f64) * 1_000_000_000.0).round() as i64;

    if nanos == 1_000_000_000 {
        whole_seconds += 1;
        nanos = 0;
    }

    DateTime::<Utc>::from_timestamp(whole_seconds, nanos as u32)
        .ok_or_else(|| de::Error::custom("invalid Foundation date value"))
}

fn datetime_to_cocoa_seconds(value: &DateTime<Utc>) -> f64 {
    value.timestamp() as f64 + f64::from(value.timestamp_subsec_nanos()) / 1_000_000_000.0
        - COCOA_REFERENCE_UNIX_SECONDS
}

pub mod foundation_date {
    use super::*;

    pub fn serialize<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(datetime_to_cocoa_seconds(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match NumericDate::deserialize(deserializer)? {
            NumericDate::Integer(value) => cocoa_seconds_to_datetime(value as f64),
            NumericDate::Float(value) => cocoa_seconds_to_datetime(value),
        }
    }
}

pub mod option_foundation_date {
    use super::*;

    pub fn serialize<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_some(&datetime_to_cocoa_seconds(value)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Option::<NumericDate>::deserialize(deserializer)?;
        raw.map(|value| match value {
            NumericDate::Integer(value) => cocoa_seconds_to_datetime(value as f64),
            NumericDate::Float(value) => cocoa_seconds_to_datetime(value),
        })
        .transpose()
    }
}

pub mod iso8601_date {
    use super::*;

    pub fn serialize<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Secs, true))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        chrono::DateTime::parse_from_rfc3339(&raw)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| de::Error::custom(format!("invalid ISO-8601 date: {error}")))
    }
}
