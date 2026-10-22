# mnemra-core — project instructions

The core context layer for Mnemra: an MCP-native host that gives agents persistent, structured,
queryable context. It is a Rust workspace containing the host (`libs/mnemra-host`), the
binary (`cmd/mnemra`), WebAssembly plugins loaded under a signature (`plugins/`, `wit/`,
`artifacts/`), and an embedded Postgres + pgvector store. The design record lives beside the code
under `docs/`.

⛔ **This repository is PUBLIC** (`github.com/mnemra/mnemra-core`). Everything pushed is published,
**including commit messages, commit bodies and branch names**. See "Public-repo disclosure" before
naming anything.

Written 2026-09-14 (task `#3567`) as the repo's first `CLAUDE.md`, when mnemra moved onto the new
working method. Every claim carries an evidence label and a date. **MEASURED** means run or read
against the code at `cf8210e` (the #4064 rewrite of the measured commit; the rewrite changed wording in docs only, INFERRED). **INHERITED** means taken from a record and not re-checked. **INFERRED**
means reasoned, not observed.

## Read this before treating anything here as settled

- **The working method is mid-migration.** `/brief`, `/build` and `/verify` are not migrated yet
  (`#3314`). Implementation work is dispatched directly to agents against plans that live **outside**
  this repo, in `~/work/brain/projects/mnemra/plans/`. Those are the legacy `/build`-era plans, still
  readable. ⚠ **Every `~/work/brain` path and `brain` command in this file resolves only on a machine
  with the maintainer's private brain checkout**, not from a public clone or in CI.
- **Where the work currently stands is not in this file.** It is in the session carrier
  (`brain carrier get --json`, run from the root you are in) and in the tracker
  (`brain task list --repo mnemra-core`). ⚠ Tasks filed before 2026-09-14 mostly carry **no repo
  tag**. Search the `mnemra` project for them too, or you will conclude work is untracked when it
  is not.
- **A half-built or self-contradictory state is expected.** Surface an inconsistency you weren't
  asked about; don't fix it.

## Two lanes, one repository

⭐ **Peter, 2026-09-14:** *"this instance of puck is the implementation portion of mnemra which used
to be mnemra-impl … There is a separate worktree running a puck for design."*

| Lane | Rooted in | Owns |
|---|---|---|
| Implementation | the primary checkout | code, tests, CI, landing implementation work |
| Design | a session launched in a worktree named `mnemra-design` (`.claude/worktrees/mnemra-design`) | intake, Frames, specs, ADRs, the feature register |

- ⛔ **The design worktree is a LANE worktree.** It will often be clean and level with `main`, which
  is exactly what a finished task worktree looks like. **Do not tear it down.** The tell is the name:
  a lane name is standing; an `agent-<hex>` or task-named worktree is finished when its task closes.
  ⚠ **It can disappear anyway.** On 2026-09-14 it was removed when the design session ended, although
  that lane's own carrier had just marked it standing (MEASURED). The cause is INFERRED: harness
  cleanup of a clean worktree when its session exits. Its absence means nothing about the design
  lane's state; read that lane's carrier.
- **The lanes coordinate by message** (`/puck:msg`), and they cross repo boundaries only through
  the tracker. ⚠ **Nothing in a task row separates the two lanes** (INFERRED from the schema,
  2026-09-14): both read the same `mnemra` rows, so a row assigned to Puck is not automatically
  yours. The fix is open.
- ⚠ **Open, NOT RULED: which lane may edit which files.** The design records are `docs/intent/`,
  `docs/specs/`, `docs/src/adrs/`, `docs/src/intent/` (brief, register, core Frame) and
  `docs/src/architecture/overview.md`. Until Peter rules, the interim default runs both ways:
  - The implementation lane reads the design records and **routes any change to them through the
    design lane**.
  - The design lane reads code, tests and CI and **routes any change to them through the
    implementation lane**.

  That default is a recommendation from both lanes, not a ruling. What *is* written down is in
  "Design records" below.
- ⚠ **The two lanes share `$TMPDIR`, and that is where embedded Postgres keeps its data.** See the
  reap trap under "Embedded Postgres". A red that looks like Postgres dying mid-test may be the
  other lane's failing `just ci`.

## ⛔ Known defect: host data does not survive a restart

⭐ **Peter, 2026-09-14:** *"If mnemra.core on restart loses its data that is a non-starter. it must be
persisted on the local filesystem."* And: *"the whole point of mnemra was to keep context persistent
… that is a ground level requirements defect."* Peter's concern, as relayed by the design lane:
*"agents reading the code now are assuming this is how it is meant to work."*

**It is not how it is meant to work.** MEASURED 2026-09-14:
- `EmbeddedEngine::start()` (`libs/mnemra-host/storage/postgres/engine.rs`) builds its settings with
  `.temporary(true)`, so every start gets a fresh data directory, deleted on drop.
- That one constructor serves the **production** host (`mnemra_host.rs:555`), `mnemra init`
  (`cmd/mnemra/cmd/init.rs:25`) and the tests.
- The database passwords are also derived per start (the `app_password` derivation just below the
  settings), so keeping the directory without the credentials would not reopen it (INFERRED).

- ⛔ **Do not copy the production engine settings as intended behaviour.** Do not write code, tests
  or docs that assume data is gone after a restart. **Temporary engines are correct only in test
  fixtures** (`tests/common/shared_engine.rs`).
- **Anything that presupposes durability runs against a temporary directory today.**
  - The `backup` trigger (spec R-0011-d).
  - **The ingestion spec (W2-2):** its intake requires catch-up across a restart
    (`docs/intent/ingestion-pipeline.md`, success criterion 11), and its Frame locks durable raw
    staging. Implementing ingestion against today's engine builds on a store that forgets.
- **Tracked in `#3565`.** The design lane has confirmed the gap. A canon-conformance review of the
  core intent, Frame, V0 spec and ingestion spec is running (`#3573`); the requirement amendment
  follows, pending Peter, and the code fix follows that. ⛔ **Delete this section when `#3565`
  lands.**

## Layout

| Path | Holds |
|---|---|
| `Cargo.toml` | Workspace: `libs/mnemra-host`, `cmd/mnemra` (the only default member), `cmd/sign-ceremony`, `plugins/mnemra-echo`. Edition 2024 |
| `libs/mnemra-host/` | The host library: `auth/`, `builtins/`, `coordination/`, `mcp/`, `plugin/`, `projection/`, `schema/`, `signing/`, `startup/`, `storage/`, `health.rs` |
| `libs/mnemra-host/tests/` | 42 integration-test binaries; `common/` helpers included via `#[path]`; `ui/no_test_seams/` trybuild fixtures |
| `cmd/mnemra/` | The host binary. No subcommand serves MCP on stdio; `init` bootstraps. Logs are JSON on **stderr**; stdout is MCP only |
| `cmd/sign-ceremony/` | The maintainer's signing tool. The runtime host never links it |
| `plugins/mnemra-echo/` | The V0 fixture plugin (wasm32-wasip2). `manifest.toml`'s `[component]` and `[signature]` sections are GENERATED by the ceremony |
| `wit/` | The host/plugin contract (`host.wit`, `echo.wit`) |
| `artifacts/mnemra-echo/mnemra_echo.wasm` | The **committed, signed** plugin binary. See the signed-artifact trap |
| `scripts/` | `ci-reap.sh` (sourced by `just ci`), `flake-runner.sh`, the three uv docs scripts |
| `tests/` (repo root) | pytest for the docs scripts |
| `docs/src/` | AUTHORED book source: brief, register, core Frame, ADRs, glossary, architecture |
| `docs/_published/` | ⛔ **GENERATED. Never hand-edit.** Translations plus `llms.txt` / `llms-full.txt` |
| `docs/intent/`, `docs/specs/` | Feature intakes and Frames; specs with `.bom.toml` sidecars and verify verdicts |
| `docs/prompts/`, `docs/runbooks/` | Translation prompts; the signing-ceremony runbook |
| `.claude/commands/docs-translate.md` | The translation command, which runs inside a Claude session |
| `.github/workflows/` | `ci.yml` (PR + push to main), `docs.yml` (gh-pages deploy) |

## Verify command — what green means, per surface

**The local gate is `just ci-full`.** It runs `just ci` (the full verify chain, coverage included,
inside the postmaster-reap wrapper) and then `just docs-check`. MEASURED 2026-09-14 on the commit that `cf8210e` rewrites (#4064; the rewrite changed wording in docs only, INFERRED):
exit 0 in 14 minutes **with a warm cache** (`~/.theseus` and `target/` already populated). A cold
machine is substantially slower; that was not measured. Since `#3597` the chain also carries
`verify-cold-start`, which downloads on every run regardless of cache: measured between 25s and 131s
locally on the same machine, so treat it as network-dependent rather than fixed.

- ⛔ **`just ci` alone is NOT the gate.** It skips `docs-check`, and GitHub's required "CI gates" job
  runs it (`justfile:662-680`, `ci.yml:75,78`; `#2543` tracks folding it in).
- ⛔ **`just ci` and `just ci-full` now REFUSE without `GITHUB_TOKEN`** (`#3597`). `verify-cold-start`
  always downloads Postgres and pgvector through the GitHub API, and the unauthenticated limit is 60
  calls an hour against roughly 6 per cold install, so a second run within the hour used to fail
  mid-chain on a 403. The gate stops up front instead. Mint a short-lived rate-limit-only token
  per run: `GITHUB_TOKEN="$(sigil-helper token --rate-limit-only mnemra/mnemra-core)" just ci-full`
  (metadata:read on one repo, valid 1 hour; `#4002`). Without sigil, export a fine-grained personal
  access token with no permissions and no repository access as `GITHUB_TOKEN`. Both source repos are
  public, so either buys only the authenticated rate limit. ⛔ Never `gh auth token` or any broad token: the recipe's environment is
  inherited by the Postgres binaries, which run before their own hash check (`#3616`).
- ⛔ **`just check` is not the gate either**, even though `README.md` names it. It is orphaned from
  CI parity.
- **Each gate prints `GATE <name> PASS|FAIL`.** Read those lines, not just the exit status.
  ⛔ `just ci-full | tail` exits with **tail's** status, so a failing gate reports success. Capture
  the output to a file and check `$?`.
- **The toolchain is pinned** (`#4107`) in `rust-toolchain.toml` (the exact three-part `channel`),
  the single source: locally rustup reads it, and every CI install step runs
  `rustup toolchain install` against the same file. A clippy or trybuild red with no code change is
  no longer a new stable release; check `rustc --version` against the file. **Bump:** edit
  `channel` in `rust-toolchain.toml`, run `just ci-full`, commit that one line (plus any new lint
  fixes). **Dependabot opens the bump** (`#4155`; policy is Peter's 2026-10-22 "always be on the
  latest update", and the cross-repo form of it is `#4154` in the shared tracker, not this repo):
  `.github/dependabot.yml` has one `rust-toolchain` entry, daily, `cooldown.default-days: 0` so a
  security point release is not held for Dependabot's default 3 days. It opens a PR per Rust
  release, point releases included, because the pin has three parts; a two-part pin would never
  receive them. The entry's shape is MEASURED against GitHub's Dependabot options reference on
  2026-10-22. ⛔ **The Dependabot path is INFERRED, and the hand check stays in force until it has
  been seen to run:** check the pin by hand at each new Rust stable release (about every 6 weeks)
  until a "Dependabot Updates" workflow run for `rust-toolchain` has been observed
  (`gh run list -R mnemra/mnemra-core --workflow "Dependabot Updates"`; its log should show it
  found `rust-toolchain.toml` and that the pin is current). **How a bump lands:** Dependabot's PR
  is not merged as-is. Its change, plus any lint fixes, is re-cut in a worktree as a normal commit
  carrying a tracker task number and landed through the usual PR; Dependabot then closes its own PR
  because the pin matches (Peter, 2026-10-22: "Re-cut as a task commit"). ⚠ **"A merge waits on
  Peter's go-ahead" is a convention, not a gate** (MEASURED 2026-10-22, `gh api
  repos/mnemra/mnemra-core/rulesets/25405721`: the `protect-main` pull-request rule has
  `required_approving_review_count: 0`; `gh api repos/mnemra/mnemra-core --jq .allow_auto_merge`:
  `true`), so anyone with write access can merge a green PR. Cargo and GitHub Actions are not
  covered by version updates (scope open on `#4154`); Dependabot security updates for Cargo run
  from the repo setting (open example: PR #1, rmcp).

| Surface | Green means | ⛔ What it does NOT cover |
|---|---|---|
| Host library | `just ci-full` | Clippy runs with `--all-targets` (`#2410`), so `tests/` is linted. **Test code gated behind `test-hooks` is still unlinted**: the gate passes no `--features test-hooks` to clippy (`#4009`) |
| A new integration-test binary | Add it to `PG_TEST_FLAGS` or `NONPG_TEST_FLAGS` **and** bump that list's count pin (`justfile:477`/`:481`) **in the same commit** — or, when it must not run three times over (an always-downloading suite, say), give it its own gate and wire that into **both** `_ci-verify-chain` and `_ci-verify-chain-nocoverage`, as `verify-smoke` and `verify-cold-start` do. ⛔ CI's gates job runs the `-nocoverage` variant, so a gate missing from it silently never runs. Any new `.rs` file under `libs/mnemra-host/tests/` must also bump `EXPECTED_TEST_FILE_COUNT` (`lint_engine_construction.rs:1198`) in the same commit, or `verify-lint` goes red on correct work | ⛔ **SEEDED 2026-09-14:** a failing `tests/zz_seeded_unlisted.rs` in neither list → `verify-coverage-membership` PASS. `verify-test` runs only listed binaries (`justfile:204-205`), so it never runs it. Since `#2408` landed, the same seed would trip the R-0032 lint's file-count pin in `verify-lint` (INFERRED from `lint_engine_construction.rs:1198`, not re-run), but nothing yet reconciles the files against the lists (`#2409`). Forgetting the pin bump instead turns the gate red on correct work |
| Tests in `coordination_messages.rs` | Bump the exact-count literals `18 passed` (`justfile:266`) and, for test-hooks tests, `20 passed` (`:356`) | ⚠ The comments beside those pins say 6 and 8. **Trust the literals** |
| A new `test-hooks` seam | Add a trybuild fixture, its `.stderr`, and a `compile_fail` line in `tests/no_test_seams.rs` | Without them, removing the seam's `cfg` gate fails nothing |
| CLI binary | `verify-build` + `verify-smoke`, the only gate that spawns the real binary | There are no unit tests for `cmd/mnemra` |
| Plugin / WIT | `just plugin` builds the wasm component; `verify-test`, `verify-test-hooks` and coverage depend on it | ⛔ **Bare `cargo test` does not rebuild the plugin**, so tests silently load a stale `target/wasm32-wasip2/release/mnemra_echo.wasm` (INFERRED). The committed signed artifact is a separate problem, below |
| Docs under `docs/src/` | `/docs-translate` → `just docs-llms` → `just docs-check`, then commit `docs/src/` and `docs/_published/` together | ⛔ **SEEDED 2026-09-14:** appending to `glossary.md` → `docs-check` exit 0; appending to a page → exit 1. **Edits to `SUMMARY.md` or `glossary.md` pass both drift checks.** Run the full sequence anyway. `mdbook build` is ungated (run `just docs`). `docs.yml` deploys on push to main without a drift check and never runs on PRs |
| CI workflow files | Nothing | `ci.yml` is not linted by anything |
| Postgres engine on a cold machine | `verify-cold-start` — its own gate in both CI chains (`#3597`). Four scenarios against an isolated `HOME`: a genuinely cold start, a tampered pgvector library, a present `vector.control` with the library missing, and a tampered `postgres` binary. It never touches the real `~/.theseus` and always downloads | ⛔ It does NOT cover verifying pgvector **only when the install step was skipped** — that wrong fix passes all four, and closing it needs a seam the API does not expose (named in the suite header). It also does not cover the `initdb`-runs-first gap (`#3616`). The earlier `SKIP`-with-no-assertion trap in `postgres_engine.rs` is gone: that test now hashes a throwaway file in a tempdir and runs everywhere |
| Supply chain | `verify-advisories` (`#3566`) — `cargo deny check` across advisories, bans, licenses and sources, wired into both `_ci-verify-chain` and `_ci-verify-chain-nocoverage`; `just ci` now needs `cargo-deny` available to `cargo` locally (checked via `cargo deny --version`, not raw PATH; a missing one is reported, with an install hint, only when `verify-advisories` runs — fourth in both chains, after `verify-type`, `verify-lint` and `verify-coverage-membership` — not up front). The recipe first refreshes cargo's local crates.io index cache with `cargo update --dry-run --locked` (`#4096`), because the `yanked` check reads that cache and neither `cargo deny check` nor `cargo deny fetch` refreshes it; a network-shaped refresh failure, an offline or quiet cargo, or a changed `Cargo.lock` fails the gate with a labelled `GATE advisories FAIL (apparatus — …)` line rather than passing on stale data, and CI is hardened too (rust-cache keeps sparse-index entries, so its cache can be stale). Three ignores (`#4017`): RUSTSEC-2026-0367, -0368 and -0369 (RustSec's import of GHSA-vv5m-jxxh-4627, GHSA-4v9h-p7m9-rh26 and GHSA-4684-h2q9-2vcp). ID-scoped: these silence the advisory for any zip version it affects, and no ban stops a second zip arriving. The `zip@8.6.0` yanked-crate ignore was removed: the zip owner yanked 8.6.0 on 2026-10-16 and un-yanked it the same day, the other 8.x versions followed on 2026-10-17, and on 2026-10-18 the maintainer repealed yanking known-insecure versions (zip-rs/zip2#1075). ⛔ **GHSA-m665-v7pm-7grj** (critical "Zip Cloaking" parser differential, zip <= 9.0.4 including 8.6.0, patched only in 9.0.5) is in neither RustSec nor GitHub's global database, so this gate and Dependabot do not see it. Not reachable here (MEASURED for `postgresql_*` 0.20.4: the only caller, `postgresql_archive` `src/extractor/zip_extractor.rs`, walks `ZipArchive::by_index` and uses neither `ZipStreamReader` nor `extract()`; "no other caller" is INFERRED, and the read goes stale on any `postgresql_*` bump). When RustSec imports it this gate goes red on an unchanged tree, and that needs Peter's ruling: a fourth id ignore, or the `#4018` exit. ⚠ **Interim**, per Peter's 2026-10-18 ruling ("extend it for now"): the fix is zip >=9.0.5 at minimum, blocked while `postgresql_archive` requires ^8. Exit is `#4018` (the embedded-Postgres decision), where all three lines must be deleted or `unused-ignored-advisory` fails the gate. **PARKED:** a patched 8.x backport (offered in `zip-rs/zip2#1075`) would fix this by `cargo update -p zip`, but `unused-ignored-advisory` fires only once all three are fixed, so "blocked" can go false with no signal; check by hand at the maintainer's stated 2026-10-30 backport date. The four wasmtime advisories were removed by the wasmtime 48.0.6 bump (`#3646`), which also turned component-model async off (`default-features = false`, `concurrency_support(false)`, `tests/component_async_rejected.rs`). `unused-ignored-advisory` turns any future stale ignore into a failure | Four named limits. **Red on an unchanged tree when a pinned or required version is yanked**: the refresh's fresh resolve fails with a plain `GATE advisories FAIL` showing cargo's text, and cargo-deny does not run (a real finding, not a network fault). **RustSec-only** (misses what GitHub's own advisory database carries; accepted per the `#3566` ruling). **Feature-resolved graph only**: `rustls`/`spin` are locked-but-never-compiled today and this gate never reports on either — a crate that starts being compiled is picked up automatically with no config change needed; the residual is no signal while it stays uncompiled. `unmaintained = "all"` can turn this gate red on an **unchanged** tree the day RustSec publishes a new advisory in that category — usually with no fixed version, so the fix is a maintainer call, not a patch bump |

## Remote and merge posture (current, measured 2026-09-14)

- **Remote:** `origin` → `github.com/mnemra/mnemra-core`, public. Remote head branches are deleted
  on merge.
- **`main` is protected by the `protect-main` ruleset** (active, no bypass actors). Rules:
  - a pull request is required;
  - **rebase-merge only** (squash and merge commits are also disabled at the repo level);
  - linear history;
  - no force-push and no deletion;
  - the status check **"CI gates"** is required, strictly (branch up to date).
  - ⚠ **The classic branch-protection API returns 404, and that does NOT mean unprotected.** Check
    `gh api repos/mnemra/mnemra-core/rulesets`.
- ⛔ **The two coverage jobs ("Coverage — PG shard", "Coverage — rest shard") are NOT required
  checks.** A PR can merge with coverage red. Read their result before asking for a merge.
- **Rebase-merge lands every commit on the branch individually.** Squash to one commit per delivery
  locally before pushing, and put the brain task number in the message.
- **Land with the `merge` skill.** For a GitHub-backed repo it names the by-hand procedure; follow
  it rather than hand-rolling `git push` + `gh pr create`.
- ⚠ **Carried from the July lane charter, re-offered to Peter 2026-09-14, not re-ruled:** each
  merge on the implementation lane waits for Peter's go-ahead. A review loop that reaches round 3
  with more than trivia goes to a design conversation, not a round 4.

## Security and integrity — enforced versus convention

MEASURED 2026-09-14 (Sage fact pass, key lines re-checked, then reviewed by Warden). ⛔ **"Convention
only" means a violation passes every gate.** The full gap list, with file:line, is `#3575`.

⛔ **Tenant isolation is host-side only.** There is no database row-level security at V0: RLS is
deferred to V0.1+ as accepted risk R-0001 (`docs/src/architecture/overview.md`). Despite the ADR named
`P-0009-rls-admin-token`, **no database layer backs up a missed workspace filter.** The first row below
is the whole tenant boundary.

| Invariant | Enforced by | ⛔ Gap |
|---|---|---|
| Every read path filters by workspace (R-0006-d) | `tests/lint_workspace_clause.rs`, default-deny (`#3575`) | **Presence only**: every `fn` (free, impl, trait default, nested) in any `.rs` under `libs/mnemra-host`, `cmd` or `plugins` (not a crate's own `tests/`/`target/`) whose body holds a SQL `SELECT` literal must read `.workspace_id` from a binding **declared** `WorkspaceCtx` or `CoordinationTxn`, matched by declared type token and module path (a bare name, or the host's own `auth::workspace_ctx` / `coordination::write_path` prefix), never by the receiver's name. It does NOT check that the value reaches the WHERE clause (`#1656`). The population is computed and count-pinned (`EXPECTED_SQL_FILE_COUNT`); unparseable files and stale exclusions fail; each exclusion fixes its SELECT count, and `CALL_PINS` pins production callers of the no-caller and trusted-identity functions. ⛔ A read with no workspace filter is a bug, never an `EXCLUSIONS` entry (`auth/token.rs::rotate` is the known gap, `#4014`). Not matched: writes with no `select` word, SQL split across literals, a trusted name re-bound by `let` in the same unit, a bare `WorkspaceCtx` that resolves elsewhere (held only by the `types` and `WitWorkspaceCtx` ident pins), two-segment `Call` pins missing a `use`d bare call or a value reference (the distinctive names are exact `Ident` pins), a forged `WorkspaceCtx::new` (`#1774`) |
| One `WorkspaceCtx` construction site | `tests/workspace_ctx.rs` syn scan | `cmd/` isn't scanned; unparseable files are skipped; `new` is `pub` (`#1774`) |
| Privileged coordination writes fail closed, audited in the same transaction | `tests/coordination_failclosed.rs` | Runs only under `test-hooks` (`verify-test-hooks`) |
| `coordination_audit` is append-only | **Convention only** | No REVOKE, trigger or CHECK |
| Test seams unreachable in the default build | trybuild `tests/no_test_seams.rs` | `insert_test_entry`, `plugin::sql_observe` and `inject_epoch_death_for_test` are `test-hooks`-gated today but have no fixture, so **removing a gate would fail nothing** |
| Plugins load only if signature and content hash verify | `signing/verify.rs`; `signing_chain`, `content_hash_binding`, `smoke_e2e` | — |
| A plugin may import only the host functions its SIGNED manifest declares (P-0003, R-0003-b), `#3842` | `component::build_linker` refuses an undeclared `mnemra:host/*` import at link time. `register_module` and `build_linker` take `VerifiedGrants`, whose constructor is `pub(crate)` and called only by `PluginRuntime::load` over the signed slice (trybuild: `unverified_allowlist_cannot_reach_register_module`). The MCP verb list and verb gate read the pool's verified grants, with no embedded manifest copy. `tests/host_fn_allowlist_enforcement.rs` drives `populate_verified_pool_from_dir` | Enforcement is by import, not by call, and mediates only `mnemra:host/*`: WASI imports stay inert trap-stubs. Linking a WASI host is blocked by a `deny.toml` ban on exactly three crates, `wasmtime-wasi`, `wasmtime-wasi-io` and `wasmtime-wasi-http` (a dependency-graph check, so `bans` is what fires); a hand-written `wasi:*` host body in `build_linker` is NOT covered by the ban; `plugin_world_imports_only_mnemra_host_interfaces` fails if the `plugin` world imports outside the package. `VerifiedGrants::deny_all()` is `pub` and grants nothing. The signed manifest's `[verbs]` and `[host_fns]` are trusted as far as the trust anchor the caller of `PluginRuntime::load` passes, and the ceremony, are; only `mnemra_host.rs` passes the embedded `ROOT` and nothing pins that |
| Signing root pin | `verify-signing-root` (requires exactly `1 passed`) | — |
| Admin token mode 600 before any listener binds | Startup check | Mode checks for the signing material and LLM key are defined but **not wired** |
| Outbound hostname allowlist (R-0014-b) | Tested | Not wired, but **no outbound call site exists yet**: no HTTP client in `libs/mnemra-host` (Warden, MEASURED). The risk is prospective: wire the allowlist in the same change that adds the first outbound call (`#1744`) |
| Raw token never in errors or logs (R-0004-h) | **Convention only** | — |
| Log payloads as structured fields (R-0075-e) | Tested at two sites | Convention for every new log site |
| Engine construction only through the shared fixture (R-0032) | `tests/lint_engine_construction.rs` (`#2408`): a token walk of every `.rs` file under `libs/mnemra-host/tests/` (count pinned at 62, `:1198`) for `EmbeddedEngine::start`, `start_embedded` and `postgresql_embedded`, skipping attributes, literals and `common/shared_engine.rs` (`:1241`). Runs in `verify-lint` (`justfile:178`; in both CI chains, `:885`/`:898`) and in `NONPG_TEST_FLAGS` (`:153`). 4 exceptions (`tests/common/no_migrate_set.rs:223-239`): whole-binary `startup_run_full` (must suppress exactly 0 sites), `schema_init` and `postgres_engine_cold_start`; function-only `coordination_schema::coordination_migrations_idempotent_and_tables_present_after_reinit` | ⛔ **A whole-binary exception covers every item in its file** (`no_migrate_set.rs:212`, no upper bound at `lint_engine_construction.rs:1325`), so a new raw engine in `schema_init.rs` or `postgres_engine_cold_start.rs` passes (cold-start: accepted cost, `no_migrate_set.rs:146-148`; `schema_init`: exits with `#2069`, `:116-122`). Also not caught: aliases (`use … EmbeddedEngine as Eng`, `type` aliases) and `Self::start()`, since the matcher needs the literal `EmbeddedEngine` ident followed by `start` (`lint_engine_construction.rs:217-277`); attribute arguments (skipped, `:222-237`); macros defined outside `tests/`, library entry points that build an engine (`run_with`) and anything outside `tests/`, since the walk is rooted at `tests/` (`:1189-1191`). Full list at `:69-89`. A new `.rs` under `tests/` turns the gate red until the count pin is bumped. Never construct an engine in a new test binary; use `tests/common/shared_engine.rs` |
| `tests/common/mod.rs` and `tests/storage_contract.rs` byte-locked (R-0037) | **Convention only** | Add helpers as new `#[path]` modules |
| `unsafe` code | **Convention only** (no `forbid`, no lints table; `#2550`) | — |
| Embedded Postgres binaries hash-pinned | `KNOWN_GOOD_HASHES` and `verify_pinned_artifacts` in `engine.rs`, called once per `ArtifactKind`: the `postgres` binary after `setup()`, the pgvector library after the install-or-skip step, both before `server.start()` (`#3597`) | **Pinned for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu` only**; other platforms refuse to start. ⛔ Only two files are pinned: `initdb`, `pg_ctl`, `pg_config` and pgvector's `.control`/`.sql` are not, and `setup()` runs `initdb` — so the binaries execute BEFORE the check (`#3616`) |
| Destructive migrations refused (A-18) | `schema/migrations.rs:105-122` | Matches by substring, so it also refuses an additive `ALTER TABLE … ADD COLUMN`. It fails loudly at init. Whether that was intended is NOT RECORDED |

## Embedded Postgres

- **First run downloads** Postgres 16.4.0 (theseus-rs GitHub releases) into `~/.theseus/postgresql/`
  and pgvector (portalcorp). It needs network access, and `GITHUB_TOKEN` is read if set (MEASURED).
  Roughly 6 API calls per cold install, against 60 an hour unauthenticated — which is why
  `verify-cold-start` demands a token up front.
- **The empty-`~/.theseus` failure is FIXED** (`#3597`, landed `fd4f807`). The pgvector pin check used
  to run before `install_extension()` wrote the file, so a cold install could never start and every
  Postgres CI job went red once the cache expired. Verification is now split by artifact kind. CI run
  `35159636684` (a pre-#4064 run that no longer resolves) proves it cold: all three jobs logged `Cache not found` and passed.
- **Postgres test binaries run with `--test-threads 1`** because of a teardown race (`#1852`). ⛔ Do
  not parallelise them to save time.
- ⛔ **`just ci` reaps leaked postmasters on failure or interrupt, and it can reap SOMEONE ELSE'S.**
  `scripts/ci-reap.sh` (MEASURED, `:24-36`): *"a concurrent `just ci` on THIS codebase whose own
  postmaster starts AFTER this run's baseline snapshot … WILL be reaped if this run then fails …
  that window is live, not hypothetical."* Both lanes share `$TMPDIR`. If a gate goes red with the
  engine dying mid-run, ask whether the other lane was running `just ci` before debugging the code.
  Bare `cargo test` has no reap at all.
- **`/health` binds 127.0.0.1:8877 by default.** Two local runs collide; set `MNEMRA_HEALTH_PORT`.
- **The binary is not relocatable.** Its root is the compile-time `CARGO_MANIFEST_DIR`, and it loads
  `plugins/` and `artifacts/` from there. A binary built in a worktree reads that worktree's files
  (MEASURED `mnemra_host.rs:664-669`).

## ⛔ The signed plugin artifact cannot be fixed by an agent

Production and `verify-smoke` load the **committed** `artifacts/mnemra-echo/mnemra_echo.wasm`, whose
BLAKE3 hash is signed into `plugins/mnemra-echo/manifest.toml`. Integration tests load the
`target/` rebuild instead.

- Editing `mnemra_echo.rs` or `wit/*.wit` leaves the tests green and the committed artifact stale.
- Editing a signed section of the manifest fails signature verification at startup.
- **Re-signing is a maintainer ceremony.** `docs/runbooks/signing-ceremony.md` states: *"No agent or
  automation ever touches the private key."*
- ⇒ **When a change would require re-signing, stop and report it.** There is no agent path.

## Design records — where they live and how they change

Sources: the design lane (MEASURED in its worktree, 2026-09-14) and the Sage fact pass.

- **Where each record lives:**
  - intake: `docs/intent/<slug>.md`; Frame: `docs/intent/<slug>-frame.md`
  - core product brief, feature register and core Frame: `docs/src/intent/`
  - spec: `docs/specs/<YYYY-MM-DD>-<slug>.md`, plus a `.bom.toml` sidecar holding the audit chain
  - project ADRs: `docs/src/adrs/P-NNNN-*.md`, with P-numbers reserved in
    `placeholder-resolution.md`
  - `docs/src/architecture/overview.md`: the **core Frame's quality-attribute tree** (from line 128)
    and constraint inventory, as well as the accepted-risk register. ⚠ The core Frame hands its
    quality attributes to this file, so reading only `mnemra-core-frame.md` misses them, and this
    tree is where durability went missing (design lane, MEASURED).
  - implementation plans: brain, not here
- ⛔ **A spec whose frontmatter says `status: approved` is LOCKED.** It changes by a recorded amendment
  or erratum, never by a silent edit. **An implementer who finds a spec wrong reports the deviation;
  they never correct the spec inside a code diff.** ⚠ **An accepted ADR changes by a dated, in-place
  `## Amendment YYYY-MM-DD` section** (maintainer ruling 2026-10-19), with supersession kept for a
  decision wholly replaced by a new record. The rule is stated in `docs/src/adrs/README.md`.
- **Register tiers:** idea → proposed → designed → committed → live, validated by which artifacts
  exist. Per the design lane, no tier authorizes a build, not even `committed`.
- ⛔ **Requirement IDs collide.** R-NNNN is meant to be one series across all specs (highest R-0115
  on 2026-09-14), but 16 IDs are defined in two specs each, and R-0001–R-0010 also name entries in
  the accepted-risk register (MEASURED, Sage). **Always cite the spec file with the R-ID**, and check
  the max across `docs/specs/` before assigning a new one (`#2126`).
- **Locked specs pin a base commit.** Check that pin for freshness when a plan picks the spec up;
  every landing ages it.
- ⚠ **G-NNNN citations (the general ADRs projected into `docs/src/adrs/DEFAULTS.md`) are being
  dissolved** (`#3299`, INHERITED). ⛔ **A citation in a locked document to canon or a G- ADR that no
  longer resolves, or whose text has changed since the document locked, gets flagged, not "fixed".**
  Live case (design lane, MEASURED): the ingestion spec's R-0113 anchors a reliability-value clause
  that canon has since struck.
- ⚠ **Some prose reads as authoritative and is wrong.** Trust the code over:
  - `README.md:13` (bundled Postgres; `#3376`)
  - the ADR README's "logs to stdout" (it's stderr)
  - `docs/src/specs/README.md` ("currently empty")
  - `DEFAULTS.md` G-0002 ("CI invokes only `just ci`")

## Public-repo disclosure

- **Gate #3**, a standing constraint on public Mnemra prose: **do not name the held plugin families
  or expose the personal dogfood domain.** Describe plugins only as extensible. It is scoped to plugin
  family names. ⛔ **Which names are held is deliberately NOT written here**, because writing them
  here would publish them. The ruling is at
  `~/work/brain/projects/mnemra/2026-07-15-blog-outline-gate-decisions.md`.
- ⚠ **Open, NOT RULED: whether gate #3 binds source code** (`#3377`). A whole-word, case-insensitive
  match for the held names hits 20 tracked files (MEASURED 2026-10-22). The breakdown and
  a reading of each hit are on `#4147`, not here, because describing them would narrow which names are
  held. ⛔ **Do not add new occurrences, and do not mass-rewrite the existing ones unasked.**
- ⛔ **The platform safety net is partial.** GitHub secret scanning, secret scanning push protection,
  generic (non-provider) pattern scanning and Dependabot security updates are all **enabled**
  (MEASURED 2026-10-22, `gh api repos/mnemra/mnemra-core --jq .security_and_analysis`). On
  2026-09-14 the first, second and fourth read **disabled**; who changed them and when is not
  known. Generic patterns were turned on through the API on 2026-10-22 by Puck on Peter's
  go-ahead. Per GitHub's supported-patterns page
  (`docs.github.com/en/code-security/secret-scanning/introduction/supported-secret-scanning-patterns`,
  read 2026-10-22), the generic set covers private keys (RSA, EC, OpenSSH, PGP, generic PEM),
  PostgreSQL, MySQL and MongoDB connection strings, and HTTP Basic and Bearer authentication
  headers. Still **disabled** on 2026-10-22: AI detection, validity checks and delegated bypass
  and dismissal. **Still uncovered:** personal details, machine paths and hostnames pass every
  scanner, and push protection blocks only the patterns it supports. The same page's capability
  table marks generic patterns as not covered by push protection by default and covered by
  configurable push protection; whether this repo's setting blocks generic patterns at push is
  **unverified** (the API field shows push protection enabled, not which pattern sets it blocks).
  A pushed secret or personal detail is public the moment it lands.
- Commit bodies and branch names publish too. Keep machine paths, hostnames and personal details
  out of them.

## What is still open

| Question | Where |
|---|---|
| Host data persistence and the missing durability requirement | `#3565` |
| Bundling Postgres into the binary | `#3562` |
| Which lane may edit the design records | Not recorded; Peter |
| Whether gate #3 binds source code | `#3377` |
| Making `just ci` the complete gate | `#2543` |
| Instance-setup fuel/epoch budget in `PluginPool::instantiate_live_slot` is an interim placeholder, unruled | `#4008` |
| Postgres binaries execute before their hash check; most install files unpinned | `#3616` |
| Cold-start gate follow-ups (template leak, stage enum, cache key, remediation hint) | `#3632` |
| Everything else the onboarding fact passes found unenforced | `#3575` |
