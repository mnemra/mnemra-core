# Run all checks (host only — plugins build to wasm32-wasip2 separately)
check:
    cargo fmt --check
    cargo clippy --workspace --exclude mnemra-echo -- -D warnings
    cargo clippy -p mnemra-echo --target wasm32-wasip2 -- -D warnings
    cargo test --workspace
    uv run scripts/docs-llms.py --check
    uv run --with pytest pytest tests/test_docs_llms.py

# Format code
fmt:
    cargo fmt

# Run tests (host)
test:
    cargo test

# Generate docs/_published/llms.txt + docs/_published/llms-full.txt from docs/src/.
# Requires: uv on PATH.
docs-llms:
    uv run scripts/docs-llms.py

# Build the docs site.
# Requires: mdbook, mdbook-mermaid, mdbook-d2, and the d2 CLI on PATH.
# Install via: cargo install --locked mdbook mdbook-mermaid mdbook-d2
# d2 CLI: https://d2lang.com/tour/install
docs:
    mdbook build docs/

# Serve the docs site locally with live reload.
docs-serve:
    mdbook serve docs/

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
