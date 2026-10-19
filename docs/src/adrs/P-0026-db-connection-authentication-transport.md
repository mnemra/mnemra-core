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

`accepted` — authored `proposed` 2026-10-19 at the #4018 gate (maintainer ruling 7793: all seven of Warden's proposed control-class ADRs are written; this is ADR 1). Moves to `accepted` at the amendment's final gate. Pending the maintainer's gate; nothing here is locked.

## Context and Problem Statement

Under [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 the host no longer owns its cluster; it is a client of an operator-run PostgreSQL instance it reaches over a unix socket or TCP, with a credential that can exist. The embedded design (spec R-0116-e, superseded) had dissolved every transport question by owning a host-only socket. Three facts set the problem (Warden, MEASURED in sqlx-postgres 0.9.0 during review): the driver's URL parser accepts far more than the host inspects (`?password=`, `hostaddr`, `options`, multi-host, `PG*` defaults, an always-run `~/.pgpass` lookup, unknown keys logged with their values); neither loopback TCP nor a unix socket authenticates the server; and the driver answers cleartext and MD5 password requests with no SCRAM channel binding. The maintainer ruled that local server spoofing is **not** an accepted risk (#4018 activity 7771) — it must be mitigated.

## Decision Drivers

- **Default-on security, one posture local and remote** — the maintainer's standing direction ("acts like remote even when local", activity 7787) applied to the database connection: a password never travels unprotected, on any transport.
- **Enumerate the permitted** — a denylist on a grammar wider than the host's list fails open on the first omission (`P-SecurityLayered`).
- **No secret in the environment, argv, SQL history or logs** (V-Security, V-DataMinimization).
- **Honesty about the driver** — two mechanisms cannot be built with today's driver; they are named and pinned, not implied (V-Honesty, `P-TrustworthySignal`).

## Considered Options

1. **Explicit, allowlisted connection options; TLS `verify-full` for every TCP connection; peer-only sockets with a pre-write peer-uid check; environment and `.pgpass` made inert (chosen).**
2. **Pass `MNEMRA_DATABASE_URL` to the driver's parser with a denylist of forbidden keys.**
3. **Require TLS only off-loopback; allow password auth on loopback and sockets.**
4. **Peer authentication only (no password path at all), carrying R-0116-e's shape forward.**

## Decision Outcome

Chosen option: **1.** The host parses `MNEMRA_DATABASE_URL` itself against a closed grammar and builds the driver's options field by field; the driver never sees the URL or a `PG*` variable; `PG*` is scrubbed in `main` before any thread starts; `~/.pgpass` is made inert and proven so by a seeded test. Any TCP connection — loopback included — uses `sslmode = verify-full` against the operator's CA, and a password is only ever sent over such a connection; a unix socket is used only with peer authentication and no password, after the peer uid is matched to the socket directory's owner and the directory carries no group or other write bit. The host refuses cleartext and MD5 authentication requests — an implementation-time mechanism (the driver exposes no option), pinned as a named expected failure with an exit detector keyed on the driver version. The "same connection, before the first byte" form of the peer-uid check is likewise pinned. *(Anchors: maintainer ruling 7771 — local spoofing mitigated, not accepted; `P-SecurityLayered` — enumerate the permitted, a control that errors denies; V-Security; V-DataMinimization — peer authentication holds no secret; `P-GuaranteeByMechanism` — refusals are mechanisms, not conventions; `P-Defer` DF1 — the two implementation-time gaps have a named exit and a detector.)*

**Implemented by** spec R-0119-a (allowlist, `PG*` scrub), R-0119-c (peer), R-0119-d (transport floor), R-0119-e (`.pgpass` inert), R-0117-f (transport verification, socket-directory rule, the two pinned mechanisms), R-0120-h (expected failures) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`. This ADR records the decision; the clauses carry the observables.

### Consequences

**Good:** one transport posture local and remote; no credential reaches the wire except over an authenticated channel; the driver's parser is out of the trust boundary; the operator's `pg_hba` cannot downgrade the host into cleartext once the refusal mechanism lands; until then the only protection against a downgrade request is that the password travels inside a `verify-full` TLS session to an authenticated server — the `pg_hba` obligation (R-0117-j item 6) is unverified and catches nothing, which is why the refusal is pinned as a named expected failure rather than left to the document.

**Bad / Trade-offs:** an operator running Postgres on the same machine must either provision a socket with peer authentication or a TLS certificate for loopback — there is no "just a password on localhost" mode; two mechanisms are known gaps until the driver or the host's own connection handling closes them (`known_gaps` on `/ready` while they are, R-0118-f); the socket-directory check does not walk ancestors (residual named in R-0117-f).

## Pros and Cons of the Options

### Option 1 — allowlisted options, TLS everywhere on TCP, peer-only sockets (chosen)

- Pro: the only option under which every bypass Warden measured has a seeded test.
- Pro: one code path for local and remote, matching the maintainer's direction.
- Con: local password-on-loopback is refused; the operator must choose socket+peer or TLS.

### Option 2 — driver URL parser plus a denylist

- Pro: least code.
- Con: the denylist is shorter than the driver's grammar by construction; an unlisted key (`passfile`, `options`, a future one) fails open, and unknown keys were measured to be logged with their values.

### Option 3 — TLS only off-loopback

- Pro: the familiar shape.
- Con: loopback authenticates nothing — a same-host process that binds the port receives the password; rejected by ruling 7771.

### Option 4 — peer authentication only

- Pro: no credential at rest, the R-0116-e property.
- Con: a remote or managed instance has no unix socket, so the host could not reach the deployment shapes the amendment exists for.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE5, AE1 ruled); [P-0029](P-0029-db-credential-files.md) owns the credential file itself; [P-0027](P-0027-db-authorization-role-separation.md) owns what the authenticated role may do.
- The probe listener's own transport (`/health`, `/ready`) is ADR 7, P-0032, written with the HTTPS shared-service transport amendment (#4127).
- Threat rows: overview "Operator-instance retype (Amended 2026-10-19 r2)" — `DF-health-probe`/`DF-substrate-rw` over local transports (S), remote TCP (S, T, I).
