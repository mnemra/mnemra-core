---
status: locked
supersedes: null
superseded_by: null
---

# Feature: mdBook docs scaffolding + mdbook-d2 preprocessor

> **Spec for:** mnemra-core developer documentation home + D2 diagram preprocessor
> **Date:** 2026-05-07

## Purpose

Stand up mnemra-core's developer documentation site and the D2 diagram preprocessor it depends on, so that subsequent ADRs and architecture documents have an authoritative rendering home with both Mermaid (GitHub-native render) and D2 (architecture-grade render in the built docs site) support from day one.

## Requirements

The crate `mdbook-d2` SHALL be added to the Cargo workspace at `tools/mdbook-d2/`.

The `mdbook-d2` binary MUST implement the [mdBook preprocessor JSON protocol](https://rust-lang.github.io/mdBook/for_developers/preprocessors.html): read the (preprocessor-context, book) JSON tuple from stdin, emit a transformed book JSON to stdout, exit 0 on success, non-zero on failure.

The `mdbook-d2` binary SHALL detect fenced code blocks tagged with the `d2` language identifier in book chapters, render each block by shelling out to the `d2` CLI, and replace the block with an inline SVG element in the rendered HTML.

The `mdbook-d2` binary SHALL support the `supports` subcommand returning exit code 0 for the `html` renderer and non-zero for any other renderer (per the mdBook preprocessor protocol).

The `mdbook-d2` binary SHALL pass D2 source to the `d2` CLI via stdin (or via a temporary file whose path is constructed without interpolation of fenced-block content), NEVER via shell-interpreted command-line arguments. The preprocessor SHALL NOT invoke `d2` through `sh -c`, `bash -c`, or any other shell wrapper.

For edge inputs (empty fenced block, whitespace-only content), the preprocessor SHALL NOT pre-validate the source — it SHALL pass the content to the `d2` CLI unchanged and surface whatever exit code and stderr the CLI produces. This preserves the thin-wrapper invariant: the preprocessor adds no special-case behavior beyond what the CLI does.

When the `d2` CLI is not on `PATH`, the preprocessor SHALL exit with a non-zero status and emit a clear, actionable error to stderr naming the missing binary and the install hint.

When the `d2` CLI returns a non-zero exit on a given block, the preprocessor SHALL surface the `d2` stderr verbatim and exit non-zero — it MUST NOT swallow the error or emit partial output. The preprocessor SHALL NOT enrich `d2`'s stderr output with environment variables, host paths, or any value not already present in `d2`'s native stderr.

The `mdbook-d2` binary SHALL NOT introduce a runtime dependency on `tokio`, async runtimes, or any HTTP client. It is a synchronous shell-out wrapper.

The `mdbook-d2` binary SHALL NOT pin the `d2` CLI version internally; the version is controlled by the `d2` install on the build host (CI pins it explicitly in the workflow).

A `docs/` directory SHALL be created at the repo root containing the mdBook source (`book.toml`, `src/SUMMARY.md`, and the chapter files listed under "Tasks").

The `docs/book.toml` configuration SHALL declare both `mdbook-mermaid` and `mdbook-d2` as preprocessors and SHALL configure only the `html` renderer for V0.

The `docs/book.toml` configuration SHALL set `book.title = "mnemra-core"` and SHALL set `output.html.git-repository-url = "https://github.com/mnemra/mnemra-core"`.

The `docs/src/SUMMARY.md` file SHALL list at minimum: an introduction chapter, an ADR section header, and an ADR template chapter. Additional chapters MAY be listed as placeholders if helpful for navigation, but the chapters they reference MUST exist as files (no broken `SUMMARY.md` entries).

The `docs/src/adrs/template.md` file SHALL be MADR-shaped with YAML frontmatter containing the fields `status`, `date`, `decision-makers`, `consulted`, `informed`, `supersedes`, and `superseded_by`. The `status` field SHALL accept the enum values `proposed | rejected | accepted | deprecated | superseded`. The page body SHALL contain explanatory prose describing how to use the `superseded_by` field so future ADR authors can use it correctly.

A `.github/workflows/docs.yml` workflow file SHALL be created. It SHALL trigger on push to `main` (paths-filtered to `docs/**`, `tools/mdbook-d2/**`, and the workflow file itself) and on `workflow_dispatch`. It SHALL NOT trigger on `pull_request_target`, `workflow_run`, `repository_dispatch`, or `schedule`. It SHALL deploy the built site to the `gh-pages` branch.

The workflow SHALL build `mdbook-d2` first (`cargo build --release -p mdbook-d2`), prepend its target directory to `PATH`, install the pinned `d2` CLI, install `mdbook` via `peaceiris/actions-mdbook`, install `mdbook-mermaid` separately (e.g., via `cargo install --locked mdbook-mermaid` pinned to a specific version, or via a dedicated setup action), then run `mdbook build docs/`.

The workflow SHALL include an `actionlint` step (or equivalent enforcement mechanism) that fails the run when any third-party action reference is not a 40-character commit SHA — i.e., flags `@main`, `@latest`, or any tagged version like `@v2`. This is the structural enforcement of the pin discipline; the SHALL clause above lives in human-review territory without it.

The workflow SHALL declare a top-level `concurrency` block: `concurrency: { group: pages-deploy, cancel-in-progress: false }`. Two rapid pushes serialize; `cancel-in-progress: false` is intentional — a cancelled deploy mid-publish leaves `gh-pages` in an undefined state, which is worse than a slightly-stale site.

The workflow SHALL NOT push to any branch other than `gh-pages`. It MUST NOT modify `main` or any other long-lived branch.

The workflow SHALL grant only `contents: write` permission and SHALL NOT request `id-token`, `pages` deploy, or any other elevated permission for V0.

The `justfile` `check` recipe SHALL be modified so that the `cargo test` invocation includes the new crate. The simplest discharge is to change `cargo test` to `cargo test --workspace` (matching the parallel `cargo clippy --workspace` line); verify that the existing `--exclude mnemra-echo` carve-out does not accidentally exclude the new crate.

The `justfile` SHALL gain a `docs` recipe that builds the preprocessor and runs `mdbook build docs/`.

The `justfile` SHALL gain a `docs-serve` recipe that builds the preprocessor and runs `mdbook serve docs/` for live local preview.

The `Cargo.toml` workspace `members` array SHALL include `tools/mdbook-d2`. The `default-members` array MAY remain unchanged.

## Out of Scope

- **Authoring any actual ADRs** (e.g., ADR-14 through ADR-17, tracked separately). They consume this docs home; their content is not part of this spec.
- **README.md updates linking to the docs site.** Added in a separate follow-up commit after the maintainer manually enables GitHub Pages with the `gh-pages` branch as source. Doing it in this spec produces a broken link until the manual settings step lands.
- **Authoring D2 source diagrams.** None ship in V0; the preprocessor is verified by the smoke-test scenario alone.
- **Authoring Mermaid diagrams beyond what the ADR template includes.** Same scope discipline as D2 above.
- **Adding mdBook preprocessors or backends beyond `mdbook-mermaid` and `mdbook-d2`.** No `mdbook-toc`, `mdbook-katex`, `mdbook-linkcheck`, etc., even if helpful — every additional preprocessor expands the V0 surface.
- **Multi-repo docs aggregation** (Antora, etc.). Per the workspace tooling decision, per-repo mdBook with workspace-level link-out is the current answer.
- **The PDF / EPUB renderers** (`mdbook` supports them via additional backends; not in V0).
- **Custom mdBook themes / CSS overrides.** Default theme only for V0.
- **Pinning a specific `d2` CLI version inside the preprocessor crate.** Version is workflow-controlled, not crate-controlled.
- **Caching `d2` invocations** across mdBook runs (any future caching is post-V0 if rebuild speed becomes a problem).
- **Making the preprocessor available as a published crate on crates.io** in V0. It lives inside the mnemra-core workspace; publishing is post-V0 if external projects want to depend on it.
- **Rapid rollback runbook for the docs site.** The site has no SLO that justifies one (see Operational Requirements / Rollback).

## Scenarios

### Scenario: Preprocessor smoke test with sample D2 input

**Given** the `d2` CLI is installed and on `PATH`
**And** a test fixture mdBook page contains a fenced block tagged `d2` with valid D2 source (e.g., `a -> b`)
**When** `mdbook build docs/` runs
**Then** the build exits 0
**And** the rendered HTML contains an inline `<svg>` element where the fenced block was
**And** the original D2 source does NOT appear verbatim in the rendered output

### Scenario: `supports` subcommand accepts html, rejects others

**Given** the `mdbook-d2` binary is built
**When** `mdbook-d2 supports html` is run
**Then** the process exits 0
**And** when `mdbook-d2 supports epub` (or any non-html renderer like `pdf`, `latex`) is run, the process exits non-zero

### Scenario: Preprocessor errors clearly when `d2` is missing

**Given** the `d2` CLI is NOT on `PATH`
**And** a test fixture mdBook page contains any fenced block tagged `d2`
**When** `mdbook build docs/` runs
**Then** the build exits non-zero
**And** stderr contains a message naming the missing `d2` binary and an install hint

### Scenario: Preprocessor surfaces D2's compile errors verbatim

**Given** the `d2` CLI is installed
**And** a test fixture mdBook page contains a fenced block tagged `d2` with malformed D2 syntax
**When** `mdbook build docs/` runs
**Then** the build exits non-zero
**And** stderr contains the full `d2` error output (line numbers, error message)
**And** the preprocessor adds no environment-derived enrichment to the error
**And** no partial SVG is emitted in the rendered output

### Scenario: Empty D2 fenced block surfaces d2's behavior

**Given** the `d2` CLI is installed
**And** a test fixture mdBook page contains a fenced block tagged `d2` with empty content (zero bytes between the fences)
**When** `mdbook build docs/` runs
**Then** the preprocessor passes the empty content to `d2` unchanged
**And** the build outcome matches `d2`'s native behavior (exit 0 with empty SVG, or non-zero with `d2`'s error verbatim)
**And** the preprocessor adds no special-case handling

### Scenario: D2 source with shell metacharacters does not execute unintended commands

**Given** the `d2` CLI is installed
**And** a test fixture mdBook page contains a fenced block tagged `d2` whose content includes shell metacharacters (semicolons, backticks, `$(echo pwned)`)
**When** `mdbook build docs/` runs
**Then** the `d2` CLI receives the content as a literal argument, not interpreted by a shell
**And** no injected command is executed (verifiable by absence of expected side-effect file)
**And** the build either succeeds (if `d2` accepts the content) or fails with `d2`'s error verbatim

### Scenario: Preprocessor leaves non-D2 blocks untouched

**Given** the `d2` CLI is installed
**And** a test fixture mdBook page contains a fenced block tagged `mermaid` and another tagged `rust`
**When** `mdbook build docs/` runs
**Then** the build exits 0
**And** the `mermaid` block is not modified by `mdbook-d2` (mdbook-mermaid handles it separately, transforming it to its own render target)
**And** the `rust` block is rendered as a normal code block

### Scenario: ADR template frontmatter is complete and rendered correctly

**Given** the `docs/src/adrs/template.md` file exists
**When** the file is read
**Then** the YAML frontmatter contains all seven fields: `status`, `date`, `decision-makers`, `consulted`, `informed`, `supersedes`, `superseded_by`
**And** the `status` field documents the allowed enum values: `proposed | rejected | accepted | deprecated | superseded`

**Given** the docs site has been built locally with `just docs`
**When** opening the rendered ADR template page in a browser
**Then** the page displays the MADR sections (Status, Context and Problem Statement, Decision Drivers, Considered Options, Decision Outcome, Pros and Cons of the Options, More Information)
**And** the YAML frontmatter is NOT rendered as visible page content
**And** the page body contains explanatory prose for the `superseded_by` field

### Scenario: Local docs serve picks up edits

**Given** `just docs-serve` is running
**When** an editor saves a change to any file under `docs/src/`
**Then** the served site rebuilds within a few seconds
**And** the browser auto-reloads to show the change

### Scenario: Deploy workflow runs end-to-end

**Given** a commit lands on `main` that touches `docs/src/intro.md`
**When** the `docs.yml` workflow runs
**Then** `actionlint` (or chosen pin enforcement step) runs and exits 0
**And** it builds the preprocessor crate
**And** it installs the pinned `d2` CLI
**And** it installs `mdbook` and `mdbook-mermaid`
**And** `mdbook build` produces output under `docs/book/`
**And** `peaceiris/actions-gh-pages` publishes that output to the `gh-pages` branch
**And** the workflow exits 0

### Scenario: Deploy workflow does not run on unrelated changes

**Given** a commit lands on `main` that touches only `cmd/mnemra/main.rs`
**When** the GitHub Actions trigger evaluates the path filter
**Then** the `docs.yml` workflow does NOT run

### Scenario: `cargo test --workspace` runs preprocessor tests

**Given** the new crate is a workspace member
**And** the `justfile` `check` recipe has been updated to invoke `cargo test --workspace`
**When** `just check` runs
**Then** the preprocessor's tests are included in the run
**And** all tests pass

## Constraints

The workspace tooling decision (mdBook + Mermaid + D2 via custom preprocessor; D2 architecture-grade in built site, Mermaid GitHub-native in committed `.md`) is binding for this spec. No alternative tools are in scope.

Crate layout SHALL follow the existing mnemra-core convention:
- Directory: `tools/mdbook-d2/` (hyphenated package name as directory name)
- Binary entry: `tools/mdbook-d2/main.rs` with `[[bin]]` declaration in `Cargo.toml` setting `name = "mdbook-d2"` and `path = "./main.rs"`
- Module files (if any) live in the crate root alongside `main.rs`; no `src/` subdirectory
- Workspace dependencies declared in root `Cargo.toml` and consumed via `workspace = true`

Edition: 2024 (workspace default).

Error handling: `Result<T, E>` with descriptive error types; no `.unwrap()` or `.expect()` in non-test code; use `?` with `From` impls for propagation.

Formatting: `cargo fmt` clean, `cargo clippy --workspace -- -D warnings` clean.

License: MIT (workspace default; preprocessor inherits).

The preprocessor SHOULD use stdlib `std::process::Command` to invoke `d2`; `tokio::process` is unnecessary for a synchronous shell-out and adds runtime weight.

JSON parsing for the mdBook preprocessor protocol SHOULD use `serde_json` with `serde`-derived types for the relevant book/chapter shapes; round-tripping through `serde_json::Value` for the entire document is acceptable but typed structs are preferred for the fields actually inspected.

The `peaceiris/actions-mdbook` and `peaceiris/actions-gh-pages` actions SHALL be pinned to specific 40-character commit SHAs. Tagged versions (`@v2`) and floating refs (`@main`, `@latest`) SHALL NOT be used; tags are owner-mutable and re-tagging is the documented supply-chain attack vector (e.g., the `tj-actions/changed-files` 2025 incident). The `actionlint` workflow step enforces this structurally.

The `d2` CLI installation in the workflow SHALL pin a specific release version (e.g., via the official install script with a version flag, or by downloading a specific GitHub Release asset).

GitHub Pages source MUST be the `gh-pages` branch (manual settings step performed by the maintainer post-merge — out of scope for the agent).

Push to the public `mnemra/mnemra-core` repository remains gated by explicit maintainer approval; the squash-merge to `main` happens in the local repo, push happens on a separate human-confirmed step.

## Operational Requirements

### Deployment Impact

The workflow performs a force-push to the `gh-pages` branch on every successful run. The `gh-pages` branch is the deploy artifact, not a source-of-truth branch — it SHALL NOT be edited by humans, and `main` SHALL NOT be configured to merge from it. First-deploy creates the branch; the GitHub Pages settings step (selecting `gh-pages` branch as source) is performed AFTER the first successful run.

### State Changes

The `gh-pages` branch accumulates one commit per deploy. `peaceiris/actions-gh-pages` supports history pruning via `keep_files`/`force_orphan` options; V0 accepts unbounded history. Pruning policy is post-V0 if branch size becomes operationally problematic (target trigger: gh-pages history >100 commits or >100 MB).

### Health & Availability

The docs site is best-effort in V0 — no SLA, no synthetic monitoring, no alerting on deploy failure beyond GitHub's default workflow-failure email to the repo admin. The site is read-only published content; no live system to monitor.

### Observability

Workflow run logs in the GitHub Actions UI are the sole observability surface. No external log aggregation, no metrics export. Failed runs surface as red checks on the commit and via GitHub's default email notification to the repo admin.

### Degradation Behavior

A failed workflow run leaves the previously-deployed `gh-pages` content in place. The site does not "go down" on deploy failure — it stays on the prior version until a successful redeploy. A failed `mdbook build` step (e.g., D2 syntax error in a chapter) does NOT publish a partial site.

### Rollback

Rollback is **docs-not-code posture**: broken or wrong content is fixed by the next commit. Build time (preprocessor + d2 install + mdbook build) is fast enough that revert-and-redeploy is the appropriate path. **Rapid rollback runbook is explicitly out of scope** — the docs site has no SLO that justifies the runbook overhead. (Privacy note: accidental publication of private content is treated as a content-leak incident, not a rollback scenario; force-push to `gh-pages` does not unpublish content already mirrored to GitHub caches.)

### Concurrency

The workflow declares `concurrency: { group: pages-deploy, cancel-in-progress: false }`. Two rapid pushes to `docs/**` are serialized — the second deploy waits for the first to complete. `cancel-in-progress: false` is intentional: a cancelled deploy mid-publish leaves `gh-pages` in an undefined state, which is worse than a slightly-stale site.

### Pin Enforcement

Pin discipline (SHA-pinned third-party actions, version-pinned `d2` CLI) is enforced structurally via an `actionlint` step in the workflow itself. Pin drift is caught in CI on the workflow's own next change, not at human-review time.

## Operational Test Scenarios

Black-box test scenarios for QA / acceptance verification, complementing the Scenarios section above. Most are runtime-only (post-merge); flagged where applicable.

- **Concurrent-push race:** push two consecutive commits to `main` each touching `docs/src/intro.md` within ~30 seconds; verify both runs queue serially, both complete successfully, and the second run's content is what ends up published on `gh-pages`. *(post-merge)*
- **Path-filter negative test:** push a commit touching only `cmd/mnemra/main.rs`; verify the `docs.yml` workflow does not run. *(post-merge)*
- **Path-filter positive test (workflow self-edit):** push a commit touching only `.github/workflows/docs.yml` (e.g., a comment change); verify the workflow runs and deploys (path filter includes the workflow file itself). *(post-merge)*
- **Workflow-dispatch manual trigger:** trigger the workflow manually via the GH Actions UI; verify it builds and deploys end-to-end without any push. *(post-merge)*
- **Failed build leaves prior site live:** push a commit introducing a malformed D2 block; verify `mdbook build` fails, no `gh-pages` push happens, and the previously-published site is unchanged. *(post-merge)*
- **Floating-ref drift detection:** open a PR that changes `peaceiris/actions-mdbook@<sha>` to `peaceiris/actions-mdbook@v2`; verify `actionlint` (or chosen enforcement mechanism) fails the run. *(pre- or post-merge via `gh workflow run`)*
- **First-deploy on empty repo:** on a fresh fork or test-repo with no `gh-pages` branch, trigger the workflow; verify peaceiris creates the branch and the run succeeds. *(post-merge / first-run)*
- **Rollback via revert:** simulate a bad deploy (e.g., a chapter with broken Mermaid that builds but renders broken), revert the source commit on `main`, verify next workflow run republishes the prior good content within ~5 minutes. *(post-merge)*
- **Permissions assertion:** inspect the workflow's `permissions:` block via `gh workflow view` or repo settings; verify `contents: write` only — no `id-token`, no `pages: write`. *(pre-merge static)*
- **Pinned d2 version assertion:** read the `d2` install step from the workflow file; verify the pinned version matches what's actually installed (no version drift via install-script default). *(post-merge runtime)*
- **Stale pages-source warning:** before the manual settings step is performed, verify the deployed `gh-pages` branch exists but the published site URL returns the GitHub default placeholder (this confirms the manual step is genuinely required, not a no-op). *(post-merge / first-run)*
- **Cache absence (V0 expectation):** time the workflow's preprocessor build step on two consecutive runs without source changes; verify rebuild happens (cache strategy is intentionally absent in V0; flag if rebuild time becomes a real cost — current expectation is small Rust crate, ~30–60s). *(post-merge)*
- **Public-repo log exposure:** inspect the public workflow run logs; verify no secrets, tokens, or workspace-private paths appear in stdout/stderr (logs are public on a public repo — this is a correctness check, not a security one, given the workflow has no secrets to leak in V0). *(post-merge)*

---

## Tasks

### Task 1: mdbook-d2 preprocessor crate

**Files:** `tools/mdbook-d2/Cargo.toml`, `tools/mdbook-d2/main.rs`, `tools/mdbook-d2/<modules>.rs` (as needed), `tools/mdbook-d2/<test files>.rs` (as needed), root `Cargo.toml` (workspace members)
**Type:** backend (Rust crate)
**Depends on:** None

**What:** Build a synchronous Rust binary that implements the mdBook preprocessor JSON protocol, detects `d2`-tagged fenced blocks, shells out to the `d2` CLI to render each block, and replaces the block with inline SVG. Adds the crate to the workspace.

**Acceptance Criteria:**
- [ ] `tools/mdbook-d2/` directory exists and is a valid Cargo package
- [ ] Root `Cargo.toml` `members` array includes `tools/mdbook-d2`
- [ ] Binary exits 0 on the `supports html` subcommand and non-zero on `supports <other>` (verify with at least: `epub`, `pdf`, `latex`)
- [ ] Given a book JSON containing a `d2` fenced block on stdin, the binary emits transformed JSON on stdout where the block is replaced with inline SVG
- [ ] Missing `d2` CLI produces a clear stderr message and a non-zero exit
- [ ] Malformed D2 source surfaces `d2`'s stderr verbatim and produces a non-zero exit; preprocessor adds no environment-derived enrichment to the error
- [ ] Empty / whitespace-only `d2` blocks pass through to `d2` unchanged; outcome matches `d2`'s native behavior with no preprocessor special-casing
- [ ] Shell-metacharacter content in `d2` blocks is passed as a literal argument to `d2`, NOT interpreted by a shell (uses `std::process::Command` without `sh -c`)
- [ ] Non-`d2` fenced blocks pass through unchanged
- [ ] `cargo clippy --workspace -- -D warnings` passes (including this crate)
- [ ] `cargo fmt --check` passes
- [ ] `cargo test --workspace` runs and passes the crate's tests

**Test Expectations:**
- A unit/integration test that pipes a sample mdBook JSON document containing a single `d2` block to the binary and asserts the output JSON contains an SVG element in place of the block
- A test for the `supports` subcommand behavior (cover html and at least one rejected renderer)
- A test (or guarded test) for the missing-`d2`-CLI error path — may use a doctored `PATH` to simulate
- A test for the malformed-D2 error path that asserts stderr propagation and absence of preprocessor enrichment
- A test for the empty-D2-block edge case that asserts the preprocessor does not pre-validate
- A security-relevant test for shell-metacharacter D2 content that confirms no shell interpretation occurred (e.g., side-effect file absence)
- A test for the non-`d2`-block passthrough behavior

### Task 2: mdBook scaffolding and content

**Files:** `docs/book.toml`, `docs/src/SUMMARY.md`, `docs/src/intro.md`, `docs/src/adrs/README.md`, `docs/src/adrs/template.md`
**Type:** docs / config
**Depends on:** Task 1 (preprocessor must build for `mdbook build` to succeed locally during verification)

**What:** Create the mdBook source tree with introduction, ADR section, and the MADR-shaped ADR template per the constraints above.

**Acceptance Criteria:**
- [ ] `docs/book.toml` exists with `book.title = "mnemra-core"`, `output.html.git-repository-url = "https://github.com/mnemra/mnemra-core"`, and both `mdbook-mermaid` and `mdbook-d2` registered as preprocessors
- [ ] `docs/src/SUMMARY.md` lists every chapter file that exists in `docs/src/`; no broken links
- [ ] `docs/src/intro.md` contains a brief project overview and a link back to the repository `README.md`
- [ ] `docs/src/adrs/README.md` exists as the ADR section landing page (initially explains the ADR convention; ADR list is empty or "none yet")
- [ ] `docs/src/adrs/template.md` is MADR-shaped with all seven required frontmatter fields and the body section structure named in the constraints, including explanatory prose for the `superseded_by` field
- [ ] `mdbook build docs/` succeeds locally (with the preprocessor crate built and on `PATH`, plus `mdbook` and `mdbook-mermaid` and `d2` installed locally)
- [ ] Default mdBook theme renders without errors

**Test Expectations:**
- Local `mdbook build docs/` produces output under `docs/book/` with the introduction page, ADR template page, and ADR section index reachable via the rendered nav
- Manual sanity check that the ADR template page displays the MADR sections, does not show raw frontmatter, and contains explanatory prose for the `superseded_by` field

### Task 3: GitHub Actions deploy workflow

**Files:** `.github/workflows/docs.yml`
**Type:** CI / deploy
**Depends on:** Task 1, Task 2

**What:** Add the GitHub Actions workflow that builds and deploys the docs site to GitHub Pages on push to `main`, with path filtering, pinned action versions (enforced via `actionlint`), concurrency control, and the build sequencing (`actionlint` → preprocessor build → d2 install → mdbook + mdbook-mermaid install → mdbook build → gh-pages deploy).

**Acceptance Criteria:**
- [ ] `.github/workflows/docs.yml` exists and is valid GitHub Actions YAML
- [ ] Triggers: `push` to `main` filtered to `docs/**`, `tools/mdbook-d2/**`, and `.github/workflows/docs.yml`; plus `workflow_dispatch`. Triggers `pull_request_target`, `workflow_run`, `repository_dispatch`, and `schedule` are explicitly absent
- [ ] All third-party actions are pinned to a specific 40-character commit SHA (no floating refs, no `@main`, no `@latest`, no `@v[0-9]` tagged refs)
- [ ] Workflow includes an `actionlint` lint step (or equivalent) that fails the run on any non-SHA action ref
- [ ] Workflow declares `concurrency: { group: pages-deploy, cancel-in-progress: false }` at top level
- [ ] The `d2` CLI install step pins a specific release version
- [ ] Workflow `permissions:` block grants only `contents: write`
- [ ] The build steps run in the correct order: actionlint → preprocessor build → d2 install → mdbook + mdbook-mermaid install → mdbook build → gh-pages deploy
- [ ] Workflow targets the `gh-pages` branch as the deploy destination

**Test Expectations:**
- Verification of workflow syntax via `actionlint` (which is now part of the workflow itself)
- After merge, a no-op trigger of the workflow (via `workflow_dispatch`) completes successfully end-to-end
- A subsequent change to a non-`docs/**` file does not cause the workflow to run
- A floating-ref drift test (PR introducing `@v2` or `@main`) is caught by the `actionlint` step (workflow fails)

### Task 4: justfile recipes and check-recipe update

**Files:** `justfile`
**Type:** developer tooling
**Depends on:** Task 1, Task 2

**What:** Add `docs` and `docs-serve` recipes that build the preprocessor first then run `mdbook build` or `mdbook serve` against `docs/`. Update the existing `check` recipe so its `cargo test` invocation includes the new crate.

**Acceptance Criteria:**
- [ ] `just docs` builds the preprocessor and runs `mdbook build docs/`, produces output under `docs/book/`, exits 0
- [ ] `just docs-serve` builds the preprocessor and runs `mdbook serve docs/`, serves on the default port, picks up edits to `docs/src/**`
- [ ] The existing `check` recipe's `cargo test` line has been updated (e.g., to `cargo test --workspace`) so that `mdbook-d2`'s tests are included
- [ ] `just check` exercises `mdbook-d2`'s tests (verifiable by introducing a deliberately-failing test in `mdbook-d2`, confirming `just check` fails, then removing or fixing the test before commit)
- [ ] The `check` recipe continues to pass with the new crate present (no breakage to the workspace lint/test pipeline; verify the existing `--exclude mnemra-echo` carve-out does not exclude the new crate)

**Test Expectations:**
- Manual smoke: run `just docs` end-to-end on a clean checkout (with prerequisites installed), confirm the rendered output exists
- Manual smoke: run `just docs-serve`, edit `docs/src/intro.md`, confirm the served site updates
- Manual verification: `just check` runs `cargo test` and includes `tools/mdbook-d2`'s tests in the run
