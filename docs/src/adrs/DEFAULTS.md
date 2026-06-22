---
title: Project Standards
summary: "Project defaults projected from workspace G-* canon — baseline for mnemra-core."
primary-audience: agent
---

# Project Standards

General architecture and engineering standards applied to this project.
These were projected on 2026-05-20 and may be overridden by project-specific
decisions (P-*.md files in this directory).

---

> **2026-06-22 renumber.** The workspace general-ADR corpus was renumbered (G-0001…G-0015), with amendment chains consolidated and stance-shaped ADRs elevated to workspace principles (`P-*`). Entries below are remapped to their post-reset homes per `brain/decisions/MIGRATION-MAP.md`. Standards whose substance moved to a principle or skill are kept here as pointers — the standard still applies to mnemra-core; only its canonical home changed.

## G-0001: Two-Tier Architecture Decision Records

General standards live in this DEFAULTS.md file, projected once and owned by the project. Project-specific decisions go in P-NNNN-*.md files in this directory. If a project-specific decision overrides a default, it says so explicitly in its status field.

---

## G-0002: Justfile as CI Contract

CI YAML invokes `just ci` and nothing else. Every project uses fixed `verify-*` recipe names (verify-test, verify-lint, verify-type, verify-coverage, verify-build, verify-smoke, ci). Each `verify-*` emits a `GATE <name> <PASS|FAIL>` line and is idempotent from any cwd. No `--fix` side effects under verify-*; auto-fix recipes live under `fix-*`.

---

## G-0003: Merge Governance

Consolidated merge-governance standard (folds in the prior Stage-6 closed-enum label vocabulary, Stage-6 label-set authorization + required-status-check enforcement, and the post-plan-approval autonomous implementation loop). See workspace `brain/decisions/G-0003-merge-governance.md`.

---

## G-0004: Review Finding Identity and Per-PR Per-Round Persistence

Review finding identity is the `(file, content-anchor, severity)` triple. Content-anchor is SHA-256 of a normalized 5-line window (cited line + 2 lines of context above and below, trailing whitespace stripped per line). Reviewer output format includes the anchor inline so downstream dedup logic doesn't need to re-read the file at the cited round's commit.

---

## G-0005: Devcontainer Architecture — Per-Repo Upstream FROM + GHCR Push

Per-repo devcontainer FROM upstream language-native base, built and pushed to GHCR by the repo's own CI on `.devcontainer/**` change, SHA-pinned in the repo. Every `cargo install` line in a devcontainer Dockerfile MUST pass both `--locked` and `--version <X.Y.Z>`. Polyglot repos layer additional language bases in their own Dockerfile; no workspace-shared polyglot base.

---

## G-0006: Layered Secret Detection — Pre-Commit + GitHub-Native + Stage 5 Alerts Query

Three layers per in-scope repo: (1) Pre-commit `lefthook` hooks with a workspace-shared narrow regex; (2) GitHub-native `secret_scanning` + `secret_scanning_push_protection` enabled continuously; (3) Stage 5 `verify-secrets` recipe in `just ci` queries the secret-scanning alerts API with `permissions.security-events: read` and fails the build on any open alert.

---

## G-0007: Feature Flags — FlagProvider Trait + Per-Repo Crate

Each in-scope repo defines its own feature-flag crate implementing a small `FlagProvider` trait. Default backend is env-var-driven (`FLAG_<KEY>`); keys are kebab-case in source, env vars are SCREAMING_SNAKE_CASE with `FLAG_` prefix. Extract to a shared workspace crate only when commonality emerges across three or more repos (rule of three).

---

## G-0008: PR-Merge Apparatus

Consolidated PR-merge apparatus (folds in the prior workspace merge template + auto-merge recovery cap, tag-race serialization, and GitHub Merge Queue for R26b enforcement). See workspace `brain/decisions/G-0008-pr-merge-apparatus.md`.

---

## G-0009: Rust Release Apparatus

Consolidated Rust release apparatus (folds in the prior release-plz automation standard and the monotonic non-decreasing version policy). See workspace `brain/decisions/G-0009-rust-release-apparatus.md`.

---

## G-0010: Embargo Flow Architecture — β Default + GHSA Private-Fork

Default posture: no embargo workflow built. When an embargoed disclosure arrives, use GitHub Security Advisories (GHSA) with temporary private fork pattern. Mode A flag-flip merges are suspended in the affected repo during embargo; an audit-PR opens at T0 to re-engage Stage 4/5 gates before the GitHub release publishes. Trip-wires upgrade this to a built workflow if embargoes become recurring.

---

## G-0011: Internal IPC Uses Typed Binary Encoding; JSON for Stored State and External Surfaces

Internal IPC defaults to a typed binary encoding (Protocol Buffers or FlatBuffers); JSON is reserved for stored state and external surfaces. Internal = both endpoints under the same project / team's control, payload not persisted as the canonical record, no trust-domain boundary. Choice between protobuf and flatbuffers is per-use-case (protobuf for typical request/response with cross-language tooling; flatbuffers for read-heavy zero-copy / mmap-able payloads).

---

## G-0012: Project Dev Docs — mdBook + D2 + Mermaid

mdBook (Apache-2.0 / MIT, rust-lang official) as static-site generator per project repo; CI deploys to GitHub Pages. D2 for architecture-grade diagrams in built docs sites; Mermaid for diagrams in committed `.md` read in the GitHub web UI. Integration via mdBook custom preprocessors (`mdbook-d2` + `mdbook-mermaid`).

---

## G-0013: Agent-First Workflow Shape — /brief + /verify

Adopt an agent-first workflow shape with two skills: `/brief` (unified intake → frame → spec, two human touchpoints, mandatory Stage 0 baseline load of values + principles + ADRs + constraint graph) and `/verify` (paired, four-signal stack: tests, property-based tests, intent-conformance review, decomposer spot-check). `/brief` is the single canonical entry for any work that produces a spec — `/discover`, `/spec`, `/clarify` survive only as internal mechanisms reused by stages.

---

## G-0014: Publish-Time Human Render via Bidirectional Translation

Agent-primary doc repos publish two surfaces from one canonical source tree (`docs/src/`): mdBook HTML for humans and llms.txt + llms-full.txt for agents. Each page declares `primary-audience: agent | human`; the opposite-audience render is generated via a local EXPLAIN pass (jargon resolution + narrative + glossary links) or STRIP pass (tighten to facts + cross-refs). Translations + the `_published/` tree are committed; CI is dumb. Hash-gated regeneration via `.translation-manifest.json` ensures translation runs only on actual source change.

---

## Elevated to workspace principles / skills

These standards still apply to mnemra-core; their canonical home moved out of the numbered ADR corpus in the 2026-06-22 reset.

- **Testing Philosophy — 90% Coverage, Inverted Pyramid** → workspace principle `P-TDDPairs` (coverage-floor + pyramid section). 90% code coverage threshold (lines and functions) as the enforced default; inverted test pyramid; documented threshold exceptions.
- **Agent-Primary Source Artifacts; Human Views Derivative** → workspace principle `P-AgentPrimarySource`. Source-of-truth artifacts authored/stored in agent-primary form (structured, machine-addressable, stable IDs); human-readable views are derivative and never the source of truth.
- **No Compile-Time Asset Embedding in Containerized Projects** → `skills/rust.md` §`<build>`. No `rust_embed`-style compile-time asset embedding for Docker-deployed Rust; serve static files at runtime with startup directory validation.
- **SQL File-Based Migrations for Embedded SQLite** → `skills/rust.md` §`<sqlite>`. SQL file-based, compile-time-embedded, forward-only migrations.
- **Hybrid XML Format for Agent Profiles** → `skills/xml-profile.md`. XML tags for structural sections, markdown inside tags, max two nesting levels.
- **Knowledge-Extraction standards** → `skills/knowledge-extraction.md` and project-23 `P-0001`–`P-0004` (nudge hook, capture table schema, duplicate-detection scope, advisory-drift detection, worked-example encoding, safety ceilings).
