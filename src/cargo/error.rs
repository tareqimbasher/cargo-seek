//! Error type for failures from the `cargo` subprocess.

/// A failure from shelling out to the `cargo` CLI.
///
/// Most fallible app code propagates type-erased [`eyre::Report`]s, but cargo failures stay
/// concrete so the app can recognise them (via [`Report::downcast_ref`]) and surface cargo's own
/// diagnostics to the user instead of a generic "command failed" message.
///
/// [`eyre::Report`]: color_eyre::eyre::Report
/// [`Report::downcast_ref`]: color_eyre::eyre::Report::downcast_ref
#[derive(thiserror::Error, Debug)]
pub enum CargoError {
    /// The `cargo` subprocess ran but exited unsuccessfully.
    ///
    /// `stderr` holds whatever the subprocess wrote to standard error. It may be empty if cargo
    /// failed without printing diagnostics.
    #[error("`cargo {command}` failed")]
    Failed { command: String, stderr: String },
}

impl CargoError {
    /// A concise, single-line description suitable for the one-line status bar.
    ///
    /// Prefers cargo's own headline `error:` line, falls back to the last non-empty line of
    /// stderr, and finally to the generic [`Display`](std::fmt::Display) message.
    pub fn summary(&self) -> String {
        match self {
            CargoError::Failed { command, stderr } => {
                summarize_stderr(stderr).unwrap_or_else(|| format!("`cargo {command}` failed"))
            }
        }
    }
}

/// Picks the most informative single line out of cargo's stderr.
///
/// Cargo prints progress lines (`Updating`, `Compiling`, …) and, when something goes wrong, an
/// `error: <message>` headline. We surface that headline (with the redundant marker stripped, since
/// callers add their own framing); absent one, we fall back to the last non-empty line.
fn summarize_stderr(stderr: &str) -> Option<String> {
    let mut last = None;
    for line in stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(message) = strip_error_prefix(line) {
            return Some(message.to_string());
        }
        last = Some(line.to_string());
    }
    last
}

/// Strips cargo's leading `error:` / `error[E0123]:` marker, returning the message body. Returns
/// `None` for lines that are not error headlines (so `"errored"` is not mistaken for one).
fn strip_error_prefix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("error")?;
    // Skip an optional `[E0123]` code, then require the colon so plain words starting with
    // "error" are not matched.
    let rest = rest.trim_start_matches(|c: char| c != ':');
    Some(rest.strip_prefix(':')?.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn failed(stderr: &str) -> CargoError {
        CargoError::Failed {
            command: "add".to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn summary_prefers_the_error_headline_over_progress_lines() {
        let stderr = "    Updating crates.io index\nerror: the crate `nope` could not be found\n";
        assert_eq!(
            failed(stderr).summary(),
            "the crate `nope` could not be found"
        );
    }

    #[test]
    fn summary_handles_rustc_style_error_codes() {
        assert_eq!(
            failed("error[E0432]: unresolved import").summary(),
            "unresolved import"
        );
    }

    #[test]
    fn summary_falls_back_to_the_last_non_empty_line() {
        assert_eq!(
            failed("    Updating crates.io index\n    Blocking waiting\n\n").summary(),
            "Blocking waiting"
        );
    }

    #[test]
    fn summary_does_not_match_words_merely_starting_with_error() {
        // No colon -> not an error headline; falls back to the (only) line.
        assert_eq!(
            failed("errored out somewhere").summary(),
            "errored out somewhere"
        );
    }

    #[test]
    fn summary_falls_back_to_command_when_stderr_is_empty() {
        assert_eq!(failed("").summary(), "`cargo add` failed");
    }
}
