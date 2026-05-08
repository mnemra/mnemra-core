# Run all checks (host only — plugins build to wasm32-wasip2 separately)
check:
    cargo fmt --check
    cargo clippy --workspace --exclude mnemra-echo -- -D warnings
    cargo clippy -p mnemra-echo --target wasm32-wasip2 -- -D warnings
    cargo test --workspace

# Format code
fmt:
    cargo fmt

# Run tests (host)
test:
    cargo test

# Build the docs site (builds mdbook-d2 preprocessor first)
docs:
    cargo build --release -p mdbook-d2
    env PATH="{{justfile_directory()}}/target/release:${PATH}" mdbook build docs/

# Serve the docs site locally with live reload (builds mdbook-d2 preprocessor first)
docs-serve:
    cargo build --release -p mdbook-d2
    env PATH="{{justfile_directory()}}/target/release:${PATH}" mdbook serve docs/

# Build host (debug)
build:
    cargo build

# Build host (release)
release:
    cargo build --release

# Build plugin to wasm32-wasip2 component (release)
plugin:
    cargo build --release -p mnemra-echo --target wasm32-wasip2

# Build plugin then run host (the spike)
run: plugin
    cargo run --release -p mnemra

# Inspect the plugin component's WIT
plugin-wit: plugin
    wasm-tools component wit target/wasm32-wasip2/release/mnemra_echo.wasm
