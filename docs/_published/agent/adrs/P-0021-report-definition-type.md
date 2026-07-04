---
title: "P-0021: Report Definition Type"
summary: "The report-definition content type: a host-core-owned reference-cluster content artifact written through the existing host-fn content-write machinery — deliberately NOT a P-0018 core FK-target entity — with a locked definition schema, write-time validation semantics (including rejection of data-modifying CTEs so an invalid definition never registers), full P-0015 policy-record inheritance, reserved-name reservation, and (workspace_id, name) uniqueness."
primary-audience: agent
---

---
status: "accepted"
date: "2026-07-03"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
overrides: null
---

# P-0021: Report Definition Type

**Project:** mnemra-core

## Status

`proposed`

Authored at the reporting-engine Stage-3 spec; flipped to `accepted` at the spec-exit gate. Resolves the Frame's `{{P-ReportDefinitionType}}` open ADR slot. Binding requirement text: [reporting-engine spec](../../specs/2026-07-03-reporting-engine.md) R-0041–R-0042, R-0053, R-0057, R-0061–R-0062.

## Context and Problem Statement

The reporting engine's headline capability is **runtime extensibility** (intake SC1): a named report definition can be added while the system is deployed — no recompile, no redeploy — and then invoked through the locked MCP surface. That requires report definitions to be **durable data, not code**. Intake Non-goal 2 forbids the engine growing bespoke storage machinery beyond what the definition-persistence decision sanctions; the Frame's 2a-2 direction resolved that decision: report definitions are **content artifacts written through the existing host-fn content-write machinery** — no new writer, no new schema beyond the one per-artifact-type table every content type gets.

Three questions this ADR closes, each of which the reporting spec gates on:

1. **What kind of content type is a report definition, and where does it sit relative to the [P-0018](P-0018-core-entity-manifest.md) core-entity set?** It is host-core-owned (like the core entities, it lives above the plugin boundary) but it is deliberately **not** a P-0018 hard-FK-target core entity. The ADR must *argue the kind's place* against P-0018's short-list/portability value, not merely name it: a host-core content type is inherited by every adopter, so its place on (or off) the short list is a real commitment.
2. **What is the definition schema, and what does write-time validation enforce?** A definition's body is caller-authored SQL; the validation semantics are a contract definition authors (human and agent) write against, and one clause — the rejection of data-modifying CTEs at write time (the folded review-L1 finding) — is load-bearing: an invalid definition must never register.
3. **How do policy, naming, and uniqueness bind?** Definitions inherit the [P-0015](P-0015-provenance-envelope-source-roles.md) policy record (their `sql` text can embed sensitive filter literals); reserved built-in names are excluded; `(workspace_id, name)` is unique.

This ADR does **not** own the execution security model (the read-only role, RLS, session-key integrity, cost containment) — that is [`{{P-ReportExecutionContext}}`](P-0020-report-execution-context.md) (P-0020). It owns the *definition-as-content-artifact* contract: the type, the schema, and the write-time gate.

## Decision Drivers

- **SC1 runtime extensibility requires durable data, not code (Decomposition).** A definition that lands must be resolvable on the next lookup with no restart; the only canon-clean home under Non-goal 2 is the existing content-artifact machinery.
- **Non-goal 2 — no new writer, no bespoke store (Simplicity, Migration cost).** Reusing the content-write internals (validation, `WorkspaceCtx` threading, workspace scoping, P-0015 policy columns, audit emission) means no new writer class.
- **The P-0018 negative space is load-bearing (Honesty, portability).** [P-0018](P-0018-core-entity-manifest.md) closes the core FK-target set at four (projects, actors, tags, attachments) because each core entity is a permanent commitment every adopter and every alternative orchestrator inherits. A report-definition type that became a core entity would force that inheritance for no FK-target reason — it is referenced by name at lookup, not FK'd to.
- **Every content type must be classified (P-0017 D-CM).** The four-shape taxonomy (state-bearing / narrative / reference / associative) attaches shared policy to a shape; a definition must be classified.
- **Validator before field (P-ShiftLeft D2).** The definition schema's fields (name pattern, typed parameters, single-read-statement body) each have a mechanical validator run at write time; a definition failing any clause never registers.
- **The write-time gate is the first read-only layer, not the security boundary.** The P-0020 role + read-only transaction are the security back-stop; the write-time single-read-statement check is validation-completeness (an invalid definition never registers, rather than registering and failing at every invocation). Both exist; neither is a single gate.

## Considered Options

The Frame ratified the **content-artifact-through-existing-write-machinery** direction (2a-2), so the store-shape option is decided. The genuine options this ADR closes are the **kind/placement** relative to P-0018 and the **cluster classification**.

1. **K1 — Host-core-owned `reference`-cluster content type, NOT a P-0018 core entity (chosen).** A per-artifact-type content table under C1, classified `reference`, written through the existing machinery; referenced by name at lookup, never a hard FK target.
2. **K2 — A P-0018 core entity.** Add report-definitions to the closed core-entity set as a fifth hard-FK-target entity.
3. **K3 — A bespoke definition store** (its own writer/table shape outside the content machinery). Disfavored by Non-goal 2.

## Decision Outcome

**K1 — a host-core-owned `reference`-cluster content type that is deliberately not a P-0018 core entity.** Four decisions lock.

### D-KIND — Host-core content type, argued off the P-0018 short list

A report definition is a **content artifact**: one row in its own per-artifact-type table under the C1 layout (JSONB frontmatter + body + system fields per [P-0001](P-0001-storage-layout.md)), owned by **host-core**. Like the [P-0018](P-0018-core-entity-manifest.md) core entities it lives above the plugin boundary (report SQL spans plugin table families — [D1 of the Frame / P-0002](P-0002-core-plugin-partition.md) places cross-plugin projection host-side); **unlike** them it is *not* a hard-FK target and does not join P-0018's closed core-entity set.

**Argued against P-0018's short-list/portability value.** P-0018 draws the core/plugin line at *universality of reference target*, not frequency: a core entity is a permanent commitment every adopter and every alternative orchestrator inherits, so the bar is "genuinely universal FK target." A report definition fails that bar in the right direction: it is **referenced by name at registry lookup** (R-0041-a), never FK'd to — nothing in the system holds a `report_definition_id` foreign key. It is host-core-owned for a *placement* reason (cross-plugin read projection is host-side, P-0002), not a *reference-target* reason. Making it a core entity (K2) would force every adopter to inherit a workflow-shaped content type as a hard-FK target for no integrity need — exactly the negative-space error P-0018 D-BOUNDARY guards against (workflow primitives are plugin-introduced; a host-core *projection/read-surface* artifact is a third category, neither a core FK entity nor a plugin content type). It is host-owned because the read surface it powers is host-mediated, and it is off the FK-target short list because nothing references it by key. *(Anchors: 2a-2 ratified; [P-0018](P-0018-core-entity-manifest.md) D-ENT/D-BOUNDARY — the closed four-entity set + the negative space; [P-0002](P-0002-core-plugin-partition.md) — host-side cross-plugin projection; [P-0001](P-0001-storage-layout.md) C1. Binding text: spec R-0042-a.)*

### D-CLUSTER — `reference` cluster classification

The definition classifies as the [P-0017](P-0017-storage-cluster-model.md) **`reference`** cluster shape: a read-mostly, long-lived, named lookup — authored rarely, invoked repeatedly, resolved by exact name — the same shape as the taxonomy's `templates` exemplar. It is not `state-bearing` (it carries no mutating lifecycle state at V0) and not `narrative` (frontmatter dominates; the body is one SQL statement). Audit on authoring writes comes from the write path itself (admin-gated, attributable — the PE-6 pattern), not from a per-shape policy. The classification was proposed at the Frame and **accepted at the Frame-exit gate** (P-0017's maintainer-call rule for non-self-evident shapes). *(Anchors: [P-0017](P-0017-storage-cluster-model.md) D-CM (the four-shape taxonomy; `reference` shape); Frame-exit gate acceptance. Binding text: spec R-0042-a.)*

### D-SCHEMA — The definition schema contract

A definition's frontmatter carries:

- `name` — string, REQUIRED, matching `^[a-z0-9][a-z0-9-]*$` (lowercase alphanumeric + hyphen, not starting with a hyphen), and NOT a reserved built-in name (D-VALID); the registry key.
- `description` — string, REQUIRED; the one-line human/agent description surfaced by `report.list`/`report.get`.
- Typed parameter declarations — zero or more, each declaring a `type` from the closed V0 set `{string, date, int}` with `required` (default `true`) and optional `default` semantics: an omitted parameter with a declared `default` binds the default at invocation; an unrecognized parameter key at invocation is rejected (spec R-0057, all four required/default/NULL cases + the unknown-key mirror). Richer types are a spec amendment.
- A default output format.

The body is **exactly one read statement** (D-VALID). Uniqueness is `(workspace_id, name)` among definitions, plus the reserved-name exclusion. Any unknown top-level frontmatter key is ignored (forward-compat) and does not invalidate the definition. *(Anchors: [P-0001](P-0001-storage-layout.md) C1; carried worked-reference R86/R93; P-LockContract — the schema is the locked contract, storage mechanics vary behind the `Storage` seam. Binding text: spec R-0042-b, R-0057.)*

### D-VALID — Write-time validation; a data-modifying CTE never registers

Before the row lands, the write path runs:

1. **Schema validity** at declared types.
2. **Name rules + built-in-name reservation** — a create/update naming a reserved built-in name is rejected with the collision named; the definition does not register (R-0041-b). `(workspace_id, name)` uniqueness is enforced (R-0041-c).
3. **The single-read-statement check** — the body parses, is exactly one statement, and is a read (`SELECT` / `WITH…SELECT`), with **every CTE itself read-only**. A **data-modifying CTE** such as `WITH t AS (DELETE … RETURNING *) SELECT …` is `WITH…SELECT`-shaped yet writes, and is **rejected at write time** (the folded review-L1 finding). The P-0020 role + read-only transaction already reject it at execution, so this bar is validation-completeness, not the security boundary — an invalid definition never registers rather than registering and failing at every invocation.
4. **Placeholder↔parameter reconciliation** — a `:param`-style placeholder with no declaration is a hard error naming the placeholder; a declared-but-unused parameter is a warning naming it, and the definition still registers (carried worked-reference R94).
5. **Parameter `default` values** pass their own type validation.

An invalid definition never registers. The worked reference design's validity-vs-not-a-manifest discrimination (its R87) collapses here to plain schema validation, because the store contains only rows written through this gate; the "unrelated file in a shared directory" case has no Postgres analog and is **dropped** with the filesystem discovery model. *(Anchors: P-ShiftLeft D2 (validator before field); the folded review-L1 data-modifying-CTE finding; carried worked-reference R86/R87/R94. Binding text: spec R-0042-c, threat-test T5.)*

### D-POLICY — Full P-0015 policy-record inheritance; naming and uniqueness

A definition inherits the full [P-0015](P-0015-provenance-envelope-source-roles.md) policy record with unchanged semantics:

- `visibility` governs who can list/read/invoke a definition — its body text can embed sensitive filter literals (the intake's stated motivation). `admin-only` serves callers whose session role is `admin` (the canonical lowercase P-0009 `mnemra.role` encoding — spec R-0045-a); `owner-only` follows PE-4 V0 semantics (serves no one until per-user identity lands); a `visibility`-withheld definition resolves **not-found**, indistinguishable from a nonexistent name (PE-4 not-found-over-stub — spec R-0053-b).
- `dont_use` is the curatorial kill switch: a killed definition resolves to the **metadata-only stub** with the policy reason (PE-4 `dont_use` disposition — curatorial policy announces itself; spec R-0053-b).
- **Disposition precedence when both flags apply (r2 fold):** `visibility` entitlement is evaluated FIRST — a `visibility`-unentitled caller receives not-found regardless of `dont_use`; the stub reaches only a caller already `visibility`-entitled (the retrieval precedent: the R-0025-g edge filter gates on the `visibility` serving predicate before any other disposition). Evaluating `dont_use` first would leak a withheld definition's existence and kill reason — the existence oracle PE-4's indistinguishability exists to prevent (spec R-0053-b, threat-test T6's both-flags fixture).
- `tenant_share` is the structural workspace-only constant at V0.

Every policy-field write on a definition rides the PE-6 admin-gated path with an attributable audit row (actor token identity NOT NULL). The definition table carries the P-0015 policy columns with their permissive not-set DDL defaults (PE-2), applied through the content-write path. *(Anchors: [P-0015](P-0015-provenance-envelope-source-roles.md) PE-2/PE-4/PE-6; carried worked-reference R83 (name reservation) adapted to write-time rejection + `(workspace_id, name)` uniqueness. Binding text: spec R-0042-d, R-0041-b/-c, R-0053.)*

### Consequences

**Good:**
- SC1 runtime extensibility is write-then-invoke with no restart: a definition that lands through the gate is resolvable on the next lookup, because the store is the source of truth (P-0017 D-SoT) and the registry cache is a rebuildable derivation.
- No new writer, no bespoke store — the content-write internals carry validation, `WorkspaceCtx` threading, workspace scoping, policy columns, and audit, satisfying Non-goal 2 on everything but the one gate-accepted definition table.
- The P-0018 short list stays closed at four; portability is preserved — an alternative orchestrator adopts mnemra-core without inheriting a report-definition FK target.
- The write-time gate makes an invalid definition never register (validation-completeness), and the data-modifying-CTE rejection closes the L1 finding at authoring time as well as execution time.
- Definitions inherit the policy envelope, so a definition whose body embeds sensitive literals is `visibility`-governed like any content.

**Bad / Trade-offs:**
- One new per-artifact-type table strains a literal reading of Non-goal 2 ("no new schema, tables, or migrations") — named and gate-accepted (the intent self-report strain flag a); the execution path adds zero schema.
- The definition type is a *third category* (host-core projection/read-surface content) alongside P-0018's core-entity and plugin-content categories; the ADR names it explicitly rather than forcing it into one of the two existing kinds, which is a small conceptual addition future host-owned content types can reuse.
- Write-time SQL parsing to enforce the single-read-statement + no-data-modifying-CTE check is a parse-shape dependency; the spec assesses any SQL-parsing need against in-stack Postgres paths first (the `Storage` seam can prepare-and-inspect) before adding a parser dependency, under the workspace license-tier gate.

## Pros and Cons of the Options

### K1 — Host-core `reference`-cluster content type, not a core entity (chosen)

- Pro: reuses the content-write machinery (Non-goal 2); classified into the existing taxonomy; policy-inherited.
- Pro: keeps the P-0018 short list closed — no adopter inherits a report-definition FK target.
- Con: adds one new content table (the gate-accepted Non-goal-2 strain); introduces a third host-owned-content category the ADR must name.

### K2 — A P-0018 core entity

- Con: forces every adopter and alternative orchestrator to inherit a workflow-shaped content type as a hard-FK target for no integrity need — the negative-space error P-0018 D-BOUNDARY guards against.
- Con: nothing references a definition by key, so the FK-target commitment buys nothing.

### K3 — A bespoke definition store

- Con: a new writer class + bespoke table shape outside the content machinery — the exact bespoke-storage growth Non-goal 2 forbids.
- Con: re-implements validation, `WorkspaceCtx` threading, policy columns, and audit that the content path already provides.

## More Information

- Frame open ADR slot: `{{P-ReportDefinitionType}}` ([Frame](../intent/reporting-engine-frame.md) §Open ADR slots). This ADR resolves it.
- Binding requirement text: [reporting-engine spec](../../specs/2026-07-03-reporting-engine.md) R-0041 (registry + reservation + uniqueness), R-0042 (content type + schema + write-time validation), R-0053 (enumeration/disposition), R-0057 (parameters), R-0061 (live-schema contract), R-0062 (tenancy invariants); threat-test T5 (data-modifying-CTE rejection).
- Depends on / cites: [P-0001](P-0001-storage-layout.md) (C1 single-document layout — the definition row shape); [P-0017](P-0017-storage-cluster-model.md) (D-CM `reference` cluster classification; D-SoT store-as-source-of-truth); [P-0018](P-0018-core-entity-manifest.md) (D-ENT/D-BOUNDARY — the closed core set + the negative space this type argues off of); [P-0002](P-0002-core-plugin-partition.md) (host-side cross-plugin projection — the placement reason); [P-0015](P-0015-provenance-envelope-source-roles.md) (PE-2/PE-4/PE-6 — policy inheritance, dispositions, write gate).
- Companion: [`{{P-ReportExecutionContext}}`](P-0020-report-execution-context.md) (P-0020) — the read-only execution context whose role + transaction back-stop the write-time single-read-statement check; the two together are the defense-in-depth (validation-completeness at write, security at execution).
