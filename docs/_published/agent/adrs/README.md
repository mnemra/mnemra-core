---
title: Architecture Decision Records
summary: "How architecture decisions are structured here (G-* / P-* prefixes, MADR format)."
primary-audience: human
---

# Architecture Decision Records

mnemra-core architectural decisions use the [MADR](https://adr.github.io/madr/) (Markdown Architectural Decision Records) format.

## What is an ADR?

An ADR captures a single architectural decision with its context and rationale. ADRs are immutable once accepted — superseded by newer ADRs when decisions evolve, never edited in place.

## ADR Lifecycle

ADR statuses:

| Status | Meaning |
|--------|---------|
| `proposed` | Under discussion; not yet accepted |
| `rejected` | Discussed and explicitly declined |
| `accepted` | The current decision in effect |
| `deprecated` | Was accepted; no longer applies (context changed) |
| `superseded` | Replaced by a newer ADR; see `superseded_by` field |

## G/P Prefix Projection Pattern

Two-tier identifier convention encoding decision origin and propagation.

- **G-NNNN (general)** — workspace-wide architecture decisions. Not stored as individual files here; projected once into [`DEFAULTS.md`](DEFAULTS.md) as a concise entry without the full rationale chain. DEFAULTS.md is frozen at projection; upstream G-* changes do not auto-sync.
- **P-NNNN (project)** — project-specific decisions, stored as individual `P-NNNN-*.md` files here. Two sub-categories:
  - *Project-specific:* a decision with no workspace-wide analog — a constraint or tradeoff unique to mnemra-core.
  - *Override:* a deviation from a `DEFAULTS.md` baseline entry. The override P-ADR names the overridden default in its Status field (e.g., `Status: accepted, Overrides G-0017`).

*The default is the starting point; any deviation is a new P-ADR.*

## Using the Template

To record a project ADR: copy [`template.md`](template.md) to a numbered file (e.g., `adrs/P-0001-topic.md`), fill all frontmatter fields, write the body sections. If the decision overrides a `DEFAULTS.md` entry, state that in the Status section.

## Current ADRs

None yet. P-ADRs are added as project-specific decisions are ratified. See [`DEFAULTS.md`](DEFAULTS.md) for the projected general standards in force.
