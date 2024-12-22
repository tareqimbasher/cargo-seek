use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub req: String,
    pub kind: Option<String>,
    pub optional: bool,
}

pub struct InstalledBinary {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub max_version: String,
    pub max_stable_version: Option<String>,
    pub downloads: Option<u64>,
    pub recent_downloads: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub exact_match: bool,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default)]
    pub is_installed: bool,
}

impl Crate {
    pub fn version(&self) -> &str {
        match &self.max_stable_version {
            Some(v) => v,
            None => self.max_version.as_str(),
        }
    }
}
