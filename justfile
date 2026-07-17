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

# PG-touching integration test binaries (23 members).
# Defined once; verify-test, verify-test-hooks, and verify-coverage-pg all
# reference this variable so the list stays in sync (R-0022: identical
# serialization directive across all three). verify-coverage-membership
# below count-pins this at 23 as a mechanical drift guard.
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
# coordination_failclosed (Task 3 sub-run c, R-0074-b/R-0075-c fault-injection):
# same posture as artifact_list_paging_whitebox — the whole file is crate-level
# `#![cfg(feature = "test-hooks")]`-gated (it names CoordinationFault), so it is
# structurally zero-test under verify-test / verify-coverage and meaningfully
# active only under verify-test-hooks. AC1/AC2 acquire the shared embedded engine
# via tests/common/shared_engine.rs, so it inherits the same --test-threads 1
# serialization; it belongs in this shared list, guarded by the non-vacuity check
# in verify-test-hooks below.
# coordination_leases (Task 5 slice b1, R-0065/R-0067/R-0073/R-0075 — `claim
# acquire`): drives the real `claim` MCP tool via the same
# tests/common/shared_engine.rs harness as coordination_session_plane, so it
# inherits the same --test-threads 1 serialization; it belongs in this shared
# list, guarded by the non-vacuity check in verify-test below (mirrors the
# coordination_session_plane guard — silent-failure class #2004).
# coordination_messages (Task 7 slice a, R-0068-a/R-0070-b/R-0075-b/R-0075-e —
# `message send`): drives the real `message` MCP tool via the same
# tests/common/shared_engine.rs harness as coordination_leases /
# coordination_session_plane, so it inherits the same --test-threads 1
# serialization; it belongs in this shared list (silent-failure class #2004 —
# a PG-touching coordination suite left out of PG_TEST_FLAGS never runs under
# verify-test and passes anyway).
PG_TEST_FLAGS := "--test actors_entity --test admin_token --test admin_token_behavior --test artifact_list_paging --test artifact_list_paging_whitebox --test artifact_machinery --test content_schema --test coordination_failclosed --test coordination_leases --test coordination_messages --test coordination_schema --test coordination_session_plane --test identity_builtins --test invoke_health_gate --test mcp_server --test mcp_slice1_e2e --test mcp_verb_gate --test postgres_engine --test schema_init --test startup_population --test startup_run_full --test storage_contract_postgres --test tenancy_isolation"

# Non-PG integration test binaries (18 members).
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
# coordination_message_types (Task 6, coordination-wedge, R-0070/R-0071):
# pure host-code closed-schema validation over in-memory JSON payloads — no
# embedded Postgres engine needed, so it belongs here, not in PG_TEST_FLAGS.
NONPG_TEST_FLAGS := "--test abi_contract --test build_gate --test ci_reap_baseline --test content_hash_binding --test coordination_message_types --test health_listener --test lint_workspace_clause --test llm_key_allowlist --test manifest_load --test mcp_feature_guard --test no_test_seams --test permissions --test plugin_output_validation --test resource_limits --test signing_chain --test startup_run_ordering --test storage_contract --test workspace_ctx"

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
        :
    else
        echo "GATE test FAIL"
        exit 1
    fi
    # coordination_session_plane non-vacuity check (Task 4, R-0064): unlike the
    # test-hooks binaries guarded in verify-test-hooks, this suite is NOT
    # cfg-gated — it drives the real MCP path under the default feature set, so it
    # runs in the combined PG_TEST_FLAGS pass above. But a binary silently dropped
    # from that list — or emptied by a refactor — passes vacuously (#2004, the
    # silent-failure class). A SCOPED rerun asserts a non-zero pass count so an
    # emptied/dropped suite FAILS this gate rather than passing on cargo's
    # exit-0-on-empty-run. Same shape as the coordination_failclosed guard in
    # verify-test-hooks below.
    set +e
    csp_output="$(cargo test -p mnemra-host --test coordination_session_plane -- --test-threads 1 2>&1)"
    csp_code=$?
    set -e
    echo "$csp_output"
    if [[ "$csp_code" -ne 0 ]]; then
        echo "GATE test FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. [1-9][0-9]* passed; 0 failed;' <<< "$csp_output"; then
        echo "coordination_session_plane non-vacuity check failed: no non-zero pass count found (#2004 silent-failure class)"
        echo "GATE test FAIL"
        exit 1
    fi
    # coordination_leases non-vacuity check (Task 5 slice b1, R-0065/R-0067/
    # R-0073/R-0075): same #2004 false-green class as coordination_session_plane
    # above — this suite is NOT cfg-gated either (drives the real `claim` MCP
    # tool under the default feature set), so it runs in the combined
    # PG_TEST_FLAGS pass above, but a binary silently dropped from that list —
    # or emptied by a refactor — would pass vacuously on cargo's
    # exit-0-on-empty-run. Scoped rerun so the pass count checked is
    # unambiguously this binary's own.
    set +e
    cl_output="$(cargo test -p mnemra-host --test coordination_leases -- --test-threads 1 2>&1)"
    cl_code=$?
    set -e
    echo "$cl_output"
    if [[ "$cl_code" -ne 0 ]]; then
        echo "GATE test FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. [1-9][0-9]* passed; 0 failed;' <<< "$cl_output"; then
        echo "coordination_leases non-vacuity check failed: no non-zero pass count found (#2004 silent-failure class)"
        echo "GATE test FAIL"
        exit 1
    fi
    # coordination_messages non-vacuity COUNT-PIN (Task 7a, Warden T7a finding —
    # silent-failure gap): this suite is MIXED, not all-or-nothing like
    # coordination_session_plane/coordination_leases above — 6 of its 8 tests
    # (1/2/3/6/7/8) run under the DEFAULT feature set; the other 2 (tests 4/5 —
    # the send-ordering pin and registration-audit-iff-minted, both
    # security-critical) are `#[cfg(feature = "test-hooks")]`-gated and only
    # compile/run under verify-test-hooks below. A bare non-zero check (the
    # pattern used above) would NOT catch tests 4/5 being silently dropped from
    # the test-hooks-gated set, since the remaining 6 default tests already
    # produce a non-zero pass count on their own — so this check pins the EXACT
    # expected count (6) instead. Scoped rerun so the pass count checked is
    # unambiguously this binary's own, not a count borrowed from the combined
    # PG_TEST_FLAGS run above (#2004 silent-failure class).
    set +e
    cm_output="$(cargo test -p mnemra-host --test coordination_messages -- --test-threads 1 2>&1)"
    cm_code=$?
    set -e
    echo "$cm_output"
    if [[ "$cm_code" -ne 0 ]]; then
        echo "GATE test FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. 18 passed; 0 failed;' <<< "$cm_output"; then
        echo "coordination_messages non-vacuity count-pin failed: expected exactly 18 passed under the default feature set (slice a tests 1/2/3/6/7/8 + slice b tests 9-20); got a different count — a silent drop would otherwise pass vacuously (Task 7a, Warden finding, #2004 silent-failure class)"
        echo "GATE test FAIL"
        exit 1
    fi
    echo "GATE test PASS"

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
    # coordination_failclosed non-vacuity check (Task 3, R-0074-b/R-0075-c): same
    # false-green class as artifact_list_paging_whitebox — the file is crate-level
    # `#![cfg(feature = "test-hooks")]`-gated, so it is structurally zero-test
    # under verify-test / verify-coverage and meaningful only here. A broken
    # cfg-gate or an accidentally-emptied fault-injection suite must FAIL this
    # gate, not pass silently on `cargo test`'s exit-0-on-empty-run. Scoped rerun
    # so the pass count is unambiguously this binary's own.
    set +e
    cf_output="$(cargo test -p mnemra-host --features test-hooks --test coordination_failclosed -- --test-threads 1 2>&1)"
    cf_code=$?
    set -e
    echo "$cf_output"
    if [[ "$cf_code" -ne 0 ]]; then
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. [1-9][0-9]* passed; 0 failed;' <<< "$cf_output"; then
        echo "coordination_failclosed non-vacuity check failed: no non-zero pass count found (test-hooks fault-injection suite must not silently empty)"
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    # coordination_messages non-vacuity COUNT-PIN (Task 7a, Warden T7a finding —
    # same mixed suite as the verify-test guard above). Under
    # `--features test-hooks` all 8 tests compile and run — the 6 default-feature
    # tests PLUS tests 4/5 (the send-ordering pin and registration-audit-iff-
    # minted, both security-critical). A bare non-zero check would not catch
    # tests 4/5 alone being silently dropped (or their #[cfg] gate breaking) while
    # the other 6 keep passing, since 6 is already non-zero — so this check pins
    # the EXACT expected count (8) instead, mirroring the verify-test guard's
    # exact-count discipline. Scoped rerun so the pass count checked is
    # unambiguously this binary's own, not a count borrowed from the combined
    # PG_TEST_FLAGS run above.
    set +e
    cm_th_output="$(cargo test -p mnemra-host --features test-hooks --test coordination_messages -- --test-threads 1 2>&1)"
    cm_th_code=$?
    set -e
    echo "$cm_th_output"
    if [[ "$cm_th_code" -ne 0 ]]; then
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    if ! grep -qE 'test result: ok\. 20 passed; 0 failed;' <<< "$cm_th_output"; then
        echo "coordination_messages non-vacuity count-pin failed: expected exactly 20 passed under --features test-hooks (slice a tests 1-8, including the security-critical send-ordering-pin and registration-audit-iff-minted tests 4/5, + slice b tests 9-20); got a different count — a silent drop of the security-critical tests alone would otherwise pass vacuously behind the others (Task 7a, Warden finding)"
        echo "GATE test-hooks FAIL"
        exit 1
    fi
    echo "GATE test-hooks PASS"

# ---------------------------------------------------------------------------
# Coverage sharding (R-0093-R-0098, docs/specs/2026-07-15-ci-coverage-shard.md)
#
# The CI coverage gate intermittently failed with "No space left on device"
# on GitHub's mixed hosted-runner fleet (a smaller-disk image variant could
# not fit the full instrumented binary set `cargo llvm-cov` builds — the
# dominant disk consumer of the whole `just ci` run). Fix: split coverage
# across independent CI jobs (R-0094, Shape B — see the spec's "load-bearing
# decision" section for why NOT a cross-job merge), each on its own fresh
# runner disk, each emitting its OWN per-shard coverage number (R-0095 —
# union coverage across shards is NOT computed; acceptable only while this
# gate stays emit-only). `verify-coverage` below delegates to the shard
# recipes so local and CI enumerate identical shard membership by
# construction (R-0096); `verify-coverage-membership` is the MECHANICAL
# reconciliation guard on top of that (not eyeballed, per S2).
# ---------------------------------------------------------------------------

# Verify: coverage-shard membership reconciliation (R-0096, mechanical guard).
# Confirms union(verify-coverage-pg, verify-coverage-rest) selectors equals
# the pre-shard verify-coverage selector set exactly — PG_TEST_FLAGS in the
# PG shard only; NONPG_TEST_FLAGS + `-p mnemra-host --lib` +
# `--workspace --exclude mnemra-host` in the rest shard only; no selector
# dropped and none duplicated across shards (R-0096 AC1, S2).
#
# Reads the justfile SOURCE TEXT directly via sed/grep (not `just --show`,
# which does not interpolate {{...}} tokens and is a red herring here) plus
# `just --evaluate` for the two variables' raw values — both are external
# subprocess calls whose output is plain captured text, so this recipe's own
# body never needs to embed a literal `{{`/`}}` pair (which `just` would
# otherwise try to interpolate before bash ever sees it).
#
# PG_TEST_FLAGS-vs-NONPG_TEST_FLAGS disambiguation: "NONPG_TEST_FLAGS"
# textually CONTAINS "PG_TEST_FLAGS" as a substring, so a naive grep for
# PG_TEST_FLAGS would false-positive inside NONPG_TEST_FLAGS. Strip every
# NONPG_TEST_FLAGS occurrence first, then check for PG_TEST_FLAGS in what's
# left — that unambiguously finds only genuine standalone references.
#
# Count-pins PG_TEST_FLAGS=23 / NONPG_TEST_FLAGS=18 (mirrors the project's
# non-vacuity count-pin convention, e.g. verify-test's coordination_messages
# 6/8 pin) so a silently emptied or partially-dropped variable fails loudly
# rather than passing on token-presence alone.
verify-coverage-membership:
    #!/usr/bin/env bash
    set -euo pipefail
    fail=0

    pg_body="$(sed -n '/^verify-coverage-pg:/,/^$/p' justfile)"
    rest_body="$(sed -n '/^verify-coverage-rest:/,/^$/p' justfile)"

    if [[ -z "$pg_body" || -z "$rest_body" ]]; then
        echo "membership check FAIL: could not locate verify-coverage-pg / verify-coverage-rest recipe bodies in justfile"
        echo "GATE coverage-membership FAIL"
        exit 1
    fi

    pg_body_no_nonpg="${pg_body//NONPG_TEST_FLAGS/}"
    rest_body_no_nonpg="${rest_body//NONPG_TEST_FLAGS/}"

    # PG_TEST_FLAGS: PG shard only, not the rest shard.
    if ! grep -qF 'PG_TEST_FLAGS' <<< "$pg_body_no_nonpg"; then
        echo "membership check FAIL: verify-coverage-pg does not reference PG_TEST_FLAGS"
        fail=1
    fi
    if grep -qF 'PG_TEST_FLAGS' <<< "$rest_body_no_nonpg"; then
        echo "membership check FAIL: PG_TEST_FLAGS referenced in verify-coverage-rest (duplicated across shards)"
        fail=1
    fi

    # NONPG_TEST_FLAGS: rest shard only, not the PG shard.
    if ! grep -qF 'NONPG_TEST_FLAGS' <<< "$rest_body"; then
        echo "membership check FAIL: verify-coverage-rest does not reference NONPG_TEST_FLAGS"
        fail=1
    fi
    if grep -qF 'NONPG_TEST_FLAGS' <<< "$pg_body"; then
        echo "membership check FAIL: NONPG_TEST_FLAGS referenced in verify-coverage-pg (duplicated across shards)"
        fail=1
    fi

    # The two non-enumerable selectors: rest shard only, never PG.
    if ! grep -qF -- '-p mnemra-host --lib' <<< "$rest_body"; then
        echo "membership check FAIL: verify-coverage-rest missing '-p mnemra-host --lib' selector"
        fail=1
    fi
    if grep -qF -- '-p mnemra-host --lib' <<< "$pg_body"; then
        echo "membership check FAIL: '-p mnemra-host --lib' selector duplicated into verify-coverage-pg"
        fail=1
    fi
    if ! grep -qF -- '--workspace --exclude mnemra-host' <<< "$rest_body"; then
        echo "membership check FAIL: verify-coverage-rest missing '--workspace --exclude mnemra-host' selector"
        fail=1
    fi
    if grep -qF -- '--workspace --exclude mnemra-host' <<< "$pg_body"; then
        echo "membership check FAIL: '--workspace --exclude mnemra-host' selector duplicated into verify-coverage-pg"
        fail=1
    fi

    # Count-pins: catch a silently emptied or partially-dropped variable that
    # token-presence checks above would miss.
    pg_count="$(just --evaluate PG_TEST_FLAGS | grep -oE -- '--test [A-Za-z0-9_]+' | wc -l | tr -d ' ')"
    nonpg_count="$(just --evaluate NONPG_TEST_FLAGS | grep -oE -- '--test [A-Za-z0-9_]+' | wc -l | tr -d ' ')"
    if [[ "$pg_count" -ne 23 ]]; then
        echo "membership check FAIL: PG_TEST_FLAGS has $pg_count members, expected 23 (update this pin if the variable's membership intentionally changed)"
        fail=1
    fi
    if [[ "$nonpg_count" -ne 18 ]]; then
        echo "membership check FAIL: NONPG_TEST_FLAGS has $nonpg_count members, expected 18 (update this pin if the variable's membership intentionally changed)"
        fail=1
    fi

    # No test binary name present in BOTH variables (guards the source
    # variables themselves, not just the shard recipes' wiring).
    dup_tests="$(comm -12 \
        <(just --evaluate PG_TEST_FLAGS | grep -oE -- '--test [A-Za-z0-9_]+' | awk '{print $2}' | sort -u) \
        <(just --evaluate NONPG_TEST_FLAGS | grep -oE -- '--test [A-Za-z0-9_]+' | awk '{print $2}' | sort -u))"
    if [[ -n "$dup_tests" ]]; then
        echo "membership check FAIL: test binary name(s) present in BOTH PG_TEST_FLAGS and NONPG_TEST_FLAGS: $dup_tests"
        fail=1
    fi

    if [[ "$fail" -ne 0 ]]; then
        echo "GATE coverage-membership FAIL"
        exit 1
    fi
    echo "GATE coverage-membership PASS (union(verify-coverage-pg, verify-coverage-rest) == pre-shard selector set: PG_TEST_FLAGS [23], NONPG_TEST_FLAGS [18], -p mnemra-host --lib, --workspace --exclude mnemra-host; no drop, no dup)"

# Verify: coverage — PG shard (embedded-Postgres binary set; the disk-heavy
# set that drove the runner-disk roulette — see the spec's Purpose/context).
# Emits its OWN per-shard coverage number over PG_TEST_FLAGS only (R-0095 —
# union coverage across shards is not computed; see verify-coverage below).
#
# `cargo llvm-cov clean --workspace` first so this shard's report reflects
# ONLY its own subset's profile data, never polluted by a prior shard's
# accumulated data on the same filesystem (the "Clean per-shard slate"
# constraint — matters for the local delegated run below; a no-op on a fresh
# CI runner, where each shard already gets its own disk). --test-threads 1
# mirrors verify-test / verify-test-hooks (R-0022).
verify-coverage-pg: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    cargo llvm-cov clean --workspace
    if cargo llvm-cov --no-report -p mnemra-host {{PG_TEST_FLAGS}} -- --test-threads 1 2>&1 \
        && cargo llvm-cov report 2>&1; then
        echo "GATE coverage-pg PASS (per-shard number over the embedded-Postgres test subset only — union coverage not computed under Shape B, see verify-coverage)"
    else
        echo "GATE coverage-pg FAIL"
        exit 1
    fi

# Verify: coverage — rest shard (non-PG integration binaries + host lib +
# remaining workspace crates). Emits its OWN per-shard coverage number
# (R-0095). Same clean-per-shard-slate reason as verify-coverage-pg above.
verify-coverage-rest: plugin
    #!/usr/bin/env bash
    set -euo pipefail
    cargo llvm-cov clean --workspace
    if cargo llvm-cov --no-report -p mnemra-host {{NONPG_TEST_FLAGS}} 2>&1 \
        && cargo llvm-cov --no-report -p mnemra-host --lib 2>&1 \
        && cargo llvm-cov --no-report --workspace --exclude mnemra-host 2>&1 \
        && cargo llvm-cov report 2>&1; then
        echo "GATE coverage-rest PASS (per-shard number over the non-PG test subset + host lib + remaining workspace crates — union coverage not computed under Shape B, see verify-coverage)"
    else
        echo "GATE coverage-rest FAIL"
        exit 1
    fi

# Verify: coverage (emit number; no threshold gate at scaffold stage)
# Delegates to the shard recipes above — single source of truth for shard
# membership (R-0096): both shards draw from the SAME PG_TEST_FLAGS /
# NONPG_TEST_FLAGS variables the pre-shard recipe used, so local and CI
# enumerate the identical set by construction; verify-coverage-membership
# above is the mechanical guard on top. Depends on `plugin` for the same
# wasm-artifact reason as verify-test.
#
# Shape B (independent shards, no cross-job merge — LOCKED, see
# docs/specs/2026-07-15-ci-coverage-shard.md § "The load-bearing decision").
# Each shard emits its OWN coverage number over its own subset; there is no
# cross-job merge. UNION COVERAGE ACROSS SHARDS IS NOT COMPUTED — a line
# covered only by another shard's tests reads as uncovered in this shard's
# report. Acceptable ONLY while this gate is emit-only (no pass/fail
# threshold, per the header above). Tripwire (R-0095): before any coverage
# THRESHOLD gate is introduced, the union-coverage question SHALL be
# resolved first — either adopt cross-job profile-data merge (Shape A) or
# define per-shard thresholds. Fires on a concrete event (a PR adding a
# pass/fail threshold to this gate), not "later."
#
# In CI the shards run as SEPARATE jobs (R-0094) that invoke
# verify-coverage-pg / verify-coverage-rest directly, NOT through this
# delegator — that is the whole point of R-0094 (fresh disk per job).
# Locally this recipe runs both shards in one process (no disk roulette
# locally), so `just ci` still proves full-chain coverage (R-0098).
verify-coverage: verify-coverage-pg verify-coverage-rest
    #!/usr/bin/env bash
    set -euo pipefail
    echo "GATE coverage PASS (delegated: verify-coverage-pg + verify-coverage-rest each emitted their own per-shard GATE line above; union coverage not computed under Shape B)"

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
#
# Local `just ci` retains FULL-chain coverage (R-0098): verify-coverage below
# delegates to BOTH shard recipes, so this chain runs every shard.
_ci-verify-chain: verify-type verify-lint verify-coverage-membership verify-test verify-test-hooks verify-coverage verify-build verify-smoke verify-signing-root

# Internal: the verify-* prerequisite chain MINUS coverage (R-0094) — used by
# the CI workflow's main "CI gates" job. Coverage runs in its own separate CI
# job(s) instead (`verify-coverage-pg` / `verify-coverage-rest`, invoked
# directly by the CI workflow, not through this chain) so this job's runner
# disk never accumulates the disk-heavy coverage instrumentation set. Every
# other gate — including verify-coverage-membership, which is a cheap
# justfile-introspection check, not a coverage build — is unchanged from
# `_ci-verify-chain` above (R-0098: all non-coverage gate semantics
# preserved). Local `just ci` does NOT use this recipe — it always runs the
# full chain above, coverage included; local disk has no runner-fleet
# roulette (R-0098).
_ci-verify-chain-nocoverage: verify-type verify-lint verify-coverage-membership verify-test verify-test-hooks verify-build verify-smoke verify-signing-root

# CI entry point (R-0018-c, R-0018-f) — runs the given `chain` recipe (default:
# `_ci-verify-chain`, the full chain including coverage) wrapped in a
# baseline-PID self-reap safety net (R-9, self-verify-long-jobs spec,
# baseline-reap mechanism amendment 2026-07-06, #2119; see scripts/ci-reap.sh
# for the mechanics AND its "ACTUAL GUARANTEE" note — read that before relying
# on this for concurrent ci safety; also exercised directly by
# tests/ci_reap_baseline.rs).
#
# The `chain` parameter (R-0094) lets the CI workflow's main "CI gates" job
# reuse this same reap-wrapped body while running `_ci-verify-chain-nocoverage`
# instead of the default — that job still runs verify-test / verify-test-hooks
# / verify-smoke, which touch embedded Postgres, so it keeps the same
# baseline-reap protection without duplicating the reap-net bash. The
# coverage-shard jobs (R-0094) do NOT go through this recipe at all — they
# invoke verify-coverage-pg / verify-coverage-rest directly; ephemeral
# single-use CI shard runners self-clean on teardown and don't need the reap
# net re-wired per shard (see the spec's Illustrative implementation shape).
# `ci` with no argument is behaviorally IDENTICAL to before this change (the
# default `_ci-verify-chain` is the exact prior recipe list) — `just ci`,
# `ci-full: ci docs-check`, and every other existing caller are unaffected.
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
ci chain='_ci-verify-chain':
    #!/usr/bin/env bash
    set -euo pipefail
    source scripts/ci-reap.sh
    ci_reap_capture_baseline
    trap 'ci_reap_own_postmasters; exit 130' INT TERM
    if just {{chain}}; then
        rc=0
    else
        rc=$?
    fi
    trap - INT TERM
    if [ "$rc" -ne 0 ]; then
        ci_reap_own_postmasters
    fi
    exit "$rc"

# Complete local gate — mirrors GitHub CI (Rust verify chain + docs drift). Run this to prove CI-green.
ci-full: ci docs-check

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
