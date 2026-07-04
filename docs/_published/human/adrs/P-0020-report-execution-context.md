---
title: "P-0020: Report Execution Context"
summary: "The dedicated read-only Postgres execution context for the reporting engine: a SELECT-only report role plus READ ONLY transaction (two independent write-blocks), a derived table grant set + two-part function-EXECUTE lock with nameable-complete default-deny exclusions, a scoped RLS exception keyed on per-execution session settings with session-key tamper-resistance (GUC-setter revocation), the ownership-without-FORCE host-bypass model as a correctness precondition (BYPASSRLS excluded), timeout/row-cap/bounded-pool cost containment with a caller-visible saturation contract, and the OC-10 reconciliation check with a standing migration-apply trigger — the scoped exception to the P-0006/P-0009 deferred-RLS posture on exactly the caller-authored-SQL path."
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

# P-0020: Report Execution Context

**Project:** mnemra-core

## Status

`proposed`

Authored at the reporting-engine Stage-3 spec, then flipped to `accepted` at the spec-exit gate. It resolves the Frame's `{{P-ReportExecutionContext}}` open ADR slot. Binding requirement text: [reporting-engine spec](../../specs/2026-07-03-reporting-engine.md) R-0043–R-0046, R-0048, R-0050, R-0060.

## Context and Problem Statement

The reporting engine (register post-`1.0.0`, `1.3.0`+ candidate) lets operators and agents author declarative reports whose bodies are **caller-authored SQL** run against the shared mnemra-core substrate. That single fact graduates the read-only-execution guard from the worked reference design's "operator-mistake guard, single operator, local file" (its trust-boundary NONE) to an **adversary guard**. Authenticated principals author and invoke reports against one shared Postgres instance: multiple humans and agents per workspace, roles per [P-0009](P-0009-rls-admin-token.md), workspaces per [P-0006](P-0006-v0-tenant-enforcement.md), and a policy envelope per [P-0015](P-0015-provenance-envelope-source-roles.md).

Two mnemra invariants don't extend to this path:

1. **Workspace isolation.** [P-0006](P-0006-v0-tenant-enforcement.md) enforces `workspace_id` by threading a typed `WorkspaceCtx` into a **host-built** `WHERE` clause. A caller-written SELECT has no host-built `WHERE` clause to thread, so the P-0006 mechanism can't bind it. And general row-level-security (RLS, Postgres's per-row access-policy feature) enforcement is deferred at V0 (accepted risk `R-0001`).
2. **Read-only.** Every other read path is host-constructed. Here the statement text comes from the caller.

This ADR locks the **dedicated read-only execution context**: the mechanism by which report SQL (both user definitions and built-ins) runs with the read-only, workspace-isolation, policy-envelope, and cost-containment guarantees the host can't otherwise provide on caller-authored SQL. The Frame ratified the direction (2a-3) and locked the property space. Several sub-mechanisms had a fixed candidate space that the Frame delegated to this ADR to pick and argue. The Frame's D3 is the elaborated source; this ADR is the durable, adopter-facing record of the engine's headline security guarantee.

A scoped exception to a two-ADR accepted-risk posture ([P-0006](P-0006-v0-tenant-enforcement.md)/[P-0009](P-0009-rls-admin-token.md) defer RLS) needs its own ADR. Its reversal or drift forces re-derivation of the engine's headline security guarantee, and external adopters plus every downstream security review reference it.

## Decision Drivers

- **P-SecurityLayered, each layer independently load-bearing.** Read-only must survive the loss of any single mechanism; workspace and policy isolation must be database-enforced on the one path where application-layer discipline can't reach.
- **The `Security ⇄ Simplicity` conflict edge, resolved default-to-Security.** Caller-authored SQL is an adversarial surface, so the extra mechanism is warranted (constraint-edges: "default-to-Security per default-on, never opt-in").
- **The general R-0001 RLS deferral must stand unchanged.** The exception is scoped to exactly the caller-authored-SQL path. Host-constructed queries keep the P-0006 application-layer posture, and the R-0001 trip-wire (first multi-workspace production deployment) is unchanged. The exception must **converge** with the eventual general RLS activation, not fork from it.
- **`ENABLE ROW LEVEL SECURITY` activates RLS for every role.** Attaching a report-role policy to a table turns RLS on for the host's primary role too. The host stays untouched only if it provably bypasses. This is a correctness precondition of the "deferral-stands" claim, not spec-tier detail.
- **A `READ ONLY` transaction does NOT block `set_config()`.** Postgres doesn't treat GUC changes (grand-unified-configuration settings, the server's runtime config parameters) as writes, and `EXECUTE` on `set_config` is a default `PUBLIC` grant. The session-key integrity property must be proven, never assumed from transaction mode (the folded review-H1 correction).
- **TrustworthySignal TS2, an execution-gating enumeration ships with its reconciliation check.** The grant sets (tables, functions) and RLS coverage are hand-maintained allow-lists that decide what a caller can read and execute. They carry a computed-reality-vs-declared check that fails on a seeded violation.
- **Cost containment is an availability primitive P-0007 doesn't provide.** [P-0007](P-0007-plugin-resource-limits.md) bounds plugin CPU (fuel/epoch), not Postgres query cost, so the report path needs its own timeout/row-cap/pool.

## Considered Options

The Frame ratified the **dedicated read-only execution context** direction (2a-3) and locked the property space. This ADR's genuine option is the **session-key tamper-resistance mechanism**, whose candidate space the Frame fixed at exactly two. Introducing a third isn't on the table.

1. **S-a, revoke the GUC-setter surface from the report role.** Remove `set_config(text,text,boolean)` and its `pg_catalog` kin from the report role's function-EXECUTE allow-list; RLS policies stay keyed on the host-set session GUCs (the [P-0009](P-0009-rls-admin-token.md) V0.1 keying shape). No permitted function may set a GUC directly or indirectly.
2. **S-b, key the report-path RLS on a caller-unreachable value.** Per-execution `SET ROLE` to a workspace-bound role, so policies key on `current_user` rather than a settable GUC.

## Decision Outcome

**The dedicated read-only execution context, with session-key tamper-resistance delivered by S-a (revoke the GUC-setter surface).** Six mechanisms lock.

### D-CTX — All report SQL executes in a dedicated read-only context

All report SQL, user definitions AND built-ins, executes in a dedicated execution context: a dedicated Postgres role, `READ ONLY` transactions, RLS policies scoped to that role, per-execution session settings carrying the **caller's** identity, `statement_timeout`, a host-enforced row cap, and a bounded, separate connection pool. Running built-ins here too is deliberate. Intake Non-goal 1 makes read-only the engine's identity "of any kind," so the execution posture is uniform, and built-ins carry a *third* layer (their host-built SQL keeps the P-0006 host-threaded workspace predicate). *(Anchors: 2a-3 ratified; P-SecurityLayered; the `Security ⇄ Simplicity` edge resolved default-to-Security. Binding text: spec R-0043-a, R-0060-a.)*

### D-RO — Two independent write-blocks, each alone sufficient

The worked reference design's R85 defense-in-depth pair, re-derived for Postgres:

1. **Report role holds `SELECT`-only privileges:** no `INSERT`/`UPDATE`/`DELETE`, no DDL (data-definition statements like `CREATE`/`ALTER`), no sequence usage.
2. **Every report transaction is `READ ONLY`** (`SET TRANSACTION READ ONLY` / `default_transaction_read_only`).

Either alone blocks a write; both apply together. The spec's threat tests prove each independently (spec R-0043-c / threat-test T2). *(Anchors: P-SecurityLayered, each layer independently load-bearing; worked-reference R85. Binding text: spec R-0043.)*

### D-GRANT — Table + function-EXECUTE allow-lists, default-deny exclusions, reconciliation

The report role's readable and callable surface is an **allow-list**, the Postgres re-derivation of the worked reference design's SQLite authorizer allow-set.

- **Table grant allow-list, a derivation rule over two canonical sources.** The granted-table set is *derived*, not hand-listed (the substrate's content tables are generated per-artifact-type from the content-type registry, manifest-extensible, so a hand-maintained list would drift): **(i)** every per-artifact-type content table created through the host content machinery is in the grant set, and receives its report-role RLS policies at table creation through that same machinery, **primary tables only** (the machinery's history/shadow tables are excluded, deferred behind the first versioned-rows report need); **(ii)** the product-measurement tables (the dispatch/skill-run/activity measurement families) are enumerated by name in the provisioning DDL, the canonical enumeration source the reconciliation parses. Changing the membership *rules* (adding a family, granting a core-entity or substrate table, removing an exclusion) is an amendment to this ADR, not a config knob. A new content type flowing through the content machinery is the rule applying, not an extension.
- **Default-deny exclusions, nameable-complete.** No grant means unreadable. Excluded by default-deny, by name: the identity/auth/session tables (`admin_tokens` foremost, `users`, `sessions`, `workspaces`, since readable workspace rows would disclose tenant existence); the P-0015 policy-audit stores (`policy_write_audit`, `policy_decision_audit`); the retrieval-cluster operator-side instrumentation/audit tables (`egress_events`, whose `payload_audit` carries actual egressed content, plus `retrieval_runs`, `traversal_log`, `index_builds`); the content machinery's history/shadow tables; and the operator-internal substrate tables (`state_config`, `schema_migrations`). These are operator-side signals whose exposure through caller SQL would itself be an existence-disclosure or content-egress channel. The core-entity tables (`projects`, `agents`, and kin) are simply not granted at V0. They aren't sensitive-class; adding one is an amendment.
- **Function-EXECUTE surface, the two-part lock.** Postgres grants `EXECUTE` to `PUBLIC` by default, so a table-only allow-list is default-*allow* on the function axis. The callable surface is locked as a two-part structure. **(1) Non-`pg_catalog` functions are default-deny:** executable only via an explicit per-function grant enumerated in the provisioning DDL (EMPTY at V0; adding an entry is an amendment to this ADR); any in-schema `SECURITY DEFINER` function is categorically outside the permittable set. **(2) `pg_catalog` built-ins are callable except the enumerated revocation classes:** (a) GUC/session mutators, `set_config` and kin (D-KEY); (b) the server-info/statistics/file-access family, the `pg_stat_*` view+function family, `pg_ls_*`, `pg_read_file`-style file access, `pg_current_logfile`. A literal positive enumeration of `pg_catalog`'s thousands of built-ins is deliberately not the mechanism; it would be unauditable and version-drift-prone. The auditable invariant is *no revocation-class function and no non-`pg_catalog` function is executable*, reconciliation-asserted. Concretely permitted (the spec's constructible positive cases): `count`, `sum`, `avg`, `min`, `max`, `percentile_cont`, `percentile_disc`, `coalesce`, `date_trunc`, `now`, `pg_sleep` (bounded by `statement_timeout`).
- **PUBLIC-default revocation mechanics.** A plain `REVOKE … FROM <report-role>` doesn't remove a `PUBLIC` grant the role inherits, so the revocation SHALL operate on the `PUBLIC` grant itself for the report path: either `REVOKE EXECUTE ON FUNCTION … FROM PUBLIC` (making function EXECUTE opt-in) with explicit re-grants to the roles that need them, or a report-role that isn't a member of `PUBLIC`-inheriting groups for the relevant functions. The implementation picks within this shape; the invariant is that no `PUBLIC`-default `EXECUTE` reaches the report role for a revocation-class or non-`pg_catalog` function.
- **Reconciliation check (OC-10 / TS2), the full assertion matrix.** The grant sets and RLS coverage ship with a reconciliation check in the same change, demonstrably failing on a seeded violation. Per granted table it asserts: RLS enabled AND the spec R-0050 per-family predicates present (content-artifact tables: the workspace-RLS policy AND the P-0015 policy-envelope predicates; measurement tables: the workspace-RLS policy) AND the host primary path provably bypasses via ownership-without-`FORCE` AND the report role provably does not (D-RLS) AND relkind is constrained to base tables or `security_invoker` views. (A non-`security_invoker` view would evaluate base-table RLS as its owner, silently bypassing the report-role policies; a materialized view can't carry RLS and fails the RLS-enabled assertion fail-closed.) On the function axis: no revocation-class function executable, no non-`pg_catalog` function beyond the (empty) inventory, no `SECURITY DEFINER` function executable, and the executable surface is GUC-setter-free (D-KEY's inside-set assertion).
- **Provisioning, standing trigger, fail-closed, rollback + drift recovery (spec R-0044-e).** The role/grant/revocation/RLS DDL ships migration-borne through the forward-only migration runner, idempotent across fresh init and upgrade-in-place; later-registered content types get grants+policies at table-generation time through the content machinery. The reconciliation has a standing trigger, every migration-apply completion and artifact-table generation, not a one-time ship-time gate. Its failure semantics are **fail-closed at every firing point**: a reconciliation failure aborts the operation that fired it (a failing startup apply means the host refuses to start; a failing table generation means the table never becomes report-readable), never log-and-continue, and there's deliberately no skip-reconciliation switch (a bypass flag would be the fail-open backdoor). Recovery from a drift lockout is **diagnose-then-reapply** (the structured error names each violated assertion; the idempotent provisioning DDL restores missing state; surplus state is reversed by hand, `REVOKE` the extra grant, drop the seeded object), a runbook entry distinct from intentional teardown. The provisioning change includes a documented ordered rollback procedure (`DROP POLICY` then `REVOKE` then `DISABLE ROW LEVEL SECURITY` then `DROP ROLE`; `REVOKE` precedes `DISABLE` so the report role stays fail-closed until its access is gone, avoiding a transient grants-live-RLS-off cross-tenant window). The forward-only migration system doesn't auto-reverse security DDL, and an out-of-order teardown can strand RLS enabled without its host-bypass precondition or open that transient window.

*(Anchors: worked-reference R85.2 (allow-set); P-TrustworthySignal TS2; the discovery-stage security review's default-`PUBLIC` finding. Binding text: spec R-0044, R-0060-b, threat-tests T3/T8/T9/T11.)*

### D-RLS — The scoped RLS exception + the ownership-without-FORCE host-bypass precondition

RLS is **enabled on every table in the grant set, with policies scoped `TO` the report role**, keyed on session settings the host sets per execution from the caller's validated `WorkspaceCtx` (`SET LOCAL` of the workspace id and role, the [P-0009](P-0009-rls-admin-token.md) V0.1 keying shape, adopted early on this one path). Per family (spec R-0050): content-artifact tables (policy columns present) carry the workspace-RLS predicate AND the [P-0015](P-0015-provenance-envelope-source-roles.md) policy-envelope predicates; measurement/metric tables (no policy columns) carry the workspace-RLS predicate only.

**The host-bypass model is a correctness precondition, locked here, and the mechanism is ownership-without-`FORCE` ONLY.** `ALTER TABLE … ENABLE ROW LEVEL SECURITY`, required to attach the report-role policy, activates RLS on that table for **every** role, including the roles the retrieval path reads under. The host's primary role stays untouched only if it provably bypasses the newly-enabled RLS: **the primary role owns each granted table and no granted table carries `FORCE ROW LEVEL SECURITY`** (table owners bypass RLS unless `FORCE`d). This single mechanism keeps both the host-isolation claim and retrieval's own reads intact. **`BYPASSRLS` on the host/application role is NOT an available alternative** (r2 fold): the spec's R-0062-b carries "the application role holds no `BYPASSRLS`" as a P-0009/P-0010 RLS precondition, and a `BYPASSRLS` host role would bypass *every* policy when general RLS activates, defeating the convergence this decision guarantees and forcing a `BYPASSRLS`-removal at activation instead of P-0009's additive `CREATE POLICY` path. The two mechanisms aren't equivalent; offering both would hand the implementer a latent misconfiguration. The report role is a non-owner, non-`BYPASSRLS`, non-superuser role, so its policies bind. The OC-10 reconciliation (D-GRANT) asserts this per granted table: RLS enabled AND the host primary role owns without `FORCE` AND no application role holds `BYPASSRLS` AND the report role provably does not bypass.

**Convergence with the general R-0001 deferral.** The general V0 RLS deferral stands: application-layer `WorkspaceCtx` discipline remains the enforcement on every host-constructed query, and the R-0001 trip-wire is unchanged. The exception is scoped to exactly the path where the deferral's premise (host-built `WHERE` clauses) fails. When R-0001 fires and general RLS activates, the report-path policies **converge** with the general activation: D-KEY's mechanism pick preserves the P-0009 V0.1 session-setting keying verbatim, so the general activation adds host-role policies without reshaping the report-role policies. *(Anchors: 2a-3; P-0006/P-0009 (R-0001); P-0009 V0.1 policy shape; P-SecurityLayered. Binding text: spec R-0045, R-0050.)*

### D-KEY — Session-key tamper-resistance: revoke the GUC-setter surface (S-a)

**The locked property:** the RLS session keys MUST be unreachable from caller-authored SQL, proven by threat test, never assumed from transaction mode. A caller's single prepared SELECT, including a `set_config()` call in an expression or inside a `MATERIALIZED` CTE (common-table-expression, a `WITH` sub-query), MUST NOT re-key the report-path RLS.

**The mechanism (S-a):** revoke the GUC-setter surface, `set_config(text,text,boolean)` and its `pg_catalog` kin, from the report role via the D-GRANT function-EXECUTE allow-list, with the end-to-end argument that no permitted function can set a GUC directly or indirectly.

**Why S-a over S-b (the end-to-end argument).** The Frame fixed the candidate space at two. S-b (key on `current_user` via per-execution `SET ROLE`) doesn't escape the GUC channel: `role` is itself a settable GUC in Postgres, `set_config('role', …)` is reachable from a SELECT expression, and under a role the report role is a member of, a `SET ROLE` is caller-reversible. So S-b's `current_user` key is only tamper-resistant if the GUC-setter surface is *also* revoked. S-b presupposes S-a and then adds per-workspace role machinery (one Postgres role per workspace, or a role-switching layer) on top. S-a delivers the property with the smaller mechanism AND preserves the P-0009 V0.1 session-setting keying verbatim, which is D-RLS's convergence path; S-b would trade keying identity for a role-isolation boundary and owe a separate convergence argument. **The end-to-end argument, channel by channel (r2 fold, every GUC-writing path a caller could reach, and what closes it):** (1) **the function channel:** `set_config` and GUC-setting kin are revoked under the D-GRANT revocation class (a); (2) **the utility-statement channel:** `SET` / `SET LOCAL` / `RESET` are utility statements, structurally absent from the report path, since the executor prepares exactly **one** statement (spec R-0047-a) and the write-time gate admits only read statements (spec R-0042-c), so no utility statement is ever prepared; (3) **the catalog-write channel:** `UPDATE pg_settings` never reaches the report path. It's a non-`SELECT` statement, rejected at the write-time gate (spec R-0042-c admits only `SELECT`/`WITH…SELECT` bodies and rejects data-modifying CTEs), and its rule-rewritten form (`pg_settings` is an updatable view whose rule rewrites the UPDATE into `SELECT set_config(…)`) is closed by the channel-(1) `set_config` EXECUTE revocation. The D-RO write-blocks aren't the closers here (the rewrite leaves no DML node for the `READ ONLY` block to fire on, and the rewritten call needs `EXECUTE`, not table-write privilege); (4) **the indirect channel:** no *permitted* function can set a GUC on the caller's behalf, because the executable surface is reconciliation-asserted GUC-setter-free (no `SECURITY DEFINER` function, no non-`pg_catalog` function beyond the (empty) inventory, every revocation-class function non-executable, per spec R-0044-d, threat-test T9's inside-set sweep). The write discipline is that the host writes the settings once per execution (`SET LOCAL`, inside the transaction, before the user statement is prepared), and the caller has no channel through which to overwrite them. The threat test seeds a `set_config`/materialized-CTE re-key attempt that demonstrably fails (spec threat-test T1). *(Anchors: Frame D3 fixed two-candidate space; P-SecurityLayered; Simplicity; the folded review-H1 correction. Binding text: spec R-0046.)*

### D-COST — Query-cost containment

- **`SET LOCAL statement_timeout`** per execution (default 10 s, spec-pinned; config-tunable). An availability primitive [P-0007](P-0007-plugin-resource-limits.md)'s plugin CPU limits don't provide.
- **A host-enforced row cap** (default 10,000, spec-pinned) with **reported truncation**: the result envelope carries a truncation flag and the capped count, never a silent trim (P-TrustworthySignal).
- **A bounded, separate connection pool** (default max 4, spec-pinned; process-global at V0, where the noisy-neighbor trade-off is named plainly in the spec's calibrations, with per-workspace scoping deferred) so runaway report load can't starve the host's primary pool. **Saturation is caller-visible, never a hang** (r2 fold): acquisition uses a bounded acquire-timeout (default 2 s, spec-pinned); on expiry the caller gets a structured pool-saturated error, retry-safe. This is the fail-fast posture P-0007 takes against indistinguishable hangs on the plugin path, taken here too. On host shutdown, in-flight report queries are cancelled, safe by construction, since the transactions are read-only.
- **The host-enforced calibrations are config-backed and startup-validated** (spec R-0048-e): unset means the spec default; an invalid value fails startup naming the key. Hardcoded constants would silently void the config-tunable property the calibrations promise.
- **A cost-*estimate* ceiling (planner-cost rejection before execution) is deferred.** Decision content: whether to add an `EXPLAIN`-based pre-execution cost gate and its threshold. Deferral anchor: P-Defer (timeout + cap bound the damage; a cost model is mechanism ahead of evidence) with the `P-LockContract ⇄ P-PreserveDecisionSpace` discriminator. Firing instrument: the spec's telemetry reporting timeout-kill events at or above the spec-pinned rate, computed over the retained emitted stream (spec R-0059-b).

*(Anchors: P-0007 (the gap); P-TrustworthySignal; P-Defer. Binding text: spec R-0048.)*

### Consequences

**Good:**
- Read-only holds adversarially: two independent write-blocks, each alone sufficient; single-statement-by-construction; bind-never-interpolate.
- Workspace isolation is database-enforced on the one path where the P-0006 application layer can't reach, with the general R-0001 posture unchanged elsewhere and a clean convergence path.
- The session-key integrity property is proven, not assumed. The folded review-H1 factual error (that `READ ONLY` blocks `set_config`) is corrected by the revoke-the-setter mechanism.
- The grant sets and RLS coverage carry a reconciliation check that fails loud on drift (TS2). The silent-gap class (a granted table without a policy) fails the check, not the tenant.
- Cost containment closes the availability gap P-0007 structurally can't.

**Bad / Trade-offs:**
- The report path activates RLS early on one role, ahead of the general activation. That's added mechanism relative to a pure application-layer posture, warranted by the adversarial surface (the `Security ⇄ Simplicity` edge, resolved default-to-Security).
- The ownership-without-`FORCE` host-bypass model is a correctness precondition the reconciliation must assert per granted table. A misconfiguration here silently weakens host-path isolation, which is why it's locked scope (with `BYPASSRLS` explicitly excluded) and reconciliation-checked rather than left to review.
- The grant-surface enumerations that remain hand-maintained (the measurement-table list in the provisioning DDL, the exclusion list, the function revocation classes) are drift surfaces. The reconciliation check *with its standing migration-apply trigger* is the mitigation, the content-table side is derived by rule rather than listed, and extension-by-amendment (not config) keeps the rules auditable.
- System-catalog readability: `pg_catalog` metadata (schema shape, not row data) is readable by default; disclosure is bounded to the structure of a shared-schema instance. The spec carries the hardening-checklist artifact ([spec §pg_catalog hardening checklist](../../specs/2026-07-03-reporting-engine.md), authored at the r2 fold: the `pg_stats`/`pg_statistic`/`pg_stats_ext` sampled-value surfaces recorded closed via the built-in RLS guard, contingent on R-0045 enablement and pinned to the embedded Postgres 16.4; what can and can't be revoked without breaking query planning enumerated); accepted as Low with this ADR owning the disposition.

## Pros and Cons of the Options

### S-a — Revoke the GUC-setter surface (chosen)

- Pro: delivers the locked property with the smallest mechanism; the function-EXECUTE allow-list already gives it structural footing.
- Pro: preserves the P-0009 V0.1 session-setting keying verbatim, the D-RLS convergence path.
- Con: relies on the completeness of the GUC-setter revocation (every direct and indirect GUC-setting function); the end-to-end argument and the T9 function-allow-list threat test are the mitigation.

### S-b — Key RLS on `current_user` via per-execution `SET ROLE`

- Con: doesn't escape the GUC channel (`role` is a settable GUC; `SET ROLE` is caller-reversible under shared role membership), so it presupposes S-a's revocation anyway.
- Con: adds per-workspace role machinery (a role per workspace or a role-switching layer) on top of the revocation S-a already needs.
- Con: trades keying identity for a role-isolation boundary and owes a separate convergence argument against the general R-0001 activation.

## More Information

- Frame open ADR slot: `{{P-ReportExecutionContext}}` ([Frame](../intent/reporting-engine-frame.md) §Open ADR slots). This ADR resolves it.
- Binding requirement text: [reporting-engine spec](../../specs/2026-07-03-reporting-engine.md) R-0043–R-0046, R-0048, R-0050, R-0060; threat-test suite T1–T4, T8, T9, T11.
- Depends on / cites: [P-0006](P-0006-v0-tenant-enforcement.md) (the R-0001 deferral this scopes an exception to; the `WorkspaceCtx` mechanism built on); [P-0009](P-0009-rls-admin-token.md) (the V0.1 session-setting RLS policy shape adopted early; the role matrix); [P-0015](P-0015-provenance-envelope-source-roles.md) (the policy-envelope predicates the content-artifact tables carry, per D-RLS; the fragment module the spec's R-0049 consumes); [P-0007](P-0007-plugin-resource-limits.md) (the plugin-CPU limits that do NOT cover Postgres query cost, the D-COST gap); [P-0010](P-0010-storage-substrate-engine.md) (the Postgres substrate behind the `Storage` seam; telemetry emitted, not stored).
- Companion: [`{{P-ReportDefinitionType}}`](P-0021-report-definition-type.md) (P-0021), the definition content type whose bodies execute in this context. Its write-time validation is the first read-only layer (a non-read statement never registers); this context is the back-stop.
