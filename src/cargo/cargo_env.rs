use std::path::PathBuf;

use tracing::warn;

use crate::cargo::{InstalledBinary, Project, get_installed_binaries};

/// The current cargo environment (installed binaries and current project, if any)
pub struct CargoEnv {
    pub project: Option<Project>,
    pub installed_binaries: Vec<InstalledBinary>,
    project_dir: Option<PathBuf>,
}

/// Snapshot returned by [`CargoEnv::gather`] and consumed by [`CargoEnv::apply`].
pub struct GatheredEnv {
    installed_binaries: Vec<InstalledBinary>,
    project: Option<Project>,
}

impl CargoEnv {
    pub fn new(project_dir: Option<PathBuf>) -> Self {
        Self {
            project_dir,
            project: None,
            installed_binaries: Vec::new(),
        }
    }

    /// Runs the blocking cargo subprocesses (`cargo install --list`, `cargo metadata`) that
    /// populate the environment. Pass the existing `project` (if any) so a transient manifest
    /// read failure retains the last-good data rather than clearing it.
    pub fn gather(project_dir: Option<PathBuf>, project: Option<Project>) -> GatheredEnv {
        let installed_binaries = get_installed_binaries().unwrap_or_else(|err| {
            warn!("failed to list installed binaries: {err:#}");
            Vec::new()
        });

        let project = project
            .or_else(|| project_dir.as_deref().and_then(Project::from))
            .map(|mut project| {
                if let Err(err) = project.read() {
                    warn!("failed to read project manifest: {err:#}");
                }
                project
            });

        GatheredEnv {
            installed_binaries,
            project,
        }
    }

    /// Stores a [`GatheredEnv`]. No I/O.
    pub fn apply(&mut self, gathered: GatheredEnv) {
        self.installed_binaries = gathered.installed_binaries;
        self.project = gathered.project;
    }

    /// Gathers and applies the environment inline. Blocks on the cargo subprocesses, so use only
    /// before the UI is up; the running app refreshes off the event-loop task instead.
    pub fn refresh_blocking(&mut self) {
        let gathered = Self::gather(self.project_dir.clone(), self.project.take());
        self.apply(gathered);
    }

    pub fn project_dir(&self) -> Option<PathBuf> {
        self.project_dir.clone()
    }

    /// Gets the installed version of the given crate name if it is installed, None otherwise.
    pub fn get_installed_version(&self, name: &str) -> Option<String> {
        self.installed_binaries
            .iter()
            .find(|binary| binary.name == name)
            .map(|binary| binary.version.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn binary(name: &str, version: &str) -> InstalledBinary {
        InstalledBinary {
            name: name.into(),
            version: version.into(),
        }
    }

    fn env(installed_binaries: Vec<InstalledBinary>) -> CargoEnv {
        CargoEnv {
            project: None,
            installed_binaries,
            project_dir: None,
        }
    }

    #[test]
    fn get_installed_version_returns_the_version_of_an_installed_binary() {
        let env = env(vec![
            binary("ripgrep", "14.1.0"),
            binary("fd-find", "10.2.0"),
        ]);
        assert_eq!(
            env.get_installed_version("ripgrep"),
            Some("14.1.0".to_string())
        );
    }

    #[test]
    fn get_installed_version_is_none_when_not_installed() {
        let env = env(vec![binary("ripgrep", "14.1.0")]);
        assert_eq!(env.get_installed_version("bat"), None);
    }
}
