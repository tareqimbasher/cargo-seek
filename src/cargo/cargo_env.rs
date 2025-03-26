use std::collections::HashSet;
use std::path::PathBuf;

use crate::cargo::{get_installed_binaries, InstalledBinary, Project};
use crate::errors::AppResult;

pub struct CargoEnv {
    pub root: Option<PathBuf>,
    pub project: Option<Project>,
    pub installed: Vec<InstalledBinary>,
    installed_bin_names: HashSet<String>,
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
            installed_bin_names: HashSet::new(),
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

        self.installed_bin_names =
            HashSet::from_iter(self.installed.iter().map(|i| i.name.clone()));

        if let Some(project) = self.project.as_mut() {
            project.read().ok();
        }

        Ok(())
    }

    /// Checks if a given binary is installed.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the binary to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the binary name is found in the list of installed binaries,
    /// otherwise `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// let installed = manager.is_installed("my_binary");
    /// if installed {
    ///     println!("Binary is installed!");
    /// } else {
    ///     println!("Binary is not installed!");
    /// }
    /// ```
    pub fn is_installed(&self, name: &str) -> bool {
        self.installed_bin_names.contains(name)
    }
}
