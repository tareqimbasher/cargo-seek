use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::errors::AppResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub manifest_file_path: PathBuf,
    pub packages: Vec<Package>,
    pub dependency_kinds: HashMap<String, Vec<String>>,
}

impl Project {
    pub fn read(path: PathBuf) -> Option<Project> {
        if !path.exists() || !path.is_dir() {
            return None;
        }

        let files = std::fs::read_dir(path);

        if files.is_err() {
            return None;
        }

        // Iterate over files and check if we have a match. Iteration was chosen because
        // checking if specific paths exists is error prone. Ex: checking if "cargo.toml" exists
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

        if manifest_file.is_none() {
            return None;
        }

        let manifest_file_path = manifest_file.unwrap().path();

        let metadata = Self::get_metadata(&manifest_file_path).ok()?;

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

        Some(Project {
            manifest_file_path,
            packages,
            dependency_kinds,
        })
    }

    fn get_metadata(manifest_path: &PathBuf) -> AppResult<Metadata> {
        let output = Command::new("cargo")
            .arg("metadata")
            .arg("--no-deps")
            .arg("--format-version")
            .arg("1")
            .arg("--manifest-path")
            .arg(manifest_path)
            .output()?;

        let stdout = String::from_utf8(output.stdout)?;
        let metadata: Metadata = serde_json::from_str(&stdout)?;
        Ok(metadata)
    }

    pub fn contains_package(&self, package_name: String) -> bool {
        self.packages.iter().any(|p| p.name == package_name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    name: String,
    version: Option<String>,
    description: Option<String>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Dependency {
    name: String,
    req: String,
    kind: Option<String>,
    optional: bool,
}
