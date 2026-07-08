# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-09

### Added

- Feature-flag support: a crate's features are shown in the details pane and can be
  selected when adding or installing, building the matching `cargo add` / `cargo install`
  command (passing `--no-default-features` when a default feature is unchecked).
- Confirmation prompt before removing or uninstalling a crate.
- Enhanced the details pane to show more info like features, version history, license, size, MSRV, releases, yank
  markers and more...
- Key commands now work while the Help screen is open.
- Prebuilt binaries for Linux (arm64) and Windows (arm64).

### Changed

- Default search scope is now "All".
- The crate details pane is larger and scrollable.
- Cargo operations run off the UI event loop, so the interface stays responsive while
  adding, removing, installing, or uninstalling.
- Cargo commands target the correct manifest and are guarded by the crate's current state.
- Upgraded the ratatui UI stack to 0.30.
- Modernized dependencies and overhauled error handling.
- Component-update failures are now non-fatal.
- Better status bar notifications
- Switched TLS to rustls with the OS trust store plus bundled roots, dropping the runtime dependency on system OpenSSL.
- Linux binaries are now statically linked (musl), so they run on any distribution regardless of glibc version.

### Fixed

- Search pagination no longer exceeds crates.io's serving limits.
- Pagination underflow on page 0.
- Button focus no longer switches unintuitively when some buttons are hidden.
- Help screen scrolling is bounded to the actual content height.
- Hardened color/style and config parsing against malformed input.

## [0.1.0] - 2025-04-23

- Initial release.

[Unreleased]: https://github.com/tareqimbasher/cargo-seek/compare/v0.2.0...HEAD

[0.2.0]: https://github.com/tareqimbasher/cargo-seek/compare/v0.1.0...v0.2.0

[0.1.0]: https://github.com/tareqimbasher/cargo-seek/releases/tag/v0.1.0
