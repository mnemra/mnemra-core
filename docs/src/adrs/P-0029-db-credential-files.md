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

`accepted` — authored `proposed` 2026-10-19 at the #4018 gate (ruling 7793, ADR 4). Accepted 2026-10-19 by the maintainer (#4018 activity 7800) with the operator-run Postgres amendment at commit `08f4b8e`.

## Context and Problem Statement

R-0116-e (superseded) held no database secret at all. Under the operator-run instance a password can exist for each of the two roles ([P-0027](P-0027-db-authorization-role-separation.md)), and the questions the embedded design dissolved return: where the secret lives, how it is read without a check-then-open window, what happens when it is wrong, how it rotates, and how the serving process is kept from the owner's secret. Warden measured that the existing startup file-mode check wires only the admin token, checks no ownership and is path-based (`libs/mnemra-host/mnemra_host.rs:521`, `startup/file_mode_check.rs:67,:106` at `57cfeca`); Bolt measured that Kubernetes mounts Secrets root-owned at `0644` by default, so an exit on a bad mode crash-loops the default setup — the failure the restart-loop ruling forbids; the maintainer ruled the database password file stay-up-and-report (activity 7762).

## Decision Drivers

- **No secret in the environment, argv, SQL history or logs** (V-Security, V-DataMinimization).
- **The check is on the descriptor the read uses** — no path-based TOCTOU (`P-CheckCoversTheClaim`).
- **Stay up and report** on a misconfigured file (maintainer ruling 7762; V-Reliability).
- **Least authority at serve time** — the owner secret is not reachable from the serving process (`P-LeastAuthority`).
- **Consistency** with the existing file-shaped secrets (R-0008-e, R-0014-d) in shape, not in mechanism.

## Considered Options

1. **Files only, descriptor-checked on every read, stay-up on failure, container shape opt-in and evidence-gated, owner secret absent at serve time, `\password` for setting (chosen).**
2. **Password in the URL or an environment variable.**
3. **Reuse the existing 5-pre file-mode check and exit on failure, as R-0008-e / R-0014-d do.**
4. **One credential file shared by `init` and the serving host.**

## Decision Outcome

Chosen option: **1.** The serving password lives only in `MNEMRA_DATABASE_PASSWORD_FILE` (default beside the admin token file), opened `O_NOFOLLOW` and `fstat`-checked on the open descriptor at boot, on every `SIGHUP` and by `mnemra check-db`: a regular file, no group or world write bit, no world read bit, owner = host uid at `0600`; the root-owned group-readable Secret-mount shape is accepted only with `MNEMRA_CREDENTIAL_MOUNT=1` **and** container evidence (marker files, cgroup runtime line, or an orchestrator variable — markers labelled INFERRED; residual named). A failed check is the hard not-ready reason `credential_file_rejected` with a remedy, never an exit, and it drops any previously loaded secret so a stale one cannot shadow a later good read. Rotation is the operator's: set the password on the instance through `\password`, write the file, `SIGHUP`; the pool is rebuilt. The owner credential (`MNEMRA_DATABASE_OWNER_URL` / `_PASSWORD_FILE`) is read by `mnemra init` only, and its presence at serve time — either variable set, or the default owner file readable by the host uid — is the hard not-ready reason `owner_credential_present`; the limit of that check (it sees the two variables and the default path only) is named, and "unreachable from the serving uid at all" stays an operator obligation. `--setup-sql` prints no password literal. *(Anchors: maintainer ruling 7762; V-Security; V-DataMinimization; `P-CheckCoversTheClaim`; `P-LeastAuthority`; V-Consistency — the R-0008-e / R-0014-d file shape; `P-GuaranteeByMechanism` — refusals are mechanisms.)*

**Implemented by** spec R-0119-b (serving file), R-0119-g (owner credential), R-0119-a (no password in URL or environment; `PG*` scrub), R-0118-d (`\password` in the setup SQL), R-0118-b (stay-up), R-0117-j item 5 (the obligation) in `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`.

### Consequences

**Good:** a mis-mounted Secret is a visible not-ready state with a remedy, not a crash loop; the serving process cannot be handed the owner secret by configuration; no secret transits SQL history or logs; rotation has one documented path.

**Bad / Trade-offs:** the container relaxation is a new variable with an evidence gate whose markers are plantable by a root-level writer (residual named); a bare host that wants a root-owned Secret-style file is refused unless it says so; the two pre-existing own-file checks (admin token, signing material, LLM key) still exit — moving them is #4125.

## Pros and Cons of the Options

### Option 1 (chosen)

- Pro: closes the measured TOCTOU and ownership gaps; honours the stay-up ruling; keeps the owner secret off the serving process.
- Con: a second credential-file code path beside the existing 5-pre check until #4125 unifies them.

### Option 2 — password in URL or environment

- Pro: zero file handling.
- Con: readable by every same-uid process, present in crash dumps and diagnostics; refused by mechanism, not convention.

### Option 3 — reuse the 5-pre check, exit on failure

- Pro: one mechanism.
- Con: the mechanism is path-based and ownership-blind (measured), and an exit crash-loops the default Kubernetes Secret mount — rejected by ruling 7762.

### Option 4 — one credential file for `init` and serving

- Pro: one file to manage.
- Con: the serving process would hold the schema-changing credential as standing capability — the first draft's single-role defect in file form.

## More Information

- [P-0010](P-0010-storage-substrate-engine.md) §Amendment 2026-10-19 (DE5); [P-0026](P-0026-db-connection-authentication-transport.md) decides where a password may travel; [P-0027](P-0027-db-authorization-role-separation.md) decides what each role may do; [P-0008](P-0008-admin-token-shape.md) is the file-shaped precedent.
- Threat rows: overview retype subsection — `DS-db-password-serving`, `DS-db-password-owner` (I).
- Follow-up: #4125 (own-file refusals to the stay-up posture).
