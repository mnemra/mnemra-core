---
title: "P-0031: Test-substrate isolation"
summary: "Tests get their Postgres from an operator- or CI-provided server through the existing fixture entry point and never start one: two required URLs (fail, never skip), three roles per run (harness, run-scoped owner, run-scoped serving), per-test databases from a template under a gate-level run id that fits the 63-byte identifier limit, the superuser URL confined to one gated binary, a fail-closed refusal of any server holding a production store, an orphan sweep keyed on dead pids and a 24-hour age bound, a digest-pinned CI service with a scheduled bump trigger, a fail-not-skip proof step, and a fresh-machine job inside the required check. Control-class ADR 6 of the operator-Postgres amendment."
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

`accepted`. This [P-* ADR](../glossary.md#p--adr) (a decision scoped to this project) was authored as `proposed` on 2026-10-19 at the #4018 gate (ruling 7793, ADR 6). The maintainer accepted it the same day (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`. The CI tier-2 spec (`docs/specs/2026-07-02-ci-flake-tier2-shared-engine.md`) still names the embedded engine. Its amendment is tracked as #4119, and this ADR states what carries over into it.

## Context and Problem Statement

With no embedded engine, every Postgres-backed test needs a server it didn't start itself. The repository has been burned twice already. Once by a skip-with-no-assertion trap, where a test passed without checking anything. And once by a cross-lane reap, where one session's cleanup killed another session's postmaster (both recorded in `CLAUDE.md`). So the substrate has to fail loudly when it's absent, and it must never touch objects that belong to another run.

Several further constraints shape the design:

- Two lanes (parallel working sessions) share one machine and may share one server.
- CI runs Postgres as a service container.
- The host-under-test must satisfy R-0117-c ([R-codes](../glossary.md#r-codes) are requirement identifiers from the project's specs). The harness role needs `CREATEDB` and `CREATEROLE`, so it can't satisfy that requirement.
- A superuser connection is needed for a handful of negative tests and nowhere else.
- PostgreSQL silently truncates identifiers at 63 bytes.
- The production-store guard must not fail open on a database the harness can't connect to.

## Decision Drivers

- **No skip, no environment-dependent red** (`P-TrustworthySignal`, the principle that a verification signal must mean what it reports; R-0018-b requires a real Postgres, never a mock).
- **The server is an injected input** (`P-TestableByDesign`, design components so their dependencies can be supplied by tests).
- **Least authority**: the superuser is confined to one binary, and the host-under-test never runs as the harness role (`P-LeastAuthority`, hold only the authority a role needs).
- **Fail closed on the production-store guard** (`P-SecurityLayered`, security controls compose in independent layers and deny on error).
- **One mechanism for local, CI and smoke runs** (V-Consistency, the value that the same problem gets the same shape everywhere). The supply chain of the CI image is pinned by digest, with a bump trigger that actually fires (`P-SecurityLayered`). Dependabot is disabled on this repository, so nothing else would flag a stale image.

## Considered Options

1. **Operator/CI-provided server, two required URLs, three roles per run, template databases, gate-level run id, dead-pid and age sweeps, confined superuser, fail-closed guard, digest-pinned service with a bump trigger (chosen).**
2. **A fixture-started disposable cluster from the operator's `initdb`/`pg_ctl`.**
3. **Skip Postgres tests when no server is configured.**
4. **One role for harness and host-under-test.**

## Decision Outcome

Chosen option: **1.**

The fixture reads two URLs. `MNEMRA_TEST_DATABASE_URL` connects as the harness role (`LOGIN CREATEDB CREATEROLE`, not superuser). `MNEMRA_TEST_SUPERUSER_URL` is read by one gated binary only, and a source check enforces that. If either is absent or unreachable, the first test that tries to acquire a database fails, by name.

Per run, the fixture creates a run-scoped owner role and a run-scoped serving role, so the smoke host satisfies R-0117-c. Per test, it creates a database cloned from `mnemra_test_template`, which has pgvector pre-installed by the setup.

The run id `<run>` identifies the gate invocation. `just ci` exports `MNEMRA_TEST_RUN_ID` once; outside `just ci`, a standalone binary generates it. Its form is `<hosthash>_<pid>_<epoch>`, where the hash covers hostname, boot id and container id. Every name built from it is sized to fit 63 bytes and is parsed from the right.

Before provisioning anything, the fixture refuses any server that holds a non-test database carrying the migrations table, or a database it can't connect to. It caches that scan per run.

The sweep removes three kinds of object:

- the run's own objects;
- objects with the same host hash whose pid is dead or whose epoch doesn't match;
- any `mnemra_test_*` object older than 24 h.

It never touches a live run under a different hash. Its limit with pid namespaces is stated.

On the CI side, the decision provides:

- a digest-pinned pgvector/PostgreSQL 16 service with a `pg_isready` wait;
- a scheduled workflow that fails when the tag's digest moves;
- a step that runs with the variables unset, proving the suite fails rather than skips;
- a `just ci` preflight that names the required variables;
- a fresh-machine job inside the required "CI gates" check that runs the documented operator path end to end.

Two transport mechanisms are settled at implementation time, and their expected failures are named entries. Each carries a signature, a count pin, an admission rule and an exit detector keyed on the sqlx version. *(Anchors: R-0018-b; `P-TrustworthySignal`; `P-TestableByDesign`; `P-LeastAuthority`; `P-SecurityLayered`; `P-CheckCoversTheClaim` (a conclusion may be no wider than what the check covered), so "a fresh machine reaches serving" is claimed only by a job that actually does it; V-Consistency; maintainer ruling 7091, which rules out a host-driven lifecycle.)*

**Implemented by** spec R-0120-a through R-0120-h and R-0121-c in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`. The tier-2 spec's own amendment (#4119) restates its R-0026/R-0027/R-0032 wording against this decision.

### Consequences

**Good:** no test can run against a production store, start a server, skip silently, or sweep another lane's live run. The operator's documented setup path is the path that gets tested. And the host-under-test is exercised as exactly the role it'll run as in production.

**Bad / Trade-offs:** a local developer must provide a server (two paths are documented). Three roles and a run id mean more fixture code than before. The 24-hour age sweep assumes no run lives that long, and that assumption is stated. The orchestrator-marker and container-id reads are INFERRED until they're measured in the CI container.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: every trap the repository has recorded gets a mechanism and a seeded case.
- Con: the most fixture code of the options.

### Option 2 — fixture-started disposable cluster

- Pro: zero operator setup for tests.
- Con: it re-creates the host-driven lifecycle that the engine decision rejected (ruling 7091, option 2). Packaging the binaries it would need is also an open question, tracked in #4019.

### Option 3 — skip when unconfigured

- Pro: convenient locally.
- Con: this is the skip-with-no-assertion class that already burned the repository. Coverage narrows while the gate stays green.

### Option 4 — one role for harness and host

- Pro: simpler.
- Con: it can't satisfy R-0117-c, because the harness needs `CREATEDB`. The smoke host could never serve.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE6 and DE9 cover what exits in CI and tests); [P-0026](P-0026-db-connection-authentication-transport.md) is the subject of the expected-failure entries; the tier-2 spec amendment is #4119. The ingestion re-read, #4121, is unrelated to this ADR.
- Packaging and the local install experience: #4019.
