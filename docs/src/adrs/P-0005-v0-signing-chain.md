---
title: "P-0005: V0 Signing Chain"
summary: "Defines the V0 minimum-viable custody decision for the mnemra root signing key: build-host-on-disk for dogfood, synchronous verification on plugin load, and a multi-deployment trip-wire to Tier-C hardening."
primary-audience: agent
---

---
status: "accepted"
date: "2026-05-24"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0005: V0 Signing Chain

## Status

`accepted`

## Context and Problem Statement

Mnemra-core's `0.1.0` substrate build pipeline must emit signed `core: true` plugin artifacts. The runtime must verify those signatures synchronously before any plugin instance is loaded. This requires a concrete custody decision for the mnemra root signing key before the substrate build begins — the build pipeline cannot sign artifacts under a deferred custody decision.

The Frame's Tier-A table splits the original `{{P-SigningKeyCustody}}` slot (previously Tier C) into two:

- **`{{P-V0SigningChain}}`** (Tier A) — the minimum-viable V0 custody position that lets `0.1.0` ship signed plugins.
- **`{{P-SigningKeyCustodyHardening}}`** (Tier C) — production-grade hardening (HSM-backed / runtime-fetch / never-on-node), activated by the multi-deployment trip-wire.

This ADR locks the Tier-A half. The question: what is the smallest custody decision that (a) lets V0 build-pipeline signing work, (b) makes the V0 risk explicit and bounded, and (c) structurally prevents the decision from silently extending to multi-deployment use?

The security reviewer classified `DS-mnemra-root-key`/I as **Critical** (severity 85): if the key material leaks from the deployment node, an attacker can sign forged `core: true` plugins. The V0 position must be named, not merely acknowledged.

## Decision Drivers

- **V0 must ship signed plugins.** The product brief's Hard constraints commit to signed `core: true` plugins in `0.1.0`. A deferred custody decision blocks the build pipeline.
- **P-Defer: name the V0 mechanism AND its trip-wire.** The workspace principle requires that any Open/Deferred custody decision names both the V0 operating mechanism and the condition that forces the hardened decision. Silence is not a V0 mechanism.
- **Single-deployment dogfood scope.** V0 is the maintainer's single deployment. The custody risk profile at single-instance-maintainer-controlled is categorically different from multi-deployment.
- **Synchronous verification is non-negotiable.** The security reviewer flagged `P-plugin-runtime`/E as Critical (severity 70): a "verify-async" or "defer to background" path that allows execution before verification completes is unacceptable. Fail-closed is structural.
- **Trip-wire is structural, not advisory.** The moment mnemra-core is deployed beyond the maintainer's single dogfood instance, `{{P-SigningKeyCustodyHardening}}` must lock. This is not a recommendation; it is the condition that retires `R-0004` from the accepted-risk register.

## Considered Options

1. **Option A — V0 key on build host (on-disk, mode 600, build-host-only).** The mnemra root signing key lives on the build host's filesystem at mode 600. Signing happens at build time; the runtime on the deployment node receives only signed artifacts and the verification key (or cert chain). Key material never flows to the deployment node. Multi-deployment trip-wire is explicit: `{{P-SigningKeyCustodyHardening}}` locks before any deployment beyond the maintainer's dogfood instance.

2. **Option B — Key on deployment node (on-disk, mode 600).** The signing key lives on the deployment node alongside the runtime, enabling in-process or admin-API-triggered re-signing. Simpler initial setup for a single-node deployment.

3. **Option C — Deferred: no key, no signing at V0.** Plugin verification is disabled until `{{P-SigningKeyCustodyHardening}}` is authored. V0 ships unsigned plugins; the runtime skips signature checks.

## Decision Outcome

**Option A** — build-host-on-disk, mode 600, build-host-only. Key material on the build host; deployment node receives signed artifacts and verification material only.

**Rationale:** Option A separates the signing surface from the runtime surface. Even at V0 dogfood scale, keeping the signing key off the deployment node is a structural control: it limits the blast radius of a deployment-node compromise to the artifacts already signed, not to future signing authority. The custody position is named and bounded; `R-0004` records the residual risk explicitly.

Option B (key on deployment node) conflates the signing surface with the runtime surface. Any host-read primitive on the deployment node recovers the signing key (`DS-mnemra-root-key`/I, Critical). For V0 dogfood, the practical difference from Option A is small — the deployment node is also the build host in single-developer setups — but the principle matters: the design must not silently normalize key-on-deployment-node as acceptable.

Option C (no signing at V0) is a Hard constraint violation. The product brief commits to signed `core: true` plugins; disabling verification is not a valid V0 posture.

### V0 key custody parameters

| Parameter | V0 value | Rationale |
|---|---|---|
| Signing key location | Build host filesystem, `mode 600`, owner = build-pipeline process UID | Keeps key off deployment node; OS-enforced read restriction on build host |
| Key type | Ed25519 | Fast, compact signatures; well-supported in Rust signing crates; no parameter-choice footgun |
| Verification posture | Synchronous on plugin load; load fails closed | Eliminates the verify-async path (`P-plugin-runtime`/E Critical) |
| V0 root cert / key distribution | Embedded in the mnemra-core binary at build time | No runtime key-fetch at V0; binary carries the verification material |
| Signing scope at V0 | `core: true` plugins only | Third-party plugin install is V0.1+ scope; runtime rejects any non-`core` plugin at V0 |
| Multi-deployment trip-wire | Any deployment beyond the maintainer's single dogfood instance | Fires `{{P-SigningKeyCustodyHardening}}` (Tier C); risk-register entry `R-0004` retires at that point |

### Signing verification invariants (V0 floor)

The following invariants must hold structurally at V0 — not as comments, but as code paths that enforce them:

1. **Fail-closed on load.** If signature verification fails (malformed signature, unknown key, cert-chain break), the plugin load is rejected and the host returns an error. No "best-effort load" path.
2. **No verify-async path.** A plugin instance is not created until `verify()` returns `Ok`. There is no background task that runs verification after the instance is handed to the caller.
3. **`core: true` only.** At V0, the runtime rejects any plugin whose manifest does not carry `core: true` signed by the mnemra root. Non-core plugin installation is blocked at the load path, not at a policy layer.
4. **File-mode invariant check at startup.** On host startup, the admin-token and signing-verification-material file modes are checked. If a file is world-readable, the host fails to start. This check mirrors the `DS-admin-token`/I mitigation in the trust-boundary model.

### Trip-wire activation (V0 → hardening)

The multi-deployment trip-wire is defined as: **the moment mnemra-core is deployed beyond the maintainer's single dogfood instance.** When the trip-wire fires:

1. `{{P-SigningKeyCustodyHardening}}` (Tier C) must be authored before the second deployment proceeds.
2. `R-0004` in the accepted-risk register retires.
3. The hardening slot covers: offline-root pattern, HSM-backed key, never-on-deployment-node, transparency log (sigsum or rekor-style), build-pipeline attestation integrity.

`{{P-SigningKeyCustodyHardening}}` is cited here as a placeholder (Tier C ADR slot, not yet authored). It is not a file link — the ADR does not exist until the trip-wire fires.

### Consequences

**Good:**
- V0 build pipeline has a concrete, named custody decision. Signed `core: true` plugin artifacts are achievable for `0.1.0`.
- `DS-mnemra-root-key`/I (Critical) is mitigated for the V0 dogfood position: key material does not live on the deployment node; only verification material is embedded in the binary.
- `P-plugin-runtime`/E (Critical) is mitigated structurally: synchronous fail-closed verification eliminates the async-race attack surface.
- `R-0004` is a named, bounded accepted risk: single-deployment dogfood scope, explicit trip-wire, clear retirement condition.
- The trip-wire structure (per `P-Defer`) prevents the V0 decision from silently extending to multi-deployment.

**Bad / Trade-offs:**
- Key-on-build-host and key-on-deployment-host are often the same machine for a single-developer setup. The structural separation (naming the build host as the custody surface, not the deployment node) matters for when they diverge — but for V0 single-developer dogfood, the practical isolation is limited. `R-0004` acknowledges this.
- Embedding verification material in the binary means a root key rotation requires a new binary release. For V0 dogfood this is acceptable; at production scale it is a forcing function toward the hardened model in `{{P-SigningKeyCustodyHardening}}`.

## Pros and Cons of the Options

### Option A — Key on build host (on-disk, mode 600, build-host-only)

- Pro: Signing surface is separated from runtime surface; deployment node compromise cannot recover the signing key.
- Pro: Synchronous fail-closed verification is structurally consistent with this custody model (no runtime key fetch path to race against).
- Pro: Satisfies `P-Defer`: V0 mechanism named, trip-wire stated.
- Con: For single-developer V0 dogfood, build host and deployment node are often the same machine — the structural separation is aspirational at this scale.
- Con: Root key rotation requires a new binary release (verification material embedded at build time).

### Option B — Key on deployment node (on-disk, mode 600)

- Con: Conflates signing authority with runtime; `DS-mnemra-root-key`/I (Critical) blast radius extends to future signing, not just current artifacts.
- Con: Any host-read primitive (privilege escalation, backup-tooling leak) on the deployment node recovers the root key.
- Con: Normalizes a pattern (`key-on-deployment-node`) that `{{P-SigningKeyCustodyHardening}}` explicitly prohibits; V0 posture bleeds into production framing.

### Option C — No signing at V0

- Con: Hard constraint violation — product brief commits to signed `core: true` plugins.
- Con: No structural enforcement of plugin provenance at V0; any WASM module could be loaded as a `core: true` plugin.
- Con: `P-plugin-runtime`/E (Critical) mitigation disappears: without signature verification, there is no structural barrier against a tampered plugin executing before an eventual check.

## Amendment 2026-05-24 — Core-signature binding locked at V0

The maintainer locked an explicit invariant during the WS-E-2 spec-exit gate review (2026-05-24): the runtime SHALL determine `core` status by signature provenance, not by manifest-field trust. The `core: true` flag is honored ONLY when the manifest signature chains to the mnemra root key/certificate.

This amendment narrows the V0SigningChain mechanism to lock the binding at V0 rather than treating it as a V0.1+ concern. The amendment is sanctioned because the binding is intrinsic to the core-plugin identity (mnemra-team-defined) and not part of the V0.1+ non-core-plugin-installation surface. The V0 substrate Spec (R-0005-h, `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`) carries this as a SHALL.

This amendment does NOT change the V0 signing key custody mechanism (build-host-on-disk, mode 600, multi-deployment trip-wire to P-SigningKeyCustodyHardening); it narrows a previously-implicit semantic to be explicit.

## More Information

- Frame doc open ADR slot: `{{P-V0SigningChain}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table).
- Tier-C complement (not yet authored): `{{P-SigningKeyCustodyHardening}}` — activated by the multi-deployment trip-wire defined in this ADR.
- Threat references: `DS-mnemra-root-key`/I,T; `EE-mnemra-root`/S,R; `P-plugin-runtime`/E,T; `DF-signing-attest`/T; `DF-key-custody`/I; `TB-build-pipeline`/`TB-mnemra-host` trust boundary ([Overview](../architecture/overview.md)).
- Accepted risk `R-0004` in overview: V0 signing-key custody is dogfood-scoped under this ADR; retires when `{{P-SigningKeyCustodyHardening}}` locks.
- Stage 2a direction H1: split `{{P-SigningKeyCustody}}` into Tier-A `{{P-V0SigningChain}}` and Tier-C `{{P-SigningKeyCustodyHardening}}`. This ADR locks the Tier-A half.
