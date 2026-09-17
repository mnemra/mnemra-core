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
against the code at `1faf996`. **INHERITED** means taken from a record and not re-checked. **INFERRED**
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
inside the postmaster-reap wrapper) and then `just docs-check`. MEASURED 2026-09-14 on `1faf996`:
exit 0 in 14 minutes **with a warm cache** (`~/.theseus` and `target/` already populated). A cold
machine is substantially slower; that was not measured. Since `#3597` the chain also carries
`verify-cold-start`, which downloads on every run regardless of cache: measured between 25s and 131s
locally on the same machine, so treat it as network-dependent rather than fixed.

- ⛔ **`just ci` alone is NOT the gate.** It skips `docs-check`, and GitHub's required "CI gates" job
  runs it (`justfile:662-680`, `ci.yml:75,78`; `#2543` tracks folding it in).
- ⛔ **`just ci` and `just ci-full` now REFUSE without `GITHUB_TOKEN`** (`#3597`). `verify-cold-start`
  always downloads Postgres and pgvector through the GitHub API, and the unauthenticated limit is 60
  calls an hour against roughly 6 per cold install, so a second run within the hour used to fail
  mid-chain on a 403. The gate stops up front instead. Export a fine-grained token with **no
  permissions and no repository access** — both source repos are public, so it buys only the
  authenticated rate limit. ⛔ Never `gh auth token` or any broad token: the recipe's environment is
  inherited by the Postgres binaries, which run before their own hash check (`#3616`).
- ⛔ **`just check` is not the gate either**, even though `README.md` names it. It is orphaned from
  CI parity.
- **Each gate prints `GATE <name> PASS|FAIL`.** Read those lines, not just the exit status.
  ⛔ `just ci-full | tail` exits with **tail's** status, so a failing gate reports success. Capture
  the output to a file and check `$?`.
- ⚠ **The toolchain floats.** `rust-toolchain.toml` says `channel = "stable"`, and CI uses
  `@stable`. A clippy or trybuild red with no code change may be a new stable release; check
  `rustc --version` before debugging the diff.

| Surface | Green means | ⛔ What it does NOT cover |
|---|---|---|
| Host library | `just ci-full` | **Clippy never compiles `tests/`**: there is no `--all-targets` (MEASURED by Bolt, 2026-09-14; `#2410`). A lint confined to `tests/*.rs` passes |
| A new integration-test binary | Add it to `PG_TEST_FLAGS` or `NONPG_TEST_FLAGS` **and** bump that list's count pin (`justfile:463`/`:467`) **in the same commit** — or, when it must not run three times over (an always-downloading suite, say), give it its own gate and wire that into **both** `_ci-verify-chain` and `_ci-verify-chain-nocoverage`, as `verify-smoke` and `verify-cold-start` do. ⛔ CI's gates job runs the `-nocoverage` variant, so a gate missing from it silently never runs | ⛔ **SEEDED 2026-09-14:** a failing `tests/zz_seeded_unlisted.rs` in neither list → `verify-coverage-membership` PASS. `verify-test` runs only listed binaries (`justfile:190-193`), so it never runs it. Nothing reconciles the files against the lists (`#2409`). Forgetting the pin bump instead turns the gate red on correct work |
| Tests in `coordination_messages.rs` | Bump the exact-count literals `18 passed` (`justfile:266`) and, for test-hooks tests, `20 passed` (`:356`) | ⚠ The comments beside those pins say 6 and 8. **Trust the literals** |
| A new `test-hooks` seam | Add a trybuild fixture, its `.stderr`, and a `compile_fail` line in `tests/no_test_seams.rs` | Without them, removing the seam's `cfg` gate fails nothing |
| CLI binary | `verify-build` + `verify-smoke`, the only gate that spawns the real binary | There are no unit tests for `cmd/mnemra` |
| Plugin / WIT | `just plugin` builds the wasm component; `verify-test`, `verify-test-hooks` and coverage depend on it | ⛔ **Bare `cargo test` does not rebuild the plugin**, so tests silently load a stale `target/wasm32-wasip2/release/mnemra_echo.wasm` (INFERRED). The committed signed artifact is a separate problem, below |
| Docs under `docs/src/` | `/docs-translate` → `just docs-llms` → `just docs-check`, then commit `docs/src/` and `docs/_published/` together | ⛔ **SEEDED 2026-09-14:** appending to `glossary.md` → `docs-check` exit 0; appending to a page → exit 1. **Edits to `SUMMARY.md` or `glossary.md` pass both drift checks.** Run the full sequence anyway. `mdbook build` is ungated (run `just docs`). `docs.yml` deploys on push to main without a drift check and never runs on PRs |
| CI workflow files | Nothing | `ci.yml` is not linted by anything |
| Postgres engine on a cold machine | `verify-cold-start` — its own gate in both CI chains (`#3597`). Four scenarios against an isolated `HOME`: a genuinely cold start, a tampered pgvector library, a present `vector.control` with the library missing, and a tampered `postgres` binary. It never touches the real `~/.theseus` and always downloads | ⛔ It does NOT cover verifying pgvector **only when the install step was skipped** — that wrong fix passes all four, and closing it needs a seam the API does not expose (named in the suite header). It also does not cover the `initdb`-runs-first gap (`#3616`). The earlier `SKIP`-with-no-assertion trap in `postgres_engine.rs` is gone: that test now hashes a throwaway file in a tempdir and runs everywhere |
| Supply chain | Nothing | ⛔ **No advisory or licence scan runs anywhere.** `deny.toml` exists but `cargo deny` is never invoked (`#3566`), and GitHub's Dependabot security updates are disabled |

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
| Every read path filters by workspace (R-0006-d) | `tests/lint_workspace_clause.rs` | **Scans 5 hard-coded files** (`:623-640`). 16 other production files contain SQL and are outside the list, including all of `coordination/` (`#3575`). Within a scanned file it inspects only top-level `fn` items whose return type text contains `Option` or `Vec` (`:150-165`), so impl methods and other wrapper return types are invisible. It checks only that `ctx.workspace_id` appears. **A read path anywhere else passes silently, and a renamed scanned file is skipped with a message and passes.** Add any new SQL-bearing file to the list |
| One `WorkspaceCtx` construction site | `tests/workspace_ctx.rs` syn scan | `cmd/` isn't scanned; unparseable files are skipped; `new` is `pub` (`#1774`) |
| Privileged coordination writes fail closed, audited in the same transaction | `tests/coordination_failclosed.rs` | Runs only under `test-hooks` (`verify-test-hooks`) |
| `coordination_audit` is append-only | **Convention only** | No REVOKE, trigger or CHECK |
| Test seams unreachable in the default build | trybuild `tests/no_test_seams.rs` | `insert_test_entry`, `plugin::sql_observe` and `inject_epoch_death_for_test` are `test-hooks`-gated today but have no fixture, so **removing a gate would fail nothing** |
| Plugins load only if signature and content hash verify | `signing/verify.rs`; `signing_chain`, `content_hash_binding`, `smoke_e2e` | — |
| Signing root pin | `verify-signing-root` (requires exactly `1 passed`) | — |
| Admin token mode 600 before any listener binds | Startup check | Mode checks for the signing material and LLM key are defined but **not wired** |
| Outbound hostname allowlist (R-0014-b) | Tested | Not wired, but **no outbound call site exists yet**: no HTTP client in `libs/mnemra-host` (Warden, MEASURED). The risk is prospective: wire the allowlist in the same change that adds the first outbound call (`#1744`) |
| Raw token never in errors or logs (R-0004-h) | **Convention only** | — |
| Log payloads as structured fields (R-0075-e) | Tested at two sites | Convention for every new log site |
| Engine construction only through the shared fixture (R-0032) | ⛔ **Nothing on `main`.** The guard lint is unlanded work tracked in `#2408`; its branch is not on `origin`, so check the task, not `git branch -r` | Never construct an engine in a new test binary; use `tests/common/shared_engine.rs` |
| `tests/common/mod.rs` and `tests/storage_contract.rs` byte-locked (R-0037) | **Convention only** | Add helpers as new `#[path]` modules |
| `unsafe` code | **Convention only** (no `forbid`, no lints table; `#2550`) | — |
| Embedded Postgres binaries hash-pinned | `KNOWN_GOOD_HASHES` and `verify_pinned_artifacts` in `engine.rs`, called once per `ArtifactKind`: the `postgres` binary after `setup()`, the pgvector library after the install-or-skip step, both before `server.start()` (`#3597`) | **Pinned for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu` only**; other platforms refuse to start. ⛔ Only two files are pinned: `initdb`, `pg_ctl`, `pg_config` and pgvector's `.control`/`.sql` are not, and `setup()` runs `initdb` — so the binaries execute BEFORE the check (`#3616`) |
| Destructive migrations refused (A-18) | `schema/migrations.rs:105-122` | Matches by substring, so it also refuses an additive `ALTER TABLE … ADD COLUMN`. It fails loudly at init. Whether that was intended is NOT RECORDED |

## Embedded Postgres

- **First run downloads** Postgres 16.4.0 (theseus-rs GitHub releases) into `~/.theseus/postgresql/`
  and pgvector (portalcorp). It needs network access, and `GITHUB_TOKEN` is read if set (MEASURED).
  Roughly 6 API calls per cold install, against 60 an hour unauthenticated — which is why
  `verify-cold-start` demands a token up front.
- **The empty-`~/.theseus` failure is FIXED** (`#3597`, landed `72f6437`). The pgvector pin check used
  to run before `install_extension()` wrote the file, so a cold install could never start and every
  Postgres CI job went red once the cache expired. Verification is now split by artifact kind. CI run
  `35159636684` proves it cold: all three jobs logged `Cache not found` and passed.
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
  they never correct the spec inside a code diff.** ⚠ **Accepted ADRs have changed two ways:** by
  supersession, or by a dated `§Amendment` section added in place (precedent:
  `P-0003-plugin-manifest.md` "Amendment 2026-07-07"). `docs/src/adrs/README.md` says supersession
  only; practice diverged (MEASURED). Follow the precedent and flag the conflict; don't pick
  silently.
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
- ⚠ **Open, NOT RULED: whether gate #3 binds source code** (`#3377`). A held family name already
  appears in 17 tracked files (MEASURED 2026-09-14). ⛔ **Do not add new occurrences, and do not
  mass-rewrite the existing ones unasked.**
- ⛔ **No platform safety net catches a leak.** On this repo, GitHub secret scanning, push protection
  and Dependabot security updates are all **disabled** (MEASURED 2026-09-14,
  `gh api repos/mnemra/mnemra-core --jq .security_and_analysis`). A pushed secret or personal detail
  is public the moment it lands.
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
| No advisory or licence scan | `#3566` |
| Untracked advisories, including wasmtime RUSTSEC-2026-0269 (8.8) | `#3617` |
| Postgres binaries execute before their hash check; most install files unpinned | `#3616` |
| Cold-start gate follow-ups (template leak, stage enum, cache key, remediation hint) | `#3632` |
| Everything else the onboarding fact passes found unenforced | `#3575` |
