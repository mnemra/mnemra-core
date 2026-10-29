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

## Amendment 2026-10-29 — the floors: a policy in the design, values in host code (#4291)

**Status: the direction is the maintainer's; the wording is the #4291 draft's until his exit approval of the #4291 amendment to spec R-0117 and R-0120 (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`).** In-place amendment per the [ADR README](README.md); nothing above is rewritten, and each superseded phrase is quoted here with what replaces it. Maintainer rulings: 2026-10-25 (#4291, by AskUserQuestion), "Floor in spec, set in code"; and 2026-10-29 (#4291 activity 9171, Puck's record of his selections, the option wording Puck's), "Drift job, with lead time" for the end of life of a supported major, "Same shape as Postgres" for pgvector, and "At most two, test-enforced" for the number of supported majors; and 2026-10-29 (#4291 activity 9325), "Split: approve policy now, drift later", which defers the floor-drift workflow to task #4444. The spec states each interface minimum (PostgreSQL 16 or later; pgvector with HNSW) and the floor policy. The supported majors, their floors and the pgvector floor are one host code constant, so adding a major, or raising a floor, is a code change gated by tests, never a spec edit, and the host still refuses a major nobody has tested. The requirement text is spec R-0117-a, R-0117-b and R-0120-c as amended 2026-10-29. [P-0031](P-0031-test-substrate-isolation.md) carries the matching amendment for the CI images. ⚠ [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19, AE6, still reads `16.15` and `0.8.7` as the floors ruled, and the P-0030 entry of its follow-ups table reads "floors 16.15 / 0.8.7 from AE6"; it is a lagging copy, reconciled by whichever of #4291 and #4412 lands second; recorded as an acceptance criterion on #4412 (activity 9313) and in the exit list of the #4291 amendment to the V0 spec.

**Superseded in this record, with what replaces it:**

| Superseded text | Replaced by | Anchor |
|---|---|---|
| Decision Outcome: "the PostgreSQL major (`16`) and a minor floor" | A major in the host's supported-major table (PostgreSQL 16 or later, at most two majors) and that major's minor floor. A major enters the table only in a change whose required check runs the whole verify chain and the fresh-machine job on it, and stays only while they keep running on it (spec R-0117-a (1), (4) and (5) as amended) | rulings 2026-10-25 and 2026-10-29 (#4291); `P-LockContract` — the contract locks, the values vary behind it; `P-CheckCoversTheClaim` — "supported" is checked on the major it names |
| Decision Outcome: "Both floors are the latest release that closes the published advisories, labelled with what each closes, MEASURED by Sage 2026-10-19: **PostgreSQL 16.15** (…) and **pgvector 0.8.7** (…)", read as the floors this record decides | The policy stands, and it is what this record decides: each supported major's floor is the latest minor of that major that the PostgreSQL security page lists as fixing a published advisory, or that major's `.0` when it lists none; the pgvector floor is the latest pgvector release that fixes a published advisory, one value for every major. The values are the host code constant's, set by the policy on the day they land. `16.15` and `0.8.7`, with their advisory citations, stay as MEASURED 2026-10-19 provenance, not as the constant's values (spec R-0117-a (3), R-0117-b (2) and (3) as amended) | ruling 7793 (floors from security history), carried; rulings 2026-10-25 and 2026-10-29 (#4291); #4249 activity 8380 (provenance keeps its version) |
| Decision Outcome: "the weekly R-0120-c workflow fails when a newer 16.x minor (next scheduled 2026-11-12) or pgvector release exists above the recorded floor, or when the CI image falls below either floor" | Deferred to task #4444 (maintainer ruling 2026-10-29, #4291 activity 9325, "Split: approve policy now, drift later"). The weekly workflow is redesigned there, per key and against the policies above, bound by the end-of-life ruling (#4291 activity 9171, item 1: warn about 180 days and fail about 90 days before the upstream end-of-life date) and the waiver ruling (activity 9257). It gates #4161's plan task T16 only, and no drift workflow is built before it lands. The response to a lagging floor stays a change to the constant, never a spec edit; until #4444 lands, a floor is checked only by review of the change that sets it (spec R-0117-a (3), R-0117-b (3) and R-0120-c as amended) | ruling 9325; `P-Defer` DF1 — #4444 is a prerequisite of T16, recorded on #4444 |
| Status: "The two version floors were filled the same day from Sage's measurement (ruling 7793, AE6): **PostgreSQL 16.15** and **pgvector 0.8.7**" | Kept unchanged as the record of 2026-10-19. Read it with the row above: those are MEASURED provenance, not values this record locks | #4249 activity 8380 |

*Rejected, kept with reasons:* the values written into the spec (the 2026-10-19 text) — every security minor and every added major would reopen a locked spec, which the 2026-10-25 ruling refuses; the pgvector floor kept as a minimum value in the spec (#4249 activity 8541, a lead decision) — superseded by the 2026-10-29 ruling, because the same problem takes the same shape (V-Consistency); "PostgreSQL 16 or later" with no closed set of majors — the host would admit a major nobody has tested, which the 2026-10-25 ruling refuses; running only the smoke and fresh-machine jobs on majors beyond the first — a major whose Postgres test cells never ran is not tested (V-Quality); for end of life, the host refusing a past-end-of-life major outright, and parking it with a human cadence — the options not selected on 2026-10-29, the first because it strands operators with no lead time, the second because the failure it parks is silent.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE2, DE9 "what exits", AE6 floors); [P-0027](P-0027-db-authorization-role-separation.md) owns the role-model clauses (R-0117-c/-e), which are delegated policies too but decided there.
- Accepted risks `AR-0011` (supply chain and storage), `AR-0012` (durability window), `AR-0013` (hostile operator / other principals), overview register.
- Packaging — who supplies Postgres, pgvector and the host binary — is #4019's, not this ADR's.
