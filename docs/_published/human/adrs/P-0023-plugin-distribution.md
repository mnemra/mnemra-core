---
title: "P-0023: Plugin Distribution — OCI bundle store, keyed-in-tree package signing, verified-fetch pipeline"
summary: "The distribution layer's contract decisions (W2-1): every plugin is a uniform multi-artifact OCI bundle (N≥1, wasm = layers[0], signed inner manifest as config descriptor, one flat manifest); the package signature is the P-0005 Ed25519 root over the domain-separated outer-manifest digest, attached as an OCI 1.1 referrer, verified under signer-key-pinning; two R-NoExternalHost transports (image-layout filesystem, self-hosted distribution API) behind one store contract; the bounds-first pre-phase ordering (fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds) as a distribution pre-phase ahead of P-0019 D6 state 1; the PackageVerifier seam locked now with the TUF adapter deferred behind it (R-0005-e); hard cutover with no legacy accept path; single-root custody exposure recorded (split at R-0005-e). Resolves the plugin-distribution Frame {{P-PluginDistribution}} slot; pulls P-0019 DEF-2's single-publisher scope ahead of its tripwire and supersedes its cosign-on-wkg shape."
primary-audience: agent
---

---
status: "accepted"
date: "2026-07-07"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator", "the researcher", "the security reviewer"]
informed: []
supersedes: null
superseded_by: null
---

# P-0023: Plugin Distribution — OCI bundle store, keyed-in-tree package signing, verified-fetch pipeline

**Project:** mnemra-core

## Status

`accepted` (2026-07-11), locked at the plugin-distribution spec-exit gate. It came in as part of the Stage 3 package (the [Spec](../glossary.md#spec) stage) that the gate reviewed next to [the cluster spec](../../specs/2026-07-07-plugin-distribution.md), following the P-0022 precedent. The decisions this record captures (H1, R-NoExternalHost, Decision A, Q2/Q3/Q5/Q7, and the 2026-07-07 pre-gate ratifications) were locked by the maintainer before the record existed. The gate ratified the *record*, not the decisions.

**A note on numbering.** P-0022 (the coordination cluster) landed on `main` 2026-07-07 (commit `8f27cc5`), after this ADR's authoring branch had already cut from its original merge-base (`451f53b`). The branch has since rebased onto it, so both records show up in this listing. P-0023 was the next free number. No renumber was needed at the rebase.

## Context and Problem Statement

Until this decision, a mnemra plugin existed only as files loaded from known local paths. Two checks gated it: the inner Ed25519 manifest signature ([P-0005](P-0005-v0-signing-chain.md)), and the R-0021 BLAKE3 content-hash over the `.wasm` component ([signing-to-runnable spec](../../specs/2026-06-30-signing-to-runnable.md)). R-0021 is one of the project's numbered requirement identifiers ([R-codes](../glossary.md#r-codes)). The **distributable unit did not exist**. There was no packaging shape, the package as a whole was unsigned, and the non-wasm artifacts (data, assets, schemas, secondary components) were each individually uncovered. That left a live per-artifact substitution and tamper hole, the gap the 2026-07-01 V0 signing ceremony named (tasks #1943 and #1944).

The job is narrow. A consumer establishes provenance and integrity of the **whole bundle before unpacking**, from a filesystem or a self-hosted registry, with zero required round-trip to a host we don't control. Then every blob is re-verified at the provenance anchor at load time. The governing chain of locked, [designed](../glossary.md#designed)-tier work (the tier at which the permanent "what to build" is complete) runs through four artifacts: the locked intake ([`docs/intent/plugin-distribution.md`](../../intent/plugin-distribution.md), blob `9c8e1577`) from the [Intake](../glossary.md#intake) stage, the locked [Frame](../glossary.md#frame) ([`docs/intent/plugin-distribution-frame.md`](../../intent/plugin-distribution-frame.md), blob `60c437c5`), the locked research survey (2026-07-01, r3), and the Stage 3 spec ([`docs/specs/2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md), R-0078 through R-0092). This ADR records the distribution layer's **contract decisions** at ADR precision and **cites the spec for requirement-level detail**, following the P-0019 pattern where the capstone cites rather than restates. It doesn't repeat the R-IDs.

Three maintainer decisions were locked 2026-07-01. They're the fixed frame, not options weighed here. **H1**: a uniform multi-artifact OCI bundle, N≥1, with wasm as artifact #1 and no bare load path (the *path* is banned, not N=1 content). **R-NoExternalHost**: filesystem and self-hosted-server self-hostability, with zero uncontrolled-host round-trip. **Decision A**: the inner signed manifest binds every blob through an `[[artifacts]]` N≥1 list, and each blob is verified with R-0021's single-read, complete-mediation, fail-closed discipline at the provenance anchor. Four shape questions were maintainer-ratified 2026-07-07: Q7 (hard cutover), Q5 (single-root custody), Q2 (one flat manifest), and Q3 (seam-now, TUF-later). Five spec-tier mechanism choices were ratified at the 2026-07-07 pre-gate ruling: the bound floors, uncompressed-only blobs, config-descriptor placement, the `vnd.mnemra.artifact.id` join key, and strict `[component]` rejection.

## Decision Drivers

- **`P-SecurityLayered`** (security enforced in independent layers). The distribution anchor adds a layer over the existing provenance-at-load gates. The layers are scope-independent. The compromise-independence caveat under single-root reuse is recorded, not hidden; see D6 below. Fail-closed is structural at every stage. And the fetch/unpack availability surface (its bounds) is part of the layer, not an afterthought.
- **`P-StackDiscipline` / Simplicity** (stay within the chosen stack; keep the mechanism count low). Keyed-in-tree signing reuses the in-tree `ed25519-dalek` root. That means zero new signing dependency and no Go-CLI (`cosign`) foreign-ecosystem cost, the same reuse logic R-0021 applied to in-tree BLAKE3. OCI's image-layout is identical to its registry structure, so one abstraction serves both transports.
- **[`P-LockContract`](../glossary.md#p-lockcontract)** (lock the contract, vary the implementation), plus the when-to-lock discipline. The `PackageVerifier` seam is intrinsic to the layer's identity, so it locks now. The TUF adapter is a separable future option deferred behind it. This follows the P-0010 D5 precedent of an engine-agnostic seam.
- **[`P-Defer`](../glossary.md#p-defer)** (defer a mechanism choice until evidence forces it), on fireable tripwires. Four mechanisms are deferred: the TUF mechanism, the distribution-key split, sigstore keyless, and SLSA attestations. The first three wait behind **R-0005-e** ([P-0005](P-0005-v0-signing-chain.md)'s multi-deployment condition, co-fired by third-party publishers); SLSA waits on the reproducible-builds work item. Each carries its decision content in the Frame's deferral table, and each is self-announcing.
- **[`P-PreserveDecisionSpace`](../glossary.md#p-preservedecisionspace)** (keep the rejected options visible). The rejected store and signing options are recorded below with reasons. The image-index forward shape and the compression amendment path are named, not erased.
- **R-0005-h** (core-by-provenance). Every per-artifact identifier lives in the signed canonical body. That residence constraint shaped the `[[artifacts]]` schema ([P-0003](P-0003-plugin-manifest.md) §Amendment 2026-07-07).

## Considered Options

**Store standard** (research-locked 2026-07-01; recorded per `P-PreserveDecisionSpace`):

1. **OCI artifacts, authored directly (chosen).** This is the image-spec 1.1 manifest with the distribution API and image-layout, the ORAS-style arbitrary-artifact path.
2. **warg / wasm-pkg registry (rejected).** Two problems. It's near maintenance-death (the ecosystem converged on OCI, and the crates have been stale since 2025-07), and it has an intrinsic mismatch with the uniform bundle (its unit is a component-package, with no home for non-wasm artifacts). Its signed-append-log concept is reachable through TUF later.
3. **Bespoke CAS / zip-over-HTTP (rejected).** It reinvents the filesystem-plus-web abstraction, signature attachment, caching, and tooling that OCI already standardizes with Green in-stack crates. Kept only as a documented fallback of last resort.

**Signing mechanism** (research-locked; Q5 ratified):

4. **Keyed-in-tree (chosen).** The P-0005 root signs the manifest digest, verified in Rust.
5. **cosign/sigstore keyless (rejected at this tier).** Public keyless fails R-NoExternalHost as shipped. `sigstore-rs` is verify-only and pre-1.0, and its signing side is a Go CLI (`P-StackDiscipline` S2). Self-hosted keyless is the recorded heavy-lift forward-allowance on R-0005-e.

**Seam posture** (Q3): **6. Lock the `PackageVerifier` seam now, defer TUF (chosen)** versus **7. Defer the seam too (rejected).** If the seam were deferred too, TUF would arrive as rework instead of an adapter.

**Cutover posture** (Q7): **8. Hard cutover (chosen)** versus **9. Bounded dual-accept window (rejected).** A legacy accept path is the bare path reborn through deployment, which would defeat H1 by way of migration compatibility.

## Decision Outcome

Six locked decisions follow. Requirement-level acceptance criteria live in the spec, with R-IDs cited per decision. This ADR doesn't duplicate them.

### D1 — Packaging shape: uniform OCI bundle, one flat manifest, config-descriptor inner manifest

Every plugin ships as one **OCI image manifest** with `artifactType = application/vnd.mnemra.plugin.v1`. The **config descriptor** carries the **signed inner TOML manifest** (`application/vnd.mnemra.plugin.manifest.v1+toml`). The **layers** are the N≥1 artifacts, with **`layers[0]` set to the component `.wasm`** (`application/wasm`). Each layer carries the `vnd.mnemra.artifact.id` annotation that joins it to its inner `[[artifacts]]` entry. The manifest is flat. An image index is rejected at this tier; the index is the recorded forward shape if secondary components ever need independent addressing or signing. Blobs are **uncompressed at V0** (compression suffixes are rejected, and admitting compression later is a spec amendment that carries decompression caps). There is **no bare load path**. *Anchors: H1; Q2; the pre-gate ratifications of 2026-07-07; Simplicity.* *(Spec: R-0078, R-0079.)*

### D2 — Package signing: domain-separated keyed-in-tree signature as an OCI 1.1 referrer, signer-key-pinned

The package signature is the **P-0005 Ed25519 root** over `"mnemra-oci-manifest-v1:" || <alg>:<hex>`. That's the outer-manifest digest, domain-separated, so the root never signs a bare digest in this domain. It's attached as an **OCI 1.1 referrer** (`application/vnd.mnemra.signature.v1`, with subject set to the bundle manifest). Verification runs in Rust on fetch under **signer-key-pinning**: enumerate the referrers (bounded), accept only the pinned root, require at least one valid signature, and fail closed on zero. Never trust-on-first-use, and never an unsigned fall-through. Key custody is unchanged from P-0005: it stays on the build-host at mode 600, and the deployment binary carries verification material only. *Anchors: research §5; P-StackDiscipline S2; Simplicity; P-0005.* *(Spec: R-0080.)*

**Domain-separation hygiene note (one-sided prefix, recorded rationale).** The prefix protects the *new* message domain, the OCI-manifest digest. The *existing* inner domain, the canonical TOML body P-0005 already signs, stays unprefixed. Cross-domain confusion is closed in one direction by the prefix and in the other by format structure: a canonical TOML body can't begin with the prefix-then-digest byte sequence. That second leg rests on format shape rather than an explicit tag. It's accepted at this tier and recorded here so it stays a known asymmetry, not an oversight. If the asymmetry ever becomes load-bearing, the clean-up path is to add an inner-domain prefix at the next natural re-sign boundary.

### D3 — Store: one contract, two R-NoExternalHost transports

A single store contract fronts two transports. One is the **OCI image-layout filesystem** transport, a true air-gap over removable media or a local directory. The other is the **self-hosted OCI distribution-API registry** transport, for a restricted-egress LAN. Resolution for load is **digest-pinned**, never a mutable tag. Every digest is recomputed over the **received bytes**, so store-supplied digest claims are never trusted. No code path and no test requires an uncontrolled external host. `wkg` isn't the bundle path; it stays confined to WIT and component build-time dependency pulls. *Anchors: R-NoExternalHost; P-LockContract; research §2(a)/§5.* *(Spec: R-0081, R-0090.)*

### D4 — The verified-fetch pipeline: bounds-first canonical ordering behind the `PackageVerifier` seam

The distribution pre-phase has a **canonical ordering**, single-sourced here, with [P-0019](P-0019-plugin-contract.md) D6 cross-referencing it:

> **fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds**

**Every read is gated by its bound before the read feeds a verifier.** Metadata caps come ahead of the outer-manifest and referrers reads; size and N caps come ahead of blob reads. Every stage fails closed, and no later stage is reachable without its predecessor. The pre-phase completes ahead of D6 state 1 ("Discovered"), and D6 states 2 through 9 run unchanged. The existing inner gates change in *coverage*, now covering all blobs per Decision A and the [P-0003 `[[artifacts]]` amendment](P-0003-plugin-manifest.md), never in *primitive*. The pipeline sits behind the **`PackageVerifier` seam**. That seam is the single chokepoint by which a bundle becomes load-eligible, and it's the locked slot behind which the TUF adapter composes when R-0005-e fires. Fetch and unpack resource bounds exist on the metadata and blob dimensions, with conservative, config-tunable floors (ratified as set on 2026-07-07). *Anchors: Q3; P-LockContract plus the P-0010 D5 seam precedent; P-SecurityLayered; P-0019 D6.* *(Spec: R-0083, R-0084, R-0087, R-0088.)*

**Mediated-access rule (recorded).** Consumers obtain artifact bytes **only** through the load-time mediation gate. That gate is single-read and dual-digest: the inner BLAKE3 at the provenance anchor is primary, and the outer sha256 content-address is the complementary distribution-anchor check, kept on algorithm and implementation-diversity grounds. Direct reads of the unpacked layout or the blob cache are **out of contract**. *(Spec: R-0087-b.)*

### D5 — Hard cutover: one atomic flip, no legacy accept path

The uniform-packaging invariant lands as **one change**. The existing `core: true` plugin set is re-packaged and re-signed (with `[[artifacts]]` manifests) in the same change that activates the bundle-only loader. No commit range accepts both shapes. No flag, feature, or code path re-enables a pre-OCI load. Pre-OCI load tests are retired or converted to rejection tests in the same change. After cutover, a signed manifest carrying the legacy `[component]` table is rejected, because one binding schema is in force (ratified 2026-07-07). *Anchors: Q7; H1.* *(Spec: R-0086-e, R-0089.)*

### D6 — Single-root custody at this tier: the recorded exposure

At this tier, the package signature (the distribution anchor) and the inner manifest signature (the provenance anchor) are both made by the **one P-0005 root**. So the layers are **scope-independent but NOT compromise-independent**. A single root-key theft forges both at once, and detecting a stolen-key forgery needs a transparency-log or TUF witness, which is deferred. This is a deliberate, maintainer-ratified trade (Q5, one custody story at single-publisher `core: true` scope, on the axis between Security and Simplicity). It's recorded here **so it is never silent**. The accepted-risk register carries it as `R-0009`, with the R-0005-e retirement condition: split to a distinct distribution key, or move to TUF delegated roles. *Anchors: Q5; P-SecurityLayered (honest accounting); P-LeastAuthority (grant only the authority needed, and the concentration is the named exposure); P-0005 R-0005-e.*

### Deferrals

All four deferrals ride tripwires. Full decision content is recorded in the Frame's deferral table (blob `60c437c5`) and the spec's Out-of-scope section, cited here rather than restated. The **TUF mechanism** waits on R-0005-e (the seam is its slot). The **distribution-key split** waits on R-0005-e (it restores compromise-independence). **sigstore keyless** waits on R-0005-e (the self-hosted variant for R-NoExternalHost). **SLSA/in-toto attestations** wait on the reproducible-builds landing; they're the answer to the build-time dependency-confusion residual (register `R-0010`). The Tier-C custody-hardening slot `{{P-SigningKeyCustodyHardening}}` stays unauthored and untouched.

### Consequences

**Good:**

- The per-artifact substitution hole closes at **both** anchors. The signed OCI manifest digest-pins every blob at the distribution anchor, and the inner `[[artifacts]]` gate complete-mediates every blob at the provenance anchor. That closes the fetch-to-load TOCTOU for non-wasm blobs for the first time.
- Both R-NoExternalHost cases (the air-gap filesystem and the restricted-egress LAN registry) are first-class from one abstraction, with zero new signing dependencies.
- The bounds-first ordering makes the availability surface (oversized metadata or blobs, bomb-shaped inputs) fail closed **before** the integrity gates can be size-attacked. And because it's single-sourced here, a future refactor that reorders bounds after digests visibly violates this ADR.
- The seam-now, TUF-later split means rollback protection arrives as an adapter, not as rework.
- Install is a **working-state → working-state** transition. No failure, crash, or kill point leaves the host's serving or load-eligible state indeterminate, and the residue of a partial or killed attempt is inert. This comes from the single verification chokepoint (D4) plus load-time re-verification, not from a bespoke transaction. *(Spec: R-0092.)* The system-wide principle is routed to canon separately (#2248).

**Bad / Trade-offs:**

- **Rollback and downgrade are an accepted residual** until R-0005-e (register `R-0008`). A stale but validly signed bundle still verifies. It's named, trip-wired, and not silent.
- **Single-root compromise-independence** (D6, register `R-0009`). Honest accounting is the only mitigation at this tier, and the split is deferred.
- **Build-time dependency confusion** (register `R-0010`). R-0021 binds bytes-run to bytes-signed, which is integrity, not provenance of inputs. The SLSA answer waits on reproducible builds.
- The hard cutover makes the flip change larger, since it re-packages, re-signs, and retires tests in one change. That's accepted to keep the bare path from surviving as a migration window.

## Pros and Cons of the Options

### OCI artifacts, authored directly (chosen)

- Pro: a native N-blob-per-manifest bundle; image-layout equals registry structure, so one abstraction covers both R-NoExternalHost transports; referrer-attached signatures; and mature Green Rust clients (`oci-client`, `oci-spec`).
- Con: it's genuinely weak on rollback and rotation, which is exactly why the TUF deferral exists; and `ocidir`'s thinner maturity requires the L3 layout-write-path audit before adoption (a spec acceptance gate).

### warg / wasm-pkg registry (rejected)

- Pro: a signed append-only per-package log, which gives strong rollback and transparency.
- Con: it's no longer actively developed, since the ecosystem converged on OCI; and its unit is a component-package with no home for non-wasm artifacts, an intrinsic mismatch with the uniform-bundle contract.

### Bespoke CAS / zip (rejected)

- Pro: zero new concepts, and it reuses in-tree BLAKE3.
- Con: it reinvents everything OCI standardizes, which is more surface rather than less under Simplicity's least-standing-mechanism reading; a fallback of last resort only.

### Keyed-in-tree signing (chosen) vs cosign/sigstore keyless (rejected at this tier)

- Pro (keyed): zero new dependency; no external host, so it's R-NoExternalHost-clean; Rust-native verify; and one custody story at single-publisher scope.
- Con (keyed): no public transparency, and compromise-independence is deferred (D6). Pro (keyless): short-lived identities and public auditability. Con (keyless): the public infrastructure fails R-NoExternalHost as shipped; Rust signing doesn't exist, so it's a Go CLI with the S2 cost; and self-hosted Fulcio/Rekor is a heavy operational lift, the recorded forward-allowance.

## More Information

- **Resolves** the plugin-distribution Frame's `{{P-PluginDistribution}}` slot ([Frame](../../intent/plugin-distribution-frame.md), blob `60c437c5`, the ADR landing map).
- **Spec:** [`docs/specs/2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md), covering R-0078 through R-0092 (R-0082 is skipped; see the spec's numbering note), the `[[artifacts]]` schema detail, the error grammar, and the numeric floors.
- **Sibling authority:** [P-0003](P-0003-plugin-manifest.md) owns the manifest schema (§Amendment 2026-07-07, the `[[artifacts]]` binding). [P-0019](P-0019-plugin-contract.md) owns the lifecycle capstone (DEF-2 was dispositioned 2026-07-07, and its D6 cross-references this ADR's pre-phase). [P-0005](P-0005-v0-signing-chain.md) owns the root key, custody, and the R-0005-e trip-wire. [P-0007](P-0007-plugin-resource-limits.md) owns execution-time limits, and this ADR's bounds are the fetch and unpack-time sibling, distinct by design.
- **Threat references:** typed elements per the [architecture overview's distribution extension](../architecture/overview.md#trust-boundaries) (2026-07-07): `EE-plugin-store`, `P-bundle-builder`, `P-fetch-verify`, `DS-oci-store`, `DS-bundle-cache`, `DF-publish`, `DF-fetch`, `DF-referrers`, and the `TB-plugin-store` boundary row. The STRIDE-per-element rows land at this cluster's pre-implementation security review, following the P-0014 typed-DFD-extension precedent. Accepted risks are `R-0008` (rollback residual), `R-0009` (single-root exposure), and `R-0010` (build-time dependency confusion); see overview §Accepted risks.
- **Research:** the plugin artifact-repository and package-signing standards survey (locked 2026-07-01, r3), covering the store and signing landscape and the ten load-path invariants this design carries.
