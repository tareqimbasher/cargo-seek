use std::collections::{HashMap};
use std::path::PathBuf;

use crate::cargo::{get_installed_binaries, InstalledBinary, Project};
use crate::errors::AppResult;

pub struct CargoEnv {
    pub root: Option<PathBuf>,
    pub project: Option<Project>,
    pub installed: Vec<InstalledBinary>,
    installed_map: HashMap<String, String>,
}

/// The current cargo environment (installed binaries and current project, if any)
impl CargoEnv {
    pub fn new(root: Option<PathBuf>) -> Self {
        let project = match root.clone() {
            Some(p) => Project::from(p),
            None => None,
        };

        Self {
            root,
            project,
            installed: Vec::new(),
            installed_map: HashMap::new(),
        }
    }

    /// Reads the current Cargo environment and updates the internal state.
    pub fn read(&mut self) -> AppResult<()> {
        if let Some(root) = &self.root {
            if self.project.is_none() {
                self.project = Project::from(root.clone());
            }
        }

        self.installed = get_installed_binaries().ok().unwrap_or_default();

        self.installed_map = self.installed
            .iter()
            .map(|bin| (bin.name.clone(), bin.version.clone()))
            .collect();

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        Ok(())
    }

    pub fn get_installed_version(&self, name: &str) -> Option<String> {
        self.installed_map.get(name).cloned()
    }
}
