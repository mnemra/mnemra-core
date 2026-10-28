---
title: "P-0031: Test-substrate isolation"
summary: "Tests get their Postgres from an operator- or CI-provided server through the existing fixture entry point and never start one: two required URLs (fail, never skip) (Amendment 2026-10-22, #4119: one required harness URL, parsed by the host's own parser, with PG* variables refused; the superuser credential a password-less URL plus a 0600 file from a named location, in the gated cell's environment only, the cell required locally and reachable from every chain `just ci` runs, locally or in CI; the harness role checked against an allowlist; a name-allowlist store guard with no cache; one run owner tearing down on every exit path; SCRAM verifiers for run-scoped passwords; a CI container started by job steps with per-run TLS; the fresh-machine job as its own job), three roles per run (harness, run-scoped owner, run-scoped serving), per-test databases from a template under a gate-level run id that fits the 63-byte identifier limit, the superuser URL confined to one gated binary, a fail-closed refusal of any server holding a production store, an orphan sweep keyed on dead pids and a 24-hour age bound, a digest-pinned CI service with a scheduled bump trigger, a fail-not-skip proof step, and a fresh-machine job inside the required check. Control-class ADR 6 of the operator-Postgres amendment."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review)", "Bolt (operations review, rounds 1–4)", "Charon (canon review)"]
informed: ["the CI tier-2 spec amendment, #4119"]
supersedes: null
superseded_by: null
---

# P-0031: Test-substrate isolation

## Status

`accepted`. This [P-* ADR](../glossary.md#p--adr) (an architecture decision scoped to this project) was authored as `proposed` on 2026-10-19 at the #4018 gate (ruling 7793, ADR 6). The maintainer accepted it the same day (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`. The CI tier-2 spec (`docs/specs/2026-07-02-ci-flake-tier2-shared-engine.md`) still names the embedded engine. Its amendment is #4119, and this ADR states what carries over into it.

## Context and Problem Statement

With no embedded engine, every Postgres-backed test needs a server it didn't start. The repository has already been burned twice: once by a skip-with-no-assertion trap, where a test passed without checking anything, and once by a cross-lane reap, where one session's cleanup killed another session's postmaster (both recorded in `CLAUDE.md`). So the substrate must fail loudly when it's absent. And it must never touch another run's objects.

Several more constraints apply. Two lanes (parallel working sessions) share one machine and may share one server. CI runs a service container. The host-under-test must satisfy R-0117-c ([R-codes](../glossary.md#r-codes) are requirement identifiers defined in the project's specs), which the harness role can't, because the harness role needs `CREATEDB` and `CREATEROLE`. A superuser connection is needed for a handful of negative tests and nowhere else. PostgreSQL truncates identifiers at 63 bytes, silently. And the production-store guard must not fail open on a database the harness can't connect to.

## Decision Drivers

- **No skip, no environment-dependent red** (`P-TrustworthySignal`, the principle that a verification signal must mean what it reports; R-0018-b requires a real Postgres, never a mock).
- **The server is an injected input** (`P-TestableByDesign`, design components so tests can supply their dependencies).
- **Least authority**: the superuser is confined to one binary, and the host-under-test never runs as the harness role (`P-LeastAuthority`, hold only the authority a role needs).
- **Fail closed on the production-store guard** (`P-SecurityLayered`, security controls compose in independent layers and deny when they error).
- **One mechanism for local, CI and smoke** (V-Consistency, the value that the same problem gets the same shape everywhere). The supply chain of the CI image is pinned by digest, with a bump trigger that actually fires (`P-SecurityLayered`). Dependabot is disabled here, so nothing else would flag a stale image.

## Considered Options

1. **Operator/CI-provided server, two required URLs, three roles per run, template databases, gate-level run id, dead-pid and age sweeps, confined superuser, fail-closed guard, digest-pinned service with a bump trigger (chosen).**
2. **A fixture-started disposable cluster from the operator's `initdb`/`pg_ctl`.**
3. **Skip Postgres tests when no server is configured.**
4. **One role for harness and host-under-test.**

## Decision Outcome

Chosen option: **1.** The fixture reads `MNEMRA_TEST_DATABASE_URL` (harness role: `LOGIN CREATEDB CREATEROLE`, not superuser) and `MNEMRA_TEST_SUPERUSER_URL` (read by one gated binary only, enforced by a source check); either absent or unreachable fails the first acquiring test by name. Per run, it creates a run-scoped owner role and a run-scoped serving role, so the smoke host satisfies R-0117-c. Per test, it creates a database cloned from `mnemra_test_template`, which the setup pre-installs pgvector into.

`<run>` is the gate invocation. `just ci` exports `MNEMRA_TEST_RUN_ID` once; otherwise a standalone binary generates it. Its form is `<hosthash>_<pid>_<epoch>`, where the hash is computed over hostname, boot id and container id. Every name is built to fit 63 bytes and is parsed from the right.

Before provisioning, the fixture refuses a server holding any non-test database that carries the migrations table or that it cannot connect to, caching the scan per run. The sweep removes the run's own objects, same-hash objects whose pid is dead or whose epoch mismatches, and any `mnemra_test_*` object older than 24 h. It never touches another hash's live run (its limit under pid namespaces is stated).

CI provides a digest-pinned pgvector/PostgreSQL 16 service with a `pg_isready` wait. A scheduled workflow fails when the tag's digest moves. An env-unset step proves the suite fails rather than skips. There's a `just ci` preflight naming the variables, and a fresh-machine job inside the required "CI gates" check runs the documented operator path end to end.

Two transport mechanisms are settled at implementation time. Their expected failures are named entries, each with a signature, a count pin, an admission rule and an exit detector keyed on the sqlx version. *(Anchors: R-0018-b; `P-TrustworthySignal`; `P-TestableByDesign`; `P-LeastAuthority`; `P-SecurityLayered`; `P-CheckCoversTheClaim` (a conclusion is no wider than what its check ranged over), so "a fresh machine reaches serving" is claimed only by a job that does it; V-Consistency; maintainer ruling 7091, which rules out a host-driven lifecycle.)*

**Implemented by** spec R-0120-a through R-0120-h and R-0121-c in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`. The tier-2 spec's own amendment (#4119) restates its R-0026/R-0027/R-0032 wording against this decision.

### Consequences

**Good:** no test can run against a production store, start a server, skip silently, or sweep another lane's live run. The operator's documented setup path is the tested path. And the host-under-test is exercised as exactly the role it'll run as.

**Bad / Trade-offs:** a local developer must provide a server (two paths are documented). Three roles and a run id mean more fixture than before. The 24-hour age sweep assumes no run lives that long, and that assumption is stated. The orchestrator-marker and container-id reads are INFERRED (reasoned, not yet observed) until they're measured in the CI container.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: every trap the repository has recorded has a mechanism and a seeded case.
- Con: the most fixture code of the options.

### Option 2 — fixture-started disposable cluster

- Pro: zero operator setup for tests.
- Con: it re-creates the host-driven lifecycle the engine decision rejected (ruling 7091, option 2). And packaging the binaries it would need is #4019's open question.

### Option 3 — skip when unconfigured

- Pro: convenient locally.
- Con: it's the skip-with-no-assertion class this repository was burned by. Coverage narrows while the gate stays green.

### Option 4 — one role for harness and host

- Pro: simpler.
- Con: it can't satisfy R-0117-c, because the harness needs `CREATEDB`. The smoke host could never serve.

## Amendment 2026-10-22 — the superuser credential, the harness check and the CI container (#4119)

**Status: APPROVED at the spec-exit of the #4119 amendment to the CI specs and V0 spec R-0120 (with R-0119-a's repeated-key refusal, amended 2026-10-25), 2026-10-25: the maintainer's conditional approval (#4119 activities 8420, 8438 and 8452), whose condition is recorded as met at #4119 activity 8494. It takes effect at that spec-exit, as ruled 2026-10-20 (#4119 activity 7849 item 3).** The spec-exit is the human gate that closes the [Spec](../glossary.md#spec) stage. This amendment is dated 2026-10-22 because it records the maintainer's ruling of that day (#4119 activity 7867); the corrections on the spec side carry *(Amended 2026-10-20, #4119)* markers. It's an in-place amendment per the [ADR README](README.md), so nothing above is rewritten. The requirement text is spec R-0120 in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md` as amended 2026-10-20, plus its R-0119-a as amended 2026-10-25. [P-0010](P-0010-storage-substrate-engine.md) carries the matching amendment for DE6 and DE9.

**Superseded in this record, with what replaces it:**

1. **"Two required URLs" (summary, Considered Options item 1) and, in Decision Outcome, "`MNEMRA_TEST_SUPERUSER_URL` (read by one gated binary only, enforced by a source check); either absent or unreachable fails the first acquiring test by name."** Every Postgres test binary requires one variable, `MNEMRA_TEST_DATABASE_URL`. The superuser credential comes in two parts: `MNEMRA_TEST_SUPERUSER_URL` with no password, plus `MNEMRA_TEST_SUPERUSER_PASSWORD_FILE` (`0600`, checked on the opened descriptor the same way R-0119-b checks the host's file). Both are required of the one gated cell, `tests/instance_check_privileged.rs`, and they're **not inherited through any environment**. The cell's recipe builds the binary without them and places them in the built executable's environment only, never in a workflow, job or `just ci` environment. It reads them from the superuser location `~/.config/mnemra/test-superuser/` (under `$XDG_CONFIG_HOME` when set), which the CI setup step and the documented local setup both write. A process with the same uid can still read that file and the cell's environment (accepted risk `AR-0015`); *superseded wording (Warden R3-W10): "held by that process alone"*. The recipe never exits 0 without having run the binary, so a local `just ci` runs the cell (ruling 8067 item 3). And the cell counts as executed only when its recipe is reachable from every chain `just ci` runs, locally or in CI (tier-2 R-0033 as amended). A source check alone confined nothing, because every child inherits the environment. So the shared fixture fails closed with `superuser_url_leaked` when either variable is set in a process whose **compile-time** test-target name isn't `instance_check_privileged`. `just ci`'s preflight requires the harness URL and refuses either superuser variable and any `PG*` variable, superseding Decision Outcome's "a `just ci` preflight naming the variables". Every test-side spawn of the production binary starts from `env_clear()`. *(Anchors: P-LeastAuthority; V-Consistency, for the R-0119-a/-b credential shape; P-CheckCoversTheClaim, since identity is keyed on an attribute a leak can't set; P-TrustworthySignal, never a skip; rulings 7867 item 1, 8067 item 3.)*
2. **Added: the harness role is checked against an allowlist, in R-0117-c's query shape** (ruling 8037). On a connection to `postgres`, the fixture requires a server of PostgreSQL 16 or later and `session_user = current_user`. The role must hold no `SUPERUSER`, `BYPASSRLS` or `REPLICATION`. And, checked by `pg_has_role(session_user, …, 'MEMBER')` over every other role, it may be a member only of run-grammar roles (roles named by the run-id grammar) that hold none of those three attributes. Anything else fails closed with `harness_overprivileged` (spec R-0120-a(6)). *Superseded draft wording (round 3): "reads `rolsuper`, `rolbypassrls` and `rolreplication` for the harness role and every role it can `SET ROLE` to, checks membership in the three server-file predefined roles". Because it was keyed on `current_user` and the `SET` privilege, it missed `SET FALSE` and `ADMIN`-only grants and every unlisted predefined role (MEASURED, #4119 activity 8047 Q2, Q4).* *(Anchors: P-LeastAuthority; P-SecurityLayered: fail closed, enumerate the permitted; V-Consistency: R-0117-c's shape.)*
3. **Context, "CI runs a service container"; Considered Options item 1, "digest-pinned service with a bump trigger"; and Decision Outcome, "CI provides a digest-pinned pgvector/PostgreSQL 16 service with a `pg_isready` wait".** CI starts a container from the same digest-pinned image **by job steps**, so it can present a per-run certificate. It does so under the six conditions of ruling 7867 item 1. The SAN contains the URL host. The server key's owner and mode are stated. The port is bound to `127.0.0.1`. Passwords are `add-mask`ed. `docker logs` runs on failure. And no superuser URL appears in the job or workflow environment. Tests connect over TLS. The host under test connects exactly as production does (verify-full plus a password file); the harness uses its throwaway password in URL userinfo over verified TLS (ruling 7867 item 3). *(Anchors: ruling 7849 item 2; ruling 7867 items 1 and 3; R-0119-d; P-SecurityLayered.)*
4. **Decision Outcome, "The sweep removes the run's own objects …"**. Whether an object is the run's "own" is decided by parsing each name right to left into fixed fields and comparing them, never by a prefix or `LIKE`. A drop takes only names the process created, re-checked by `datdba`, and teardown failures are loud. The process that generated the run id owns the run's roles and directory. Each binary drops only its own databases. The sweep of "the run's own objects" no longer runs inside a run the sweeping process doesn't own. And the `ci` recipe's `EXIT` trap, which fires on every exit path, names any leftover database, tears the run down and prints `GATE leaked-objects` (R-0120-a(3)/(5), R-0120-b as amended). *Superseded draft wording (round 3): "`GATE leaked-objects` ends the gate", a chain step that couldn't pass on a green run.* *(Anchors: P-CheckCoversTheClaim; P-TrustworthySignal; P-LeastAuthority: one owner per object.)*
5. **Added: one URL-to-options path.** Every test URL is parsed by the parser spec R-0119-a specifies for the host, in one of four modes (`host`, `superuser`, `harness`, `sentinel`). A repeated query key is refused in every mode, the host's included (spec R-0119-a as amended 2026-10-25, maintainer ruling 2026-10-25, #4119 activity 8289). `PG*` variables are refused in test processes, never scrubbed there. Messages print parsed fields only (spec R-0120-a(7); ruling 8037). *(Anchors: P-SecurityLayered; V-Simplicity; V-Consistency.)*
6. **Decision Outcome, "Before provisioning, the fixture refuses a server holding any non-test database that carries the migrations table or that it cannot connect to, caching the scan per run."** The guard is a name allowlist. Any database other than `template0`, `template1`, `postgres` or a `mnemra_test_*` name refuses the server. `postgres` is checked for the migrations table. There's no cache (spec R-0120-a(8); ruling 8067 item 2). *(Anchors: P-SecurityLayered: enumerate the permitted; V-Simplicity.)*
7. **Decision Outcome, "a fresh-machine job inside the required "CI gates" check".** The fresh-machine job is its own job with its own container. The gates job `needs:` it and fails first unless it succeeded (spec R-0120-g as amended; threat model F-4). *(Anchors: P-SecurityLayered: fail closed; control-integrity CI5, a rule from the control-integrity checklist.)*
8. **Added: run-scoped passwords as SCRAM verifiers, and the PostgreSQL 16 grants.** The fixture sets each run-scoped password as a client-computed SCRAM-SHA-256 verifier, so no statement carries a plaintext. It self-grants `SET` on both run roles (which also yields `INHERIT`), creates each test database `OWNER` the run owner role, never grants itself `pg_signal_backend`, and clones a template flagged `IS_TEMPLATE true` (spec R-0120-a(2), (9); MEASURED, #4119 activities 8047 Q1, Q3, Q5 and 8064 I-18). *(Anchors: V-Security; P-LeastAuthority.)*

*Rejected, kept with reasons:* the superuser password in an environment variable was rejected because the gated scenario's spawned host needs a password file under R-0119-d anyway, and R-0119-a rejects an environment-held password. A leak check that fires on every holder (the round-2 draft) was rejected because the gated binary is a fixture consumer, so the check fired on the permitted case. A `services:` container was rejected because no step can generate a certificate it will present. The marker scan with a per-run cache was rejected because a non-dedicated server passed it, `postgres` was exempt, and the cache was a forgeable file. A harness check keyed on `current_user` with the `SET` privilege and a three-role denylist (round 3) was rejected because a startup `role` setting, a `SET FALSE` grant, an `ADMIN`-only grant and every unlisted predefined role all passed it.

## Amendment 2026-10-28 — one server per lane (#4363)

**Status: DRAFT, pending the maintainer's exit approval of the #4363 amendment to spec R-0120 (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`).** The direction is the maintainer's. On 2026-10-28 he selected only "Separate server per lane (Recommended)", whose option text read "each lane gets its own Postgres server and port, so lanes cannot interfere". He didn't select "one harness role per lane (D2), keep sharing and patch in round 3, cut scope and park" (#4363 activity 9016). For the lane's identity he chose "Declared lane name" (#4363 activity 9037). Both records were written by Puck (the orchestrating agent), and so was the option wording. The text below is the draft's.

**Superseded in this record, with what replaces it:**

1. **Context, "Two lanes share one machine and may share one server".** Lanes share one machine, and each lane has its own server. A lane claim enforces this. The server's superuser writes the claim on `mnemra_test_template`, and every acquisition, the teardown executor and the gated cell read it and fail closed on it (spec R-0120-i(a)). The lane's identity is a declared lane name, bound to the worktree it was written for. Concurrent runs of one lane still share that lane's server: two runs in one worktree, say, or a task worktree beside its lane's main one. R-0120-i keeps the guarantee for them.
2. **Decision Outcome, "Good: … or sweep another lane's live run".** This now reads "or sweep a concurrent run's live objects". Another lane's runs aren't on the server at all.
3. **Accepted risk `AR-0016`** (`docs/src/architecture/overview.md`) is amended in the same change. Its acceptance of lanes sharing one server and one harness role is retired. Its acceptance for concurrent runs within one lane is kept.

The chosen option, its drivers and the other options are unchanged. *Rejected, kept with reasons:* the three directions the maintainer didn't select, as recorded in activity 9016. They were one harness role per lane, keeping one shared server and patching it, and cutting scope and parking. None of them had a reason given against it on that record beyond the selected option's own, "so lanes cannot interfere". The shared-server class had drawn new review findings in two rounds running (#4363 activity 8988). *(Anchors: [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace), since the superseded premise stays above, with what replaces it; P-GuaranteeByMechanism (enforce a guarantee with something that fails loudly when violated), since the one-server rule is a check, not a convention.)*

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE6 and DE9 cover what exits in CI and tests); [P-0026](P-0026-db-connection-authentication-transport.md) is the subject of the expected-failure entries; the tier-2 spec amendment is #4119. The ingestion re-read, #4121, is unrelated to this ADR.
- Packaging and the local install experience: #4019.
