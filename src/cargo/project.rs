use std::collections::BTreeSet;
use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use color_eyre::eyre::{WrapErr, bail};

use crate::cargo::{Package, get_metadata};
use crate::errors::AppResult;

/// A local cargo project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub manifest_file_path: PathBuf,
    pub packages: Vec<Package>,
}

impl Project {
    pub fn from(path: &Path) -> Option<Project> {
        if !path.try_exists().ok().unwrap_or_default() || !path.is_dir() {
            return None;
        }

        if let Ok(Some(manifest_file_path)) = find_project_manifest(path) {
            Some(Project {
                manifest_file_path,
                packages: Vec::new(),
            })
        } else {
            None
        }
    }

    /// Reads the current project and updates internal state.
    pub fn read(&mut self) -> AppResult<()> {
        if !self.manifest_file_path.exists() {
            bail!(
                "manifest file no longer exists: {}",
                self.manifest_file_path.display()
            );
        }

        let metadata = get_metadata(&self.manifest_file_path).wrap_err_with(|| {
            format!(
                "failed to read project metadata from {}",
                self.manifest_file_path.display()
            )
        })?;

        self.packages = metadata.packages;

        Ok(())
    }

    /// The version requirement(s) under which `package_name` is declared in the project, or `None`
    /// if it isn't a dependency. Workspace members can declare the same crate at differing reqs, so
    /// the distinct reqs are returned joined (e.g. `"1.0, 2.0"`).
    pub fn get_local_version(&self, package_name: &str) -> Option<String> {
        let reqs: BTreeSet<&str> = self
            .packages
            .iter()
            .flat_map(|package| &package.dependencies)
            .filter(|dependency| dependency.name == package_name)
            .map(|dependency| dependency.req.as_str())
            .collect();

        if reqs.is_empty() {
            None
        } else {
            Some(reqs.into_iter().collect::<Vec<_>>().join(", "))
        }
    }
}

fn find_project_manifest(starting_dir_path: &Path) -> AppResult<Option<PathBuf>> {
    let mut search_path = Some(starting_dir_path);
    let mut manifest_file: Option<DirEntry> = None;

    while search_path.is_some() && manifest_file.is_none() {
        let path = search_path.unwrap();

        let found = fs::read_dir(path)?.find(|f| {
            if let Ok(file) = f
                && file
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("Cargo.toml")
            {
                return true;
            }
            false
        });

        if let Some(found) = found {
            manifest_file = Some(found?);
            break;
        }

        search_path = path.parent();
    }

    if let Some(manifest_file) = manifest_file {
        Ok(Some(manifest_file.path()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Dependency;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    fn dep(name: &str, req: &str) -> Dependency {
        Dependency {
            name: name.into(),
            req: req.into(),
            kind: None,
            optional: false,
        }
    }

    fn package(name: &str, dependencies: Vec<Dependency>) -> Package {
        Package {
            name: name.into(),
            version: None,
            description: None,
            dependencies,
        }
    }

    fn project(packages: Vec<Package>) -> Project {
        Project {
            manifest_file_path: PathBuf::from("Cargo.toml"),
            packages,
        }
    }

    #[test]
    fn finds_manifest_in_the_starting_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            find_project_manifest(dir.path()).unwrap(),
            Some(dir.path().join("Cargo.toml"))
        );
    }

    #[test]
    fn walks_up_to_find_a_manifest_in_an_ancestor() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
        let nested = root.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(
            find_project_manifest(&nested).unwrap(),
            Some(root.path().join("Cargo.toml"))
        );
    }

    #[test]
    fn matches_cargo_toml_case_insensitively() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("cargo.toml"), "[package]").unwrap();
        assert_eq!(
            find_project_manifest(dir.path()).unwrap(),
            Some(dir.path().join("cargo.toml"))
        );
    }

    #[test]
    fn returns_none_when_no_manifest_in_the_tree() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("x").join("y");
        fs::create_dir_all(&nested).unwrap();
        // Walks up to the filesystem root without finding a manifest.
        assert_eq!(find_project_manifest(&nested).unwrap(), None);
    }

    #[test]
    fn get_local_version_returns_the_declared_req() {
        let project = project(vec![package(
            "app",
            vec![dep("serde", "1.0"), dep("tokio", "1")],
        )]);
        assert_eq!(project.get_local_version("serde"), Some("1.0".to_string()));
        assert_eq!(project.get_local_version("tokio"), Some("1".to_string()));
    }

    #[test]
    fn get_local_version_is_none_for_a_non_dependency() {
        let project = project(vec![package("app", vec![dep("serde", "1.0")])]);
        assert_eq!(project.get_local_version("rand"), None);
    }

    #[test]
    fn get_local_version_joins_distinct_reqs_across_members() {
        let project = project(vec![
            package("member_a", vec![dep("serde", "2.0")]),
            package("member_b", vec![dep("serde", "1.0")]),
        ]);
        // Distinct reqs are deduplicated and returned in sorted order, regardless of member order.
        assert_eq!(
            project.get_local_version("serde"),
            Some("1.0, 2.0".to_string())
        );
    }

    #[test]
    fn get_local_version_dedups_identical_reqs() {
        let project = project(vec![
            package("member_a", vec![dep("serde", "1.0")]),
            package("member_b", vec![dep("serde", "1.0")]),
        ]);
        assert_eq!(project.get_local_version("serde"), Some("1.0".to_string()));
    }
}
