use std::collections::HashMap;
use std::path::PathBuf;

use crate::cargo::{get_installed_binaries, InstalledBinary, Project};
use crate::errors::AppResult;

pub struct CargoEnv {
    pub current_dir: Option<PathBuf>,
    pub project: Option<Project>,
    pub installed: Vec<InstalledBinary>,
    installed_versions: HashMap<String, String>,
}

/// The current cargo environment (installed binaries and current project, if any)
impl CargoEnv {
    pub fn new(current_dir: Option<PathBuf>) -> Self {
        Self {
            current_dir,
            project: None,
            installed: Vec::new(),
            installed_versions: HashMap::new(),
        }
    }

    /// Reads the current Cargo environment and updates the internal state.
    pub fn read(&mut self) -> AppResult<()> {
        self.installed = get_installed_binaries().ok().unwrap_or_default();

        self.installed_versions = self
            .installed
            .iter()
            .map(|bin| (bin.name.clone(), bin.version.clone()))
            .collect();

        if self.project.is_none() {
            if let Some(current_dir) = &self.current_dir {
                self.project = Project::from(current_dir);
            }
        }

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        Ok(())
    }

    /// Gets the installed version of the given crate name, if any.
    pub fn get_installed_version(&self, name: &str) -> Option<String> {
        self.installed_versions.get(name).cloned()
    }
}
