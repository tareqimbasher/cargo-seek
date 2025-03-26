use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub version: String,
    pub max_version: Option<String>,
    pub max_stable_version: Option<String>,
    pub downloads: Option<u64>,
    pub recent_downloads: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub exact_match: bool,
    #[serde(default)]
    pub local_version: Option<String>,
    #[serde(default)]
    pub installed_version: Option<String>,
}

