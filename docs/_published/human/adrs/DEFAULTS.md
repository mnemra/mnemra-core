---
title: Project Standards
summary: "Project defaults projected from workspace G-* canon — baseline for mnemra-core."
primary-audience: agent
---

# Project Standards

General architecture and engineering standards applied to this project. These are the baseline that mnemra-core inherits from the workspace. They were projected once, on 2026-05-20, and a project-specific decision can override any of them. Those overrides are the `P-*.md` files in this same directory, the [P-* ADRs](../glossary.md#p--adr) scoped to a single project.

---

> **2026-06-22 renumber.** The workspace general-ADR corpus was renumbered (G-0001 through G-0015). Amendment chains were consolidated, and ADRs that expressed a stance rather than a one-off decision were elevated to workspace principles (`P-*`). The entries below are remapped to their post-reset homes per `brain/decisions/MIGRATION-MAP.md`. A few standards whose substance moved to a principle or a skill are kept here as pointers: the standard still applies to mnemra-core, only its canonical home changed.

## G-0001: Two-Tier Architecture Decision Records

There are two tiers. General standards live in this [DEFAULTS.md](../glossary.md#defaultsmd) file, which is the project's architectural baseline: projected once, then owned by the project. Project-specific decisions go in `P-NNNN-*.md` files in this directory. When a project-specific decision overrides a default, it says so explicitly in its Status field.

---

## G-0002: Justfile as CI Contract

CI YAML invokes `just ci` and nothing else. Every project uses the same fixed `verify-*` recipe names: verify-test, verify-lint, verify-type, verify-coverage, verify-build, verify-smoke, ci. Each `verify-*` recipe emits a `GATE <name> <PASS|FAIL>` line and is idempotent from any working directory. No `--fix` side effects are allowed under `verify-*`. Auto-fix recipes live under `fix-*` instead.

---

## G-0003: Merge Governance

A single consolidated merge-governance standard. It folds in three things that used to be separate: the Stage-6 closed-enum label vocabulary, the Stage-6 label-set authorization plus required-status-check enforcement, and the autonomous implementation loop that runs after plan approval. See workspace `brain/decisions/G-0003-merge-governance.md`.

---

## G-0004: Review Finding Identity and Per-PR Per-Round Persistence

A review finding's identity is the `(file, content-anchor, severity)` triple. The content-anchor is the SHA-256 of a normalized 5-line window: the cited line plus two lines of context above and below, with trailing whitespace stripped per line. Reviewer output includes the anchor inline. That way downstream dedup logic doesn't need to re-read the file at the cited round's commit to recompute it.

---

## G-0005: Devcontainer Architecture — Per-Repo Upstream FROM + GHCR Push

Each repo's devcontainer builds `FROM` an upstream, language-native base image. It's built and pushed to GHCR by the repo's own CI whenever `.devcontainer/**` changes, and SHA-pinned in the repo. Every `cargo install` line in a devcontainer Dockerfile must pass both `--locked` and `--version <X.Y.Z>`. Polyglot repos layer the additional language bases in their own Dockerfile. There's no workspace-shared polyglot base.

---

## G-0006: Layered Secret Detection — Pre-Commit + GitHub-Native + Stage 5 Alerts Query

Three layers run in every in-scope repo. First, pre-commit `lefthook` hooks using a workspace-shared narrow regex. Second, GitHub-native `secret_scanning` and `secret_scanning_push_protection`, enabled continuously. Third, a Stage 5 `verify-secrets` recipe in `just ci` that queries the secret-scanning alerts API with `permissions.security-events: read` and fails the build on any open alert.

---

## G-0007: Feature Flags — FlagProvider Trait + Per-Repo Crate

Each in-scope repo defines its own feature-flag crate that implements a small `FlagProvider` trait. The default backend is env-var-driven (`FLAG_<KEY>`). Keys are kebab-case in source; the env vars are SCREAMING_SNAKE_CASE with a `FLAG_` prefix. Extraction to a shared workspace crate waits until commonality emerges across three or more repos. That's the rule of three, the [P-PerRepoFirst](../glossary.md#p-perrepofirst) principle (per-repo first, extract on rule-of-three) applied to flags.

---

## G-0008: PR-Merge Apparatus

A single consolidated PR-merge apparatus. It folds in the prior workspace merge template plus the auto-merge recovery cap, tag-race serialization, and the GitHub Merge Queue used for `R26b` enforcement. (`R26b` is an [R-code](../glossary.md#r-codes), a stable identifier for a numbered requirement; its full text lives in the requirements document that defines it.) See workspace `brain/decisions/G-0008-pr-merge-apparatus.md`.

---

## G-0009: Rust Release Apparatus

A single consolidated Rust release apparatus. It folds in the prior release-plz automation standard and the monotonic, non-decreasing version policy. See workspace `brain/decisions/G-0009-rust-release-apparatus.md`.

---

## G-0010: Embargo Flow Architecture — β Default + GHSA Private-Fork

The default posture is to build no embargo workflow at all. When an embargoed disclosure actually arrives, use GitHub Security Advisories (GHSA) with the temporary private-fork pattern. During an embargo, Mode A flag-flip merges are suspended in the affected repo. An audit-PR opens at T0 to re-engage the Stage 4 and Stage 5 gates before the GitHub release publishes. Trip-wires upgrade this to a built workflow if embargoes start recurring.

---

## G-0011: Internal IPC Uses Typed Binary Encoding; JSON for Stored State and External Surfaces

Internal IPC defaults to a typed binary encoding, either Protocol Buffers or FlatBuffers. JSON is reserved for stored state and external surfaces. "Internal" has a precise meaning here: both endpoints are under the same project's or team's control, the payload isn't persisted as the canonical record, and there's no trust-domain boundary. The choice between protobuf and flatbuffers is per use case. Protobuf fits typical request/response with cross-language tooling. Flatbuffers fits read-heavy, zero-copy, mmap-able payloads.

---

## G-0012: Project Dev Docs — mdBook + D2 + Mermaid

mdBook (Apache-2.0 / MIT, the rust-lang official tool) is the static-site generator, one per project repo, and CI deploys it to GitHub Pages. D2 handles architecture-grade diagrams in the built docs sites. Mermaid handles diagrams in committed `.md` files that are read in the GitHub web UI. Integration is through mdBook custom preprocessors: `mdbook-d2` and `mdbook-mermaid`.

---

## G-0013: Agent-First Workflow Shape — /brief + /verify

The workflow is agent-first and built on two skills. `/brief` is the unified pipeline through [Intake](../glossary.md#intake) (Stage 1, capture validated intent), [Frame](../glossary.md#frame) (Stage 2, synthesize the constraint summary), and [Spec](../glossary.md#spec) (Stage 3, produce the testable contract). It has two human touchpoints and a mandatory Stage 0 baseline load of values, principles, ADRs, and the constraint graph. `/verify` is its pair, a four-signal stack: tests, property-based tests, intent-conformance review, and a decomposer spot-check. `/brief` is the single canonical entry for any work that produces a spec. The older `/discover`, `/spec`, and `/clarify` skills survive only as internal mechanisms that the stages reuse.

---

## G-0014: Publish-Time Human Render via Bidirectional Translation

Agent-primary doc repos publish two surfaces from one canonical source tree (`docs/src/`): mdBook HTML for humans, and llms.txt plus llms-full.txt for agents. Each page declares `primary-audience: agent | human`. The opposite-audience render is generated locally, either by an EXPLAIN pass (resolve jargon, add narrative, link the glossary) or a STRIP pass (tighten to facts plus cross-refs). Both the translations and the `_published/` tree are committed, so CI stays dumb. Regeneration is hash-gated through `.translation-manifest.json`, which means a translation runs only when its source actually changes.

---

## G-0015: Relational Substrate Default — Postgres (server-side) / SQLite (embedded), behind an engine-agnostic seam

The default relational substrate is keyed on deployment topology, not picked once globally.

Server-side, multi-tenant, or managed-tier work goes to PostgreSQL. That's chosen on affirmative merits, but only after a hard license-pass-first gate: mature RLS multi-tenancy, single-transaction atomicity for multi-write commit-or-rollback, and decades-deep backup, PITR, and observability.

Embedded, single-writer, single-process work goes to SQLite. SQLite here isn't a fallback from Postgres; it's the correct default for that topology.

A project records its substrate against its topology rather than re-litigating Postgres-vs-SQLite each time. The storage surface is locked behind an engine-agnostic `Storage` seam, treated as an intrinsic contract invariant ([P-LockContract](../glossary.md#p-lockcontract), lock the contract and vary the implementation): the seam is a trait, the chosen engine is the implementation behind it. A future swap stays bounded behind that one seam ([P-MinBlastRadius](../glossary.md#p-minblastradius), a change reaches no further than the architecture allows). Don't build a second adapter speculatively ([P-Defer](../glossary.md#p-defer), defer the mechanism until evidence forces it); the second implementation waits behind a named trip-wire.

When the Postgres lane is taken with Row-Level Security, the constraints are exact. The application role must not hold `BYPASSRLS` and must not be a superuser. `ALTER TABLE … FORCE ROW LEVEL SECURITY` is required if the app role owns the tables. The tenant key must be set per transaction (`SET LOCAL`), never per session: a session-level `SET` leaks across pooled connection checkouts.

mnemra-core's own embedded-engine choice (`postgresql_embedded`, a single self-hosted-binary posture) and its V0 stack specifics stay in the project ADRs P-0001 and P-0010. See workspace `brain/decisions/G-0015-relational-substrate-default.md`.

---

## Elevated to workspace principles / skills

These standards still apply to mnemra-core. Their canonical home moved out of the numbered ADR corpus in the 2026-06-22 reset.

- **Testing Philosophy, 90% Coverage, Inverted Pyramid** → workspace principle `P-TDDPairs` (the coverage-floor and pyramid section). A 90% code-coverage threshold (lines and functions) as the enforced default, an inverted test pyramid, and documented threshold exceptions.
- **Agent-Primary Source Artifacts; Human Views Derivative** → workspace principle `P-AgentPrimarySource`. Source-of-truth artifacts are authored and stored in agent-primary form (structured, machine-addressable, stable IDs); human-readable views are derivative and never the source of truth.
- **No Compile-Time Asset Embedding in Containerized Projects** → `skills/rust.md` §`<build>`. No `rust_embed`-style compile-time asset embedding for Docker-deployed Rust; serve static files at runtime with startup directory validation.
- **SQL File-Based Migrations for Embedded SQLite** → `skills/rust.md` §`<sqlite>`. SQL file-based, compile-time-embedded, forward-only migrations.
- **Hybrid XML Format for Agent Profiles** → `skills/xml-profile.md`. XML tags for structural sections, markdown inside the tags, at most two nesting levels.
- **Knowledge-Extraction standards** → `skills/knowledge-extraction.md` and project-23 `P-0001` through `P-0004` (nudge hook, capture-table schema, duplicate-detection scope, advisory-drift detection, worked-example encoding, safety ceilings).
