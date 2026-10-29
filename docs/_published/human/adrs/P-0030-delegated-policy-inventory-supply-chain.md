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

`accepted`. This [P-* ADR](../glossary.md#p--adr) (a project-scoped architecture decision) was authored as `proposed` on 2026-10-19 at the #4018 gate, under ruling 7793 as ADR 5 of that set. The maintainer accepted it on 2026-10-19 (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`. The two version floors were filled in the same day from Sage's measurement (ruling 7793, AE6): **PostgreSQL 16.15** and **pgvector 0.8.7**. The evidence for both is cited on spec requirements R-0117-a/-b ([R-codes](../glossary.md#r-codes) are numbered requirements defined in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`) and summarised below.

## Context and Problem Statement

The embedded engine wrote its own configuration and hash-pinned the binaries it ran (`KNOWN_GOOD_HASHES`, `libs/mnemra-host/storage/postgres/engine.rs:306` at `57cfeca`). Once the database becomes an operator-run instance, every one of those policies belongs to the operator instead. The maintainer ruled on how to handle that (#4018 activity 7261 item 1; verbatim at 7260: "when we defer required policy to an external entity we need to do due diligence"). Each delegated policy is a requirement verified at startup that fails closed, and anything the host can't check is named explicitly as the operator's responsibility.

No canon principle covers runtime verification of a policy that a product delegates to an external party. The nearest one, `P-VerifyInheritedState`, covers handoffs between agents, not this. So the canon question is filed as #4050 rather than invented here. Warden's security review widened the inventory to include password hashing, server-side parameter logging, `pg_hba`, network exposure and certificate rotation. It also asked that the lost supply-chain verification be labelled a loss, not a transfer.

## Decision Drivers

- **A delegated policy is verified or named, never assumed** (ruling 7261; `P-SecurityLayered`, the principle that security controls compose in independent layers and that a control which errors must deny).
- **Honesty about what the host can't see.** The binary emits the obligations list, and a test asserts it's equal to the document (V-Honesty, the value of separating fact from inference and naming what isn't known; `P-GuaranteeByMechanism`, the principle that a guarantee is enforced by something that fails loudly rather than by a convention someone has to remember).
- **Floors come from security history, not feature history.** A version floor is a security requirement, so it's set from the advisories it closes (ruling 7793; V-Security).
- **Every clause is hard at V0** (ruling 7761). There's no capability-scoped degradation machinery until a clause actually needs it.

## Considered Options

1. **An explicit inventory: hard checked clauses plus an emitted, document-mirrored obligations list; floors from security history (chosen).**
2. **Check what is convenient; document the rest in prose.**
3. **Keep a host-side pin table for the operator's binaries.**
4. **Floors from feature history (the HNSW-introducing pgvector release; the pinned PG minor).**

## Decision Outcome

Chosen option: **1.**

The host runs a set of checks as the serving role, on every check interval. It verifies:

- the PostgreSQL major version (`16`) and a minor-version floor;
- the pgvector extension is present at or above a floor;
- `fsync`, `full_page_writes` and `synchronous_commit` are ≠ `off`, and `data_sync_retry = off`, on a session shaped like a writer's;
- `server_encoding = UTF8`;
- the schema is at the binary's version, with no auto-migration at start;
- `password_encryption = scram-sha-256`;
- `log_parameter_max_length` and `_on_error` are `0`.

Both floors are the latest release that closes the published advisories, and each is labelled with what it closes. Sage MEASURED them on 2026-10-19.

**PostgreSQL 16.15** (`server_version_num ≥ 160015`). 55 CVEs affect 16.x. All are fixed by 16.15, and 25 are fixed only there. Representative CVSS 8.8 items are CVE-2026-16239, CVE-2026-14669, CVE-2026-14664, CVE-2026-15741 and CVE-2026-14670, plus the RLS-relevant CVE-2026-14666. Source: `postgresql.org/support/security/`.

**pgvector 0.8.7**. This floor is set by CVE-2026-103484 / GHSA-pw7q-9788-x566 (CNA CVSS 8.8), an IVFFlat out-of-bounds write that leads to arbitrary code execution. Every earlier release is affected. It also closes CVE-2026-18022 and CVE-2026-3172. Sources: NVD, OSV, and the GitHub global advisory database.

The floors are kept current by a **re-check mechanism**, not a review cadence. The weekly R-0120-c workflow fails when a newer 16.x minor (next scheduled 2026-11-12) or a newer pgvector release exists above the recorded floor, or when the CI image falls below either floor. Measured for the record: the embedded design's pins (16.4.0; portalcorp 0.16.105, which bundles pgvector 0.8.0) were below both floors. And its pgvector channel couldn't reach 0.8.7 at all.

Then there are the twelve obligations the host can't verify. They're held as one code constant, printed by `mnemra check-db --obligations` under "Not verified by mnemra", and a docs test asserts they're equal to the deployment document. The twelve are:

- persistent storage for the data directory;
- backups and point-in-time recovery;
- the supply chain of the Postgres and pgvector install. This is **a loss relative to the embedded pins, labelled as such** (accepted risk `AR-0011`);
- `max_connections` arithmetic, including the pool-rebuild transient;
- the owner credential being unreachable from the serving uid, and the ident map;
- `pg_hba` never using `trust`/`password`/`md5`;
- existing passwords being SCRAM;
- `listen_addresses` and firewall exposure;
- TLS certificate expiry and CA rotation;
- minor-version patching;
- disk-full handling and WAL archiving;
- the server log treated as content-bearing unless `log_error_verbosity = terse`, because of constraint-violation `DETAIL` lines (INFERRED).

*(Anchors: maintainer rulings 7261, 7761, 7771 (`AR-0011` through `AR-0013` signed off) and 7793 (floors from security history); `P-SecurityLayered`; `P-GuaranteeByMechanism`; V-Honesty; V-Security; [`P-Defer`](../glossary.md#p-defer) (defer mechanism choice until evidence forces it), here meaning the unverifiable half is named, not dropped; canon gap #4050.)*

**Implemented by** these requirements in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`: R-0117-a/-b (the floors with their advisory citations); R-0117-d, R-0117-g, R-0117-h, R-0117-k and R-0117-l (the checked clauses); R-0117-j (the twelve obligations); R-0118-d (`--obligations`); and R-0120-c (the CI image digest pin and its scheduled bump trigger, which is the test substrate's own supply chain). The accepted-risk register entries `AR-0011` through `AR-0013` are in `docs/src/architecture/overview.md`.

### Consequences

**Good:** the operator knows exactly what the host checks and exactly what it doesn't. A drifted setting shows up as a visible not-ready reason within one check interval. Floors move with advisories rather than with features.

**Bad / Trade-offs:** the host verifies nothing about the binaries it talks to. That's named as a loss and an accepted risk. Twelve obligations are a real burden on the operator. A minor-version floor turns an unpatched instance into a not-ready host; that's the intended pressure, but it's operationally loud. And `log_error_verbosity` is left to the operator, because forcing `terse` would take away every operator's diagnostics.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: complete and honest. Every item is either checked, with a seeded failing case, or printed and mirrored in the document.
- Con: a long list to maintain. The docs test is what keeps it from drifting.

### Option 2 — check the convenient, prose for the rest

- Pro: less to build.
- Con: the prose half is a convention a future reader must remember. The omission that matters is the one nobody wrote down.

### Option 3 — a host-side pin table for the operator's binaries

- Pro: looks like the embedded posture.
- Con: the host doesn't install or run the binaries, so a pin would check a path it never executes. That makes it a false instrument (V-Observability, the value that an instrument reporting success without verifying is worse than none).

### Option 4 — floors from feature history

- Pro: easy to state (`0.5.0` for HNSW; the embedded pin for PG).
- Con: a floor set by whatever happened to be installed or needed is a measurement, not a target (`P-ReliabilityTarget`). Rejected by ruling 7793.

## Amendment 2026-10-29 — the floors: a policy in the design, values in host code (#4291)

**Status: the direction is the maintainer's; the wording is the #4291 draft's until his exit approval of the #4291 amendment to spec R-0117 and R-0120 (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`).** This is an in-place amendment, as the [ADR README](README.md) prescribes. Nothing above is rewritten. Each superseded phrase is quoted here, with what replaces it.

The maintainer's rulings behind it:

- 2026-10-25 (#4291, by AskUserQuestion): "Floor in spec, set in code".
- 2026-10-29 (#4291 activity 9171): "Drift job, with lead time" for the end of life of a supported major, "Same shape as Postgres" for pgvector, and "At most two, test-enforced" for the number of supported majors. Puck (the orchestrating agent) recorded his selections and wrote the option wording.
- 2026-10-29 (#4291 activity 9325): "Split: approve policy now, drift later". It defers the floor-drift workflow to task #4444.

The spec states each interface minimum: PostgreSQL 16 or later, and pgvector with HNSW indexes. It also states the floor policy. The values live elsewhere. The supported majors, their floors and the pgvector floor are one constant in the host's code, so adding a major or raising a floor is a code change gated by tests, never a spec edit. And the host still refuses a major nobody has tested. The requirement text is spec R-0117-a, R-0117-b and R-0120-c as amended 2026-10-29. [P-0031](P-0031-test-substrate-isolation.md) carries the matching amendment for the CI images.

⚠ [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19, AE6, still reads `16.15` and `0.8.7` as the floors ruled, and the P-0030 entry of its follow-ups table reads "floors 16.15 / 0.8.7 from AE6". It's a lagging copy. Whichever of #4291 and #4412 lands second reconciles it; that obligation is recorded as an acceptance criterion on #4412 (activity 9313) and in the exit list of the #4291 amendment to the V0 spec.

**Superseded in this record, with what replaces it:**

| Superseded text | Replaced by | Anchor |
|---|---|---|
| Decision Outcome: "the PostgreSQL major (`16`) and a minor floor" | A major in the host's supported-major table (PostgreSQL 16 or later, at most two majors) and that major's minor floor. A major enters the table only in a change whose required check runs the whole verify chain and the fresh-machine job on it, and it stays only while they keep running on it (spec R-0117-a (1), (4) and (5) as amended) | rulings 2026-10-25 and 2026-10-29 (#4291); [P-LockContract](../glossary.md#p-lockcontract): the contract locks, and the values vary behind it; `P-CheckCoversTheClaim` (a conclusion is no wider than what the check ranged over): "supported" is checked on the major it names |
| Decision Outcome: "Both floors are the latest release that closes the published advisories, labelled with what each closes, MEASURED by Sage 2026-10-19: **PostgreSQL 16.15** (…) and **pgvector 0.8.7** (…)", read as the floors this record decides | The policy stands, and the policy is what this record decides. Each supported major's floor is the latest minor of that major that the PostgreSQL security page lists as fixing a published advisory, or that major's `.0` when it lists none. The pgvector floor is the latest pgvector release that fixes a published advisory, and it's one value for every major. The values are the host code constant's, set by the policy on the day they land. `16.15` and `0.8.7`, with their advisory citations, stay as MEASURED 2026-10-19 provenance, not as the constant's values (spec R-0117-a (3), R-0117-b (2) and (3) as amended) | ruling 7793 (floors from security history), carried; rulings 2026-10-25 and 2026-10-29 (#4291); #4249 activity 8380 (provenance keeps its version) |
| Decision Outcome: "the weekly R-0120-c workflow fails when a newer 16.x minor (next scheduled 2026-11-12) or pgvector release exists above the recorded floor, or when the CI image falls below either floor" | Deferred to task #4444 (maintainer ruling 2026-10-29, #4291 activity 9325, "Split: approve policy now, drift later"). The weekly workflow is redesigned there, per key and against the policies above. Two rulings bind that redesign: the end-of-life ruling (#4291 activity 9171, item 1: warn about 180 days and fail about 90 days before the upstream end-of-life date) and the waiver ruling (activity 9257). It gates only #4161's plan task T16, and no drift workflow is built before it lands. The response to a lagging floor is still a change to the constant, never a spec edit. Until #4444 lands, a floor is checked only by review of the change that sets it (spec R-0117-a (3), R-0117-b (3) and R-0120-c as amended) | ruling 9325; [P-Defer](../glossary.md#p-defer) DF1 (a deferral names what mechanically fires it): #4444 is a prerequisite of T16, recorded on #4444 |
| Status: "The two version floors were filled the same day from Sage's measurement (ruling 7793, AE6): **PostgreSQL 16.15** and **pgvector 0.8.7**" | Kept unchanged as the record of 2026-10-19. Read it with the row above: those values are MEASURED provenance, not values this record locks | #4249 activity 8380 |

*Rejected, kept with reasons:*

- The values written into the spec (the 2026-10-19 text). Every security minor and every added major would reopen a locked spec, and the 2026-10-25 ruling refuses that.
- The pgvector floor kept as a minimum value in the spec (#4249 activity 8541, a lead decision). The 2026-10-29 ruling superseded it, because the same problem takes the same shape (V-Consistency).
- "PostgreSQL 16 or later" with no closed set of majors. The host would admit a major nobody has tested, which the 2026-10-25 ruling refuses.
- Running only the smoke and fresh-machine jobs on majors beyond the first. A major whose Postgres test cells never ran isn't tested (V-Quality).
- For end of life, the host refusing a past-end-of-life major outright, and parking the question with a human cadence. Neither was selected on 2026-10-29. The first strands operators with no lead time. The second parks a failure that is silent.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE2, DE9 "what exits", AE6 floors). [P-0027](P-0027-db-authorization-role-separation.md) owns the role-model clauses (R-0117-c/-e). Those are delegated policies too, but they're decided there.
- Accepted risks `AR-0011` (supply chain and storage), `AR-0012` (durability window) and `AR-0013` (hostile operator / other principals), in the overview's risk register.
- Packaging (who supplies Postgres, pgvector and the host binary) belongs to #4019, not this ADR.
