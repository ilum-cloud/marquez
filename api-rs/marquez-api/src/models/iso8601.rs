// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! Custom serde (de)serialization for `DateTime<Utc>` that produces ISO 8601
//! timestamps with a `Z` suffix and microsecond precision — matching the Java
//! API output (e.g. `"2024-01-15T10:30:00.123456Z"`).
//!
//! chrono's default serde uses `+00:00` instead of `Z`, which breaks some
//! frontend date-parsing libraries.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{self, Deserialize, Deserializer, Serializer};

/// Format a `DateTime<Utc>` as an ISO 8601 string with `Z` suffix and
/// microsecond precision, suitable for embedding in `serde_json::json!()`.
pub fn fmt(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// Format an `Option<DateTime<Utc>>` — returns `serde_json::Value::Null` for
/// `None`, or a JSON string with `Z` suffix for `Some`.
pub fn fmt_opt(dt: &Option<DateTime<Utc>>) -> serde_json::Value {
    match dt {
        Some(dt) => serde_json::Value::String(fmt(dt)),
        None => serde_json::Value::Null,
    }
}

pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = dt.to_rfc3339_opts(SecondsFormat::Micros, true);
    serializer.serialize_str(&s)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom)
}

pub fn serialize_option<S>(dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match dt {
        Some(dt) => serialize(dt, serializer),
        None => serializer.serialize_none(),
    }
}

pub fn deserialize_option<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => s
            .parse::<DateTime<Utc>>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}
