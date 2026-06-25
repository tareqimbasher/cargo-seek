#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use color_eyre::eyre::WrapErr;

use crate::cargo::CargoError;
use crate::errors::AppResult;

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
        .output()
        .wrap_err("failed to run `cargo metadata`")?;

    if !output.status.success() {
        return Err(CargoError::Failed {
            command: "metadata".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
        .into());
    }

    let stdout =
        String::from_utf8(output.stdout).wrap_err("`cargo metadata` produced invalid UTF-8")?;
    serde_json::from_str(&stdout).wrap_err("failed to parse `cargo metadata` output")
}

pub fn get_installed_binaries() -> AppResult<Vec<InstalledBinary>> {
    let output = cargo_cmd()
        .arg("install")
        .arg("--list")
        .output()
        .wrap_err("failed to run `cargo install --list`")?;

    if !output.status.success() {
        return Err(CargoError::Failed {
            command: "install --list".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
        .into());
    }

    let stdout = String::from_utf8(output.stdout)
        .wrap_err("`cargo install --list` produced invalid UTF-8")?;
    Ok(parse_installed_binaries(&stdout))
}

/// Parses the output of `cargo install --list`.
///
/// Each installed package is a non-indented header line of the form
/// `"<name> v<version>[ (<source>)]:"`, followed by indented lines listing the binaries it
/// provides (which we ignore here).
fn parse_installed_binaries(stdout: &str) -> Vec<InstalledBinary> {
    let mut packages = Vec::new();

    for line in stdout.lines() {
        // Skip blank lines and the indented binary names listed under each package.
        if line.is_empty() || line.starts_with([' ', '\t']) {
            continue;
        }

        // Header format: "<name> v<version>[ (<source>)]:". Take the name and version
        // tokens and ignore any trailing source suffix.
        let mut parts = line.split_whitespace();
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            continue;
        };

        // The version token is like "v1.2.3", with a trailing ":" when the package has no
        // source suffix. Require the leading "v", then drop a trailing ":".
        let Some(version) = version.strip_prefix('v') else {
            continue;
        };
        let version = version.trim_end_matches(':');
        if version.is_empty() {
            continue;
        }

        packages.push(InstalledBinary {
            name: name.to_string(),
            version: version.to_string(),
        });
    }

    packages
}

pub fn add(crate_name: &str, version: Option<String>, print_output: bool) -> AppResult<()> {
    let crate_name = match version {
        Some(v) => format!("{crate_name}@{v}"),
        None => crate_name.to_string(),
    };

    let args = vec!["add", &crate_name];

    if print_output {
        run_cargo(args)
    } else {
        run_cargo_suppress_output(args).map(|_| ())
    }
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
    let command = args.first().copied().unwrap_or("cargo").to_string();

    // The TUI is down for the duration, so cargo inherits the real terminal and renders with full
    // color and live progress. Don't pipe/capture here — that disables cargo's color; the user is
    // watching, so the exit status alone drives success/failure.
    let status = cargo_cmd()
        .args(args)
        .status()
        .wrap_err("failed to run cargo")?;

    if !status.success() {
        return Err(CargoError::Failed {
            command,
            stderr: String::new(),
        }
        .into());
    }

    Ok(())
}

fn run_cargo_suppress_output(args: Vec<&str>) -> AppResult<String> {
    let command = args.first().copied().unwrap_or("cargo").to_string();

    let output = cargo_cmd()
        .args(args)
        .output()
        .wrap_err("failed to run cargo")?;
    let stderr =
        String::from_utf8(output.stderr).wrap_err("cargo wrote invalid UTF-8 to stderr")?;

    if !output.status.success() {
        return Err(CargoError::Failed { command, stderr }.into());
    }

    Ok(stderr)
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn bin(name: &str, version: &str) -> InstalledBinary {
        InstalledBinary {
            name: name.to_string(),
            version: version.to_string(),
        }
    }

    #[test]
    fn parses_standard_output() {
        let stdout = "cargo-seek v0.1.0:\n    cargo-seek\nripgrep v14.1.0:\n    rg\n";
        assert_eq!(
            parse_installed_binaries(stdout),
            vec![bin("cargo-seek", "0.1.0"), bin("ripgrep", "14.1.0")]
        );
    }

    #[test]
    fn parses_packages_with_a_source_suffix() {
        // Git/path installs carry a parenthesised source before the trailing colon, so the
        // version token no longer has the ":" attached. This form must still parse.
        let stdout = "foo v0.2.0 (https://github.com/x/y#abc123):\n    foo\n";
        assert_eq!(parse_installed_binaries(stdout), vec![bin("foo", "0.2.0")]);
    }

    #[test]
    fn ignores_blank_and_indented_lines() {
        assert!(parse_installed_binaries("").is_empty());
        assert!(parse_installed_binaries("\n\n    rg\n    other\n").is_empty());
    }

    #[test]
    fn skips_malformed_header_lines() {
        // A line without a "v"-prefixed version token is skipped, not panicked on.
        let stdout = "weird-line-without-version\nripgrep v14.1.0:\n";
        assert_eq!(
            parse_installed_binaries(stdout),
            vec![bin("ripgrep", "14.1.0")]
        );
    }
}
