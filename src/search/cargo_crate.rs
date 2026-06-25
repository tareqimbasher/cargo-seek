use chrono::{DateTime, Utc};

use crate::cargo::{Dependency, InstalledBinary};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// Whether full metadata has been hydrated for this crate (see [`Crate::is_metadata_loaded`]).
    pub metadata_loaded: bool,
    pub project_version: Option<String>,
    pub installed_version: Option<String>,
}

impl Crate {
    pub fn is_metadata_loaded(&self) -> bool {
        self.metadata_loaded
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
                .unwrap_or_else(|| c.max_version.clone()),
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

    /// Fills in full metadata from a crates.io response (the lazy hydration of a selected crate).
    pub fn hydrate(&mut self, response: Box<crates_io_api::CrateResponse>) {
        let data = response.crate_data;
        self.name = data.name;
        self.description = data.description;
        self.homepage = data.homepage;
        self.documentation = data.documentation;
        self.repository = data.repository;
        self.version = data
            .max_stable_version
            .clone()
            .unwrap_or_else(|| data.max_version.clone());
        self.max_version = Some(data.max_version);
        self.max_stable_version = data.max_stable_version;
        self.downloads = Some(data.downloads);
        self.recent_downloads = data.recent_downloads;
        if response.versions.is_empty() {
            self.features = Some(Vec::new());
        } else {
            let latest = &response.versions[0];
            self.features = Some(latest.features.iter().map(|x| x.0.clone()).collect())
        }
        if self.categories.is_none() {
            self.categories = Some(
                response
                    .categories
                    .iter()
                    .map(|c| c.category.clone())
                    .collect(),
            )
        }
        self.created_at = Some(data.created_at);
        self.updated_at = Some(data.updated_at);
        self.exact_match = data.exact_match.unwrap_or_default();
        self.metadata_loaded = true;
    }
}
