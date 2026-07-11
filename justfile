# Run all checks (builds the plugin first so integration tests can load the wasm component)
check: plugin
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
# Signing-ceremony producer tooling (round-1b).
# Maintainer-run at the one-shot key ceremony — see docs/runbooks/signing-ceremony.md.
# The runtime host never links or calls this bin.
# ---------------------------------------------------------------------------

# Generate a fresh Ed25519 root keypair. Writes the 32-byte private seed to
# <key_out> (mode 600) and prints the public key hex to stdout (→ ROOT + ROOT_PIN
# in round-2). Place <key_out> OUTSIDE every runtime-read dir (see the runbook).
sign-keygen key_out:
    cargo run -p sign-ceremony --quiet -- keygen {{key_out}}

# Run the signing ceremony: read the private key in place from <key>, BLAKE3-hash
# the committed <wasm>, embed [component], sign the body, populate [signature] in
# <manifest>, self-verify against verify_plugin, and print the real public key hex.
sign-ceremony key wasm manifest:
    cargo run -p sign-ceremony --quiet -- sign {{key}} {{wasm}} {{manifest}}

# ---------------------------------------------------------------------------
# CI gate recipes (R-0018-f)
# Each recipe emits exactly one "GATE <name> PASS|FAIL" line on stdout.
# No recipe has --fix side effects.
# ---------------------------------------------------------------------------

# PG-touching integration test binaries (17 members).
# Defined once; all three verify-* recipes reference this variable so the list
# stays in sync (R-0022: identical serialization directive across all recipes).
# These binaries run at --test-threads 1 to prevent concurrent embedded-Postgres
# teardown races (SIGABRT / signal 6, #1852 / Tier-1 CI-flake fix).
# startup_run_full (T5, #1992, R-0022-a): both scenarios reach real
# start_embedded() (the happy path completes it; the builtin-init injection
# fires after storage init succeeds), so it belongs here.
# artifact_list_paging / artifact_list_paging_whitebox (tier-2 T5, R-0031 AC1):
# both construct an engine via the shared-engine fixture through
# tests/common/paging_harness.rs, so they inherit the same --test-threads 1
# serialization as every other PG member (R-0034). The whitebox binary is
# crate-level `#![cfg(feature = "test-hooks")]`-gated — it stays in this
# shared, feature-agnostic list rather than a feature-scoped side list (a
# side list would break R-0033): structurally zero-test (green) under
# verify-test / verify-coverage, meaningfully active under verify-test-hooks.
PG_TEST_FLAGS := "--test actors_entity --test admin_token --test admin_token_behavior --test artifact_list_paging --test artifact_list_paging_whitebox --test artifact_machinery --test content_schema --test identity_builtins --test invoke_health_gate --test mcp_server --test mcp_slice1_e2e --test mcp_verb_gate --test postgres_engine --test schema_init --test startup_population --test startup_run_full --test storage_contract_postgres --test tenancy_isolation"

# Non-PG integration test binaries (17 members).
# These run at the default thread count — serialization is scoped to PG tests only (R-0021).
# health_listener (T4, #1991, R-0022-b/R-0004-g): uses sqlx::connect_lazy against an
# unreachable address to get a deterministic overall:"down" body — no embedded Postgres
# engine needed, so it belongs here, not in PG_TEST_FLAGS.
# startup_run_ordering (T5, #1992, R-0022-a/-e): every scenario fails before —
# or runs entirely without — real embedded Postgres (5-pre refusal, injected
# storage failure replacing start_embedded(), keystone load-path pair), so it
# belongs here, not in PG_TEST_FLAGS.
# ci_reap_baseline (R-9 baseline-reap mechanism, #2119): drives
# scripts/ci-reap.sh against synthetic marker processes (argv0-renamed real
# `sleep` invocations, never a real embedded-PG engine), so it belongs here,
# not in PG_TEST_FLAGS.
NONPG_TEST_FLAGS := "--test abi_contract --test build_gate --test ci_reap_baseline --test content_hash_binding --test health_listener --test lint_workspace_clause --test llm_key_allowlist --test manifest_load --test mcp_feature_guard --test no_test_seams --test permissions --test plugin_output_validation --test resource_limits --test signing_chain --test startup_run_ordering --test storage_contract --test workspace_ctx"

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
#
# PG-touching integration tests run at --test-threads 1 to prevent the concurrent
# embedded-Postgres teardown race (SIGABRT / signal 6, #1852).  Non-PG integration
# tests, lib unit tests, and other workspace packages run at the default thread
# count (R-0021: serialization is scoped to the PG group only).
verify-test: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test -p mnemra-host {{PG_TEST_FLAGS}} -- --test-threads 1 2>&1 \
        && cargo test -p mnemra-host {{NONPG_TEST_FLAGS}} 2>&1 \
        && cargo test -p mnemra-host --lib 2>&1 \
        && cargo test --workspace --exclude mnemra-host 2>&1; then
        echo "GATE test PASS"
    else
        echo "GATE test FAIL"
        exit 1
    fi

# Verify: test-hooks feature — runs resource_limits.rs seam tests (gated behind test-hooks).
# This is a CI gate so untrusted-path seam coverage is always exercised.
# Depends on `plugin` for the same wasm-artifact reason as verify-test.
#
# PG serialization mirrors verify-test (R-0022: identical directive in all three recipes).
#
# Non-vacuity check (R-0031 AC4, tier-2 T5 — Warden-hardening pattern, mirrors
# verify-signing-root above): artifact_list_paging_whitebox is `#![cfg(feature =
# "test-hooks")]`-gated crate-level — structurally ZERO tests under verify-test /
# verify-coverage (no test-hooks feature), meaningfully active only here. A
# broken cfg-gate, an accidentally-emptied suite, or a silent regression back to
# 0 tests under test-hooks must FAIL this gate — not pass silently because
# `cargo test` exits 0 on an empty run (the same false-green class
# verify-signing-root already guards against). We capture a SCOPED rerun of
# just this one binary (same shape as verify-signing-root's single-test
# invocation) so the pass count checked is unambiguously this binary's own,
# not a count borrowed from the combined PG_TEST_FLAGS run above.
verify-test-hooks: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test -p mnemra-host --features test-hooks {{PG_TEST_FLAGS}} -- --test-threads 1 2>&1 \
        && cargo test -p mnemra-host --features test-hooks {{NONPG_TEST_FLAGS}} 2>&1 \
        && cargo test -p mnemra-host --features test-hooks --lib 2>&1; then
        :
    else
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    set +e
    wb_output="$(cargo test -p mnemra-host --features test-hooks --test artifact_list_paging_whitebox -- --test-threads 1 2>&1)"
    wb_code=$?
    set -e
    echo "$wb_output"
    if [[ "$wb_code" -ne 0 ]]; then
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. [1-9][0-9]* passed; 0 failed;' <<< "$wb_output"; then
        echo "artifact_list_paging_whitebox non-vacuity check failed: no non-zero pass count found (R-0031 AC4)"
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    echo "GATE test-hooks PASS"

# Verify: coverage (emit number; no threshold gate at scaffold stage)
# Depends on `plugin` for the same wasm-artifact reason as verify-test.
#
# PG serialization mirrors verify-test via --no-report accumulation + final report
# (R-0022: identical directive in all three recipes).
verify-coverage: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo llvm-cov --no-report -p mnemra-host {{PG_TEST_FLAGS}} -- --test-threads 1 2>&1 \
        && cargo llvm-cov --no-report -p mnemra-host {{NONPG_TEST_FLAGS}} 2>&1 \
        && cargo llvm-cov --no-report -p mnemra-host --lib 2>&1 \
        && cargo llvm-cov --no-report --workspace --exclude mnemra-host 2>&1 \
        && cargo llvm-cov report 2>&1; then
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

# Verify: smoke tests — the REAL end-to-end gate (R-0022-c; #1993 T6).
# Depends on `build` (the HOST binary only — bare `cargo build`,
# default-members = ["cmd/mnemra"]) — deliberately NEVER `plugin`: the
# integrity-gated load path loads the COMMITTED SIGNED artifact
# (artifacts/mnemra-echo/mnemra_echo.wasm), not a target/ rebuild (§
# Constraints, docs/specs/2026-06-30-signing-to-runnable.md) — a fresh
# `target/wasm32-wasip2` rebuild is NOT the signed artifact and would
# false-reject via R-0021-e.
#
# tests/smoke_e2e.rs spawns the real `mnemra` binary as a subprocess and
# drives a real MCP initialize handshake + list_tools call over its stdio,
# asserting the production stdio serve-loop, the BLAKE3(committed artifact)
# == signed [component].hash property, and that the child owns no listening
# TCP port besides /health. It is PG-class (the child starts a real embedded
# Postgres) but deliberately NOT folded into {{PG_TEST_FLAGS}}: it is slow
# (full production startup + a real subprocess spawn) and is semantically
# its own gate (R-0022-c "the smoke gate"), not part of the general
# behavioral suite verify-test / verify-test-hooks / verify-coverage already
# run three times over. Scoped invocation mirrors verify-signing-root's
# `cargo test --test <name>` pattern.
verify-smoke: build
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo test -p mnemra-host --test smoke_e2e -- --test-threads 1 2>&1; then
        echo "GATE smoke PASS"
    else
        echo "GATE smoke FAIL"
        exit 1
    fi

# Internal: the full verify-* prerequisite chain, unchanged in order and
# semantics from before the baseline-reap wiring below. Kept as its own
# recipe (rather than inlined into `ci`'s shell body) so `just ci` still
# resolves this as a SINGLE nested `just` invocation — `plugin`/`build`
# prerequisites shared across verify-test / verify-test-hooks /
# verify-coverage are deduped exactly as they were when `ci` itself listed
# these as direct prerequisites; invoking each verify-* recipe as its own
# separate `just` process from inside `ci`'s script body would lose that
# dedup and rebuild the plugin/host binary redundantly per step.
_ci-verify-chain: verify-type verify-lint verify-test verify-test-hooks verify-coverage verify-build verify-smoke verify-signing-root

# CI entry point (R-0018-c, R-0018-f) — runs `_ci-verify-chain` (the exact
# prior `ci` dependency list) wrapped in a baseline-PID self-reap safety net
# (R-9, self-verify-long-jobs spec, baseline-reap mechanism amendment
# 2026-07-06, #2119; see scripts/ci-reap.sh for the mechanics AND its
# "ACTUAL GUARANTEE" note — read that before relying on this for concurrent
# ci safety; also exercised directly by tests/ci_reap_baseline.rs).
#
# Captures the set of live embedded-PG postmasters BEFORE the verify chain
# starts (baseline = everything alive at that moment — a concurrent agent's
# already-running engine, or a pre-existing instance, is protected). On a
# NORMAL (all-gates-pass) completion no reap runs at all — the engine
# already self-cleans on drop; the reap fires ONLY on failure or interrupt,
# and even then reaps ONLY postmasters that are BOTH absent from the
# baseline snapshot AND running against a data directory under the
# embedded-PG temp root (M1 hardening, #2119 fix round) — the latter keeps a
# developer's system Postgres or another project's engine (started after
# this run's baseline) out of the kill-set, since those normally run against
# a persistent, non-temp data directory. Baseline-absence is still a proxy
# for "this run's own leaked spawn," not a direct ownership check: a
# concurrent ci ON THIS CODEBASE whose postmaster starts AFTER this
# baseline is captured has its data dir under the temp root too, and is
# reaped if this run fails (see scripts/ci-reap.sh's ACTUAL GUARANTEE).
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    source scripts/ci-reap.sh
    ci_reap_capture_baseline
    trap 'ci_reap_own_postmasters; exit 130' INT TERM
    if just _ci-verify-chain; then
        rc=0
    else
        rc=$?
    fi
    trap - INT TERM
    if [ "$rc" -ne 0 ]; then
        ci_reap_own_postmasters
    fi
    exit "$rc"

# ---------------------------------------------------------------------------
# Signing-root pin gate (R-0005-d / R-0018-f) — wired into `ci`.
#
# PASS iff the build-embedded root (`signing::root_material::ROOT`) byte-equals
# the independently-declared pin (`ROOT_PIN`). The signing ceremony is
# complete: both are set to the real root public key (byte-equal), so this
# gate is live and enforced as part of the `ci` chain above.
#
# The check runs the now-live `root_pin_gate_matches_embedded` test in
# tests/build_gate.rs (no longer `#[ignore]`d — it also runs as part of the
# normal suite via `NONPG_TEST_FLAGS`, which `verify-test` runs). We capture
# its real exit status WITHOUT errexit (set +e), mirroring test-gate-shape's
# O1 "read the child's real exit status" discipline, and map it to the GATE
# line.
#
# Non-vacuous check (Warden hardening, round-2 false-green class): `cargo
# test` exits 0 on a ZERO-test run too — a future `#[ignore]` re-add or a test
# rename would silently make `code -eq 0` true over nothing having run. We
# capture stdout/stderr and additionally require the summary line prove
# exactly one test actually passed (`test result: ok. 1 passed; 0 failed;`).
# A 0-test run (`0 passed`) FAILs the gate instead of silently PASSing.
# ---------------------------------------------------------------------------
verify-signing-root:
    #!/usr/bin/env bash
    set -uo pipefail
    set +e
    output="$(cargo test -p mnemra-host --test build_gate -- --exact root_pin_gate_matches_embedded 2>&1)"
    code=$?
    set -e
    echo "$output"
    if [[ "$code" -ne 0 ]]; then
        echo "GATE signing-root FAIL"
        exit 1
    fi
    if [[ "$output" != *"test result: ok. 1 passed; 0 failed;"* ]]; then
        echo "GATE signing-root FAIL"
        exit 1
    fi
    echo "GATE signing-root PASS"

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
