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

`accepted` (2026-07-11), locked at the plugin-distribution spec-exit gate. That gate is the human sign-off that closes Stage 3, the [Spec](../glossary.md#spec) stage of the work-shaping pipeline. It went through the gate as part of the Stage-3 package, reviewed alongside [the cluster spec](../../specs/2026-07-07-plugin-distribution.md). That pairing follows the precedent set by P-0022, a project-scoped [ADR](../glossary.md#p--adr). The decisions it records (H1, R-NoExternalHost, Decision A, Q2/Q3/Q5/Q7, the 2026-07-07 pre-gate ratifications) were maintainer-locked before this record. The gate ratified the *record*, not the decisions.

**A note on numbering.** P-0022 (coordination cluster) landed on `main` 2026-07-07 (commit `8f27cc5`), after this branch's merge-base (`451f53b`). It's committed canon, but it doesn't show up in this working tree's `docs/src/adrs/` listing. P-0023 is definitively the next free number. No renumber is needed at rebase.

## Context and Problem Statement

Until this decision, a mnemra plugin existed only as files loaded from known local paths, gated by the inner Ed25519 manifest signature ([P-0005](P-0005-v0-signing-chain.md)) and, over the `.wasm` component, the BLAKE3 content-hash recorded as R-0021 (an [R-code](../glossary.md#r-codes): a numbered, spec-defined requirement identifier) from the [signing-to-runnable spec](../../specs/2026-06-30-signing-to-runnable.md). The **distributable unit did not exist**. There was no packaging shape, the package as a whole was unsigned, and the non-wasm artifacts (data, assets, schemas, secondary components) were individually uncovered. That left a live per-artifact substitution and tamper hole, the gap the 2026-07-01 V0 signing ceremony named (tasks #1943/#1944).

The job is this. A consumer establishes provenance and integrity of the **whole bundle before unpacking**, from a filesystem or a self-hosted registry, with zero required round-trip to a host we don't control. Then every blob is re-verified at the provenance anchor at load. The governing [designed](../glossary.md#designed)-tier chain (the tier where a locked frame and a locked spec both exist) is four artifacts: the locked [intake](../glossary.md#intake) (Stage 1's validated intent, [`docs/intent/plugin-distribution.md`](../../intent/plugin-distribution.md), blob `9c8e1577`), the locked [Frame](../glossary.md#frame) (Stage 2's constraint summary, [`docs/intent/plugin-distribution-frame.md`](../../intent/plugin-distribution-frame.md), blob `60c437c5`), the locked research survey (2026-07-01, r3), and the Stage-3 spec ([`docs/specs/2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md), R-0078 through R-0092). This ADR records the distribution layer's **contract decisions** at ADR precision and **cites the spec for requirement-level detail**, the same capstone-that-cites division of authority P-0019 uses. It doesn't restate the R-IDs.

Three maintainer decisions were locked 2026-07-01, and they're the fixed frame, not options weighed here. **H1**: a uniform multi-artifact OCI bundle, N≥1, wasm = artifact #1, no bare load path (the *path* is banned, not N=1 content). **R-NoExternalHost**: filesystem AND self-hosted-server self-hostability, zero uncontrolled-host round-trip. **Decision A**: the inner signed manifest binds every blob via an `[[artifacts]]` N≥1 list, each verified with R-0021's single-read / complete-mediation / fail-closed discipline at the provenance anchor. Four shape questions were maintainer-ratified 2026-07-07 (Q7 hard cutover, Q5 single-root custody, Q2 one flat manifest, Q3 seam-now/TUF-later). Five spec-tier mechanism choices were ratified at the 2026-07-07 pre-gate ruling (bound floors, uncompressed-only, config-descriptor placement, the `vnd.mnemra.artifact.id` join key, and strict `[component]` rejection).

## Decision Drivers

- **`P-SecurityLayered`.** The distribution anchor adds a layer over the existing provenance-at-load gates. The layers are scope-independent (the compromise-independence caveat under single-root reuse is recorded, not hidden; see D6 below). Fail-closed is structural at every stage. The fetch/unpack availability surface (bounds) is part of the layer, not an afterthought.
- **`P-StackDiscipline` / Simplicity.** Keyed-in-tree signing reuses the in-tree `ed25519-dalek` root. That means zero new signing dependency and no Go-CLI (`cosign`) foreign-ecosystem cost, the same reuse logic R-0021 applied to in-tree BLAKE3. OCI's image-layout ≡ registry-structure identity serves both transports from one abstraction.
- **`P-LockContract` (+ the when-to-lock discipline).** [P-LockContract](../glossary.md#p-lockcontract) means lock the contract and vary the implementation. The `PackageVerifier` seam is intrinsic to the layer's identity and locks now. The TUF adapter is a separable future option deferred behind it, the P-0010 D5 engine-agnostic-seam precedent.
- **`P-Defer` (fireable tripwires).** [P-Defer](../glossary.md#p-defer) holds a mechanism choice until evidence forces it. TUF mechanism, distribution-key split, sigstore keyless, and SLSA attestations are deferred behind **R-0005-e** ([P-0005](P-0005-v0-signing-chain.md)'s multi-deployment condition, co-fired by third-party publishers) and the reproducible-builds work item respectively. Each carries its decision content in the Frame's deferral table, and each is self-announcing.
- **`P-PreserveDecisionSpace`.** [P-PreserveDecisionSpace](../glossary.md#p-preservedecisionspace) keeps the rejected options on the record. The rejected store and signing options are recorded below with reasons. The image-index forward shape and the compression amendment path are named, not erased.
- **R-0005-h (core-by-provenance).** Every per-artifact identifier lives in the signed canonical body. That residence constraint shaped the `[[artifacts]]` schema ([P-0003](P-0003-plugin-manifest.md) §Amendment 2026-07-07).

## Considered Options

**Store standard** (research-locked 2026-07-01; recorded per `P-PreserveDecisionSpace`):

1. **OCI artifacts, authored directly (chosen).** Image-spec 1.1 manifest plus distribution API plus image-layout, the ORAS-style arbitrary-artifact path.
2. **warg / wasm-pkg registry (rejected).** Two problems: maintenance-death (the ecosystem converged on OCI, and the crates have been stale since 2025-07) and an intrinsic uniform-bundle mismatch (the unit is a component-package, with no home for non-wasm artifacts). Its signed-append-log concept is obtainable via TUF later.
3. **Bespoke CAS / zip-over-HTTP (rejected).** Reinvents the fs+web abstraction, signature attachment, caching, and tooling that OCI standardizes with Green in-stack crates. Kept only as a documented fallback-of-last-resort.

**Signing mechanism** (research-locked; Q5 ratified):

4. **Keyed-in-tree: P-0005 root signs the manifest digest, verified in-Rust (chosen).**
5. **cosign/sigstore keyless (rejected at this tier).** Public keyless fails R-NoExternalHost as-shipped. `sigstore-rs` is verify-only and pre-1.0, and signing is a Go CLI (`P-StackDiscipline` S2). Self-hosted keyless is the recorded heavy-lift forward-allowance on R-0005-e.

**Seam posture** (Q3): **6. lock the `PackageVerifier` seam now, defer TUF (chosen)** versus **7. defer the seam too (rejected**, because TUF would then arrive as rework instead of an adapter).

**Cutover posture** (Q7): **8. hard cutover (chosen)** versus **9. bounded dual-accept window (rejected**, because a legacy accept path is the bare path reborn by deployment, defeating H1 through migration compatibility).

## Decision Outcome

Six locked decisions. Requirement-level acceptance criteria live in the spec, with R-IDs cited per decision. This ADR doesn't duplicate them.

### D1 — Packaging shape: uniform OCI bundle, one flat manifest, config-descriptor inner manifest

Every plugin ships as one **OCI image manifest** with `artifactType = application/vnd.mnemra.plugin.v1`. The **config descriptor** carries the **signed inner TOML manifest** (`application/vnd.mnemra.plugin.manifest.v1+toml`). The **layers** are the N≥1 artifacts, with **`layers[0]` = the component `.wasm`** (`application/wasm`), and each layer carries the `vnd.mnemra.artifact.id` annotation joining it to its inner `[[artifacts]]` entry. One flat manifest. An image index is rejected at this tier (the index is the recorded forward shape if secondary components ever need independent addressing or signing). Blobs are **uncompressed at V0** (compression suffixes rejected; admitting compression is a spec amendment carrying decompression caps). There is **no bare load path**. *Anchors: H1; Q2; pre-gate ratifications 2026-07-07; Simplicity.* *(Spec: R-0078, R-0079.)*

### D2 — Package signing: domain-separated keyed-in-tree signature as an OCI 1.1 referrer, signer-key-pinned

The package signature is the **P-0005 Ed25519 root** over `"mnemra-oci-manifest-v1:" || <alg>:<hex>` (the outer-manifest digest, domain-separated, so the root never signs a bare digest in this domain), attached as an **OCI 1.1 referrer** (`application/vnd.mnemra.signature.v1`, subject = the bundle manifest). Verification runs in-Rust on fetch under **signer-key-pinning**. Enumerate referrers (bounded), accept only the pinned root, require ≥1 valid, and fail closed on zero. Never trust-on-first-use, never an unsigned fall-through. Key custody is unchanged from P-0005 (build-host, mode 600; the deployment binary carries verification material only). *Anchors: research §5; P-StackDiscipline S2; Simplicity; P-0005.* *(Spec: R-0080.)*

**Domain-separation hygiene note (one-sided prefix, recorded rationale).** The prefix protects the *new* message domain (the OCI-manifest digest). The *existing* inner domain (the canonical TOML body P-0005 already signs) stays unprefixed. Cross-domain confusion is closed in one direction by the prefix and in the other by format structure: a canonical TOML body can't begin with the prefix-then-digest byte sequence. That second leg rests on format shape rather than an explicit tag. It's accepted at this tier and recorded here so it's a known asymmetry, not an oversight. Adding an inner-domain prefix at the next natural re-sign boundary is the clean-up path if the asymmetry ever becomes load-bearing.

### D3 — Store: one contract, two R-NoExternalHost transports

A single store contract fronts two transports. The first is the **OCI image-layout filesystem** transport (true air-gap: removable media, local directory). The second is the **self-hosted OCI distribution-API registry** transport (restricted-egress LAN). Resolution for load is **digest-pinned**, never a mutable tag, and every digest is recomputed over **received bytes** (store-supplied digest claims are never trusted). No code path or test requires an uncontrolled external host. `wkg` is not the bundle path; it stays confined to WIT/component build-time dependency pulls. *Anchors: R-NoExternalHost; P-LockContract; research §2(a)/§5.* *(Spec: R-0081, R-0090.)*

### D4 — The verified-fetch pipeline: bounds-first canonical ordering behind the `PackageVerifier` seam

The distribution pre-phase's **canonical ordering** (single-sourced here; [P-0019](P-0019-plugin-contract.md) D6 cross-references it) is:

> **fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds**

**Every read is gated by its bound before the read feeds a verifier** (metadata caps ahead of the outer-manifest/referrers reads, size/N caps ahead of blob reads). Each stage fails closed, and no later stage is reachable without its predecessor. The pre-phase completes ahead of D6 state 1 ("Discovered"), and D6 states 2 through 9 run unchanged. The existing inner gates change in *coverage* (all blobs, per Decision A and the [P-0003 `[[artifacts]]` amendment](P-0003-plugin-manifest.md)), never in *primitive*. The pipeline sits behind the **`PackageVerifier` seam**, the single chokepoint by which a bundle becomes load-eligible, and the locked slot behind which the TUF adapter composes when R-0005-e fires. Fetch/unpack resource bounds exist on the metadata and blob dimensions with conservative, config-tunable floors (ratified as-set 2026-07-07). *Anchors: Q3; P-LockContract + the P-0010 D5 seam precedent; P-SecurityLayered; P-0019 D6.* *(Spec: R-0083, R-0084, R-0087, R-0088.)*

**Mediated-access rule (recorded).** Consumers obtain artifact bytes **only** through the load-time mediation gate. It's single-read and dual-digest: inner BLAKE3 at the provenance anchor is primary, and outer sha256 content-address is the complementary distribution-anchor check, retained on algorithm and implementation-diversity grounds. Direct reads of the unpacked layout or blob cache are **out of contract**. *(Spec: R-0087-b.)*

### D5 — Hard cutover: one atomic flip, no legacy accept path

The uniform-packaging invariant lands as **one change**. The existing `core: true` plugin set is re-packaged and re-signed (with `[[artifacts]]` manifests) in the same change that activates the bundle-only loader. No commit range accepts both shapes. No flag, feature, or code path re-enables a pre-OCI load. Pre-OCI load tests are retired or converted to rejection tests in the same change. A signed manifest carrying the legacy `[component]` table is rejected post-cutover (one binding schema in force, ratified 2026-07-07). *Anchors: Q7; H1.* *(Spec: R-0086-e, R-0089.)*

### D6 — Single-root custody at this tier: the recorded exposure

The package signature (distribution anchor) and the inner manifest signature (provenance anchor) are both made by the **one P-0005 root** at this tier. The layers are therefore **scope-independent but NOT compromise-independent**. A single root-key theft forges both simultaneously, and detecting a stolen-key forgery requires a transparency-log or TUF witness that's deferred. This is a deliberate, maintainer-ratified trade (Q5, one custody story at single-publisher `core: true` scope, on the Security ↔ Simplicity axis). It's recorded here **so it is never silent**: the accepted-risk register carries it as `R-0009` with the R-0005-e retirement condition (split to a distinct distribution key or TUF delegated roles). *Anchors: Q5; P-SecurityLayered (honest accounting); P-LeastAuthority (the concentration is the named exposure); P-0005 R-0005-e.*

### Deferrals

All four deferrals ride tripwires with full decision content recorded in the Frame's deferral table (blob `60c437c5`) and the spec's Out-of-scope section, cited rather than restated. They are: **TUF mechanism** (R-0005-e; the seam is its slot), **distribution-key split** (R-0005-e; restores compromise-independence), **sigstore keyless** (R-0005-e; self-hosted variant for R-NoExternalHost), and **SLSA/in-toto attestations** (reproducible-builds landing; the answer to the build-time dependency-confusion residual, register `R-0010`). The Tier-C custody hardening slot `{{P-SigningKeyCustodyHardening}}` stays unauthored and untouched.

### Consequences

**Good:**

- The per-artifact substitution hole closes at **both** anchors. The signed OCI manifest digest-pins every blob at the distribution anchor, and the inner `[[artifacts]]` gate complete-mediates every blob at the provenance anchor. The fetch↔load TOCTOU is closed for non-wasm blobs for the first time.
- Both R-NoExternalHost cases (air-gap filesystem, restricted-egress LAN registry) are first-class from one abstraction, with zero new signing dependencies.
- The bounds-first ordering makes the availability surface (oversized metadata/blobs, bomb-shaped inputs) fail closed **before** the integrity gates can be size-attacked. It's single-sourced here, so a future refactor that reorders bounds after digests visibly violates this ADR.
- The seam-now/TUF-later split means rollback protection arrives as an adapter, not rework.
- Install is a **working-state → working-state** transition. No failure, crash, or kill point leaves the host's serving or load-eligible state indeterminate, and a partial or killed attempt's residue is inert. That comes from the single verification chokepoint (D4) plus load-time re-verification, not a bespoke transaction. *(Spec: R-0092.)* The system-wide principle is routed to canon separately (#2248).

**Bad / Trade-offs:**

- **Rollback/downgrade is an accepted residual** until R-0005-e (register `R-0008`): a stale but validly-signed bundle verifies. Named, trip-wired, not silent.
- **Single-root compromise-independence** (D6, register `R-0009`): honest accounting is the mitigation at this tier, and the split is deferred.
- **Build-time dependency confusion** (register `R-0010`): R-0021 binds bytes-run == bytes-signed, which is integrity, not provenance-of-inputs. The SLSA answer waits on reproducible builds.
- The hard cutover makes the flip change larger (re-package, re-sign, and test retirement in one change). Accepted, to keep the bare path from surviving as a migration window.

## Pros and Cons of the Options

### OCI artifacts, authored directly (chosen)

- Pro: native N-blob-per-manifest bundle; image-layout == registry structure (one abstraction, both R-NoExternalHost transports); referrer-attached signatures; mature Green Rust clients (`oci-client`, `oci-spec`).
- Con: genuinely weak on rollback and rotation (the TUF deferral exists for exactly this); `ocidir`'s thinner maturity requires the L3 layout-write-path audit before adoption (spec acceptance gate).

### warg / wasm-pkg registry (rejected)

- Pro: signed append-only per-package log (strong rollback and transparency).
- Con: no longer actively developed (ecosystem converged on OCI); the unit is a component-package with no home for non-wasm artifacts, an intrinsic mismatch with the uniform-bundle contract.

### Bespoke CAS / zip (rejected)

- Pro: zero new concepts; reuses in-tree BLAKE3.
- Con: reinvents everything OCI standardizes (per Simplicity's least-standing-mechanism reading, more surface, not less); fallback-of-last-resort only.

### Keyed-in-tree signing (chosen) vs cosign/sigstore keyless (rejected at this tier)

- Pro (keyed): zero new dependency; no external host (R-NoExternalHost-clean); Rust-native verify; one custody story at single-publisher scope.
- Con (keyed): no public transparency; compromise-independence deferred (D6). Pro (keyless): short-lived identities plus public auditability. Con (keyless): public infrastructure fails R-NoExternalHost as-shipped; Rust signing doesn't exist (Go CLI, S2 cost); self-hosted Fulcio/Rekor is a heavy operational lift, the recorded forward-allowance.

## More Information

- **Resolves** the plugin-distribution Frame's `{{P-PluginDistribution}}` slot ([Frame](../../intent/plugin-distribution-frame.md), blob `60c437c5`, ADR landing map).
- **Spec:** [`docs/specs/2026-07-07-plugin-distribution.md`](../../specs/2026-07-07-plugin-distribution.md). Covers R-0078 through R-0092 (R-0082 skipped; see the spec's numbering note), the `[[artifacts]]` schema detail, the error grammar, and the numeric floors.
- **Sibling authority:** [P-0003](P-0003-plugin-manifest.md) owns the manifest schema (§Amendment 2026-07-07, the `[[artifacts]]` binding); [P-0019](P-0019-plugin-contract.md) owns the lifecycle capstone (DEF-2 dispositioned 2026-07-07; D6 cross-references this ADR's pre-phase); [P-0005](P-0005-v0-signing-chain.md) owns the root key, custody, and the R-0005-e trip-wire; [P-0007](P-0007-plugin-resource-limits.md) owns execution-time limits (this ADR's bounds are the fetch/unpack-time sibling, distinct by design).
- **Threat references:** typed elements per the [architecture overview's distribution extension](../architecture/overview.md#trust-boundaries) (2026-07-07): `EE-plugin-store`, `P-bundle-builder`, `P-fetch-verify`, `DS-oci-store`, `DS-bundle-cache`, `DF-publish`, `DF-fetch`, `DF-referrers`, and the `TB-plugin-store` boundary row. STRIDE-per-element rows land at this cluster's pre-implementation security review (the P-0014 typed-DFD-extension precedent). Accepted risks: `R-0008` (rollback residual), `R-0009` (single-root exposure), and `R-0010` (build-time dependency confusion), in the overview §Accepted risks.
- **Research:** the plugin artifact-repository and package-signing standards survey (locked 2026-07-01, r3), covering the store and signing options and the ten load-path invariants this design carries.
