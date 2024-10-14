use crate::cargo::cargo_manager::CargoManager;
use crate::cargo::metadata::InstalledPackage;
use crate::cargo::project::Project;
use crate::errors::AppResult;
use std::path::PathBuf;

pub struct CargoEnv {
    pub root: Option<PathBuf>,
    pub project: Option<Project>,
    pub installed: Vec<InstalledPackage>,
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
        }
    }

    pub fn read(&mut self) -> AppResult<()> {
        if let Some(root) = &self.root {
            if self.project.is_none() {
                self.project = Project::from(root.clone());
            }
        }

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        self.installed = CargoManager::get_globally_installed()
            .ok()
            .unwrap_or_default();

        Ok(())
    }
}
