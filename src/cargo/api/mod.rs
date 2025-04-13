use std::io::{BufRead, BufReader};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::errors::{AppError, AppResult};

mod installed_binary;
mod manifest_metadata;

pub use installed_binary::*;
pub use manifest_metadata::*;

pub fn get_metadata(manifest_path: &PathBuf) -> AppResult<ManifestMetadata> {
    let output = cargo_cmd()
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(manifest_path)
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let metadata: ManifestMetadata = serde_json::from_str(&stdout)?;
    Ok(metadata)
}

pub fn get_installed_binaries() -> AppResult<Vec<InstalledBinary>> {
    let output = cargo_cmd().arg("install").arg("--list").output()?;

    let stdout = String::from_utf8(output.stdout)?;
    let stdout = stdout
        .split('\n')
        .filter(|l| !l.is_empty() && !l.starts_with(' ') && l.contains(" v"))
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

pub fn add(mut crate_name: String, version: Option<String>, print_output: bool) -> AppResult<()> {
    if let Some(version) = version {
        crate_name = format!("{crate_name}@{version}");
    }

    if print_output {
        run_cargo(vec!["add", crate_name.as_str()])?;
    } else {
        run_cargo_suppress_output(vec!["add", crate_name.as_str()])?;
    }

    Ok(())
}

pub fn remove(crate_name: String, print_output: bool) -> AppResult<()> {
    if print_output {
        run_cargo(vec!["remove", crate_name.as_str()])?;
    } else {
        run_cargo_suppress_output(vec!["remove", crate_name.as_str()])?;
    }
    Ok(())
}

pub fn install(
    mut crate_name: String,
    version: Option<String>,
    print_output: bool,
) -> AppResult<()> {
    if let Some(version) = version {
        crate_name = format!("{crate_name}@{version}");
    }

    if print_output {
        run_cargo(vec!["install", "--locked", crate_name.as_str()])?;
    } else {
        run_cargo_suppress_output(vec!["install", "--locked", crate_name.as_str()])?;
    }

    Ok(())
}

pub fn uninstall(crate_name: String, print_output: bool) -> AppResult<()> {
    if print_output {
        run_cargo(vec!["uninstall", crate_name.as_str()])?;
    } else {
        run_cargo_suppress_output(vec!["uninstall", crate_name.as_str()])?;
    }
    Ok(())
}

fn run_cargo(args: Vec<&str>) -> AppResult<()> {
    let mut cmd = cargo_cmd();
    cmd.args(args);

    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stderr = child.stderr.take().unwrap();

    // Stream output
    let lines = BufReader::new(stderr).lines();
    for line in lines {
        println!("{}", line?);
    }

    if !child.wait()?.success() {
        return Err(AppError::Cargo("Error running cargo".into()));
    }

    Ok(())
}

fn run_cargo_suppress_output(args: Vec<&str>) -> AppResult<String> {
    let output = cargo_cmd().args(args).output()?;
    if !output.status.success() {
        return Err(AppError::Cargo(String::from_utf8(output.stderr)?));
    }
    Ok(String::from_utf8(output.stderr)?)
}

fn cargo_cmd() -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cargo");
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new("cargo")
    }
}
