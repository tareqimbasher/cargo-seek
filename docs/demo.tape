# cargo-seek demo — rendered with VHS (https://github.com/charmbracelet/vhs).
#
# Build the current version first, then record from the repository root:
#
#   cargo build --release
#   vhs docs/demo.tape
#
# The tape prepends ./target/release to PATH (in a hidden step) so `cargo seek` runs the
# freshly built binary instead of any globally installed copy, and it runs against this
# project's Cargo.toml so serde/serde_json show up as dependencies. The GIF is written to
# docs/preview.gif.
#
# The walkthrough only opens overlays and cancels them, so recording never modifies Cargo.toml.

Require cargo

Output docs/preview.gif

Set FontSize 14
Set Width 1200
Set Height 700
Set Padding 0
Set Margin 20
Set CursorBlink true
Set WindowBar Colorful
Set TypingSpeed 90ms

# Use the locally built binary rather than a globally installed cargo-seek.
Hide
Type "export PATH=$PWD/target/release:$PATH" Enter
Type "clear" Enter
Show

Type "cargo seek" Enter
Sleep 1.5s

# Search: the exact match auto-selects "serde" and focuses the results list.
Type "serde"
Enter
Sleep 2s

# Rich details pane: versions, license, MSRV, size, features.
Sleep 2.5s

# Sort results.
Ctrl+S
Sleep 1s
Down@250ms 3
Sleep 1s
Escape
Sleep 0.75s

# Change the search scope.
Ctrl+A
Sleep 1s
Down@250ms 2
Sleep 1s
Escape
Sleep 0.75s

# Confirmation prompt before removing a project dependency (serde is a dependency here).
Type "r"
Sleep 2s
Escape
Sleep 0.75s

# Feature picker: search for a crate that isn't a dependency yet, then add it and choose features.
# Add ("a") only opens the picker for a crate that isn't already a dependency, hence the fresh
# search for "rand" rather than reusing serde.
Escape
Sleep 0.5s
Backspace 8
Type "rand"
Enter
Sleep 2s
Type "a"
Sleep 1.5s
Down@300ms 2
Space
Sleep 0.75s
Down@300ms 1
Space
Sleep 1.5s
Escape
Sleep 1.5s
