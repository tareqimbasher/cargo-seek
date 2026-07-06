use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use color_eyre::eyre::{WrapErr, bail};

use crate::cargo::CargoError;
use crate::errors::AppResult;

mod installed_binary;
mod manifest_metadata;

pub use installed_binary::*;
pub use manifest_metadata::*;

/// Reads workspace metadata for the manifest at `manifest_path`.
///
/// The returned [`CargoMetadata`] lists only the workspace's own packages, not transitive
/// dependencies.
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code,
/// or a generic error if the output is not valid UTF-8, fails to parse, or uses an unsupported
/// format version.
pub fn get_metadata(manifest_path: &Path) -> AppResult<CargoMetadata> {
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
    let metadata: CargoMetadata =
        serde_json::from_str(&stdout).wrap_err("failed to parse `cargo metadata` output")?;

    if metadata.version != 1 {
        bail!(
            "`cargo metadata` returned unsupported format version {} (expected 1)",
            metadata.version
        );
    }

    Ok(metadata)
}

/// Lists the crates whose binaries are installed on this machine.
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code.
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
    Ok(parse_install_list_output(&stdout))
}

/// Parses the output of `cargo install --list`.
///
/// Each installed package is a non-indented header line of the form
/// `"<name> v<version>[ (<source>)]:"`, followed by indented lines listing the binaries it
/// provides (which we ignore here).
fn parse_install_list_output(stdout: &str) -> Vec<InstalledBinary> {
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

/// How a cargo subprocess connects to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Inherit the real terminal so cargo renders with full color and live progress.
    ///
    /// Precondition: the TUI must be down (alternate screen released) — otherwise cargo fights
    /// ratatui over the display.
    Inherit,
    /// Capture stdout/stderr; the TUI stays up and the outcome is surfaced via the status bar.
    Capture,
}

/// Adds a crate to the dependencies in `manifest_path`.
///
/// A set `version` pins the request as `name@version`. `out` controls how cargo connects to the
/// terminal; see [`OutputMode`].
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code.
pub fn add(
    manifest_path: &Path,
    crate_name: &str,
    version: Option<String>,
    features: &[String],
    no_default_features: bool,
    out: OutputMode,
) -> AppResult<()> {
    let args = add_args(
        manifest_path,
        crate_name,
        version.as_deref(),
        features,
        no_default_features,
    );
    run_cargo_with(out, args)
}

fn add_args(
    manifest_path: &Path,
    crate_name: &str,
    version: Option<&str>,
    features: &[String],
    no_default_features: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> =
        vec!["add".into(), "--manifest-path".into(), manifest_path.into()];
    args.push(crate_spec(crate_name, version).into());
    push_feature_args(&mut args, features, no_default_features);
    args
}

/// Removes a crate from the dependencies in `manifest_path`.
///
/// `kind` selects which section to remove from (`[dependencies]`, `[dev-dependencies]`, or
/// `[build-dependencies]`). `out` controls how cargo connects to the terminal; see [`OutputMode`].
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code.
pub fn remove(
    manifest_path: &Path,
    crate_name: String,
    kind: DepKind,
    out: OutputMode,
) -> AppResult<()> {
    run_cargo_with(out, remove_args(manifest_path, &crate_name, kind))
}

fn remove_args(manifest_path: &Path, crate_name: &str, kind: DepKind) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "remove".into(),
        "--manifest-path".into(),
        manifest_path.into(),
        crate_name.into(),
    ];

    if let Some(flag) = kind.cargo_flag() {
        args.push(flag.into());
    }
    args
}

/// A crate spec argument: `name@version` when a version is pinned, else the bare name.
fn crate_spec(crate_name: &str, version: Option<&str>) -> String {
    match version {
        Some(v) => format!("{crate_name}@{v}"),
        None => crate_name.to_string(),
    }
}

/// Appends the `--no-default-features` / `--features a,b` arguments.
fn push_feature_args(args: &mut Vec<OsString>, features: &[String], no_default_features: bool) {
    if no_default_features {
        args.push("--no-default-features".into());
    }
    if !features.is_empty() {
        args.push("--features".into());
        args.push(features.join(",").into());
    }
}

/// Installs a crate's binaries into the user's cargo bin directory.
///
/// A set `version` pins the request as `name@version`. `out` controls how cargo connects to the
/// terminal; see [`OutputMode`].
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code.
pub fn install(
    crate_name: String,
    version: Option<String>,
    features: &[String],
    no_default_features: bool,
    out: OutputMode,
) -> AppResult<()> {
    let args = install_args(
        &crate_name,
        version.as_deref(),
        features,
        no_default_features,
    );
    run_cargo_with(out, args)
}

fn install_args(
    crate_name: &str,
    version: Option<&str>,
    features: &[String],
    no_default_features: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["install".into(), "--locked".into()];
    args.push(crate_spec(crate_name, version).into());
    push_feature_args(&mut args, features, no_default_features);
    args
}

/// Uninstalls a crate's binaries from the user's cargo bin directory.
///
/// `out` controls how cargo connects to the terminal; see [`OutputMode`].
///
/// # Errors
/// Returns [`CargoError::Failed`] with cargo's stderr if the subprocess exits with a non-zero code.
pub fn uninstall(crate_name: String, out: OutputMode) -> AppResult<()> {
    run_cargo_with(out, vec!["uninstall".into(), crate_name.into()])
}

fn run_cargo_with(out: OutputMode, args: Vec<OsString>) -> AppResult<()> {
    match out {
        OutputMode::Inherit => run_cargo(args),
        OutputMode::Capture => run_cargo_captured(args).map(|_| ()),
    }
}

fn run_cargo(args: Vec<OsString>) -> AppResult<()> {
    let command = args
        .first()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| "cargo".to_string());

    // Use `.status()`, not `.output()`: cargo inherits the terminal and keeps its color and live
    // progress (capturing would strip the color). Nothing is captured, so the exit status alone
    // drives success/failure, hence the empty stderr in the error below.
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

fn run_cargo_captured(args: Vec<OsString>) -> AppResult<String> {
    let command = args
        .first()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| "cargo".to_string());

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

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn feats(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_standard_output() {
        let stdout = "cargo-seek v0.1.0:\n    cargo-seek\nripgrep v14.1.0:\n    rg\n";
        assert_eq!(
            parse_install_list_output(stdout),
            vec![bin("cargo-seek", "0.1.0"), bin("ripgrep", "14.1.0")]
        );
    }

    #[test]
    fn parses_packages_with_a_source_suffix() {
        // Git/path installs carry a parenthesised source before the trailing colon, so the
        // version token no longer has the ":" attached. This form must still parse.
        let stdout = "foo v0.2.0 (https://github.com/x/y#abc123):\n    foo\n";
        assert_eq!(parse_install_list_output(stdout), vec![bin("foo", "0.2.0")]);
    }

    #[test]
    fn ignores_blank_and_indented_lines() {
        assert!(parse_install_list_output("").is_empty());
        assert!(parse_install_list_output("\n\n    rg\n    other\n").is_empty());
    }

    #[test]
    fn skips_malformed_header_lines() {
        // A line without a "v"-prefixed version token is skipped, not panicked on.
        let stdout = "weird-line-without-version\nripgrep v14.1.0:\n";
        assert_eq!(
            parse_install_list_output(stdout),
            vec![bin("ripgrep", "14.1.0")]
        );
    }

    #[test]
    fn remove_args_target_the_manifest_and_only_flag_dev_or_build() {
        let path = std::path::Path::new("/proj/Cargo.toml");

        // A normal dep gets no flag — and, crucially, no trailing empty arg.
        assert_eq!(
            strings(remove_args(path, "serde", DepKind::Normal)),
            ["remove", "--manifest-path", "/proj/Cargo.toml", "serde"]
        );
        assert_eq!(
            strings(remove_args(path, "tempfile", DepKind::Dev)),
            [
                "remove",
                "--manifest-path",
                "/proj/Cargo.toml",
                "tempfile",
                "--dev"
            ]
        );
        assert_eq!(
            strings(remove_args(path, "cc", DepKind::Build)),
            [
                "remove",
                "--manifest-path",
                "/proj/Cargo.toml",
                "cc",
                "--build"
            ]
        );
    }

    #[test]
    fn add_args_pin_version_and_pass_features() {
        let path = std::path::Path::new("/proj/Cargo.toml");

        // No version and no features: nothing beyond the bare crate is appended — in particular
        // no `--features` with an empty value.
        assert_eq!(
            strings(add_args(path, "serde", None, &[], false)),
            ["add", "--manifest-path", "/proj/Cargo.toml", "serde"]
        );
        // A pinned version becomes `name@version`.
        assert_eq!(
            strings(add_args(path, "serde", Some("1.0"), &[], false)),
            ["add", "--manifest-path", "/proj/Cargo.toml", "serde@1.0"]
        );
        // Features are comma-joined and pair with `--no-default-features` when requested.
        assert_eq!(
            strings(add_args(
                path,
                "tokio",
                None,
                &feats(&["rt", "macros"]),
                true
            )),
            [
                "add",
                "--manifest-path",
                "/proj/Cargo.toml",
                "tokio",
                "--no-default-features",
                "--features",
                "rt,macros"
            ]
        );
    }

    #[test]
    fn install_args_pin_version_and_pass_features() {
        // Bare install carries `--locked` and nothing else.
        assert_eq!(
            strings(install_args("ripgrep", None, &[], false)),
            ["install", "--locked", "ripgrep"]
        );
        // Version pin plus a single feature (no `--no-default-features`).
        assert_eq!(
            strings(install_args(
                "ripgrep",
                Some("14.1.0"),
                &feats(&["pcre2"]),
                false
            )),
            [
                "install",
                "--locked",
                "ripgrep@14.1.0",
                "--features",
                "pcre2"
            ]
        );
        // `--no-default-features` with multiple comma-joined features.
        assert_eq!(
            strings(install_args("foo", None, &feats(&["a", "b"]), true)),
            [
                "install",
                "--locked",
                "foo",
                "--no-default-features",
                "--features",
                "a,b"
            ]
        );
    }
}
