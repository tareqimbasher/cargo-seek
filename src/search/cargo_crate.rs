use chrono::{DateTime, Utc};

use crate::cargo::{Dependency, InstalledBinary};

const RECENT_VERSIONS_LIMIT: usize = 5;

/// A trimmed-down record of one published version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSummary {
    pub num: String,
    pub created_at: DateTime<Utc>,
    pub yanked: bool,
}

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
    /// The crate's selectable feature names (the `default` umbrella excluded), sorted. `None` until
    /// metadata is hydrated; `Some(empty)` for a crate with no features.
    pub features: Option<Vec<String>>,
    /// Names of the features enabled by the crate's `default` feature.
    pub default_features: Vec<String>,
    pub license: Option<String>,
    /// Minimum supported Rust version declared by the latest version, if any.
    pub rust_version: Option<String>,
    /// Download size of the latest version's `.crate` tarball, in bytes.
    pub crate_size: Option<u64>,
    /// Whether the latest version is yanked.
    pub latest_yanked: bool,
    /// Number of published versions. `0` until metadata is hydrated.
    pub version_count: usize,
    /// Publisher of the latest version, formatted as `name (login)` or `login`.
    pub published_by: Option<String>,
    /// The most recent versions, newest first, capped at [`RECENT_VERSIONS_LIMIT`].
    pub recent_versions: Vec<VersionSummary>,
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,

    /// Whether this crate is an exact match for the user's search query.
    pub exact_match: bool,
    /// Whether full metadata has been hydrated for this crate (see [`Crate::is_metadata_loaded`]).
    pub metadata_loaded: bool,
    /// The version of this crate that is added to the project at the current directory, otherwise `None`.
    pub project_version: Option<String>,
    /// The version of this crate that is installed, otherwise `None`.
    pub installed_version: Option<String>,
}

impl Crate {
    pub fn is_metadata_loaded(&self) -> bool {
        self.metadata_loaded
    }

    /// Whether `feature` is enabled by the crate's default feature set.
    pub fn is_default_feature(&self, feature: &str) -> bool {
        self.default_features.iter().any(|f| f == feature)
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
    pub fn hydrate(&mut self, response: &crates_io_api::CrateResponse) {
        let data = &response.crate_data;
        self.name = data.name.clone();
        self.description = data.description.clone();
        self.homepage = data.homepage.clone();
        self.documentation = data.documentation.clone();
        self.repository = data.repository.clone();
        self.version = data
            .max_stable_version
            .clone()
            .unwrap_or_else(|| data.max_version.clone());
        self.max_version = Some(data.max_version.clone());
        self.max_stable_version = data.max_stable_version.clone();
        self.downloads = Some(data.downloads);
        self.recent_downloads = data.recent_downloads;
        self.version_count = response.versions.len();
        self.keywords = Some(
            response
                .keywords
                .iter()
                .map(|k| k.keyword.clone())
                .collect(),
        );
        self.recent_versions = response
            .versions
            .iter()
            .take(RECENT_VERSIONS_LIMIT)
            .map(|v| VersionSummary {
                num: v.num.clone(),
                created_at: v.created_at,
                yanked: v.yanked,
            })
            .collect();

        // License, MSRV, size, yank state and publisher are only meaningful per-version (the
        // crate-level `license` is deprecated and always empty), so read them from the latest.
        if let Some(latest) = response.versions.first() {
            self.default_features = latest.features.get("default").cloned().unwrap_or_default();
            // Drop `default` since it isn't an individually selectable feature.
            let mut features: Vec<String> = latest
                .features
                .keys()
                .filter(|name| name.as_str() != "default")
                .cloned()
                .collect();
            features.sort();
            self.features = Some(features);

            self.license = latest.license.clone();
            self.rust_version = latest.rust_version.clone();
            self.crate_size = latest.crate_size;
            self.latest_yanked = latest.yanked;
            self.published_by = latest.published_by.as_ref().map(format_publisher);
        } else {
            self.features = Some(Vec::new());
            self.default_features = Vec::new();
            self.license = None;
            self.rust_version = None;
            self.crate_size = None;
            self.latest_yanked = false;
            self.published_by = None;
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

/// Formats a version's publisher as `name (login)`, or just `login` when no display name is set.
fn format_publisher(user: &crates_io_api::User) -> String {
    match user.name.as_deref() {
        Some(name) if !name.is_empty() => format!("{name} ({})", user.login),
        _ => user.login.clone(),
    }
}
