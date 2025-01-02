use std::collections::HashSet;
use std::path::PathBuf;

use crate::cargo::cargo_manager::CargoManager;
use crate::errors::AppResult;
use crate::models::InstalledBinary;
use crate::models::Project;

pub struct CargoEnv {
    pub root: Option<PathBuf>,
    pub project: Option<Project>,
    pub installed: Vec<InstalledBinary>,
    installed_bin_names: HashSet<String>,
}

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
            installed_bin_names: HashSet::new(),
        }
    }

    pub fn read(&mut self) -> AppResult<()> {
        if let Some(root) = &self.root {
            if self.project.is_none() {
                self.project = Project::from(root.clone());
            }
        }

        self.installed = CargoManager::get_installed_binaries()
            .ok()
            .unwrap_or_default();

        self.installed_bin_names =
            HashSet::from_iter(self.installed.iter().map(|i| i.name.clone()));

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        Ok(())
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.installed_bin_names.contains(name)
    }
}
