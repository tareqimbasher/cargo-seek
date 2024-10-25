use std::path::PathBuf;
use std::process::Command;

use crate::cargo::metadata::{InstalledBinary, Metadata};
use crate::errors::AppResult;

pub struct CargoManager;

impl CargoManager {
    pub fn get_installed_binaries() -> AppResult<Vec<InstalledBinary>> {
        let output = Command::new("cargo")
            .arg("install")
            .arg("--list")
            .output()?;

        let stdout = String::from_utf8(output.stdout)?;
        let stdout = stdout
            .split('\n')
            .filter(|l| !l.is_empty() && !l.starts_with(' ') && l.contains("v"))
            .collect::<Vec<_>>();

        let mut packages: Vec<InstalledBinary> = Vec::new();

        for line in stdout.iter() {
            let parts = line.split(' ').collect::<Vec<_>>();
            if parts.len() != 2 {
                continue;
            }

            let name = parts[0].to_string();
            let version = parts[1].to_string();

            packages.push(InstalledBinary {
                name,
                version: version[1..(version.len() - 1)].to_string(),
            })
        }

        Ok(packages)
    }

    pub fn get_metadata(manifest_path: &PathBuf) -> AppResult<Metadata> {
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
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_globally_installed() {
//         let packages = CargoManager::get_installed_binaries().unwrap();
//         for p in packages {
//             println!("{} v{}", p.name, p.version);
//         }
//     }
// }
