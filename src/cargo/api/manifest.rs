//! Typed view of `cargo metadata` output.
//!
//! Only the fields this app needs are modeled; the rest of the (large) metadata document is ignored.

use serde::Deserialize;

/// The root of the `cargo metadata` JSON document for the target manifest.
#[derive(Debug, Deserialize)]
pub struct CargoMetadata {
    /// The workspace's own packages.
    pub packages: Vec<Package>,
    /// The metadata format version cargo emitted.
    pub version: u8,
}

/// A single package (crate) belonging to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Package {
    /// The package name as declared in its `Cargo.toml`.
    pub name: String,
    /// The package version, if declared.
    pub version: Option<String>,
    /// The package's short description, if declared.
    pub description: Option<String>,
    /// The package's declared dependencies across all kinds and targets.
    pub dependencies: Vec<Dependency>,
}

/// A dependency declared by a [`Package`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dependency {
    /// The dependency crate's name.
    pub name: String,
    /// The semver version requirement (e.g. `^1.0`).
    pub req: String,
    /// Which dependency section the entry comes from.
    pub kind: DepKind,
    /// Whether the dependency is optional (i.e. feature-gated).
    pub optional: bool,
}

/// Which `Cargo.toml` section a dependency belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepKind {
    /// A regular `[dependencies]` entry.
    #[default]
    Normal,
    /// A `[dev-dependencies]` entry.
    Dev,
    /// A `[build-dependencies]` entry.
    Build,
}

impl<'de> Deserialize<'de> for DepKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = Option::<String>::deserialize(deserializer)?;
        Ok(Self::from_metadata_kind(&raw))
    }
}

impl DepKind {
    fn from_metadata_kind(kind: &Option<String>) -> Self {
        match kind {
            None => DepKind::Normal,
            Some(k) => {
                if k == "dev" {
                    DepKind::Dev
                } else if k == "build" {
                    DepKind::Build
                } else {
                    DepKind::Normal
                }
            }
        }
    }

    /// The `cargo add`/`cargo remove` flag that targets this dependency
    /// section, or `None` for a normal dependency (cargo's default).
    pub fn cargo_flag(&self) -> Option<&'static str> {
        match self {
            DepKind::Normal => None,
            DepKind::Dev => Some("--dev"),
            DepKind::Build => Some("--build"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn dep_kind_deserializes_cargo_wire_format() {
        // cargo metadata encodes dependency kind as null | "dev" | "build".
        assert_eq!(
            serde_json::from_str::<DepKind>("null").unwrap(),
            DepKind::Normal
        );
        assert_eq!(
            serde_json::from_str::<DepKind>(r#""dev""#).unwrap(),
            DepKind::Dev
        );
        assert_eq!(
            serde_json::from_str::<DepKind>(r#""build""#).unwrap(),
            DepKind::Build
        );
    }

    #[test]
    fn dep_kind_unknown_value_falls_back_to_normal() {
        assert_eq!(
            serde_json::from_str::<DepKind>(r#""future-kind""#).unwrap(),
            DepKind::Normal
        );
    }

    #[test]
    fn manifest_metadata_parses_mixed_dependency_kinds() {
        // Mirrors the shape of `cargo metadata --format-version 1`; unknown fields are ignored.
        let json = r#"{
            "packages": [{
                "name": "demo",
                "version": "0.1.0",
                "description": null,
                "dependencies": [
                    { "name": "serde", "req": "^1.0", "kind": null, "optional": false },
                    { "name": "tempfile", "req": "3", "kind": "dev", "optional": false },
                    { "name": "cc", "req": "1", "kind": "build", "optional": true }
                ]
            }],
            "version": 1
        }"#;

        let metadata: CargoMetadata = serde_json::from_str(json).unwrap();
        let deps = &metadata.packages[0].dependencies;

        assert_eq!(deps[0].kind, DepKind::Normal);
        assert_eq!(deps[1].kind, DepKind::Dev);
        assert_eq!(deps[2].kind, DepKind::Build);
    }
}
