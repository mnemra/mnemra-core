---
spec_type: architecture
frame_relevant: true
modulation: brownfield-extension
---

# Intake: real plugin signing + supply-chain binding + runnable host (signing → run())

**Stakes:** high
**Date:** 2026-06-30
**Status:** locked
**Consumer:** mixed — primarily the implementing and verification pipeline (the resulting spec drives test-authoring and implementation); and the maintainer, who runs the resulting server as the V0 reference workload.

## JTBD

The host has a verified-load mechanism and an MCP server handler, but it has never actually run: the production entry point `run()` is an empty stub, the embedded signing root is a non-production placeholder, and the on-disk plugin manifest is placeholder-signed. The maintainer needs the host to **start as a live stdio MCP server backed by real plugin signing**, so the project can be exercised end-to-end against a real reference workload with no fallback to prior tooling.

## Non-goals

- **No production / multi-deployment key-custody hardening.** CI-signs-without-leaking, hardware-backed keys, and key rotation are deferred to the multi-deployment custody trip-wire (R-0005-e → the not-yet-authored Tier-C custody-hardening ADR), which fires before any non-maintainer deployment. V0 is maintainer-only.
- **No Streamable-HTTP transport.** stdio only at V0 (R-0010-e: HTTP transport SHALL NOT be activated at V0).
- **No admin CLI.** The control-plane CLI (Task 24) is a sibling surface, not required for the host to run.
- **No multi-plugin runtime or reachable-path host-fn allowlist enforcement.** V0 is a single embedded plugin; the allowlist remains a query surface only (#1882).
- **No non-maintainer deployment readiness.** The deployment-node / packaging story is out of scope.

## Success criteria

**M1 — real signing + supply-chain integrity**

1. The on-disk plugin manifest is signed by a real Ed25519 key; synchronous verification returns a Core result against the embedded **real** public key (not the bootstrap placeholder).
2. The embedded signing root is **not** the bootstrap placeholder. A build-time gate (R-0018-f `verify-build`) **fails the build** if the root is still the placeholder, and a CI check enforces this so a placeholder root can never ship.
3. The manifest carries a **content-hash of the component bytes in its signed body**; the verified-load gate (`populate_verified_pool`) recomputes the loaded component's hash and **rejects on mismatch** — a swapped component fails to load even though the manifest signature still verifies.
4. The private signing key is stored mode 600 **outside any directory the runtime reads** (R-0005-c custody preserved even when the build host and run host are the same machine); only the public key is embedded (R-0005-d); no runtime key-fetch.

**M2 — actually runs**

5. `run()` performs production startup in the locked order — independently assertable as:
    - **(5a)** the health loopback listener binds **before config load** and before MCP accept (R-0004-g);
    - **(5b)** all seven builtins initialize **before any plugin is loaded** (R-0002-c), and the verified plugin pool is populated **before** MCP accept (R-0016-a);
    - **(5c)** a storage pool is sourced and schema initialization runs, then the server is constructed and serves over stdio.
6. Running the host binary starts a live stdio MCP server that answers MCP requests end-to-end (an end-to-end smoke check, Task 27 `verify-smoke`, passes).

## Hard constraints

- **Signing primitive:** Ed25519 via `ed25519-dalek` (P-0005, locked).
- **Custody:** build-host on-disk key, mode 600, owner = build-pipeline UID, off the runtime-read path; only the public key embedded; no runtime key-fetch (R-0005-c, R-0005-d). The minimal V0 ceremony must honor these — "off deployment node" goes degenerate when build host == run host, so the private key must stay out of any directory the runtime reads.
- **Verification:** synchronous, fail-closed — no instance until `verify()` returns Ok; failure rejects the load with a structured error naming the plugin (R-0005-a, R-0005-b).
- **core-by-provenance:** core status is determined by signature provenance, not manifest-field trust (R-0005-h). The new content-hash field therefore MUST live in the **signed canonical body** (the bytes the signature covers, before the `[signature]` section) — a hash in the unsigned section would let an attacker swap both the component and its hash.
- **Content-hash binding lands with or before real signing** — real signing without component binding verifies the manifest but not the bytes that load.
- **Manifest schema authority:** the content-hash field extends the manifest contract every plugin must satisfy → governed by P-0003 (manifest schema). The governance path (P-0003 amendment vs. spec-local field) is resolved at Frame — see **Open items carried to Frame**.
- **Transport:** the `rmcp` MCP SDK (P-0012, v1.7.0), stdio transport, no HTTP feature (R-0010-a, R-0010-e).
- **Startup ordering:** the health loopback listener binds **before config load** and before the server accepts MCP (R-0004-g); all seven builtins initialize **before any plugin is loaded** (R-0002-c); the verified pool is populated **before** MCP accept (R-0016-a).
- **Storage source:** the production pool is sourced from the embedded-Postgres engine startup path (`start_embedded()`, governed by P-0010 storage substrate) — **not** from the LLM-key / embedding config pathway (R-0014 governs that, and does not apply here).
- **Health dependency:** the locked startup order requires a health loopback listener that does not yet exist (Task 25). Standing it up is in scope; it is an already-planned surface, not a new design.

## Evidence

- The production entry point `run()` is an empty `Ok(())` stub — the server is exercised only in tests over an in-process duplex transport, never actually run.
- A prior security review (High confidence) found that the verified-load path authenticates the manifest but nothing binds the loaded component bytes to it: after real signing, a swapped component would load undetected — a supply-chain swap. The fix must land with or before real signing.
- The embedded root is a documented non-production placeholder, and the on-disk manifest carries `public_key = "ROOT"` / a placeholder signature; neither can ship.
- The V0 milestone is defined as a full reference workload running on the host with no fallback to prior tooling; a runnable, real-signed host is the precondition.

## Risk profile

**Trust-boundary work — carried to Frame.** This is the plugin signing / supply-chain integrity boundary: the embedded root, the signature verification gate, and the manifest→component binding all sit on it. A security-mode review applies at the Frame (where the mechanism is known) and at implementation review, and a pre-push security review gates the merge. No new trust boundary is introduced; the work hardens and instantiates the existing one (placeholder → real signing; manifest-only → manifest + component bytes).

## Open items carried to Frame

These are firm-to-defer: the work is committed, but one design choice is the Frame constraint-walk's to resolve (not an unresolved intake question that blocks the lock).

- **Content-hash governance path:** the content-hash field extends the manifest contract (a P-0003-governed surface). The Frame constraint-walk decides whether it lands as a **P-0003 amendment** (ADR) or a **spec-local field** — both are valid; the walk picks based on whether the field is part of the durable manifest contract every plugin satisfies (→ P-0003 amendment) or scoped to this work (→ spec-local). The intake commits to *adding the field in the signed body*; only its governance home is deferred.

## Sequencing note

Delivered as two milestones under one spec: **M1** (real signing + content-hash binding) is the security foundation and merges first; **M2** (health listener + production run-wiring) makes the host actually serve and depends on M1. The committed-tier plan sequences the milestones as separate changes; this intake and the resulting spec scope the full runnable chain.

## Consultations

- _none at intake — scope and the V0 ceremony approach were locked with the maintainer prior to this run (full runnable chain; minimal maintainer-local ceremony); intake transcribes the locked decisions._

## Dismissed review flags

- _none yet._
