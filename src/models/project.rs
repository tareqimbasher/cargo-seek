use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cargo::CargoManager;
use crate::errors::AppResult;
use crate::models::manifest_metadata::Package;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub manifest_file_path: PathBuf,
    pub packages: Vec<Package>,
    pub dependency_kinds: HashMap<String, Vec<String>>,
    package_names: HashSet<String>,
}

impl Project {
    pub fn from(path: PathBuf) -> Option<Project> {
        if !path.exists() || !path.is_dir() {
            return None;
        }

        let files = std::fs::read_dir(path);

        if files.is_err() {
            return None;
        }

        // Iterate over files and check if we have a match. Iteration was chosen because
        // checking if specific paths exists is error-prone. Ex: checking if "cargo.toml" exists
        // on Windows returns true when the file's name is called "Cargo.toml", this causes an
        // issue in that the cargo executable wants the exact file name.
        let manifest_file = files
            .unwrap()
            .find(|f| {
                if let Ok(file) = f {
                    let file_name = file.file_name().to_str().unwrap_or_default().to_string();
                    if file_name == "Cargo.toml" || file_name == "cargo.toml" {
                        return true;
                    }
                }
                false
            })?
            .ok();

        manifest_file.as_ref()?;

        let manifest_file_path = manifest_file.unwrap().path();

        Some(Project {
            manifest_file_path,
            packages: Vec::new(),
            dependency_kinds: HashMap::new(),
            package_names: HashSet::new(),
        })
    }

    pub fn read(&mut self) -> AppResult<()> {
        let metadata = CargoManager::get_metadata(&self.manifest_file_path)?;

        let packages = metadata.packages;

        let mut dependency_kinds: HashMap<String, Vec<String>> = HashMap::new();

        for package in packages.iter() {
            for dependency in &package.dependencies {
                dependency_kinds
                    .entry(dependency.name.clone())
                    .or_default()
                    .push(dependency.kind.clone().unwrap_or_default());
            }
        }

        self.package_names = HashSet::from_iter(packages.iter().map(|p| p.name.clone()));
        self.packages = packages;
        self.dependency_kinds = dependency_kinds;

        Ok(())
    }

    pub fn contains_package(&self, package_name: &str) -> bool {
        self.package_names.contains(package_name)
    }
}
