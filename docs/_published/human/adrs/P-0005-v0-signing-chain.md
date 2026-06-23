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

Mnemra-core's `0.1.0` substrate build pipeline has to emit signed `core: true` plugin artifacts. At runtime, those signatures must be verified synchronously before any plugin instance loads. That forces a concrete custody decision for the mnemra root signing key before the substrate build starts. The build pipeline can't sign artifacts while the custody decision is still open.

The Frame document (the Stage 2 output that captures operating constraints and their rationale) carries a Tier-A table. That table splits the original `{{P-SigningKeyCustody}}` slot, which used to sit at Tier C, into two:

- **`{{P-V0SigningChain}}`** (Tier A) is the minimum-viable V0 custody position that lets `0.1.0` ship signed plugins.
- **`{{P-SigningKeyCustodyHardening}}`** (Tier C) is the production-grade hardening (HSM-backed, runtime-fetch, never-on-node), activated by the multi-deployment trip-wire.

This ADR (Architecture Decision Record: a record of one significant decision, its context, the rejected alternatives, and its consequences) locks the Tier-A half. The question is narrow. What's the smallest custody decision that (a) lets V0 build-pipeline signing work, (b) makes the V0 risk explicit and bounded, and (c) structurally prevents the decision from silently extending to multi-deployment use?

The security reviewer classified `DS-mnemra-root-key`/I as **Critical** (severity 85). If the key material leaks from the deployment node, an attacker can sign forged `core: true` plugins. The V0 position has to be named, not just acknowledged.

## Decision Drivers

- **V0 must ship signed plugins.** The product brief's Hard constraints commit to signed `core: true` plugins in `0.1.0`. Leaving the custody decision open blocks the build pipeline.
- **[P-Defer](../glossary.md#p-defer) (defer mechanism choice until evidence forces it): name the V0 mechanism AND its trip-wire.** The workspace principle requires that any open or deferred custody decision name both the V0 operating mechanism and the condition that forces the hardened decision. Silence isn't a V0 mechanism.
- **Single-deployment dogfood scope.** V0 is the maintainer's single deployment. The custody risk profile of a single, maintainer-controlled instance is categorically different from multi-deployment.
- **Synchronous verification is non-negotiable.** The security reviewer flagged `P-plugin-runtime`/E as Critical (severity 70). A "verify-async" or "defer to background" path that allows execution before verification finishes is unacceptable. Fail-closed is structural.
- **The trip-wire is structural, not advisory.** The moment mnemra-core deploys beyond the maintainer's single dogfood instance, `{{P-SigningKeyCustodyHardening}}` must lock. This isn't a recommendation. It's the condition that retires `R-0004` from the accepted-risk register. (`R-0004` is an R-code, a stable identifier for a numbered entry whose full text lives in the source document that defines it.)

## Considered Options

1. **Option A: V0 key on build host (on-disk, mode 600, build-host-only).** The mnemra root signing key lives on the build host's filesystem at mode 600. Signing happens at build time. The runtime on the deployment node receives only signed artifacts and the verification key (or cert chain). Key material never flows to the deployment node. The multi-deployment trip-wire is explicit: `{{P-SigningKeyCustodyHardening}}` locks before any deployment beyond the maintainer's dogfood instance.

2. **Option B: Key on deployment node (on-disk, mode 600).** The signing key lives on the deployment node alongside the runtime, which enables in-process or admin-API-triggered re-signing. Simpler initial setup for a single-node deployment.

3. **Option C: Deferred, no key, no signing at V0.** Plugin verification stays disabled until `{{P-SigningKeyCustodyHardening}}` is authored. V0 ships unsigned plugins, and the runtime skips signature checks.

## Decision Outcome

**Option A**: build-host-on-disk, mode 600, build-host-only. Key material stays on the build host. The deployment node receives signed artifacts and verification material only.

**Rationale:** Option A separates the signing surface from the runtime surface. Even at V0 dogfood scale, keeping the signing key off the deployment node is a structural control. It limits the blast radius of a deployment-node compromise to the artifacts that are already signed, not to future signing authority. The custody position is named and bounded, and `R-0004` records the residual risk explicitly.

Option B (key on deployment node) conflates the signing surface with the runtime surface. Any host-read primitive on the deployment node recovers the signing key (`DS-mnemra-root-key`/I, Critical). For V0 dogfood the practical difference from Option A is small, because the deployment node is also the build host in single-developer setups. But the principle matters. The design must not silently normalize key-on-deployment-node as acceptable.

Option C (no signing at V0) violates a Hard constraint. The product brief commits to signed `core: true` plugins. Disabling verification isn't a valid V0 posture.

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

These invariants have to hold structurally at V0, not as comments but as code paths that enforce them:

1. **Fail-closed on load.** If signature verification fails (malformed signature, unknown key, cert-chain break), the plugin load is rejected and the host returns an error. There's no "best-effort load" path.
2. **No verify-async path.** A plugin instance isn't created until `verify()` returns `Ok`. There's no background task that runs verification after the instance is handed to the caller.
3. **`core: true` only.** At V0, the runtime rejects any plugin whose manifest doesn't carry `core: true` signed by the mnemra root. Non-core plugin installation is blocked at the load path, not at a policy layer.
4. **File-mode invariant check at startup.** On host startup, the admin-token and signing-verification-material file modes are checked. If a file is world-readable, the host fails to start. This check mirrors the `DS-admin-token`/I mitigation in the trust-boundary model.

### Trip-wire activation (V0 to hardening)

The multi-deployment trip-wire is defined as **the moment mnemra-core is deployed beyond the maintainer's single dogfood instance.** When the trip-wire fires:

1. `{{P-SigningKeyCustodyHardening}}` (Tier C) must be authored before the second deployment proceeds.
2. `R-0004` in the accepted-risk register retires.
3. The hardening slot covers: offline-root pattern, HSM-backed key, never-on-deployment-node, transparency log (sigsum or rekor-style), and build-pipeline attestation integrity.

`{{P-SigningKeyCustodyHardening}}` is cited here as a placeholder (a Tier-C ADR slot, not yet authored). It isn't a file link. The ADR doesn't exist until the trip-wire fires.

### Consequences

**Good:**
- The V0 build pipeline has a concrete, named custody decision. Signed `core: true` plugin artifacts are achievable for `0.1.0`.
- `DS-mnemra-root-key`/I (Critical) is mitigated for the V0 dogfood position. Key material doesn't live on the deployment node; only verification material is embedded in the binary.
- `P-plugin-runtime`/E (Critical) is mitigated structurally. Synchronous fail-closed verification removes the async-race attack surface.
- `R-0004` is a named, bounded accepted risk: single-deployment dogfood scope, explicit trip-wire, clear retirement condition.
- The trip-wire structure (per `P-Defer`) prevents the V0 decision from silently extending to multi-deployment.

**Bad / Trade-offs:**
- Key-on-build-host and key-on-deployment-host are often the same machine for a single-developer setup. The structural separation (naming the build host as the custody surface, not the deployment node) matters for when they diverge. For V0 single-developer dogfood, the practical isolation is limited. `R-0004` acknowledges this.
- Embedding verification material in the binary means a root key rotation requires a new binary release. For V0 dogfood that's acceptable. At production scale it's a forcing function toward the hardened model in `{{P-SigningKeyCustodyHardening}}`.

## Pros and Cons of the Options

### Option A — Key on build host (on-disk, mode 600, build-host-only)

- Pro: The signing surface is separated from the runtime surface. A deployment-node compromise can't recover the signing key.
- Pro: Synchronous fail-closed verification is structurally consistent with this custody model. There's no runtime key-fetch path to race against.
- Pro: Satisfies `P-Defer`. The V0 mechanism is named and the trip-wire is stated.
- Con: For single-developer V0 dogfood, the build host and the deployment node are often the same machine, so the structural separation is aspirational at this scale.
- Con: Root key rotation requires a new binary release, since verification material is embedded at build time.

### Option B — Key on deployment node (on-disk, mode 600)

- Con: Conflates signing authority with runtime. The `DS-mnemra-root-key`/I (Critical) blast radius extends to future signing, not just current artifacts.
- Con: Any host-read primitive (privilege escalation, a backup-tooling leak) on the deployment node recovers the root key.
- Con: Normalizes a pattern (`key-on-deployment-node`) that `{{P-SigningKeyCustodyHardening}}` explicitly prohibits. The V0 posture bleeds into production framing.

### Option C — No signing at V0

- Con: A Hard constraint violation. The product brief commits to signed `core: true` plugins.
- Con: No structural enforcement of plugin provenance at V0. Any WASM module could be loaded as a `core: true` plugin.
- Con: The `P-plugin-runtime`/E (Critical) mitigation disappears. Without signature verification there's no structural barrier against a tampered plugin executing before an eventual check.

## Amendment 2026-05-24 — Core-signature binding locked at V0

The maintainer locked an explicit invariant during the WS-E-2 spec-exit gate review (2026-05-24): the runtime SHALL determine `core` status by signature provenance, not by manifest-field trust. The `core: true` flag is honored ONLY when the manifest signature chains to the mnemra root key or certificate.

This amendment narrows the V0SigningChain mechanism to lock the binding at V0 instead of treating it as a V0.1+ concern. The amendment is sanctioned because the binding is intrinsic to the core-plugin identity (mnemra-team-defined) and not part of the V0.1+ non-core-plugin-installation surface. The V0 substrate Spec (R-0005-h, `docs/specs/2026-05-24-mnemra-core-v0-substrate.md`) carries this as a SHALL.

This amendment does NOT change the V0 signing key custody mechanism (build-host-on-disk, mode 600, multi-deployment trip-wire to P-SigningKeyCustodyHardening). It narrows a previously-implicit semantic to be explicit.

## More Information

- Frame doc open ADR slot: `{{P-V0SigningChain}}` ([Frame](../intent/mnemra-core-frame.md), Tier A table).
- Tier-C complement (not yet authored): `{{P-SigningKeyCustodyHardening}}`, activated by the multi-deployment trip-wire defined in this ADR.
- Threat references: `DS-mnemra-root-key`/I,T; `EE-mnemra-root`/S,R; `P-plugin-runtime`/E,T; `DF-signing-attest`/T; `DF-key-custody`/I; `TB-build-pipeline`/`TB-mnemra-host` trust boundary ([Overview](../architecture/overview.md)).
- Accepted risk `R-0004` in overview: V0 signing-key custody is dogfood-scoped under this ADR; it retires when `{{P-SigningKeyCustodyHardening}}` locks.
- Stage 2a direction H1: split `{{P-SigningKeyCustody}}` into Tier-A `{{P-V0SigningChain}}` and Tier-C `{{P-SigningKeyCustodyHardening}}`. This ADR locks the Tier-A half.
