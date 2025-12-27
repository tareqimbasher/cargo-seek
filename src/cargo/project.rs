use std::collections::HashMap;
use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cargo::{Package, get_metadata};
use crate::errors::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DependencyInfo {
    kinds: Vec<String>,
    version: String,
}

/// A local cargo project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub manifest_file_path: PathBuf,
    pub packages: Vec<Package>,
    dependency_versions: HashMap<String, DependencyInfo>,
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
                dependency_versions: HashMap::new(),
            })
        } else {
            None
        }
    }

    /// Reads the current project and updates internal state.
    pub fn read(&mut self) -> AppResult<()> {
        if !self.manifest_file_path.exists() {
            return Err(AppError::Unknown(
                "Manifest file no longer exists".to_owned(),
            ));
        }

        let metadata = get_metadata(&self.manifest_file_path)?;

        let packages = metadata.packages;

        let mut dependencies: HashMap<String, DependencyInfo> = HashMap::new();

        for package in packages.iter() {
            for dependency in &package.dependencies {
                let info = dependencies
                    .entry(dependency.name.clone())
                    .or_insert(DependencyInfo {
                        kinds: vec![],
                        version: String::new(),
                    });

                info.kinds.push(dependency.kind.clone().unwrap_or_default());
                info.version = dependency.req.clone();
            }
        }

        self.packages = packages;
        self.dependency_versions = dependencies;

        Ok(())
    }

    /// Gets the version of the given crate name if it is added to the project, None otherwise.
    pub fn get_local_version(&self, package_name: &str) -> Option<String> {
        self.dependency_versions
            .get(package_name)
            .map(|dep| dep.version.clone())
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
