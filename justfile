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
    # Run every lint check; any failure routes to the FAIL branch so the GATE
    # contract (exactly one GATE line + correct exit) holds. A command in the
    # `if` condition is exempt from errexit, so a failing check reaches the else
    # branch rather than aborting — same shape as verify-type / verify-test.
    if cargo clippy --workspace --exclude mnemra-echo -- -D warnings 2>&1 \
        && cargo clippy -p mnemra-echo --target wasm32-wasip2 -- -D warnings 2>&1 \
        && cargo fmt --all --check 2>&1 \
        && cargo test --manifest-path libs/mnemra-host/Cargo.toml --test lint_workspace_clause 2>&1; then
        # Checks, in order: host-crate clippy (deny warnings); plugin clippy for
        # wasm32-wasip2; fmt check (no --fix); WHERE-clause lint (R-0018-d) — every
        # read-path host-fn must reference ctx.workspace_id.
        echo "GATE lint PASS"
    else
        echo "GATE lint FAIL"
        exit 1
    fi

# Verify: tests pass
# Depends on `plugin`: the e2e tests load target/wasm32-wasip2/release/mnemra_echo.wasm,
# which the host build does not produce — build the guest component first.
verify-test: plugin
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
# Depends on `plugin` for the same wasm-artifact reason as verify-test.
verify-test-hooks: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test -p mnemra-host --features test-hooks 2>&1; then
        echo "GATE test-hooks PASS"
    else
        echo "GATE test-hooks FAIL"
        exit 1
    fi

# Verify: coverage (emit number; no threshold gate at scaffold stage)
# Depends on `plugin` for the same wasm-artifact reason as verify-test.
verify-coverage: plugin
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
    if cargo build --workspace --exclude mnemra-echo 2>&1; then
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
    # scaffold: the `true` placeholder always passes; Task 27 swaps it for the real
    # end-to-end smoke command. The FAIL branch holds the GATE contract shape
    # (one GATE line + correct exit) so Task 27 only swaps the condition.
    # NOTE: the FAIL branch is unreachable until Task 27 supplies a fallible command.
    if true; then
        echo "GATE smoke PASS"
    else
        echo "GATE smoke FAIL"
        exit 1
    fi

# CI entry point — runs every verify-* recipe in order.
# First failure stops and exits non-zero.
# This is the sole CI entry point (R-0018-c, R-0018-f).
ci: verify-type verify-lint verify-test verify-test-hooks verify-coverage verify-build verify-smoke

# ---------------------------------------------------------------------------
# Gate-shape self-test (Test Expectation 54)
# Proves the GATE contract mechanically in BOTH directions: a passing gate
# emits "GATE <name> PASS" + exit 0, a failing gate emits "GATE <name> FAIL"
# + non-zero exit. Deliberately NOT named verify-* and NOT in the `ci` chain,
# so `just ci` stays green (Test Expectation 53). Run it directly:
#   just test-gate-shape
# ---------------------------------------------------------------------------

# Stub gate that always passes — mirrors the real verify-* PASS path.
_gate-stub-pass:
    #!/usr/bin/env bash
    set -euo pipefail
    if true; then
        echo "GATE stub PASS"
    else
        echo "GATE stub FAIL"
        exit 1
    fi

# Stub gate that always fails — mirrors the real verify-* FAIL path.
# Proves a failing condition routes to the FAIL echo (the GATE-contract shape).
_gate-stub-fail:
    #!/usr/bin/env bash
    set -euo pipefail
    if false; then
        echo "GATE stub PASS"
    else
        echo "GATE stub FAIL"
        exit 1
    fi

# Captures each stub's real exit status WITHOUT errexit (set +e around the
# invocation) so a failing stub reaches the assertions instead of aborting the
# harness — the same O1 "read the child's real exit status" discipline the gate
# recipes themselves must follow.
#
# Gate-shape self-test — assert both PASS and FAIL gate directions (Test Exp. 54).
test-gate-shape:
    #!/usr/bin/env bash
    set -uo pipefail
    rc=0

    # PASS direction: expect "GATE stub PASS" on stdout AND exit 0.
    set +e
    pass_out="$(just _gate-stub-pass 2>&1)"
    pass_code=$?
    set -e
    if [[ "$pass_out" == *"GATE stub PASS"* && "$pass_code" -eq 0 ]]; then
        echo "ok: PASS direction — emitted PASS line, exit 0"
    else
        echo "FAIL: PASS direction — out=[$pass_out] code=$pass_code"
        rc=1
    fi

    # FAIL direction: expect "GATE stub FAIL" on stdout AND non-zero exit.
    set +e
    fail_out="$(just _gate-stub-fail 2>&1)"
    fail_code=$?
    set -e
    if [[ "$fail_out" == *"GATE stub FAIL"* && "$fail_code" -ne 0 ]]; then
        echo "ok: FAIL direction — emitted FAIL line, exit $fail_code"
    else
        echo "FAIL: FAIL direction — out=[$fail_out] code=$fail_code"
        rc=1
    fi

    # Guard: a passing stub must NOT emit a FAIL line, and vice versa.
    if [[ "$pass_out" == *"GATE stub FAIL"* ]]; then
        echo "FAIL: PASS stub leaked a FAIL line"
        rc=1
    fi
    if [[ "$fail_out" == *"GATE stub PASS"* ]]; then
        echo "FAIL: FAIL stub leaked a PASS line"
        rc=1
    fi

    if [[ "$rc" -eq 0 ]]; then
        echo "GATE-SHAPE SELF-TEST PASS"
    else
        echo "GATE-SHAPE SELF-TEST FAIL"
    fi
    exit "$rc"
