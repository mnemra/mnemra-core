#!/usr/bin/env bash
# ci-reap.sh — self-reap own leaked embedded-PG postmasters for `just ci`.
#
# R-9, docs/specs (workspace-side: brain/specs/2026-07-04-agent-self-verify-long-jobs.md),
# baseline-PID mechanism amendment 2026-07-06, #2119.
#
# WHAT IT DOES
#   Captures the set of live embedded-PG postmasters BEFORE the `ci` verify
#   chain starts (the baseline), then, on failure or interrupt ONLY, reaps
#   postmasters NOT present in that baseline snapshot.
#
# ACTUAL GUARANTEE (read before relying on this)
#   A postmaster is reaped iff BOTH:
#     (i)  it is NOT present in the baseline captured before the verify
#          chain started (own-run: alive at capture time is always safe), AND
#     (ii) its data directory (parsed from its `-D <data_dir>` command-line
#          argument) is under the embedded-PG temp root ($TMPDIR, default
#          /tmp — see WHY THE TEMP-ROOT CHECK below).
#   Condition (i) alone is a proxy for "own-spawned leak," not a direct
#   ownership check: `pgrep -f "$CI_REAP_PG_PATTERN"` (default "bin/postgres")
#   matches ANY postmaster with that substring in its command line — a
#   developer's system Postgres, another project's embedded PG, or a
#   concurrent agent `just ci`'s own engine — not only a concurrent agent ci.
#   Condition (ii) narrows the blast radius: a system/other-project Postgres
#   normally runs against a persistent, non-temp data directory, so it is
#   left alone even if it started after this run's baseline capture. The
#   residual window this does NOT close: a concurrent `just ci` on THIS
#   codebase whose own postmaster starts AFTER this run's baseline
#   snapshot — its data_dir IS under the temp root too, so it is
#   indistinguishable from an own-spawned leak and WILL be reaped if this
#   run then fails. R-11 explicitly permits parallel agent-ci with no
#   serialization, so that window is live, not hypothetical. Closing it
#   needs a true start-time-scoped ownership marker (e.g. tag each spawned
#   postmaster with the spawning ci's dispatch/run id) — a design change,
#   tracked separately as #2170, out of scope for this mechanism. This
#   mirrors scripts/flake-runner.sh's existing BASELINE_PM_PIDS /
#   reap_own_postmasters mechanism (condition (i)); the temp-root check
#   (condition (ii)) is new to this ci-scoped variant.
#
# WHY THE TEMP-ROOT CHECK
#   `postgresql_embedded`'s `.temporary(true)` engine creates its data
#   directory under Rust's `env::temp_dir()` (macOS/BSD: `$TMPDIR`, Linux
#   default: `/tmp` when `$TMPDIR` is unset) — the same environment variable
#   a sourced bash function reads directly, so this check needs no Rust
#   involvement at reap time. A system-package Postgres (Homebrew, apt) or
#   another project's engine almost never runs against a data directory
#   under the temp root, so requiring it materially reduces the "matches any
#   bin/postgres" exposure the plain baseline-diff carries on its own.
#
# FAILS CLOSED ON AN UNCAPTURED BASELINE
#   `ci_reap_own_postmasters` refuses to reap (returns 0, logs a warning) if
#   `ci_reap_capture_baseline` was never called first — see the capture
#   sentinel in both functions below. This distinguishes "baseline captured
#   and legitimately empty" (safe to reap everything matching) from
#   "capture never ran" (must never reap).
#
# WHY NOT PPID
#   An earlier design discriminated leaked-vs-live by PPID (kill iff
#   PPID==1 AND data_dir under the temp root). That was falsified before
#   build: `postgresql_embedded` starts PG via `pg_ctl`, which daemonizes the
#   postmaster to PPID 1 immediately on every instance, live or leaked — the
#   discriminator would kill a concurrent agent's live engine unconditionally.
#   Baseline-PID-diff is strictly better (it protects everything alive at
#   capture time, where PPID protected nothing), but is not a complete
#   ownership proof either — see ACTUAL GUARANTEE above.
#
#   Mirrors scripts/flake-runner.sh's BASELINE_PM_PIDS / reap_own_postmasters
#   pattern (same PID-set-diff idea; flake-runner.sh is left as-is — this is
#   a second, `ci`-scoped user of the same mechanic, not a refactor of the
#   first).
#
# USAGE
#   Sourced (not executed) by the `ci` justfile recipe:
#     source scripts/ci-reap.sh
#     ci_reap_capture_baseline          # call once, before the verify chain
#     ...run the verify chain...
#     ci_reap_own_postmasters           # call ONLY on failure/interrupt
#
#   Test-support: the pattern this matches against defaults to "bin/postgres"
#   (the substring `postgresql_embedded`'s downloaded binary path always
#   contains), but is overridable via CI_REAP_PG_PATTERN so a test can
#   substitute a unique synthetic marker instead of matching real
#   postmasters — see tests/ci_reap_baseline.rs, which drives this exact
#   script (not a reimplementation) against marker processes standing in for
#   postmasters. CI_REAP_BASELINE_FILE (TEST-ONLY SEAM, M2: the `ci` recipe
#   itself never sets it) is optional cross-process baseline + capture-
#   sentinel storage for that same test — see the two functions below; the
#   `ci` recipe sources this file once and keeps both the baseline and the
#   capture sentinel in the in-process CI_REAP_BASELINE_PIDS /
#   CI_REAP_BASELINE_CAPTURED variables across both function calls instead.

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "ci-reap.sh is a function library — source it, don't execute it directly." >&2
    echo "Used by: the 'ci' justfile recipe; tests/ci_reap_baseline.rs (via" >&2
    echo "'bash -c \"source scripts/ci-reap.sh && <function>\"')." >&2
    exit 1
fi

: "${CI_REAP_PG_PATTERN:=bin/postgres}"

# Capture the current set of matching PIDs as the baseline, and set the M3
# capture sentinel (distinct from an empty-but-captured baseline — see
# ci_reap_own_postmasters). Call once, before the verify chain starts.
# Writes the baseline + sentinel to CI_REAP_BASELINE_FILE-derived paths when
# set (TEST-ONLY SEAM, M2 — see the USAGE section at the top of this file for
# why: it lets the seeded test share captured state across the two separate
# bash processes it drives); otherwise keeps both in the in-process
# CI_REAP_BASELINE_PIDS / CI_REAP_BASELINE_CAPTURED shell variables.
ci_reap_capture_baseline() {
    CI_REAP_BASELINE_PIDS="$(pgrep -f "$CI_REAP_PG_PATTERN" 2>/dev/null || true)"
    CI_REAP_BASELINE_CAPTURED=1
    if [ -n "${CI_REAP_BASELINE_FILE:-}" ]; then
        printf '%s\n' "$CI_REAP_BASELINE_PIDS" >"$CI_REAP_BASELINE_FILE"
        : >"${CI_REAP_BASELINE_FILE}.captured"
    fi
}

# Extract the `-D <data_dir>` argument from a candidate PID's command line
# (M1 condition (ii) — see ACTUAL GUARANTEE at the top of this file). Prints
# nothing if the PID has no `-D` argument or has already exited; empty
# output is treated by _ci_reap_path_under_temp_root as "not under the temp
# root" (fail closed toward NOT reaping a candidate we can't classify).
_ci_reap_data_dir_for_pid() {
    local pid="$1" args
    args="$(ps -o args= -p "$pid" 2>/dev/null || true)"
    if [[ "$args" =~ -D[[:space:]]+([^[:space:]]+) ]]; then
        printf '%s' "${BASH_REMATCH[1]}"
    fi
}

# True iff $1 is a non-empty path under the embedded-PG temp root (see WHY
# THE TEMP-ROOT CHECK at the top of this file).
_ci_reap_path_under_temp_root() {
    local path="$1" temp_root="${TMPDIR:-/tmp}"
    [ -z "$path" ] && return 1
    temp_root="${temp_root%/}"
    case "$path" in
        "$temp_root"/*) return 0 ;;
        *) return 1 ;;
    esac
}

# Reap postmasters that are BOTH (i) NOT present in the baseline captured
# above AND (ii) running against a data directory under the embedded-PG temp
# root (M1 — see ACTUAL GUARANTEE at the top of this file). Call ONLY on
# failure or interrupt — never on a normal all-gates-pass completion (the
# embedded-PG engine already self-cleans on drop in that case). Best-effort:
# always returns 0 so it's safe to call from a script that then does its own
# exit-code handling.
#
# Limitation (see ACTUAL GUARANTEE at the top of this file): condition (i) is
# a proxy for "own-spawned leak," not a direct ownership check. A concurrent
# ci's postmaster (on this same codebase) that started after the baseline
# snapshot matches both conditions and would be reaped here too.
ci_reap_own_postmasters() {
    local baseline captured current_pids pid reaped=0 data_dir
    if [ -n "${CI_REAP_BASELINE_FILE:-}" ]; then
        # TEST-ONLY SEAM (M2) — see ci_reap_capture_baseline above.
        [ -f "${CI_REAP_BASELINE_FILE}.captured" ] && captured=1
        [ -f "$CI_REAP_BASELINE_FILE" ] && baseline="$(cat "$CI_REAP_BASELINE_FILE")"
    else
        captured="${CI_REAP_BASELINE_CAPTURED:-}"
        baseline="${CI_REAP_BASELINE_PIDS:-}"
    fi

    # M3: fail closed if capture never ran. Without this, "baseline
    # captured and legitimately empty" (safe: nothing was alive at capture
    # time, reap everything matching) is indistinguishable from "capture
    # never ran" (unsafe: reap nothing, we have no idea what predates this
    # run) — both otherwise present as baseline="". Not reachable from the
    # shipped `ci` recipe (capture always precedes reap there); this guards
    # a future third caller of ci_reap_own_postmasters that forgets to call
    # ci_reap_capture_baseline first.
    if [ -z "$captured" ]; then
        echo "[ci-reap] refusing to reap: baseline was never captured (call ci_reap_capture_baseline first) — failing closed" >&2
        return 0
    fi

    current_pids="$(pgrep -f "$CI_REAP_PG_PATTERN" 2>/dev/null || true)"
    while IFS= read -r pid; do
        [ -z "$pid" ] && continue
        # (i) present in the baseline -> alive at capture time -> never reap.
        if grep -qw "$pid" <<<"$baseline"; then
            continue
        fi
        # (ii) NOT under the embedded-PG temp root -> not one of ours ->
        # never reap either, even though absent from the baseline.
        data_dir="$(_ci_reap_data_dir_for_pid "$pid")"
        if ! _ci_reap_path_under_temp_root "$data_dir"; then
            continue
        fi
        kill -KILL "$pid" 2>/dev/null && reaped=$((reaped + 1)) || true
    done <<<"$current_pids"
    if [ "$reaped" -gt 0 ]; then
        echo "[ci-reap] killed $reaped own-spawned postmaster(s) not in baseline"
    fi
    return 0
}
