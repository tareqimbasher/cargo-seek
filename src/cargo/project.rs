use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cargo::{get_metadata, Package};
use crate::errors::AppResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DependencyInfo {
    kinds: Vec<String>,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub manifest_file_path: PathBuf,
    pub packages: Vec<Package>,
    dependencies: HashMap<String, DependencyInfo>,
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
            dependencies: HashMap::new(),
        })
    }

    pub fn read(&mut self) -> AppResult<()> {
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
        self.dependencies = dependencies;

        Ok(())
    }

    pub fn get_local_version(&self, package_name: &str) -> Option<String> {
        self.dependencies.get(package_name).map(|dep| dep.version.clone())
    }
}
