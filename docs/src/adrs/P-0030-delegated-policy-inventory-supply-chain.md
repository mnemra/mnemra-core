---
title: "P-0030: Delegated-policy inventory and supply chain"
summary: "Every policy the host delegates to the operator's PostgreSQL instance is either a hard clause the host verifies on every check (server major and minor floor, pgvector floor from security history, durability settings on a writer-shaped session, UTF8, schema version, password_encryption = scram-sha-256, parameter logging off) or a named operator obligation the host emits and the deployment document carries verbatim (twelve items, NOT VERIFIED BY MNEMRA, including the lost host-side supply-chain verification). Control-class ADR 5 of the operator-Postgres amendment."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review)", "Bolt (operations review)", "Charon (canon review)", "Sage (the version floors, in progress)"]
informed: []
supersedes: null
superseded_by: null
---

# P-0030: Delegated-policy inventory and supply chain

## Status

`accepted` — authored `proposed` 2026-10-19 at the #4018 gate (ruling 7793, ADR 5). Accepted 2026-10-19 by the maintainer (#4018 activity 7800) with the operator-run Postgres amendment at commit `08f4b8e`. The two version floors were filled the same day from Sage's measurement (ruling 7793, AE6): **PostgreSQL 16.15** and **pgvector 0.8.7**; the evidence is cited on spec R-0117-a/-b and summarised below.

## Context and Problem Statement

The embedded engine wrote its own configuration and hash-pinned the binaries it ran (`KNOWN_GOOD_HASHES`, `libs/mnemra-host/storage/postgres/engine.rs:306` at `57cfeca`). Under the operator-run instance every one of those policies is the operator's, and the maintainer ruled (#4018 activity 7261 item 1; verbatim at 7260: "when we defer required policy to an external entity we need to do due diligence") that each is a requirement verified at startup and failing closed, with anything uncheckable named explicitly as the operator's. No canon principle reaches runtime verification of a policy a product delegates to an external party (`P-VerifyInheritedState` covers agent handoffs); the canon question is filed as #4050 and not invented here. Warden's review widened the inventory (password hashing, server-side parameter logging, `pg_hba`, exposure, certificate rotation) and asked that the lost supply-chain verification be labelled a loss, not a transfer.

## Decision Drivers

- **A delegated policy is verified or named, never assumed** (ruling 7261; `P-SecurityLayered` — a control that errors denies).
- **Honesty about what the host cannot see** — the obligations list is emitted by the binary and asserted equal to the document (V-Honesty; `P-GuaranteeByMechanism`).
- **Floors from security history, not feature history** — a version floor is a security requirement, so it is set from the advisories it closes (ruling 7793; V-Security).
- **Every clause hard at V0** (ruling 7761) — no capability-scoped degradation machinery until a clause needs it.

## Considered Options

1. **An explicit inventory: hard checked clauses plus an emitted, document-mirrored obligations list; floors from security history (chosen).**
2. **Check what is convenient; document the rest in prose.**
3. **Keep a host-side pin table for the operator's binaries.**
4. **Floors from feature history (the HNSW-introducing pgvector release; the pinned PG minor).**

## Decision Outcome

Chosen option: **1.** The host verifies, as the serving role on every check: the PostgreSQL major (`16`) and a minor floor; the pgvector extension present at or above a floor; `fsync`, `full_page_writes`, `synchronous_commit` ≠ `off`, `data_sync_retry = off` on a session shaped like a writer's; `server_encoding = UTF8`; the schema at the binary's version with no auto-migration at start; `password_encryption = scram-sha-256`; `log_parameter_max_length` and `_on_error` = `0`. Both floors are the latest release that closes the published advisories, labelled with what each closes, MEASURED by Sage 2026-10-19: **PostgreSQL 16.15** (`server_version_num ≥ 160015`; 55 CVEs affect 16.x, all fixed by 16.15 and 25 only there — representative CVSS 8.8 items CVE-2026-16239, CVE-2026-14669, CVE-2026-14664, CVE-2026-15741, CVE-2026-14670, and the RLS-relevant CVE-2026-14666; source `postgresql.org/support/security/`) and **pgvector 0.8.7** (set by CVE-2026-103484 / GHSA-pw7q-9788-x566, CNA CVSS 8.8, IVFFlat out-of-bounds write → arbitrary code execution, every earlier release affected; also closes CVE-2026-18022 and CVE-2026-3172; sources NVD, OSV, the GitHub global advisory database). The floors have a **re-check mechanism**, not a cadence: the weekly R-0120-c workflow fails when a newer 16.x minor (next scheduled 2026-11-12) or pgvector release exists above the recorded floor, or when the CI image falls below either floor. Measured for the record: the embedded design's pins (16.4.0; portalcorp 0.16.105 bundling pgvector 0.8.0) were below both floors, and its pgvector channel could not reach 0.8.7 at all. Twelve obligations the host cannot verify are held as one code constant, printed by `mnemra check-db --obligations` under "Not verified by mnemra", and asserted equal to the deployment document by a docs test: persistent storage for the data directory; backups and point-in-time recovery; the Postgres and pgvector install's supply chain — **a loss relative to the embedded pins, labelled as such** (accepted risk `AR-0011`); `max_connections` arithmetic including the pool-rebuild transient; the owner credential unreachable from the serving uid and the ident map; `pg_hba` never `trust`/`password`/`md5`; existing passwords SCRAM; `listen_addresses` and firewall exposure; TLS certificate expiry and CA rotation; minor-version patching; disk-full and WAL archiving; the server log treated as content-bearing unless `log_error_verbosity = terse` (constraint-violation `DETAIL` lines; INFERRED). *(Anchors: maintainer rulings 7261, 7761, 7771 (`AR-0011`–`AR-0013` signed off), 7793 (floors from security history); `P-SecurityLayered`; `P-GuaranteeByMechanism`; V-Honesty; V-Security; `P-Defer` — the unverifiable half is named, not dropped; canon gap #4050.)*

**Implemented by** spec R-0117-a/-b (floors with their advisory citations), R-0117-d, R-0117-g, R-0117-h, R-0117-k, R-0117-l (checked clauses), R-0117-j (the twelve obligations), R-0118-d (`--obligations`), R-0120-c (the CI image digest pin and its scheduled bump trigger — the test substrate's own supply chain) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`; register entries `AR-0011`–`AR-0013` in `docs/src/architecture/overview.md`.

### Consequences

**Good:** the operator knows exactly what the host checks and exactly what it does not; a drifted setting is a visible not-ready reason within one interval; floors move with advisories rather than with features.

**Bad / Trade-offs:** the host verifies nothing about the binaries it talks to — named as a loss and an accepted risk; twelve obligations are a real operator burden; a minor floor turns an unpatched instance into a not-ready host, which is the intended pressure but is operationally loud; `log_error_verbosity` is left to the operator because forcing `terse` would take away every operator's diagnostics.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: complete and honest; every item is either checked with a seeded failing case or printed and mirrored.
- Con: a long list to maintain; the docs test is what keeps it from drifting.

### Option 2 — check the convenient, prose for the rest

- Pro: less to build.
- Con: the prose half is a convention a future reader must recall; the omission that matters is the one nobody wrote down.

### Option 3 — a host-side pin table for the operator's binaries

- Pro: looks like the embedded posture.
- Con: the host does not install or run the binaries, so a pin would check a path it never executes — a false instrument (V-Observability).

### Option 4 — floors from feature history

- Pro: easy to state (`0.5.0` for HNSW; the embedded pin for PG).
- Con: a floor set by what happened to be installed or needed is a measurement, not a target (`P-ReliabilityTarget`); rejected by ruling 7793.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE2, DE9 "what exits", AE6 floors); [P-0027](P-0027-db-authorization-role-separation.md) owns the role-model clauses (R-0117-c/-e), which are delegated policies too but decided there.
- Accepted risks `AR-0011` (supply chain and storage), `AR-0012` (durability window), `AR-0013` (hostile operator / other principals), overview register.
- Packaging — who supplies Postgres, pgvector and the host binary — is #4019's, not this ADR's.
