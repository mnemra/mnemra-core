---
title: "Intake: Reporting Engine"
summary: "Intent capture for the extensible reporting engine (registry + declarative read-only user reports), re-derived for mnemra-core from the 2026-05-31 worked spec."
primary-audience: agent
spec_type: code
frame_relevant: true
---

# Intake: Reporting Engine

**Stakes:** medium
**Date:** 2026-07-03
**Status:** locked · **Locked: 2026-07-03** (intake-exit gate confirmed by the maintainer)
**Consumer:** mixed — agents (MCP clients, primary) + the maintainer/operator (secondary); downstream consumers of the resulting spec are the verification workflow and implementing agents

## JTBD

Operators and agents working over mnemra-core's stored context and measurement data (tasks, dispatches, skill runs, activity, artifacts) need repeatable, named, parameterized reports available without a code change, rebuild, or redeploy — so that recurring data cuts stop being hand-rolled one-off queries that drift and sometimes get computed wrong.

Stated as the need, not the solution: when a cut of the data has no pre-built report, the operator or agent today would hand-roll a query in-session; the recurring failure is a wrong or inconsistent hand-rolled computation where a named, reviewed, reusable report should exist. (The maintainer's recorded reasoning from the prior metrics-layer work: the read layer — the reports — is the point of a metrics/context store; without repeatable reports the data sits unused or gets misread.)

## Non-goals

Each a concrete not-this. Items 3–5 are the engine-scope extras the worked spec excluded; they carry named tripwires rather than silent absence (ratified out at the 2026-07-03 intake-exit gate; the discovery-stage security review independently endorsed all three exclusions as threat-surface reductions):

1. **No write or mutation capability in report execution, of any kind.** Read-only is the engine's identity, not a configuration.
2. **No write or mutation path in report *execution*; no new schema, tables, or migrations.** Report execution is purely a read-side consumer of whatever the substrate and plugins store. Report-definition **authoring** is necessarily a write surface and is scoped separately: its persistence + authoring mechanism is an explicit Frame decision (Open item 6) — the not-this here is the engine growing bespoke storage machinery beyond what that decision sanctions.
3. **No result materialization or caching.** Tripwire (instrumented): per-report execution-latency metrics — the engine's slice of the query-instrumentation surface P-0017 establishes off the observability baseline — crossing a latency budget the spec sets. The instrument fires the revisit; a slow report noticed by hand does not.
4. **No scheduled report execution.** Tripwire: the first recurring-report need that cannot be served by an external scheduler invoking the report surface.
5. **No report-to-destination delivery** (files, webhooks, stored artifacts). Results return to the invoking caller. Tripwire: the first consumer that cannot consume the return path.
6. **Not a BI / dashboard / visualization product.** A dashboard console is a separate register idea; this engine returns structured results.
7. **No refactor of canonical (built-in) report computations onto the user-report machinery.** Built-ins keep their compiled logic; the engine changes dispatch and rendering only (carried from the worked spec's built-in-preservation requirements).
8. **The engine does not gate V0** (phasing ratified 2026-07-03): canonical built-in reports ride their capability-family increments as part of workspace fidelity; the extensible engine is a post-`1.0.0` register entry (`1.3.0`+ candidate) — a placement that also sequences it after the policy-envelope enforcement machinery it depends on (which lands with the `1.1.0` retrieval feature).

## Success criteria

Each an observable outcome a downstream check could verify:

1. **Runtime extensibility:** a named report definition can be added while the system is deployed (no recompile, no redeploy) and then invoked through the locked surface, returning correct rows. Adding a report is authoring a definition, not shipping code.
2. **Read-only execution holds adversarially:** user-authored report queries cannot mutate state (binary-testable negative acceptance criteria: mutating/structural statements are rejected before any row is touched) and cannot read outside the invoking principal's authorization scope — workspace isolation, the role matrix, and the provenance/policy-envelope predicates hold on the report read path. The enforcement mechanism MUST be re-derived for user-authored SQL rather than assumed inherited: every other read path is host-constructed (typed `WorkspaceCtx` threading the workspace predicate into a host-built WHERE clause, per P-0006), which does not extend to a caller-written SELECT, and V0 defers general RLS enforcement. This re-derivation is the first Frame threat-model seed.
3. **Uniform output:** built-in and user reports render identically for the same output shape and format; an empty result is a success with an empty rendering, never an error; NULL values render explicitly and are never silently dropped.
4. **Worked-spec disposition map:** every requirement of the 2026-05-31 worked spec (R80–R99) is dispositioned carried / adapted (with the substrate reason) / dropped (with reason) in the new spec — no silent drops.
5. **Designed-tier completion (register-tier-exit marker, not a behavioral criterion):** locked frame + locked spec exist; the product-brief register entry for the reporting engine reaches `designed`. This marks pipeline state only — it says nothing about engine behavior and MUST NOT be read by verification as evidence the engine works; SC1–SC3 carry the behavioral bar.

## Hard constraints

Locked tech choices and integration boundaries this work inherits (constraint sources named per the project convention):

- **MCP-native agent surface** (product brief, Hard constraints): reports MUST be agent-invocable via MCP. Ratified posture (2026-07-03): MCP primary, admin-CLI convenience secondary; the tool-vs-resource verb shape is a Frame decision.
- **Postgres substrate** (P-0010): single-process Postgres + pgvector behind the engine-agnostic `Storage` trait. The worked spec's braincli-shaped mechanisms — the SQLite connection-authorizer allow-set, custom `median()`/`percentile()` aggregates, and the filesystem reports-directory discovery model (its R82/R86: manifest files in config directories) — do not transfer; Postgres natively provides `percentile_cont`/`percentile_disc`, read-only enforcement must be re-derived from Postgres mechanisms, and definition persistence/authoring must be re-derived for a single-binary MCP server (Open item 6). The re-derivations are Frame/Spec work; the invariants they must preserve are fixed here.
- **Workspace isolation** (P-0006): typed `WorkspaceCtx` binding at the host-fn boundary, WHERE-clause-mandatory discipline; tenancy scoping key structural from V0. Report execution MUST NOT provide a path around workspace scoping.
- **Role matrix** (P-0009): admin / read-observer role enforcement applies to report invocation and report-definition authoring as MCP verb categories.
- **Provenance / policy envelope** (P-0015): report execution is a serving channel; the policy permissions record (`dont_use`, `model_egress`, `visibility`, `tenant_share`) applies at named enforcement points on the report read path. The engine MUST NOT become a policy bypass.
- **Plugin architecture** (P-0019, P-0013, P-0017): storage is plugin-namespaced with cluster-carried policy; cross-plugin reads are host-mediated; plugin core logic is IO-free with host-mediated IO. Whether the engine is host-core or a plugin is a Frame decision; the contract constraints bind either way.
- **Read-only execution as defense-in-depth invariant** (carried from worked spec R85): at least two independent mechanisms, each alone sufficient to block a write; parameters are bound, never interpolated. Mechanism re-derived for Postgres at Frame/Spec.
- **Rust, no new ecosystem** (P-StackDiscipline); dependency additions pass the workspace license-tier gate.

## Evidence

- **Recorded need (the anchor):** the maintainer's reasoning captured during the prior metrics-layer work — a repeatable read layer is the point of the store; without it the orchestrator hand-rolls one-off queries and errs. Cited by decision-name + date per the provenance-pointer convention: metrics-layer stash reasoning, 2026-05-31.
- **Worked spec (proven shape):** the reporting-engine amendment, drafted 2026-05-31 — R80–R99 with binary-testable acceptance criteria, the read-only allow-set authorizer hardening, registry with two report kinds, name reservation, uniform output. Maintainer-internal record, cited by name + date. Its braincli/SQLite substrate assumptions are the delta this run re-derives.
- **Feature-option record, 2026-06-01:** the durable need statement plus the open questions this intake resolves (surface, trust boundary, substrate transfer).
- **Use case (operator, end-to-end):** the operator wants dispatch cost grouped by agent since a date; no built-in covers it. They author a declarative report definition with a typed `since` parameter; the definition registers at runtime; the operator and agents invoke it thereafter with bound parameters, getting consistent, reviewable results instead of per-session hand-rolled SQL.
- **Use case (agent, end-to-end):** an MCP-client agent preparing a retrospective invokes a registered report for skill-run flag tallies over a date range in one call, rather than issuing ad-hoc queries it must re-derive (and may get wrong) each session.

## Risk profile

**Touches a trust boundary — flagged at intake (required Stage-2 constraint).** User-authored queries execute against the shared store. The worked spec's threat model was explicitly "trust boundary NONE — operator-mistake guard, single operator, local file" (its C-D). mnemra-core changes that: authenticated principals (multiple humans and agents per workspace, roles per P-0009, workspaces per P-0006, policy envelope per P-0015) author and invoke reports against a shared Postgres instance. The read-only-execution guard graduates from mistake-guard to adversary-guard; authorization scope, policy-predicate enforcement, and resource abuse (expensive queries) enter the threat model. The security-mode review fires at Frame, where the mechanism is known.

The Frame threat model opens from these seeds (from the discovery-stage security review):

1. **Workspace isolation on caller-authored SQL** — no host-threaded WHERE clause to inherit, general RLS deferred at V0; the headline trust boundary.
2. **Policy-envelope predicates on a caller-written query** — P-0015's serving-predicate injection assumes a host-built query; a user-authored SELECT has no host-built WHERE clause to inject into.
3. **Query-cost containment** — statement timeout / row cap / cost ceiling; an availability primitive the plugin resource limits (P-0007) do not cover, since they bound plugin CPU, not Postgres query cost.
4. **Invocation-vs-authoring role split** — authoring is a write (admin-gated per the P-0009 matrix); a saved report's results scope to the *caller's* role and workspace, never the author's.
5. **Enumeration discipline** — the report-listing surface must not become an existence oracle for another principal's data (the P-0015 no-existence-oracle posture applies).

## Consultations

None yet. (Mode A consultations recorded here as they occur.)

## Dismissed review flags

None dismissed — intake review round 1 (security-reviewer lens, discovery stage, 2026-07-03) returned approve-with-conditions with zero blocker/high findings; all four conditions were folded as edits in this revision (definition-store Frame deferral, SC2 mechanism-re-derivation reword, Non-goal-3 instrumented tripwire, SC5 tier-marker annotation). Three low-severity findings were suppressed below the review's confidence threshold.

## Intake-exit gate record (confirmed 2026-07-03)

Ratifications at the gate:

1. **Phasing — split, ratified.** Canonical built-in reports ride their V0 capability-family increments (workspace fidelity); the extensible engine is its own post-`1.0.0` register entry at `proposed` (`1.3.0`+ candidate). Decided with the item-4(b) sequencing dependency visible.
2. **Engine-scope extras — all out, ratified.** Non-goals 3–5 stand with their tripwires; each exclusion removes a threat surface (stored-result retention, unattended execution, outbound delivery).
3. **Surface — MCP primary + admin-CLI convenience, ratified.** Tool-vs-resource verb shape is a Frame decision.
4. **`spec_type = code`, ratified per the pinned taxonomy rule.** `code` = any spec whose end-state includes implementation, even while parked at designed tier (the 2026-07-02 hardening). The intake review's observation that the bundle's decisions are ADR-shaped is real but does not retype the spec: ADR-shaped decisions produced alongside a code-destined spec land as P-ADRs; the spec itself keeps the impl-side verification contract. `frame_relevant` forced `true`.

Deferred to Frame, with named forcing mechanism (the Frame stage cannot produce its constraint set without resolving them):

5. **Report-definition persistence + authoring model.** Candidate shapes: definitions as content artifacts through the existing host-fn write machinery (pulls in P-0017 cluster classification + P-0003 manifest registration), deployment-configuration files, or a bespoke store (disfavored by Non-goal 2). Whichever lands, definitions are durable content whose `sql` text can embed sensitive filter literals — they inherit the P-0015 `visibility` dimension. Resolves the SC1↔Non-goal-2 tension.
6. **Two coupled Frame decisions — resolve jointly, not in separate passes:**
   - (a) **Host-core-vs-plugin ⟷ cross-plugin-read enforcement.** Report SQL over tasks + dispatches + skill-runs spans plugin-namespaced table families. A plugin is a leaf (P-0019) and cannot read sibling plugins' tables; host-core makes cross-plugin reads host-mediated (P-0017). The placement decision and the enforcement shape are one decision.
   - (b) **Phasing ⟷ P-0015-enforcement sequencing.** The policy-envelope enforcement machinery (serving predicate, egress gate, decision port) lands with the V0.1 retrieval feature. A report read path that must honor P-0015 predicates cannot ship its enforcement before that machinery exists. The ratified post-`1.0.0` placement (item 1) respects this; the Frame carries it as a stated dependency.
