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

`accepted`. This [ADR](../glossary.md#adr) (architecture decision record) was authored as `proposed` on 2026-10-19 at the #4018 gate (ruling 7793, where it was ADR 2 of the set). The maintainer accepted it the same day (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`. It coordinates with the #3591 batch. That batch owns three things this ADR depends on: the RLS policy template, the tenant key and the RLS manifest (rulings 7262, 7289).

## Context and Problem Statement

PostgreSQL row-level security (RLS) is the reference implementation for workspace-scoped isolation ([P-0010](P-0010-storage-substrate-engine.md) D1). But RLS has known bypasses. Any superuser skips it, any role with `BYPASSRLS` skips it, and so does any table owner whose table lacks `FORCE ROW LEVEL SECURITY`.

The two deployment models handle roles very differently. Under the embedded engine, the host created its own roles while connected as a bootstrap superuser. Under the operator-run instance, the host holds no `CREATEROLE` and inherits whatever roles the operator made.

The first draft of the amendment used a single login role that both owned the schema and served requests. The maintainer rejected it (activity 7771) and ruled for two roles, confirming the #3591 amendment content (activity 7289). Warden's security review then found an owner-escalation path on clusters that went through `pg_upgrade`. On clusters upgraded from PostgreSQL 14 or earlier (≤ 14), the `public` schema keeps `CREATE` for `PUBLIC`. So a function planted through SQL injection in a schema on the owner's `search_path` runs with owner authority at the next `mnemra init`.

## Decision Drivers

- **Least authority as standing capability.** The serving process must hold no schema-changing authority while it serves (`P-LeastAuthority`, the principle that a component holds only the minimum standing authority its role requires).
- **Verified, never assumed.** Every precondition RLS depends on is checked on every instance check, and the check fails closed (maintainer ruling 7261 item 1; `P-SecurityLayered`, the principle that security controls compose in independent layers and a control that errors denies rather than passes).
- **Opt-out polarity on the enumeration.** The RLS manifest is reconciled against the database catalog, so a tenant-scoped table missing from the manifest fails loud instead of going unchecked (`P-TrustworthySignal`, the principle that a verification signal must mean what it reports, and that a list gating what gets checked ships with a reconciliation against reality).
- **Simplicity.** Two roles, not four. The embedded design's four `NOLOGIN` roles were never connected as (MEASURED `libs/mnemra-host/schema/init.rs:589-595` at `57cfeca`).

## Considered Options

1. **Two roles (owner for init only, and serving for DML only, verified on every check), created by the operator from `--setup-sql` (chosen).**
2. **One login role that owns and serves, relying on `FORCE ROW LEVEL SECURITY`.**
3. **Four roles as R-0013-e originally wrote (host-fns, migration, backup, health-probe), created by the host.**
4. **The operator hand-creates roles with no printed SQL.**

## Decision Outcome

Chosen option: **1.**

The operator applies `mnemra check-db --setup-sql` once, using their own privileges. That SQL does the following:

- creates the owner role and the serving role (`LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEROLE NOCREATEDB NOREPLICATION`, `PASSWORD NULL`, with the passwords set client-side through `\password`);
- runs `REVOKE CREATE ON SCHEMA public FROM PUBLIC`;
- runs `CREATE SCHEMA mnemra AUTHORIZATION <owner>`;
- grants `CONNECT` and schema `USAGE` to both roles;
- grants DML, plus the owner's `ALTER DEFAULT PRIVILEGES`, to the serving role;
- grants `CREATE` to neither role.

`mnemra init` runs as the owner, with `search_path = pg_catalog, mnemra` and schema-qualified SQL, under an advisory lock. The serving host never holds the owner credential.

On every instance check, the host connects as the serving role and verifies each of these:

- the five attributes above;
- that the serving role owns no relation in the mnemra schema;
- that it's a transitive member of no other role, checked against an empty allowlist (the predefined `pg_*` roles and the owner role are included in what it must not belong to);
- that it has no `CREATE` on any non-system schema or on the database;
- that the mnemra schema is owned by the owner role;
- that every table in the #3591 RLS manifest has `relrowsecurity` and `relforcerowsecurity` set to true, that the found count equals the manifest count, and that every table with a tenant column is present in the manifest.

Any failure is a hard not-ready reason. *(Anchors: maintainer rulings 7771 and 7289 (two roles), 7261 (verified, fail closed), 7262 (RLS in V0); `P-LeastAuthority`; `P-SecurityLayered`; `P-TrustworthySignal` (reconciliation with opt-out polarity); [P-0010](P-0010-storage-substrate-engine.md) multi-tenancy preconditions 1 to 3.)*

**Implemented by** the following spec requirements ([R-codes](../glossary.md#r-codes), defined in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`): R-0117-c (attributes, ownership, membership, `CREATE`, schema owner), R-0117-e (forced RLS, reconciliation), R-0013-e as amended (the two roles), R-0013-a as amended (`init` as owner, `search_path`, advisory lock), R-0118-d (`--setup-sql`), and R-0119-g (owner credential held by `init` only).

### Consequences

**Good:** The serving process can't alter the schema, even if it's fully compromised at the SQL layer. RLS preconditions are facts the host observes, not assumptions. The owner's `search_path` can't be poisoned from `public`. And the first draft's residual risk, where the serving role could `NO FORCE` its own tables, goes away.

**Bad / Trade-offs:** The operator has two credentials and two passwords to set. `mnemra init` becomes a distinct step with its own credential. The check can't see a copy of the owner secret stored somewhere else; that's named as obligation R-0117-j item 5 in [P-0030](P-0030-delegated-policy-inventory-supply-chain.md). A compromised host **process** (not just its SQL) that can read the owner's credential file is outside this ADR's reach. [P-0029](P-0029-db-credential-files.md) covers that case by requiring the owner credential to be absent at serve time.

## Pros and Cons of the Options

### Option 1 — two roles, operator-created, host-verified (chosen)

- Pro: closes the owner bypass of RLS structurally, since the serving role owns nothing. It also closes the `CREATE` escalation.
- Pro: every precondition has a seeded failing test.
- Con: one more credential, and `init` can't share the serving connection.

### Option 2 — one role that owns and serves

- Pro: one credential, one setup step.
- Con: the serving process holds schema authority as standing capability, and protection against the RLS owner bypass rests on `FORCE` alone. The maintainer rejected this option at 7771.

### Option 3 — four host-created roles

- Pro: the finest-grained split.
- Con: it needs `CREATEROLE` or a bootstrap superuser, and the host no longer has either. The four roles were `NOLOGIN` and never connected as. Three of the four have no purpose under the operator model: backup belongs to the operator, and probes run as the serving role.

### Option 4 — operator hand-creates every role

- Pro: no printed SQL to maintain.
- Con: it reaches the same result as option 1 with more room for error, and nothing in CI exercises it. (The fresh-machine CI job runs the printed SQL verbatim.)

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE2, AE7 ruled). [P-0006](P-0006-v0-tenant-enforcement.md) and [P-0009](P-0009-rls-admin-token.md) own the host-side `WorkspaceCtx` enforcement, which is the layer this ADR sits beneath. [P-0020](P-0020-report-execution-context.md)'s report role is included in the setup SQL when the reporting engine is enabled.
- The #3591 batch defines the RLS manifest, the policy template and the tenant key. While the manifest is empty, R-0117-e reports `not_applicable`.
- Threat rows: in the architecture overview's retype subsection, see the serving role (E, elevation of privilege) and `DF-setup-sql` (T, tampering).
