use std::collections::HashMap;
use std::path::PathBuf;

use crate::cargo::{InstalledBinary, Project, get_installed_binaries};
use crate::errors::AppResult;

/// The current cargo environment (installed binaries and current project, if any)
pub struct CargoEnv {
    pub project: Option<Project>,
    pub installed_binaries: Vec<InstalledBinary>,
    project_dir: Option<PathBuf>,
    installed_binary_versions: HashMap<String, String>,
}

impl CargoEnv {
    pub fn new(project_dir: Option<PathBuf>) -> Self {
        Self {
            project_dir,
            project: None,
            installed_binaries: Vec::new(),
            installed_binary_versions: HashMap::new(),
        }
    }

    /// Reads the current Cargo environment and updates the internal state.
    pub fn read(&mut self) -> AppResult<()> {
        self.installed_binaries = get_installed_binaries().ok().unwrap_or_default();

        self.installed_binary_versions = self
            .installed_binaries
            .iter()
            .map(|bin| (bin.name.clone(), bin.version.clone()))
            .collect();

        if self.project.is_none()
            && let Some(project_dir) = &self.project_dir
        {
            self.project = Project::from(project_dir);
        }

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        Ok(())
    }

    /// Gets the installed version of the given crate name if it is installed, None otherwise.
    pub fn get_installed_version(&self, name: &str) -> Option<String> {
        self.installed_binary_versions.get(name).cloned()
    }
}
