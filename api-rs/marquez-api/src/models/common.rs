use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// String newtypes
// ---------------------------------------------------------------------------

macro_rules! string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn value(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }
    };
}

string_newtype!(NamespaceName);
string_newtype!(DatasetName);
string_newtype!(JobName);
string_newtype!(SourceName);
string_newtype!(TagName);
string_newtype!(OwnerName);
string_newtype!(FieldName);
string_newtype!(SourceType);

// ---------------------------------------------------------------------------
// UUID newtypes
// ---------------------------------------------------------------------------

macro_rules! uuid_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new(id: Uuid) -> Self {
                Self(id)
            }

            pub fn value(&self) -> &Uuid {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }
    };
}

uuid_newtype!(RunId);
uuid_newtype!(Version);

// ---------------------------------------------------------------------------
// Composite IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetId {
    pub namespace: NamespaceName,
    pub name: DatasetName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobId {
    pub namespace: NamespaceName,
    pub name: JobName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetVersionId {
    pub namespace: NamespaceName,
    pub name: DatasetName,
    pub version: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobVersionId {
    pub namespace: NamespaceName,
    pub name: JobName,
    pub version: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetFieldId {
    pub dataset: DatasetId,
    pub field: FieldName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetFieldVersionId {
    pub dataset: DatasetId,
    pub field: FieldName,
    pub version: Uuid,
}

// ---------------------------------------------------------------------------
// Value objects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: FieldName,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default)]
    pub tags: Vec<TagName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDatasetVersion {
    pub dataset_version_id: DatasetVersionId,
    #[serde(default)]
    pub facets: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDatasetVersion {
    pub dataset_version_id: DatasetVersionId,
    #[serde(default)]
    pub facets: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $str:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(
                #[serde(rename = $str)]
                $variant,
            )+
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant => f.write_str($str),)+
                }
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($str => Ok(Self::$variant),)+
                    _ => Err(format!("unknown {} value: {}", stringify!($name), s)),
                }
            }
        }
    };
}

string_enum!(RunState {
    New => "NEW",
    Running => "RUNNING",
    Completed => "COMPLETED",
    Aborted => "ABORTED",
    Failed => "FAILED",
    Other => "OTHER",
});

string_enum!(JobType {
    Batch => "BATCH",
    Stream => "STREAM",
    Service => "SERVICE",
});

string_enum!(DatasetType {
    DbTable => "DB_TABLE",
    Stream => "STREAM",
});

string_enum!(IoType {
    Input => "INPUT",
    Output => "OUTPUT",
});

string_enum!(FacetType {
    Run => "RUN",
    Job => "JOB",
    Dataset => "DATASET",
});

string_enum!(NodeType {
    Dataset => "DATASET",
    DatasetField => "DATASET_FIELD",
    Job => "JOB",
    Run => "RUN",
});

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_newtype_json_roundtrip() {
        let ns = NamespaceName::new("my-namespace");
        let json = serde_json::to_string(&ns).unwrap();
        assert_eq!(json, r#""my-namespace""#);
        let back: NamespaceName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ns);
    }

    #[test]
    fn uuid_newtype_json_roundtrip() {
        let id = RunId::new(Uuid::nil());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""00000000-0000-0000-0000-000000000000""#);
        let back: RunId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn string_newtype_display() {
        let ds = DatasetName::new("my-dataset");
        assert_eq!(ds.to_string(), "my-dataset");
    }

    #[test]
    fn string_newtype_from_str() {
        let jn: JobName = "my-job".into();
        assert_eq!(jn.value(), "my-job");
    }

    #[test]
    fn uuid_newtype_display() {
        let v = Version::new(Uuid::nil());
        assert_eq!(v.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn enum_display_and_from_str() {
        assert_eq!(RunState::Completed.to_string(), "COMPLETED");
        assert_eq!(
            "COMPLETED".parse::<RunState>().unwrap(),
            RunState::Completed
        );
        assert!("INVALID".parse::<RunState>().is_err());

        assert_eq!(JobType::Batch.to_string(), "BATCH");
        assert_eq!("BATCH".parse::<JobType>().unwrap(), JobType::Batch);

        assert_eq!(DatasetType::DbTable.to_string(), "DB_TABLE");
        assert_eq!(
            "DB_TABLE".parse::<DatasetType>().unwrap(),
            DatasetType::DbTable
        );

        assert_eq!(IoType::Input.to_string(), "INPUT");
        assert_eq!("INPUT".parse::<IoType>().unwrap(), IoType::Input);

        assert_eq!(FacetType::Run.to_string(), "RUN");
        assert_eq!("RUN".parse::<FacetType>().unwrap(), FacetType::Run);

        assert_eq!(NodeType::DatasetField.to_string(), "DATASET_FIELD");
        assert_eq!(
            "DATASET_FIELD".parse::<NodeType>().unwrap(),
            NodeType::DatasetField
        );
    }

    #[test]
    fn enum_json_roundtrip() {
        let state = RunState::Failed;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#""FAILED""#);
        let back: RunState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn dataset_id_json_format() {
        let id = DatasetId {
            namespace: NamespaceName::new("ns"),
            name: DatasetName::new("ds"),
        };
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json["namespace"], "ns");
        assert_eq!(json["name"], "ds");

        // Roundtrip
        let back: DatasetId = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn job_id_json_format() {
        let id = JobId {
            namespace: NamespaceName::new("ns"),
            name: JobName::new("job"),
        };
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json["namespace"], "ns");
        assert_eq!(json["name"], "job");
    }

    #[test]
    fn field_json_roundtrip() {
        let f = Field {
            name: FieldName::new("col1"),
            type_: Some("VARCHAR".into()),
            tags: vec![TagName::new("pii")],
            description: Some("A column".into()),
        };
        let json = serde_json::to_value(&f).unwrap();
        assert_eq!(json["name"], "col1");
        assert_eq!(json["type"], "VARCHAR");
        assert_eq!(json["tags"][0], "pii");

        let back: Field = serde_json::from_value(json).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn field_json_optional_fields_omitted() {
        let f = Field {
            name: FieldName::new("col1"),
            type_: None,
            tags: vec![],
            description: None,
        };
        let json = serde_json::to_value(&f).unwrap();
        assert!(json.get("type").is_none());
        assert!(json.get("description").is_none());
    }

    #[test]
    fn dataset_version_id_json_roundtrip() {
        let id = DatasetVersionId {
            namespace: NamespaceName::new("ns"),
            name: DatasetName::new("ds"),
            version: Uuid::nil(),
        };
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json["namespace"], "ns");
        assert_eq!(json["name"], "ds");

        let back: DatasetVersionId = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
    }
}
