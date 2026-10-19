---
title: "P-0027: DB authorization and role separation"
summary: "Two database roles created by the operator from host-printed SQL: an owner role that owns the schema and is used only by `mnemra init` under its own credential, and a serving role with DML grants only — owning nothing, member of nothing, no CREATE anywhere, none of the five dangerous attributes — verified by the host on every instance check, with forced RLS reconciled against the catalog. Control-class ADR 2 of the operator-Postgres amendment; coordinated with the #3591 tenancy/RLS batch."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review, rounds 1–4)", "Charon (canon review)"]
informed: ["the #3591 batched amendment (tenancy and RLS)"]
supersedes: null
superseded_by: null
---

# P-0027: DB authorization and role separation

## Status

`accepted` — authored `proposed` 2026-10-19 at the #4018 gate (ruling 7793, ADR 2). Accepted 2026-10-19 by the maintainer (#4018 activity 7800) with the operator-run Postgres amendment at commit `08f4b8e`. Coordinates with the #3591 batch, which owns the RLS policy template, the tenant key and the RLS manifest (rulings 7262, 7289).

## Context and Problem Statement

PostgreSQL row-level security is the reference implementation for workspace-scoped isolation ([P-0010](P-0010-storage-substrate-engine.md) D1), and it is bypassed by any superuser, any `BYPASSRLS` role, and any table owner whose table lacks `FORCE ROW LEVEL SECURITY`. Under the embedded engine the host created its own roles as a bootstrap superuser; under the operator-run instance the host holds no `CREATEROLE` and inherits whatever roles the operator made. The first draft of the amendment used one login role that both owned the schema and served; the maintainer rejected it (activity 7771) and ruled two roles, confirming the #3591 amendment content (activity 7289). Warden's review then found the owner-escalation path on `pg_upgrade`d clusters: `public` keeps `CREATE` for `PUBLIC` on clusters upgraded from ≤ 14, so a function planted through SQL injection in a schema on the owner's `search_path` runs with owner authority at the next `mnemra init`.

## Decision Drivers

- **Least authority as standing capability** — the serving process must hold no schema-changing authority while it serves (`P-LeastAuthority`).
- **Verified, never assumed** — every precondition RLS depends on is checked on every instance check, failing closed (maintainer ruling 7261 item 1; `P-SecurityLayered`).
- **Opt-out polarity on the enumeration** — the RLS manifest is reconciled against the catalog, so a tenant-scoped table absent from the manifest fails loud (`P-TrustworthySignal`).
- **Simplicity** — two roles, not four; the embedded design's four `NOLOGIN` roles were never connected as (MEASURED `libs/mnemra-host/schema/init.rs:589-595` at `57cfeca`).

## Considered Options

1. **Two roles — owner (init only) and serving (DML only, verified every check) — created by the operator from `--setup-sql` (chosen).**
2. **One login role that owns and serves, relying on `FORCE ROW LEVEL SECURITY`.**
3. **Four roles as R-0013-e originally wrote (host-fns, migration, backup, health-probe), created by the host.**
4. **The operator hand-creates roles with no printed SQL.**

## Decision Outcome

Chosen option: **1.** The operator applies `mnemra check-db --setup-sql` once with their own privileges: it creates the owner role and the serving role (`LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEROLE NOCREATEDB NOREPLICATION`, `PASSWORD NULL` with passwords set client-side through `\password`), `REVOKE CREATE ON SCHEMA public FROM PUBLIC`, `CREATE SCHEMA mnemra AUTHORIZATION <owner>`, `CONNECT` and schema `USAGE` to both, DML plus the owner's `ALTER DEFAULT PRIVILEGES` to the serving role, and `CREATE` to neither. `mnemra init` runs as the owner with `search_path = pg_catalog, mnemra` and schema-qualified SQL, under an advisory lock; the serving host never holds the owner credential. On every instance check the host verifies, as the serving role: the five attributes; ownership of no relation in the mnemra schema; transitive membership of no other role against an empty allowlist (predefined `pg_*` roles and the owner role included); no `CREATE` on any non-system schema or on the database; the mnemra schema owned by the owner role; and, for every table in the #3591 RLS manifest, `relrowsecurity` and `relforcerowsecurity` true with found-count equal to the manifest and every tenant-column table present in it. Any failure is a hard not-ready reason. *(Anchors: maintainer rulings 7771 and 7289 (two roles), 7261 (verified, fail closed), 7262 (RLS in V0); `P-LeastAuthority`; `P-SecurityLayered`; `P-TrustworthySignal` — reconciliation with opt-out polarity; [P-0010](P-0010-storage-substrate-engine.md) multi-tenancy preconditions 1–3.)*

**Implemented by** spec R-0117-c (attributes, ownership, membership, `CREATE`, schema owner), R-0117-e (forced RLS, reconciliation), R-0013-e as amended (the two roles), R-0013-a as amended (`init` as owner, `search_path`, advisory lock), R-0118-d (`--setup-sql`), R-0119-g (owner credential held by `init` only) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`.

### Consequences

**Good:** the serving process cannot alter the schema even if fully compromised at the SQL layer; RLS preconditions are facts the host observes, not assumptions; the owner's `search_path` cannot be poisoned from `public`; the first draft's "the serving role can `NO FORCE` its own tables" residual disappears.

**Bad / Trade-offs:** two credentials and two passwords for the operator to set; `mnemra init` is a distinct step with its own credential; the check cannot see a copy of the owner secret stored elsewhere (named as obligation R-0117-j item 5 in [P-0030](P-0030-delegated-policy-inventory-supply-chain.md)); a compromised host **process** (not just its SQL) that can read the owner's file is out of this ADR's reach — [P-0029](P-0029-db-credential-files.md) requires the owner credential to be absent at serve time.

## Pros and Cons of the Options

### Option 1 — two roles, operator-created, host-verified (chosen)

- Pro: closes owner-bypass of RLS structurally (the serving role owns nothing) and closes the `CREATE` escalation.
- Pro: every precondition has a seeded failing test.
- Con: one more credential; `init` cannot share the serving connection.

### Option 2 — one role that owns and serves

- Pro: one credential, one setup step.
- Con: the serving process holds schema authority as standing capability; RLS bypass rests on `FORCE` alone; rejected by the maintainer at 7771.

### Option 3 — four host-created roles

- Pro: finest-grained split.
- Con: needs `CREATEROLE` or a bootstrap superuser the host no longer has; the four roles were `NOLOGIN` and never connected as; three of the four have no object under the operator model (backup is the operator's; probes run as the serving role).

### Option 4 — operator hand-creates every role

- Pro: no printed SQL to maintain.
- Con: the same result as option 1 with more error surface and nothing CI exercises (the fresh-machine job runs the printed SQL verbatim).

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE2, AE7 ruled); [P-0006](P-0006-v0-tenant-enforcement.md) and [P-0009](P-0009-rls-admin-token.md) own the host-side `WorkspaceCtx` enforcement this sits beneath; [P-0020](P-0020-report-execution-context.md)'s report role is included in the setup SQL when the reporting engine is enabled.
- The #3591 batch defines the RLS manifest, the policy template and the tenant key; R-0117-e reports `not_applicable` while the manifest is empty.
- Threat rows: overview retype subsection — the serving role (E), `DF-setup-sql` (T).
