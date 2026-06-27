#!/usr/bin/env bash
# flake-runner.sh — post-fix regression harness for mnemra-core embedded-Postgres CI flakes.
# Promoted from scratch/flake-runner.sh (dispatch #1122) to a tracked script in dispatch #1131
# (Tier-1 CI-flake fix: serialize PG/plugin test binaries).
#
# WHAT IT DOES
#   Builds the host test binaries ONCE, then runs the PG-touching integration test binaries
#   N times each.  After each non-zero exit the harness reaps only the embedded-Postgres
#   postmasters it spawned (own-spawned-only, PID-set-based — never touches pre-existing
#   postmasters) before moving to the next binary.  A peak sampler tracks the live postmaster
#   count across the entire run.
#
# USAGE
#   bash scripts/flake-runner.sh [N] [TEST_THREADS] [BIN ...]
#     N             iterations per binary                (default 20)
#     TEST_THREADS  --test-threads value, or "default"   (default "default")
#                   Pass 1 to match the serialized CI recipe.
#     BIN ...       space-separated --test binary names  (default = all 14 PG binaries)
#
# SIGNATURE CLASSIFICATION (per run)
#   SIGABRT   log has "signal: 6" / "SIGABRT"                  -> #1852 teardown abort family
#   DEADLINE  log has "deadline has elapsed"                   -> #1703 startup-timeout under load
#   SHMEM     log has "could not create shared memory segment" / "shmget"
#             -> SysV shared-memory exhaustion (macOS low-SHMMNI); distinct from real disk-full
#   DISK      log has "No space left on device" WITHOUT shmget -> real runner disk-full
#   INITERR   other DatabaseInitializationError                -> startup error, uncategorized
#   FAIL      other non-zero exit (assertion / panic / other)
#   PASS      exit 0
#
# OUTPUT
#   Per-run logs:  scratch/flake-runs/<bin>_run<i>_t<threads>.log
#   Summary table + peak postmaster line printed to stdout at the end.
#
# OWN-SPAWNED REAPING (R-0024)
#   The harness captures the PID set of all bin/postgres processes alive at startup
#   (BASELINE_PM_PIDS).  After each invocation that exits non-zero the reap_own_postmasters
#   function kills only PIDs NOT in that baseline set — so pre-existing dev postmasters are
#   never touched, and leaked test postmasters are cleaned up before the next invocation.
#   Kill is SIGKILL (fast, unconditional) since these are already orphaned engines.

set -uo pipefail

# Derive REPO from the repo root (portable to the GitHub runner).
# Order: $MNEMRA_CORE env override -> git rev-parse from script dir -> hardcoded fallback.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="${MNEMRA_CORE:-$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || echo /Users/manahan/claude_workspace/repos/mnemra/mnemra-core)}"
MANIFEST="$REPO/Cargo.toml"
OUTDIR="$REPO/scratch/flake-runs"
mkdir -p "$OUTDIR"

N="${1:-20}"
THREADS="${2:-default}"
# Shift past N and THREADS so "$@" contains only binary names.
if [ "$#" -ge 1 ]; then shift; fi
if [ "$#" -ge 1 ]; then shift; fi
if [ "$#" -ge 1 ]; then
    BINS=("$@")
else
    # Default: all 14 PG-touching integration test binaries (R-0024).
    BINS=(
        admin_token
        admin_token_behavior
        artifact_machinery
        content_schema
        identity_builtins
        invoke_health_gate
        mcp_server
        mcp_slice1_e2e
        mcp_verb_gate
        postgres_engine
        schema_init
        startup_population
        storage_contract_postgres
        tenancy_isolation
    )
fi

# Count live postmasters (parent bin/postgres -D ... process; aux processes show as
# "postgres: checkpointer" etc. and do NOT match "bin/postgres").
count_postmasters() { pgrep -f "bin/postgres" 2>/dev/null | wc -l | tr -d ' '; }

# Capture baseline PID set at startup (own-spawned reap anchor).
BASELINE_PM_PIDS="$(pgrep -f "bin/postgres" 2>/dev/null || true)"

# Reap only postmasters this harness spawned (PIDs absent from the baseline set).
# Called after each non-zero invocation before the next binary.
reap_own_postmasters() {
    local current_pids pid_line
    current_pids="$(pgrep -f "bin/postgres" 2>/dev/null || true)"
    local reaped=0
    while IFS= read -r pid_line; do
        [ -z "$pid_line" ] && continue
        if ! echo "$BASELINE_PM_PIDS" | grep -qw "$pid_line"; then
            kill -KILL "$pid_line" 2>/dev/null && reaped=$(( reaped + 1 )) || true
        fi
    done <<< "$current_pids"
    [ "$reaped" -gt 0 ] && echo "  [reap] killed $reaped own-spawned postmaster(s)"
    # Always succeed: this script runs without errexit, but a defensive return 0
    # keeps the function safe if a caller ever enables `set -e`.
    return 0
}

echo "=== flake-runner: N=$N THREADS=$THREADS BINS=${BINS[*]} ==="
echo "host: $(uname -srm) | cores: $(getconf _NPROCESSORS_ONLN 2>/dev/null || echo '?') | $(date)"
echo "baseline postmaster PID count: $(echo "$BASELINE_PM_PIDS" | grep -c . 2>/dev/null || echo 0)"

# Build once: plugin wasm guest + host test binaries.
echo "--- building (once) ---"
cargo build --release -p mnemra-echo --target wasm32-wasip2 --manifest-path "$MANIFEST" >/dev/null 2>&1 \
    || { echo "FATAL: plugin build failed"; exit 2; }
cargo test -p mnemra-host --no-run --manifest-path "$MANIFEST" >/dev/null 2>&1 \
    || { echo "FATAL: test build failed"; exit 2; }

BASE_PM="$(count_postmasters)"
echo "baseline postmaster count (post-build): $BASE_PM"

# Peak postmaster sampler (background).
PEAKFILE="$OUTDIR/.peak"
echo 0 > "$PEAKFILE"
(
    peak=0
    while true; do
        c="$(count_postmasters)"
        if [ "$c" -gt "$peak" ]; then peak="$c"; echo "$peak" > "$PEAKFILE"; fi
        sleep 0.2
    done
) &
SAMPLER=$!
trap 'kill "$SAMPLER" 2>/dev/null; true' EXIT

# Tallies.
declare -A c_pass c_fail c_abort c_deadline c_shmem c_disk c_initerr
for b in "${BINS[@]}"; do
    c_pass[$b]=0; c_fail[$b]=0; c_abort[$b]=0
    c_deadline[$b]=0; c_shmem[$b]=0; c_disk[$b]=0; c_initerr[$b]=0
done

THREADARGS=()
if [ "$THREADS" != "default" ]; then THREADARGS=(--test-threads "$THREADS"); fi

START="$(date +%s)"
for i in $(seq 1 "$N"); do
    for b in "${BINS[@]}"; do
        LOG="$OUTDIR/${b}_run${i}_t${THREADS}.log"
        # No errexit is enabled (top of script is `set -uo pipefail`, not `-e`),
        # so a non-zero cargo exit is captured in $code and classified below
        # rather than aborting the sweep.
        cargo test -p mnemra-host --test "$b" --manifest-path "$MANIFEST" \
            -- "${THREADARGS[@]}" >"$LOG" 2>&1
        code=$?

        if [ "$code" -eq 0 ]; then
            c_pass[$b]=$(( ${c_pass[$b]} + 1 ))
            tag="PASS"
        elif [ "$code" -eq 134 ] || grep -qiE "signal: 6|SIGABRT|process abort signal" "$LOG"; then
            # cargo test masks child signal 6 as exit 101, not 134.
            # The SIGABRT text in the log is the reliable detector.
            c_abort[$b]=$(( ${c_abort[$b]} + 1 ))
            tag="SIGABRT"
            reap_own_postmasters
        elif grep -qiE "deadline has elapsed" "$LOG"; then
            c_deadline[$b]=$(( ${c_deadline[$b]} + 1 ))
            tag="DEADLINE"
            reap_own_postmasters
        elif grep -qiE "could not create shared memory segment|shmget" "$LOG"; then
            # SysV shm exhaustion — check BEFORE disk (shmget message also contains
            # "No space left on device").
            c_shmem[$b]=$(( ${c_shmem[$b]} + 1 ))
            tag="SHMEM"
            reap_own_postmasters
        elif grep -qiE "No space left on device" "$LOG"; then
            c_disk[$b]=$(( ${c_disk[$b]} + 1 ))
            tag="DISK"
        elif grep -qiE "DatabaseInitializationError" "$LOG"; then
            c_initerr[$b]=$(( ${c_initerr[$b]} + 1 ))
            tag="INITERR"
            reap_own_postmasters
        else
            c_fail[$b]=$(( ${c_fail[$b]} + 1 ))
            tag="FAIL($code)"
            reap_own_postmasters
        fi
        printf 'run %2d  %-30s  %-12s (exit %s)\n' "$i" "$b" "$tag" "$code"
    done
done
END="$(date +%s)"

kill "$SAMPLER" 2>/dev/null || true
PEAK="$(cat "$PEAKFILE")"
PEAK_DELTA=$(( PEAK - BASE_PM ))
[ "$PEAK_DELTA" -lt 0 ] && PEAK_DELTA=0

echo ""
echo "================= SUMMARY ================="
echo "iterations per binary (N): $N    test-threads: $THREADS"
echo "wall-clock: $(( END - START ))s"
echo "peak postmasters during run: $PEAK   (baseline $BASE_PM -> peak-above-baseline $PEAK_DELTA)"
echo ""
printf '%-30s %5s %6s %8s %6s %5s %8s %5s\n' "binary" "PASS" "ABORT" "DEADLINE" "SHMEM" "DISK" "INITERR" "FAIL"
for b in "${BINS[@]}"; do
    printf '%-30s %5s %6s %8s %6s %5s %8s %5s\n' "$b" \
        "${c_pass[$b]}" "${c_abort[$b]}" "${c_deadline[$b]}" \
        "${c_shmem[$b]}" "${c_disk[$b]}" "${c_initerr[$b]}" "${c_fail[$b]}"
done
echo "==========================================="
echo "per-run logs: $OUTDIR/"
