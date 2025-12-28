use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
    pub features: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,

    pub exact_match: bool,
    pub project_version: Option<String>,
    pub installed_version: Option<String>,
}

impl Crate {
    pub fn is_metadata_loaded(&self) -> bool {
        self.features.is_some()
    }
}
