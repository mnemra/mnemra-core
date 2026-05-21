---
title: Project Standards
summary: "Project defaults projected from workspace G-* canon — baseline for mnemra-core."
primary-audience: agent
---

# Project Standards

General architecture and engineering standards applied to this project. These are the project's architectural baseline, projected from the workspace's global architecture decisions (the G-* records) into this project's `DEFAULTS.md`. They were projected on 2026-05-20. Any of them can be overridden by a project-specific decision, which lives as a P-* ADR (a project-scoped Architecture Decision Record, in a `P-*.md` file in this directory). When that happens, the override is the new authority, not an edit to this baseline.

---

## G-0001: Testing Philosophy — 90% Coverage, Inverted Pyramid

90% code coverage threshold (lines and functions) as the enforced default. Inverted test pyramid — far more unit tests than integration tests, with integration tests only verifying actual integration seams. Threshold exceptions must be documented with explicit reasoning.

---

## G-0002: No Compile-Time Asset Embedding in Containerized Projects

Do not use `rust_embed` or similar compile-time asset embedding in Rust projects deployed via Docker containers. Serve static files at runtime (`tower_http::services::ServeDir` or equivalent) with startup directory validation. Docker multi-stage builds copy binary and frontend artifacts into the final image.

---

## G-0003: Two-Tier Architecture Decision Records

This standard sets up the two tiers the rest of the project's decisions ride on. General standards live in this `DEFAULTS.md` file, projected once and owned by the project. Project-specific decisions go in `P-NNNN-*.md` files in this directory. If a project-specific decision overrides a default, it says so explicitly in its status field.

---

## G-0004: Hybrid XML Format for Agent Profiles

Use hybrid XML format for all team agent profiles: XML tags for structural sections (`<role>`, `<persona>`, `<principles>`, etc.), markdown preserved inside tags for prose. Maximum two levels of nesting.

---

## G-0005: SQL File-Based Migrations for Embedded SQLite

Use SQL file-based migrations embedded at compile time for all Rust projects with embedded SQLite. Migrations live in `migrations/NNNN_<description>.sql` and are embedded via `include_str!()` with a shared migration runner. Forward-only migrations — no down migrations.

---

## G-0006: Justfile as CI Contract

CI YAML invokes `just ci` and nothing else. Every project uses fixed `verify-*` recipe names (verify-test, verify-lint, verify-type, verify-coverage, verify-build, verify-smoke, ci). Each `verify-*` emits a `GATE <name> <PASS|FAIL>` line and is idempotent from any cwd. No `--fix` side effects under verify-*; auto-fix recipes live under `fix-*`.

---

## G-0007: Knowledge-Extraction Skill — Task-Completion Nudge Hook

Two-hook pattern for the knowledge-extraction nudge: a `Stop` hook at turn boundaries writes pending state to a session-keyed state file; `UserPromptSubmit` (or `SessionStart` fallback) reads the pending flag and injects the nudge into Claude's context. State file path: `.claude/hooks/state/knowledge-extraction-nudge`. Two-state enum (pending | fired); carrier accept/decline are conversational events on the `skill_run_captures` row, not the state file.

---

## G-0008: Knowledge-Extraction — Capture Table Schema and Partial-State Recovery

`skill_run_captures` table (FK to `skill_runs` with ON DELETE CASCADE) holds capture state with CHECK-constrained `state` (partial | complete | abandoned), `capture_type`, `destination_type`, plus `trigger_context`, `transcript`, `event_tallies`, `round_summary_log`. Puck is the only writer. Partial-state resume is carrier-picks and session-independent — partials survive `/clear`. Cleanup is manual via `brain skill-run capture list-partials` + `abandon <id>` in v1.

---

## G-0009: Knowledge-Extraction — R17 Duplicate-Detection Scope

R17 is a numbered requirement in the knowledge-extraction requirements document; here it covers how the skill detects duplicate captures. Title-only flat grep over 5 destination types, 6 corpora (project-scoped ADRs conditional). Scope held as a config array in the skill file, not hardcoded in skill logic. Overlap judgment is LLM-driven; UPDATE diff is field-level (scope / example / counter-case side-by-side), not line-diff.

---

## G-0010: Knowledge-Extraction — R10 Worked-Example Encoding

R10 governs how worked examples are encoded in the skill. 13 initial worked examples in a `Worked Examples` appendix at the bottom of the skill file: 5 canonical (one per destination type), 5 contrasting, 3 cross-cutting. Refinement is retro-driven; soft cap of 20 examples. No CLI tooling for v1.

---

## G-0011: Knowledge-Extraction — R7 Advisory-Drift Detection

R7 covers detecting advisory drift, where a captured rule slides into vague advice instead of an observable pattern. New skill file `skills/operational-form.md` for operational-form detection. Contains advisory phrase list (~12–15 for v1, growable), positive pattern definition (observable actor + action + outcome), and worked example pairs. On candidate ambiguity, skill emits an R20 redirect probe. Soft cap: 30 phrases.

---

## G-0012: Knowledge-Extraction — R16b Safety Ceiling Values

R16b sets the safety ceilings that stop a capture run from overrunning. Safety ceilings: N=6 rounds per rule, M=20k tokens per rule, M_run=80k tokens per invocation. Transcript char-count approximation (~4 chars/token) for runtime safety check; M_run accumulator on the parent `skill_runs` row. On trip, capture row written with `state=abandoned`; abandoned captures are filed but not auto-offered for resume.

---

## G-0013: Stage 6 Approval — Closed-Enum Label Vocabulary

Closed-enum PR labels gate Stage 6 approval. Three families: `stage6-approved` (0 or 1), release mode (`release-mode-A` | `release-mode-B` | `release-mode-A-exception`, exactly 1 when approved), version bump (`bump:patch` | `bump:minor` | `bump:major` | `bump:none`, exactly 1 when approved). Cardinality failures ABORT loudly, never silently disambiguate. Stage 7 auto-merge reads labels via `gh api`.

---

## G-0014: Review Finding Identity and Per-PR Per-Round Persistence

Review finding identity is the `(file, content-anchor, severity)` triple. Content-anchor is SHA-256 of a normalized 5-line window (cited line + 2 lines of context above and below, trailing whitespace stripped per line). Reviewer output format includes the anchor inline so downstream dedup logic doesn't need to re-read the file at the cited round's commit.

---

## G-0015: Devcontainer Architecture — Per-Repo Upstream FROM + GHCR Push

Per-repo devcontainer FROM upstream language-native base, built and pushed to GHCR by the repo's own CI on `.devcontainer/**` change, SHA-pinned in the repo. Every `cargo install` line in a devcontainer Dockerfile MUST pass both `--locked` and `--version <X.Y.Z>`. Polyglot repos layer additional language bases in their own Dockerfile; no workspace-shared polyglot base.

---

## G-0016: Layered Secret Detection — Pre-Commit + GitHub-Native + Stage 5 Alerts Query

Three layers per in-scope repo: (1) Pre-commit `lefthook` hooks with a workspace-shared narrow regex; (2) GitHub-native `secret_scanning` + `secret_scanning_push_protection` enabled continuously; (3) Stage 5 `verify-secrets` recipe in `just ci` queries the secret-scanning alerts API with `permissions.security-events: read` and fails the build on any open alert.

---

## G-0017: Feature Flags — FlagProvider Trait + Per-Repo Crate

This one applies per-repo first, with extraction held off until reuse actually shows up across three repos. Each in-scope repo defines its own feature-flag crate implementing a small `FlagProvider` trait. Default backend is env-var-driven (`FLAG_<KEY>`); keys are kebab-case in source, env vars are SCREAMING_SNAKE_CASE with `FLAG_` prefix. Extract to a shared workspace crate only when commonality emerges across three or more repos (rule of three).

---

## G-0018: Workspace Merge Template + Auto-Merge Recovery Cap

Workspace template script at `bin/pr-create-with-automerge.sh` canonicalizes the Stage-6-approved → squash → push → label → auto-merge sequence. Recovery cap = 3 (count-all semantics); per-PR counter lives in PR description footer (`<!-- recovery-attempts: N -->`). Recovery-increment activity rows mirror counter changes in `brain activity log` so counter tampering is detectable retroactively.

---

## G-0019: Tag-Race Serialization — GHA Concurrency Directive + R26b

R26b is a requirement stating that at most one PR may merge to main at a time. Per-repo release workflow uses GHA `concurrency:` scoped to the *release-pr* job (Mode A), NOT the release job, with `cancel-in-progress: false`. R26b is enforced via G-0023's GitHub Merge Queue for Mode B repos. Combined: two-layer serialization across release-PR-update and merge gates; Mode B rapid-merge release-job race is accepted as a v1 known limitation with fail-loud monitoring.

---

## G-0020: Rust Release Automation = release-plz

release-plz is the canonical Rust release-automation tool for both CHANGELOG generation and semver bump computation. Per-repo `release-plz.toml`; CI invokes via the official GitHub Action gated by main branch, per-G-0019 concurrency, and a fine-grained PAT or GitHub App token (mandatory — `GITHUB_TOKEN` does not trigger downstream workflows). Release-PR mode (Mode A) or direct-release mode (Mode B) declared per-repo.

---

## G-0021: Monotonic Non-Decreasing Version Policy (Pattern A)

For Pattern A repos, the version sequence on `main` MUST be monotonic non-decreasing — every merge produces `prev_version <= new_version`. Bump labels declare intent: `bump:major | bump:minor | bump:patch | bump:none`. `bump:none` is honor-system + Stage 4 reviewer responsibility, not tool-enforced. Revert PR bump labels reflect public-API impact, not numeric distance from the reverted version.

---

## G-0022: Embargo Flow Architecture — β Default + GHSA Private-Fork

This standard chooses not to build an embargo workflow until embargoes actually recur. Default posture: no embargo workflow built. When an embargoed disclosure arrives, use GitHub Security Advisories (GHSA) with temporary private fork pattern. Mode A flag-flip merges are suspended in the affected repo during embargo; an audit-PR opens at T0 to re-engage Stage 4/5 gates before the GitHub release publishes. Trip-wires upgrade this to a built workflow if embargoes become recurring.

---

## G-0023: GitHub Merge Queue for R26b Enforcement and Intermediate-State CI

GitHub Merge Queue enabled per in-scope repo enforces R26b (at most one PR merging to main at a time) at the structural level. Required checks split: tests / build / coverage run against the queue's intermediate-state branch (need `merge_group` trigger); lint / type / secrets are PR-only. Per-repo merge-queue merge method MUST be set to **rebase** in branch protection. Admin queue bypass requires a `brain activity log` audit row.

---

## G-0024: Stage 6 Approval — Label-Set Authorization + Required-Status-Check Enforcement

Two GitHub-native enforcement layers, both required per in-scope repo: (A) repository ruleset (or `verify-stage6-labeler` workflow) restricts who may apply gating-family labels — single-actor allowlist in v1 (Peter, plus Forge / Bolt during workspace template invocation); (B) `verify-stage6-labels` required-status-check enforces cardinality at branch-protection level. Both checks are required-status-checks; both block merge on failure. Audit cross-reference at retrieval time joins `brain activity log` rows to the GitHub event log to detect forged audit rows.

---

## G-0025: Internal IPC Uses Typed Binary Encoding; JSON for Stored State and External Surfaces

Internal IPC defaults to a typed binary encoding (Protocol Buffers or FlatBuffers); JSON is reserved for stored state and external surfaces. Internal = both endpoints under the same project / team's control, payload not persisted as the canonical record, no trust-domain boundary. Choice between protobuf and flatbuffers is per-use-case (protobuf for typical request/response with cross-language tooling; flatbuffers for read-heavy zero-copy / mmap-able payloads).

---

## G-0026: Project Dev Docs — mdBook + D2 + Mermaid

mdBook (Apache-2.0 / MIT, rust-lang official) as static-site generator per project repo; CI deploys to GitHub Pages. D2 for architecture-grade diagrams in built docs sites; Mermaid for diagrams in committed `.md` read in the GitHub web UI. Integration via mdBook custom preprocessors (`mdbook-d2` + `mdbook-mermaid`).

---

## G-0027: Agent-Primary Source Artifacts; Human Views Derivative

Source-of-truth artifacts (requirements, specs, architecture, ADRs) are authored, stored, and maintained in agent-primary form: structured and machine-addressable with stable IDs paired to human-readable names, no integrated-narrative requirement, mechanical operations on the format (parse / validate / merge / archive — deterministic, not LLM-driven). Human-readable views are derivative, generated on demand from the agent-primary source; they are never the source of truth.

---

## G-0028: Agent-First Workflow Shape — /brief + /verify

Adopt an agent-first workflow shape with two skills: `/brief` (unified intake → frame → spec, two human touchpoints, mandatory Stage 0 baseline load of values + principles + ADRs + constraint graph) and `/verify` (paired, four-signal stack: tests, property-based tests, intent-conformance review, decomposer spot-check). `/brief` is the single canonical entry for any work that produces a spec — `/discover`, `/spec`, `/clarify` survive only as internal mechanisms reused by stages.

---

## G-0029: Publish-Time Human Render via Bidirectional Translation

Agent-primary doc repos publish two surfaces from one canonical source tree (`docs/src/`): mdBook HTML for humans and llms.txt + llms-full.txt for agents. Each page declares `primary-audience: agent | human`; the opposite-audience render is generated via a local EXPLAIN pass (jargon resolution + narrative + glossary links) or STRIP pass (tighten to facts + cross-refs). Translations + the `_published/` tree are committed; CI is dumb. Hash-gated regeneration via `.translation-manifest.json` ensures translation runs only on actual source change.
