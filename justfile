# List available recipes
default:
    @just --list

# Run the app; pass flags after the recipe, e.g. `just run -s tokio`
run *args:
    cargo run -- {{args}}

# Type-check without producing binaries
check:
    cargo check

# Build the debug binary
build:
    cargo build

# Build the size-optimized release binary
build-release:
    cargo build --release

# Format all code in place
format:
    cargo fmt --all

# Check formatting without writing changes
format-check:
    cargo fmt --all --check

# Lint with warnings denied
clippy:
    cargo clippy --all-targets --all-features --workspace -- -D warnings

# Run the tests; pass a name to filter, e.g. `just test page_count`
test *args:
    cargo test --locked --all-features --workspace {{args}}

# Build the docs with warnings denied
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples

# Run every CI gate — do this before pushing
ci: format-check clippy test doc

# Remove build artifacts
clean:
    cargo clean
