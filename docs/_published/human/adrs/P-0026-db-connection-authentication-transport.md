---
title: "P-0026: DB connection authentication and transport"
summary: "How the host reaches the operator-run PostgreSQL instance: connection options built from a closed allowlist (never the driver's URL parser or the environment), a password only ever over verify-full TLS (loopback included), peer authentication over a unix socket after a peer-uid and socket-directory check, PG* and ~/.pgpass made inert, cleartext/MD5 auth requests refused. Control-class ADR 1 of the operator-Postgres amendment."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review, rounds 1–4)", "Bolt (operations review)", "Charon (canon review)"]
informed: []
supersedes: null
superseded_by: null
---

# P-0026: DB connection authentication and transport

## Status

`accepted`. This [ADR](../glossary.md#adr) (Architecture Decision Record) was authored as `proposed` on 2026-10-19 at the #4018 gate. Maintainer ruling 7793 directed that all seven of the control-class ADRs Warden (the security reviewer) proposed be written, and this is ADR 1 of the seven. It moves to `accepted` at the amendment's final gate. Until the maintainer's gate passes, nothing here is locked.

## Context and Problem Statement

Under [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19, the host no longer owns its database cluster. It's now a client of an operator-run PostgreSQL instance, reached over a unix socket or TCP, with a credential that can exist. The earlier embedded design (spec R-0116-e, now superseded) never had to answer transport questions, because the host owned a socket only it could reach. That shortcut is gone.

Three facts set the problem, and Warden measured each of them in sqlx-postgres 0.9.0 during review. First, the driver's URL parser accepts far more than the host inspects: `?password=`, `hostaddr`, `options`, multi-host strings, `PG*` environment defaults, a `~/.pgpass` lookup that always runs, and unknown keys that get logged along with their values. Second, neither loopback TCP nor a unix socket authenticates the server. Third, the driver answers cleartext and MD5 password requests, with no SCRAM channel binding. The maintainer ruled that local server spoofing is **not** an accepted risk (#4018 activity 7771). It has to be mitigated.

## Decision Drivers

- **Default-on security, one posture local and remote.** The maintainer's standing direction ("acts like remote even when local", activity 7787) applies here to the database connection. A password never travels unprotected, on any transport.
- **Enumerate the permitted.** A denylist checked against a grammar wider than the host's own list fails open on the first omission (`P-SecurityLayered`, the principle that security controls compose in layers and that a control fails closed by listing what it permits).
- **No secret in the environment, argv, SQL history or logs** (V-Security, the security value; V-DataMinimization, the value of not holding data at all where possible).
- **Honesty about the driver.** Two mechanisms can't be built with today's driver. They're named and pinned, not implied (V-Honesty, the value of separating fact from inference; `P-TrustworthySignal`, the principle that a verification signal must mean what it reports).

## Considered Options

1. **Explicit, allowlisted connection options; TLS `verify-full` for every TCP connection; peer-only sockets with a pre-write peer-uid check; environment and `.pgpass` made inert (chosen).**
2. **Pass `MNEMRA_DATABASE_URL` to the driver's parser with a denylist of forbidden keys.**
3. **Require TLS only off-loopback; allow password auth on loopback and sockets.**
4. **Peer authentication only (no password path at all), carrying R-0116-e's shape forward.**

## Decision Outcome

Chosen option: **1.**

The host parses `MNEMRA_DATABASE_URL` itself, against a closed grammar, and builds the driver's options one field at a time. The driver never sees the URL or any `PG*` variable. `PG*` is scrubbed in `main` before any thread starts. `~/.pgpass` is made inert, and a seeded test proves it.

Transport depends on how the host connects. Any TCP connection, loopback included, uses `sslmode = verify-full` against the operator's CA, and a password is only ever sent over that kind of connection. A unix socket is used only with peer authentication and no password. Before using it, the host checks that the peer uid matches the owner of the socket directory, and that the directory has no group or other write bit set.

The host refuses cleartext and MD5 authentication requests. The driver exposes no option for this, so it's an implementation-time mechanism. It's pinned as a named expected failure, with an exit detector keyed on the driver version. The stronger form of the peer-uid check ("same connection, before the first byte") is pinned the same way.

*(Anchors: maintainer ruling 7771, local spoofing mitigated rather than accepted; `P-SecurityLayered`, enumerate the permitted, and a control that errors denies; V-Security; V-DataMinimization, since peer authentication holds no secret; `P-GuaranteeByMechanism` (enforce a guarantee with something that fails loudly, not a convention), so refusals are mechanisms rather than conventions; [`P-Defer`](../glossary.md#p-defer) (defer mechanism choice until evidence forces it) rule DF1, so the two implementation-time gaps each have a named exit and a detector.)*

**Implemented by** spec R-0119-a (allowlist, `PG*` scrub), R-0119-c (peer), R-0119-d (transport floor), R-0119-e (`.pgpass` inert), R-0117-f (transport verification, socket-directory rule, the two pinned mechanisms), R-0120-h (expected failures) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`. Each of these requirement codes is defined in that spec. This ADR records the decision; the spec clauses carry the observables.

### Consequences

**Good:** There's one transport posture for local and remote. No credential reaches the wire except over an authenticated channel. The driver's parser sits outside the trust boundary. Once the refusal mechanism lands, the operator's `pg_hba` can't downgrade the host into cleartext. Until then, the only protection against a downgrade request is that the password travels inside a `verify-full` TLS session to an authenticated server. The `pg_hba` obligation (R-0117-j item 6) is unverified and catches nothing. That's why the refusal is pinned as a named expected failure instead of being left to the document.

**Bad / Trade-offs:** An operator running Postgres on the same machine has to provision either a socket with peer authentication or a TLS certificate for loopback. There's no "just a password on localhost" mode. Two mechanisms stay known gaps until the driver, or the host's own connection handling, closes them; while they're open, they show up as `known_gaps` on `/ready` (R-0118-f). And the socket-directory check doesn't walk ancestor directories (the residual is named in R-0117-f).

## Pros and Cons of the Options

### Option 1 — allowlisted options, TLS everywhere on TCP, peer-only sockets (chosen)

- Pro: it's the only option under which every bypass Warden measured has a seeded test.
- Pro: one code path for local and remote, matching the maintainer's direction.
- Con: a local password over loopback without TLS is refused, so the operator must choose socket plus peer, or TLS.

### Option 2 — driver URL parser plus a denylist

- Pro: least code.
- Con: the denylist is shorter than the driver's grammar by construction. An unlisted key (`passfile`, `options`, or a future one) fails open, and unknown keys were measured being logged with their values.

### Option 3 — TLS only off-loopback

- Pro: the familiar shape.
- Con: loopback authenticates nothing. A same-host process that binds the port receives the password. Rejected by ruling 7771.

### Option 4 — peer authentication only

- Pro: no credential at rest, which is the property R-0116-e had.
- Con: a remote or managed instance has no unix socket, so the host couldn't reach the deployment shapes the amendment exists to support.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE5, AE1 ruled). [P-0029](P-0029-db-credential-files.md) owns the credential file itself. [P-0027](P-0027-db-authorization-role-separation.md) owns what the authenticated role is allowed to do.
- The probe listener's own transport (`/health`, `/ready`) is ADR 7 of the set, P-0032. It's written alongside the HTTPS shared-service transport amendment (#4127).
- Threat rows: see the overview's "Operator-instance retype (Amended 2026-10-19 r2)" section. It covers `DF-health-probe` and `DF-substrate-rw` over local transports (S, spoofing) and over remote TCP (S, T, I: spoofing, tampering, information disclosure).
