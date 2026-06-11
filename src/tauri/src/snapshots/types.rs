use std::fmt;

use serde::{Deserialize, Serialize};

use crate::projects::ProjectId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotIdError {
    Empty,
    InvalidCharacters,
}

impl fmt::Display for SnapshotIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "cannot use empty snapshot id"),
            Self::InvalidCharacters => write!(
                f,
                "cannot use snapshot id with invalid characters (only alphanumeric and dash allowed)"
            ),
        }
    }
}

impl std::error::Error for SnapshotIdError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq, specta::Type)]
#[specta(transparent)]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn new(id: impl Into<String>) -> Result<Self, SnapshotIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(SnapshotIdError::Empty);
        }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(SnapshotIdError::InvalidCharacters);
        }
        Ok(Self(id))
    }

    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_millis();
        let suffix: u16 = rand::random();
        Self(format!("snap-{millis}-{suffix:04x}"))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SnapshotId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<SnapshotId> for String {
    fn from(id: SnapshotId) -> Self {
        id.0
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = SnapshotIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SnapshotId::new(value)
    }
}

impl Serialize for SnapshotId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SnapshotId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SnapshotId::new(s).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRecord {
    pub id: SnapshotId,
    pub project_id: ProjectId,
    pub label: Option<String>,
    pub taken_at: u64,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_id_rejects_empty() {
        assert_eq!(SnapshotId::new(""), Err(SnapshotIdError::Empty));
    }

    #[test]
    fn snapshot_id_rejects_invalid_chars() {
        assert_eq!(
            SnapshotId::new("snap/foo"),
            Err(SnapshotIdError::InvalidCharacters)
        );
        assert_eq!(
            SnapshotId::new("snap with space"),
            Err(SnapshotIdError::InvalidCharacters)
        );
    }

    #[test]
    fn snapshot_id_accepts_alphanumeric_and_dash() {
        let id = SnapshotId::new("snap-1747500000000-a3f9").expect("valid id");
        assert_eq!(id.as_str(), "snap-1747500000000-a3f9");
    }

    #[test]
    fn snapshot_id_generate_starts_with_snap() {
        let id = SnapshotId::generate();
        assert!(
            id.as_str().starts_with("snap-"),
            "generated id should start with snap-: {id}"
        );
        assert!(SnapshotId::new(id.as_str()).is_ok());
    }

    #[test]
    fn snapshot_id_as_ref_and_string_conversions_roundtrip() {
        let id = SnapshotId::new("snap-123").unwrap();
        let s: &str = id.as_ref();
        assert_eq!(s, "snap-123");
        let owned: String = SnapshotId::new("snap-123").unwrap().into();
        assert_eq!(owned, "snap-123");
        let parsed = SnapshotId::try_from("snap-abc".to_string()).unwrap();
        assert_eq!(parsed.as_str(), "snap-abc");
        assert!(SnapshotId::try_from("bad/id".to_string()).is_err());
    }

    #[test]
    fn snapshot_record_serializes_camel_case() {
        let record = SnapshotRecord {
            id: SnapshotId::new("snap-1-aaaa").unwrap(),
            project_id: ProjectId::new("proj-1").unwrap(),
            label: Some("before-refactor".into()),
            taken_at: 1_747_500_000_000,
            size_bytes: 1234,
        };
        let json = serde_json::to_string(&record).expect("serialize");
        assert!(json.contains("\"projectId\":\"proj-1\""), "json: {json}");
        assert!(json.contains("\"takenAt\":1747500000000"), "json: {json}");
        assert!(json.contains("\"sizeBytes\":1234"), "json: {json}");
    }
}
