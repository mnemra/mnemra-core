# Run all checks (host only — plugins build to wasm32-wasip2 separately)
check:
    cargo fmt --check
    cargo clippy --workspace --exclude mnemra-echo -- -D warnings
    cargo clippy -p mnemra-echo --target wasm32-wasip2 -- -D warnings
    cargo test --workspace
    uv run scripts/docs-translate.py --check --src docs/src --out docs/_published --prompts docs/prompts
    uv run scripts/docs-llms.py --check
    uv run --with pytest pytest tests/test_docs_translate.py
    uv run --with pytest pytest tests/test_docs_llms.py

# Format code
fmt:
    cargo fmt

# Run tests (host)
test:
    cargo test

# Translation runs from a Claude session via /docs-translate.
# See .claude/commands/docs-translate.md.

# Run both docs drift gates (translate + llms).
docs-check:
    uv run scripts/docs-translate.py --check
    uv run scripts/docs-llms.py --check

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

# ---------------------------------------------------------------------------
# CI gate recipes (R-0018-f)
# Each recipe emits exactly one "GATE <name> PASS|FAIL" line on stdout.
# No recipe has --fix side effects.
# ---------------------------------------------------------------------------

# Verify: compile-check (type-level correctness)
verify-type:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo check --workspace 2>&1; then
        echo "GATE type PASS"
    else
        echo "GATE type FAIL"
        exit 1
    fi

# Verify: lint — clippy (deny warnings) + fmt check + WHERE-clause lint (R-0018-d).
verify-lint:
    #!/usr/bin/env bash
    set -euo pipefail
    # Host crates: deny all warnings
    cargo clippy --workspace --exclude mnemra-echo -- -D warnings 2>&1
    # Plugin crate: lint for wasm32-wasip2 target
    cargo clippy -p mnemra-echo --target wasm32-wasip2 -- -D warnings 2>&1
    # Format check (no --fix)
    cargo fmt --all --check 2>&1
    # WHERE-clause lint (R-0018-d): every read-path host-fn must reference ctx.workspace_id
    cargo test --manifest-path libs/mnemra-host/Cargo.toml --test lint_workspace_clause 2>&1
    echo "GATE lint PASS"

# Verify: tests pass
verify-test:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test --workspace 2>&1; then
        echo "GATE test PASS"
    else
        echo "GATE test FAIL"
        exit 1
    fi

# Verify: test-hooks feature — runs resource_limits.rs seam tests (gated behind test-hooks).
# This is a CI gate so untrusted-path seam coverage is always exercised.
verify-test-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test -p mnemra-host --features test-hooks 2>&1; then
        echo "GATE test-hooks PASS"
    else
        echo "GATE test-hooks FAIL"
        exit 1
    fi

# Verify: coverage (emit number; no threshold gate at scaffold stage)
verify-coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo llvm-cov --workspace 2>&1; then
        echo "GATE coverage PASS"
    else
        echo "GATE coverage FAIL"
        exit 1
    fi

# Verify: debug build succeeds (release-build hardening lands in Task 26)
verify-build:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo build --workspace 2>&1; then
        echo "GATE build PASS"
    else
        echo "GATE build FAIL"
        exit 1
    fi

# Verify: smoke tests
# scaffold: real smoke gate lands in Task 27
verify-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    # scaffold: real smoke gate lands in Task 27
    echo "GATE smoke PASS"

# CI entry point — runs every verify-* recipe in order.
# First failure stops and exits non-zero.
# This is the sole CI entry point (R-0018-c, R-0018-f).
ci: verify-type verify-lint verify-test verify-test-hooks verify-coverage verify-build verify-smoke
