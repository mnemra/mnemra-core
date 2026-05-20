---
title: Glossary
summary: "Terms and conventions used across mnemra-core's intent, ADRs, and specs."
primary-audience: human
---

# Glossary

Mnemra's documentation uses a consistent vocabulary drawn from its architecture decision records,
operating principles, and work-shaping pipeline. This glossary anchors that vocabulary for readers
who are not already familiar with these terms. Each entry is a terse reference; where a fuller
treatment exists in the architecture documentation, the entry points there.

Terms are grouped by category, then alphabetical within each group.

---

## Architecture Decision Records

### ADR

An Architecture Decision Record captures a significant technical or process decision — the context
that prompted it, the decision itself, rejected alternatives, and expected consequences. ADRs are
not changelogs; they preserve the option space a decision was made against, so future readers can
understand why the selected path was taken and what was ruled out.

Mnemra-core uses the MADR format (Markdown Architectural Decision Records): each ADR is a separate
`.md` file with structured sections. See the [ADRs section](adrs/README.md) of this documentation.

### DEFAULTS.md

The project's architectural baseline. DEFAULTS.md is a one-time snapshot of the workspace's
general architecture decisions, projected at the time the project was created (or last
re-projected). Each entry carries an ID (e.g., `G-0026`) and a concise description — 1–3 sentences
summarizing the decision — without the full rationale chain.

DEFAULTS.md is frozen at projection: updates to upstream workspace decisions do not auto-sync into
it. Any deviation from a DEFAULTS.md entry becomes a [P-* ADR](#p--adr), not an edit to DEFAULTS.md.

*The default is the starting point; any deviation is a new P-ADR.*

See also [G-* ADR](#g--adr) and [P-* ADR](#p--adr).

### G-* ADR

A G-* ADR (the G stands for global) is a workspace-wide architecture decision that applies across
all projects in the workspace. These decisions lock patterns that individual projects inherit: source
artifact format, workflow shape, docs publication strategy.

G-* ADRs live in workspace-internal canon and do not appear directly in mnemra-core's ADRs, guides,
or other documentation. They surface in mnemra-core docs only in two places: (1) [DEFAULTS.md](#defaultsmd)
entries that cite their origin G-ID, and (2) [P-* ADR](#p--adr) Status fields that mark an override
(`Overrides G-NNNN`). The codes are stable identifiers, not version numbers.

### P-* ADR

A P-* ADR (the P stands for project) is an architecture decision scoped to a single project. Two
categories:

- **Project-specific:** a decision with no workspace-wide analog — a constraint, pattern, or
  tradeoff unique to this project.
- **Override:** a deviation from a [DEFAULTS.md](#defaultsmd) baseline entry. The Status field of
  an override P-ADR names the default being overridden (`Overrides G-NNNN`).

P-* ADRs live in the project's own repository under `docs/src/adrs/`. They use the same MADR format
as other ADRs. See also [G-* ADR](#g--adr).

---

## Architecture Principles

Architecture principles are named rules that operationalize the workspace's core values. Each
principle carries a short label (P-Defer, P-WriteTimeAudience, etc.) that ADRs and design
documents use to ground their decisions. The full statement of each principle — its rationale, how
it shows up in practice, and anti-examples — lives in the workspace's architecture-principles
document. The entries below are terse pointers intended for readers encountering a principle label
in a doc.

### P-Defer

Defer mechanism choice until evidence forces it. Trip-wire-driven adoption beats anticipated-need
adoption: the shape of the evidence informs the shape of the mechanism. When a mechanism is named
before its trip-wire fires, see the open/deferred section of the relevant ADR.

### P-InstrumentBefore

Every production surface ships instrumented before launch. Metrics, structured logs, and traces
sized to the surface are in place at first use, not added after the first incident.

### P-LockContract

Lock the contract; vary the implementation. The contract is what other code depends on; a stable
contract makes implementations swappable without breaking callers.

### P-MinBlastRadius

A change reaches as far as the architecture allows. The goal is that a fix isolates to one module
and a feature lands behind one seam; when a minimal change would require touching many files in
lock-step, the architecture is reporting structural debt.

### P-PerRepoFirst

Per-repo first; extract on rule-of-three. Shared abstractions emerge from observed reuse, not from
speculation about what the reuse will look like. Extraction that precedes the third occurrence
creates shared code before it has earned its keep.

### P-PreserveDecisionSpace

Preserve the option space. Every ADR carries an Alternatives Considered section listing rejected
options with reasons. Dropping a rejected alternative because it now feels obviously wrong leaves
future readers unable to see what was on the table.

### P-TrustThenRetro

Trust the loop; retro selectively. Direction is set up front; the team executes within it; review
concentrates on outcomes and patterns. Walking every decision with the reviewer serializes work
through their attention — the failure mode this principle exists to prevent.

### P-WriteTimeAudience

Generic at write-time for repo artifacts; identity-preserving for workspace artifacts. Repo-
persisted artifacts (specs, ADRs, READMEs) use generic role labels and omit workspace-internal
vocabulary, because any commit can become a publish event. Workspace-private artifacts preserve
agent attribution and internal context because that attribution is the pattern signal worth keeping.

---

## Feature Register Lifecycle Tiers

The [feature register](intent/mnemra-core.md) tracks each product capability at exactly one
lifecycle tier. Tiers are validated by pipeline artifacts — the validator for each tier is "does
this artifact exist?", which makes the register mechanically checkable rather than prose-assessed.

### idea

A captured direction — a thought or a decision-locked pointer to a provenance record. No pipeline
artifact exists yet. The `idea` tier is the scope-anchor surface: research and discovery read
`idea`-and-up to ensure intended directions are never silently dropped to what-exists-live.

### proposed

The feature has a locked intake — it has been through intent capture. Permanent artifact; this
status does not regress.

### designed

A locked frame plus a locked spec both exist — the permanent "what to build" is complete. Permanent
artifact (kept). A feature reaches `designed` before it reaches `committed`: release-fit cannot be
judged until the design is complete.

### committed

`designed` plus a plan, release-bound. The plan is ephemeral (not kept after the work completes);
its ephemerality is why it marks commitment — a throwaway task list is generated only when work is
being actively actioned against a release.

### live

Built and verified in current code and canon.

---

## Pipeline Stages

The work-shaping pipeline turns product intent into a verifiable, implementable spec in three
stages. Each stage has a defined input, output, and owner; agents operate autonomously between the
two human touchpoints (at intake-exit and at spec-exit).

### Intake

Stage 1. The decomposer writes; agents review. Structured intent is captured — job-to-be-done,
non-goals, success criteria, hard constraints — and an agent review pass validates completeness and
flags conflicts with architecture principles and values. The decomposer iterates until intent is
locked. Output: validated intent.

### Frame

Stage 2. Agents synthesize. The constraint graph is walked from the validated intent; operating
constraints are proposed; a review pass flags conflicts; routine architecture decisions are batched,
novel ones escalated. Output: frame document (constraint summary and rationale chain).

### Spec

Stage 3. Agents synthesize. A testable spec is produced from the frame document; a review pass
validates testability and spec quality. The spec is the contract that verification consumes.
Output: locked spec.

---

## Requirement Codes

### R-codes

R-codes are stable identifiers for numbered requirements in a canonical requirements or
specification document. An R-code (e.g., `R7`, `R12`) refers to a specific requirement entry whose
text, constraints, and rationale are defined in that document.

When an R-code appears in mnemra-core documentation, it is always accompanied by its definition and
originating context — either inline at the cite site or in mnemra-core's requirements document.
R-codes do not carry stable public meaning on their own; their meaning is authoritative only in
combination with the requirements document that defines them.

Mnemra-core's R-codes will materialize once its requirements document is written. Until then, any
requirement referenced in a doc is stated in full at the point of citation.
