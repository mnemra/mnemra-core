---
title: "Spec: llms.txt + llms-full.txt generation MVP (no-translation)"
date: 2026-05-21
status: locked
parent-task: brain #1098 (subtask of #1092)
repo: mnemra-core
implementer: Forge
predecessor-spec: docs/specs/2026-05-07-mdbook-scaffolding.md
authority: scratch/puck-mnemra-core-docs-architecture.md (step 5 in sequencing); DEFAULTS.md (mnemra-core); G-0029 (publish-time human-render)
---

# Spec: llms.txt + llms-full.txt generation MVP

## Goal

Add a build-time generator that produces `docs/_published/llms.txt` and `docs/_published/llms-full.txt` from the canonical `docs/src/` tree, and wire the deployment so both files are served alongside the existing mdbook HTML on GitHub Pages.

No translation pass in this step. Outputs are derived directly from canonical source (which is currently agent-first prose); outside humans continue to see the agent-first prose on the mdbook site. Step 6 will add the EXPLAIN/STRIP translation pipeline.

## Out of scope (Forge: do not extend into these)

- Translation pipeline / EXPLAIN / STRIP passes / Claude Code invocation — step 6
- Moving `docs/specs/2026-05-07-mdbook-scaffolding.md` to `docs/src/history/` — deferred per OD-1 until first canonical spec lands
- Frontmatter `<hr>` render quirk in mdbook output — resolved at translate-time by G-0029
- Pre-push hook for `just docs-check` — later step
- Hash-gated regeneration (`.translation-manifest.json`) — belongs to step 6 (no LLM determinism issue without translation)
- Adding new pages / restructuring `docs/src/` content — out of scope; structure is post-PR-#3 state

## Acceptance criteria

1. **Hand-authored summaries.** Every page currently in `docs/src/**/*.md` has a non-empty `summary:` frontmatter field. List of pages and authored values is enumerated in §"Hand-authored summaries" below; Forge applies them verbatim during this step.

2. **Generator exists at `scripts/docs-llms.py`.** Python script invoked via `uv run`. Reads `docs/src/SUMMARY.md`, walks `docs/src/`, emits the two output files. No external dependencies beyond Python stdlib + `pyyaml` for frontmatter parsing (pin in script header per workspace `uv run --with` pattern).

3. **justfile recipe `docs-llms`** invokes the generator. Output: `docs/_published/llms.txt` and `docs/_published/llms-full.txt`. Recipe is added to existing `justfile` next to `docs` / `docs-serve`.

4. **`just check` runs `docs-llms`** and fails if generated outputs differ from committed outputs. (Drift gate — same shape as `cargo fmt --check`.) Implementation: generator supports `--check` flag that exits 1 if regeneration would change committed files.

5. **`docs/_published/` is committed.** New directory with the two `.txt` files. `.gitignore` adds `docs/_build/` (mdbook output, was already ignored implicitly via missing presence — make explicit). `.gitignore` does NOT exclude `docs/_published/`.

6. **CI workflow `docs.yml` deploys both surfaces.** After `mdbook build docs/`, a new step copies `docs/_published/llms.txt` and `docs/_published/llms-full.txt` into `docs/book/` so the gh-pages publish step picks them up automatically. Existing `peaceiris/actions-gh-pages` step unchanged.

7. **llms.txt format complies with Jeremy Howard's spec.** §"llms.txt format" below specifies the exact shape.

8. **llms-full.txt content order matches SUMMARY.md.** §"llms-full.txt format" specifies.

9. **Tests.** A `tests/test_docs_llms.py` covers: (a) fixture `docs/src/` tree → asserted exact llms.txt output; (b) fixture tree → asserted exact llms-full.txt output; (c) `--check` mode exits 0 on clean tree, 1 on drift. Tests run via `uv run pytest tests/test_docs_llms.py` and are wired into `just check` after the existing cargo checks.

10. **Local + CI parity.** Running `just docs-llms` locally on a clean main produces zero diff against committed outputs.

## Hand-authored summaries

Forge applies these `summary:` values to existing frontmatter:

| Page | summary |
|---|---|
| `docs/src/intro.md` | "Entry point for the mnemra-core documentation site." |
| `docs/src/intent/mnemra-core.md` | "Product brief locking mnemra-core's intent and feature register." |
| `docs/src/specs/README.md` | "How canonical architecture specs differ from implementation-history specs in this repo." |
| `docs/src/adrs/README.md` | "How architecture decisions are structured here (G-* / P-* prefixes, MADR format)." |
| `docs/src/adrs/DEFAULTS.md` | "Project defaults projected from workspace G-* canon — baseline for mnemra-core." |
| `docs/src/adrs/template.md` | "MADR template for authoring new P-NNNN ADRs." |
| `docs/src/glossary.md` | "Terms and conventions used across mnemra-core's intent, ADRs, and specs." |

Pages whose frontmatter does not yet exist (e.g., `docs/src/SUMMARY.md`) are NOT given frontmatter — SUMMARY.md is structural, never appears in either output.

## llms.txt format

Reference: <https://llmstxt.org/>. Required shape:

```
# mnemra-core

> Plugin-extensible context layer for AI coding workflows. This site documents the project's intent, architecture decisions, and specifications.

## Introduction

- [Introduction](intro.md): Entry point for the mnemra-core documentation site.

## Intent

- [mnemra-core](intent/mnemra-core.md): Product brief locking mnemra-core's intent and feature register.

## Specifications

- [About this section](specs/README.md): How canonical architecture specs differ from implementation-history specs in this repo.

## Architecture Decision Records

- [ADR Overview](adrs/README.md): How architecture decisions are structured here (G-* / P-* prefixes, MADR format).
- [Project Defaults](adrs/DEFAULTS.md): Project defaults projected from workspace G-* canon — baseline for mnemra-core.
- [ADR Template](adrs/template.md): MADR template for authoring new P-NNNN ADRs.

## Glossary

- [Glossary](glossary.md): Terms and conventions used across mnemra-core's intent, ADRs, and specs.
```

Generator rules:

- **H1:** Repo name (`mnemra-core` — hard-coded for now; sourcing from `Cargo.toml` workspace name is out of scope, repo has one anyway).
- **Blockquote:** One-line repo description, hard-coded in `scripts/docs-llms.py` as a top-of-file constant `REPO_DESCRIPTION`. Value above is the chosen text.
- **Sections (`## Header`):** Derived from SUMMARY.md `# Header` lines. The first SUMMARY.md `- [...](...)` entry that precedes any `# Header` (currently `[Introduction](intro.md)`) goes into a synthetic `## Introduction` section.
- **Links:** Each `- [Title](relative/path.md)` from SUMMARY.md becomes `- [Title](relative/path.md): <summary from page frontmatter>`. Paths are kept as relative URLs (no domain prefix — llms.txt consumers resolve against the page they fetched the file from).
- **Empty summary handling:** Generator exits 1 with a clear error if any SUMMARY.md-referenced page has empty `summary:` frontmatter. Hand-authored summaries (above) mean this path is unreachable on a clean tree but the check is the durable contract.
- **Pages not in SUMMARY.md:** ignored by llms.txt (manifest is curated). They DO appear in llms-full.txt — see below.

## llms-full.txt format

Concatenation of every `.md` file under `docs/src/` (except `SUMMARY.md` itself) in **SUMMARY.md order**, with non-SUMMARY pages appended at the end in lexicographic path order.

Each page is separated by:

```
<!-- ===== docs/src/relative/path.md ===== -->

<page content verbatim, including frontmatter>
```

Concatenation includes frontmatter (agents benefit from it). No transformation of content. Trailing newline normalized to one `\n` between pages.

Currently every `docs/src/` page is in SUMMARY.md → no appended tail section yet. Future history pages (when scaffolding spec moves) would land in the tail.

## Generator implementation notes (non-binding for Forge — guidance, not contract)

- Script header: `# /// script\n# requires-python = ">=3.11"\n# dependencies = ["pyyaml"]\n# ///` (uv-script inline metadata).
- CLI: `python scripts/docs-llms.py [--check]`. Default: write outputs. `--check`: exit nonzero if outputs would differ from committed; print diff summary to stderr.
- Invocation in justfile: `uv run scripts/docs-llms.py` (write) and `uv run scripts/docs-llms.py --check` (gate).
- Frontmatter parsing: regex on `^---\n(.*?)\n---\n` prelude, yaml.safe_load the body. Out-of-scope to handle frontmatter edge cases beyond what's in the current 7 pages.

## CI changes

`.github/workflows/docs.yml` — add one step between "Build docs" and "Deploy to GitHub Pages":

```yaml
- name: Stage llms.txt + llms-full.txt
  run: cp docs/_published/llms.txt docs/_published/llms-full.txt docs/book/
```

No new tool installs, no new permissions, no concurrency changes.

Optional follow-up (not in step 5): a separate `check` workflow that runs `just check` on PRs — currently no such workflow exists. Adding it expands scope; defer to a later step.

## Risks + tradeoffs

- **Committed build artifacts.** `docs/_published/` is committed even though it's generated. Justification per parent proposal: avoids LLM-API spend in CI when translation arrives in step 6. Even at step 5 (no LLM), keeping the same shape avoids a re-architecture at step 6 boundary. The `just check` drift gate is the safety net.
- **Hard-coded blockquote.** `REPO_DESCRIPTION` constant in the script is the first thing to evolve. Acceptable trade-off vs. wiring up another source-of-truth.
- **Lexicographic tail order for non-SUMMARY pages.** Arbitrary but deterministic. Revisit if and when implementation-history specs land at `docs/src/history/` and a real ordering need emerges.
- **No structured validation of llms.txt against Jeremy Howard's spec.** Format compliance is by construction, not asserted. Adding a parser-validator is out of scope.

## Dependencies added

- `pyyaml` (Python, MIT) — frontmatter parsing. **Green tier** per `feedback_green_dep_auto_proceed.md`.

## Test plan

Forge implements `tests/test_docs_llms.py` with fixtures (no shared state with mnemra core code). Required cases:

1. **Golden llms.txt:** fixture `docs/src/` minimal tree → produces exact expected `llms.txt` (string equality).
2. **Golden llms-full.txt:** same fixture → exact expected `llms-full.txt`.
3. **SUMMARY ordering preserved:** fixture with 3 pages in non-alphabetical SUMMARY order → llms-full.txt respects SUMMARY order.
4. **Tail appending:** fixture with one page in SUMMARY, one outside SUMMARY → llms-full.txt has SUMMARY page first, non-SUMMARY page in tail.
5. **Empty summary:** fixture with empty `summary:` on a SUMMARY-referenced page → generator exits 1 with error.
6. **`--check` clean:** fixture state matches committed `_published/` → exit 0.
7. **`--check` drift:** fixture state differs from committed `_published/` → exit 1.

Property-based tests are not required for this step (small surface, deterministic).

## Out-of-scope-but-noted (for sequencing memory)

- llms-ctx.txt / llms-ctx-full.txt (Jeremy Howard "context variants" — minimal vs full context surfaces). Not generated. Revisit if external integration request surfaces it.
- robots.txt / sitemap.xml updates to advertise llms.txt to crawlers. Not in scope; sitemap doesn't currently exist either.
- llms.txt schema linting in CI. Not in scope.

## Sequencing inside step 5

Suggested Forge sequence (Forge may reorder):

1. Add summaries to existing 7 frontmatter pages.
2. Add `intro.md` summary (currently no frontmatter? — verify; add `title`/`summary`/`primary-audience` block if missing).
3. Write `scripts/docs-llms.py`.
4. Write `tests/test_docs_llms.py` with goldens.
5. Add `docs-llms` justfile recipe; update `check` to invoke `--check`.
6. Run `just docs-llms`, commit `docs/_published/llms.txt` + `llms-full.txt`.
7. Update `.gitignore` (`docs/_build/` ignored; `docs/_published/` not).
8. Update `.github/workflows/docs.yml` with the stage step.
9. Local validation: `just check` clean.

Single PR, single feature branch (`step5-llms-generation`), bypassPermissions worktree.
