---
title: "P-0029: Secrets management for the DB credential files"
summary: "The two database credentials live only in files: the serving password file read safely on every use (O_NOFOLLOW, fstat on the descriptor, no world read, owner = host uid, with an opt-in evidence-gated container shape), stay-up-and-report on a bad file with the old secret dropped, reloaded on SIGHUP; the owner password file read by `mnemra init` only and absent at serve time; passwords set client-side through `\\password` so no secret lands in SQL history; rotation is the operator's with SIGHUP as the reload path. Control-class ADR 4 of the operator-Postgres amendment."
primary-audience: agent
---

---
status: "accepted"
date: "2026-10-19"
decision-makers: ["the maintainer"]
consulted: ["Warden (security review, rounds 1–4)", "Bolt (operations review)"]
informed: []
supersedes: null
superseded_by: null
---

# P-0029: Secrets management for the DB credential files

## Status

`accepted`. This [ADR](../glossary.md#adr) (architecture decision record) was authored as `proposed` on 2026-10-19 at the #4018 gate (ruling 7793, where it was ADR 4 of the set). The maintainer accepted it the same day (#4018 activity 7800), together with the operator-run Postgres amendment at commit `08f4b8e`.

## Context and Problem Statement

Requirement R-0116-e (now superseded) held no database secret at all. That worked under the embedded engine. Under the operator-run instance, though, a password can exist for each of the two database roles ([P-0027](P-0027-db-authorization-role-separation.md)). So the questions the embedded design made disappear come back:

- where the secret lives;
- how it's read without a check-then-open window;
- what happens when it's wrong;
- how it rotates;
- how the serving process is kept away from the owner's secret.

Three findings shaped the answer. Warden (the security reviewer) measured that the existing startup file-mode check is wired only for the admin token, checks no ownership, and works by path rather than on an open file (`libs/mnemra-host/mnemra_host.rs:521`, `startup/file_mode_check.rs:67,:106` at `57cfeca`). Bolt (the operations reviewer) measured that Kubernetes mounts Secrets root-owned at mode `0644` by default. An exit on a bad file mode would therefore crash-loop the default setup, which is exactly the failure the restart-loop ruling forbids. And the maintainer ruled that the database password file follows a stay-up-and-report posture (activity 7762).

## Decision Drivers

- **No secret in the environment, argv, SQL history or logs** (the security and data-minimization values, V-Security and V-DataMinimization).
- **The check is on the descriptor the read uses**, so there's no path-based TOCTOU (time-of-check to time-of-use) race. This follows `P-CheckCoversTheClaim`, the principle that a check must range over exactly what the claim is about.
- **Stay up and report** on a misconfigured file (maintainer ruling 7762; the reliability value, V-Reliability).
- **Least authority at serve time.** The owner secret isn't reachable from the serving process (`P-LeastAuthority`: hold only the authority the role needs).
- **Consistency** with the existing file-shaped secrets (R-0008-e, R-0014-d) in shape, not in mechanism.

## Considered Options

1. **Files only, descriptor-checked on every read, stay-up on failure, container shape opt-in and evidence-gated, owner secret absent at serve time, `\password` for setting (chosen).**
2. **Password in the URL or an environment variable.**
3. **Reuse the existing 5-pre file-mode check and exit on failure, as R-0008-e / R-0014-d do.**
4. **One credential file shared by `init` and the serving host.**

## Decision Outcome

Chosen option: **1.**

The serving password lives only in the file named by `MNEMRA_DATABASE_PASSWORD_FILE`. By default that file sits beside the admin token file. The host opens it with `O_NOFOLLOW` and runs `fstat` on the open descriptor at boot, on every `SIGHUP`, and when `mnemra check-db` runs. The file must pass all of these checks:

- it's a regular file;
- it has no group or world write bit;
- it has no world read bit;
- its owner is the host uid, at mode `0600`.

There's one exception. The root-owned, group-readable shape that a Kubernetes Secret mount produces is accepted only when `MNEMRA_CREDENTIAL_MOUNT=1` is set **and** the host finds container evidence: marker files, a cgroup runtime line, or an orchestrator variable. The markers are labelled INFERRED, and the residual risk is named.

A failed check never makes the host exit. It produces the hard not-ready reason `credential_file_rejected`, which carries a remedy. It also drops any previously loaded secret, so a stale one can't shadow a later good read.

Rotation belongs to the operator. The operator sets the password on the instance through `\password`, writes the file, and sends `SIGHUP`; the host then rebuilds the connection pool.

The owner credential (`MNEMRA_DATABASE_OWNER_URL` / `_PASSWORD_FILE`) is read by `mnemra init` only. If it's present at serve time, meaning either variable is set or the default owner file is readable by the host uid, the host reports the hard not-ready reason `owner_credential_present`. That check has a named limit: it sees the two variables and the default path, nothing else. Keeping the owner credential "unreachable from the serving uid at all" stays an operator obligation.

`--setup-sql` prints no password literal.

*(Anchors: maintainer ruling 7762; V-Security; V-DataMinimization; `P-CheckCoversTheClaim`; `P-LeastAuthority`; V-Consistency, via the R-0008-e / R-0014-d file shape; `P-GuaranteeByMechanism` (enforce a guarantee with something that fails loudly), because refusals are mechanisms.)*

**Implemented by** spec R-0119-b (serving file), R-0119-g (owner credential), R-0119-a (no password in URL or environment; `PG*` scrub), R-0118-d (`\password` in the setup SQL), R-0118-b (stay-up), R-0117-j item 5 (the obligation) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`. These [R-codes](../glossary.md#r-codes) are defined in that spec.

### Consequences

**Good:** a mis-mounted Secret shows up as a visible not-ready state with a remedy, not a crash loop. Configuration can't hand the serving process the owner secret. No secret passes through SQL history or logs. And rotation has one documented path.

**Bad / Trade-offs:** the container relaxation adds a new variable, and its evidence gate relies on markers that a root-level writer could plant (that residual is named). A bare host that wants a root-owned, Secret-style file is refused unless it opts in. The two pre-existing own-file checks (admin token, signing material, LLM key) still exit on failure; moving them to the stay-up posture is #4125.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: closes the measured TOCTOU and ownership gaps, honours the stay-up ruling, and keeps the owner secret off the serving process.
- Con: it's a second credential-file code path beside the existing 5-pre check until #4125 unifies them.

### Option 2 — password in URL or environment

- Pro: zero file handling.
- Con: every process running under the same uid can read it, and it shows up in crash dumps and diagnostics. It's refused by mechanism, not by convention.

### Option 3 — reuse the 5-pre check, exit on failure

- Pro: one mechanism.
- Con: that mechanism is path-based and ignores ownership (measured). And an exit crash-loops the default Kubernetes Secret mount, so ruling 7762 rejects it.

### Option 4 — one credential file for `init` and serving

- Pro: one file to manage.
- Con: the serving process would hold the schema-changing credential as standing capability. That's the first draft's single-role defect, just in file form.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE5). [P-0026](P-0026-db-connection-authentication-transport.md) decides where a password may travel; [P-0027](P-0027-db-authorization-role-separation.md) decides what each role may do; [P-0008](P-0008-admin-token-shape.md) is the file-shaped precedent.
- Threat rows: the retype subsection of the architecture overview, rows `DS-db-password-serving` and `DS-db-password-owner` (I, information disclosure).
- Follow-up: #4125 (move the own-file refusals to the stay-up posture).
