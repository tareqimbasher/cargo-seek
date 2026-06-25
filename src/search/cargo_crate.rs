use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::cargo::{Dependency, InstalledBinary};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
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

    /// Builds a stub crate from a globally installed binary.
    pub fn from_binary(bin: &InstalledBinary) -> Self {
        Crate {
            id: bin.name.clone(),
            name: bin.name.clone(),
            version: bin.version.clone(),
            installed_version: Some(bin.version.clone()),
            ..Default::default()
        }
    }

    /// Builds a stub crate from a project dependency.
    pub fn from_dependency(dep: &Dependency) -> Self {
        Crate {
            id: dep.name.clone(),
            name: dep.name.clone(),
            version: dep.req.clone(),
            project_version: Some(dep.req.clone()),
            ..Default::default()
        }
    }

    /// Builds a crate from a crates.io search result. Metadata-only fields (`features`,
    /// `project_version`, `installed_version`) are left for later hydration/annotation.
    pub fn from_crates_io(c: crates_io_api::Crate) -> Self {
        Crate {
            id: c.id,
            name: c.name,
            description: c.description,
            homepage: c.homepage,
            documentation: c.documentation,
            repository: c.repository,
            version: c
                .max_stable_version
                .clone()
                .unwrap_or(c.max_version.clone()),
            max_version: Some(c.max_version),
            max_stable_version: c.max_stable_version,
            downloads: Some(c.downloads),
            recent_downloads: c.recent_downloads,
            created_at: Some(c.created_at),
            updated_at: Some(c.updated_at),
            categories: c.categories,
            exact_match: c.exact_match.unwrap_or(false),
            ..Default::default()
        }
    }
}
