---
title: Architecture Decision Records
summary: 
primary-audience: human
---

# Architecture Decision Records

This section tracks the architectural decisions made in mnemra-core using the [MADR](https://adr.github.io/madr/) (Markdown Architectural Decision Records) format.

## What is an ADR?

An Architecture Decision Record (ADR) captures a single architectural decision along with its context and rationale. ADRs are immutable once accepted — they are not edited to reflect changing opinions, but superseded by newer ADRs when decisions evolve.

## ADR Lifecycle

Each ADR moves through the following statuses:

| Status | Meaning |
|--------|---------|
| `proposed` | Under discussion; not yet accepted |
| `rejected` | Discussed and explicitly declined |
| `accepted` | The current decision in effect |
| `deprecated` | Was accepted; no longer applies (context changed) |
| `superseded` | Replaced by a newer ADR; see `superseded_by` field |

## G/P Prefix Projection Pattern

ADRs in this project use a two-tier identifier convention that reflects where a decision originates and how it propagates.

- **G-NNNN (general)** — workspace-wide architecture decisions. These do not live as individual files in this directory. Instead, they are projected once into [`DEFAULTS.md`](DEFAULTS.md) — a one-time snapshot recording each general standard as a concise entry, without the full rationale chain. DEFAULTS.md is frozen at projection time; upstream changes to a G-* decision do not auto-sync into this file.
- **P-NNNN (project)** — project-specific architecture decisions, stored as individual `P-NNNN-*.md` files in this directory. Two sub-categories:
  - *Project-specific:* a decision with no workspace-wide analog — a constraint or tradeoff unique to mnemra-core.
  - *Override:* a deviation from a `DEFAULTS.md` baseline entry. An override P-ADR names the default it overrides explicitly in its Status field (e.g., `Status: accepted, Overrides G-0017`).

*The default is the starting point; any deviation is a new P-ADR.*

## Using the Template

To record a new project ADR, copy [`template.md`](template.md) to a numbered file (e.g., `adrs/P-0001-topic.md`), fill in all frontmatter fields, and write the body sections. If the decision overrides a `DEFAULTS.md` entry, state that explicitly in the Status section.

## Current ADRs

None yet. P-ADRs will be added as project-specific architectural decisions are made and ratified. See [`DEFAULTS.md`](DEFAULTS.md) for the projected general standards in force.
