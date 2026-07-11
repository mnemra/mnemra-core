---
status: approved
reviewed_by: Peter Manahan
reviewed_date: 2026-07-11
spec_type: code
modulation: cold-start
date: 2026-07-07
frame: docs/intent/plugin-distribution-frame.md
frame-version: 60c437c5e569eacbf00329e92cae2e7c42cebba6
intake: docs/intent/plugin-distribution.md
intake-version: 9c8e1577ed345cbcef546ba51d252a0df4db1144
base-pin: design/plugin-distribution@3b4efd8
---

# Spec: Plugin Distribution — uniform OCI packaging · package signing · verified fetch

> **Spec for:** the plugin distribution layer (W2-1; tasks #1943/#1944 subject) — the uniform multi-artifact OCI bundle, the keyed-in-tree package signature at the distribution anchor, the two R-NoExternalHost transports behind one store contract, the bounds-first fetch-verify pipeline behind the `PackageVerifier` seam, the inner `[[artifacts]]` complete-mediation schema at the provenance anchor, and the hard cutover that retires the bare load path.
> **Date:** 2026-07-07 (intake-date stem; pairs with the BOM sidecar `docs/specs/2026-07-07-plugin-distribution.bom.toml`).
> **Design ref:** [Frame](../intent/plugin-distribution-frame.md) (blob `60c437c5`, LOCKED — operating constraints OC-1..OC-15, load-path invariants #1–#10 landing map, prior-decision dispositions, deferral table); [intake](../intent/plugin-distribution.md) (locked 2026-07-07, blob `9c8e1577`, `stakes: high`, SC1–SC8, Non-goals 1–8, Hard constraints 1–8). The companion ADR slot `{{P-PluginDistribution}}` and the P-0003 / P-0019 amendment texts are **round-2 deliverables of this Stage-3 package** — cited here by placeholder; this spec is authored self-contained and does not depend on them landing first.

## Purpose / context

Today a mnemra plugin exists only as files loaded from known local paths, gated after the fact by the inner Ed25519 manifest signature ([P-0005](../src/adrs/P-0005-v0-signing-chain.md)) and the R-0021 BLAKE3 content-hash over the `.wasm` component ([2026-06-30-signing-to-runnable.md](2026-06-30-signing-to-runnable.md)). The distributable unit does not exist: no packaging shape, an unsigned package, and non-wasm artifacts individually uncovered — a live per-artifact substitution hole. This spec locks the designed tier of the distribution layer that closes it: a consumer establishes whole-bundle provenance and integrity **before unpacking**, from a filesystem or a self-hosted registry, with zero required round-trip to a host we do not control, and every blob is re-verified at the provenance anchor at load.

The closed world is the locked Frame (blob `60c437c5`). This spec maps Frame operating constraints OC-1..OC-15 and load-path invariants #1–#10 to requirement slots **R-0078–R-0091** at spec precision, plus **R-0092** (an explicit naming of the system-state determinacy property the Frame's fail-closed single-chokepoint structure already delivers, raised to a named requirement by the maintainer's 2026-07-11 spec-exit-gate ruling); nothing outside the locked Frame, the locked intake, cited canon, and that gate ruling enters here. The maintainer's locked decisions (H1, R-NoExternalHost, Decision A, Q2/Q3/Q5/Q7) are transitively off-limits to re-open. Where the Frame delegated a calibration to spec tier (the `[[artifacts]]` field-by-field schema, media-type values, resource-bound floors, the `PackageVerifier` seam contract, entry↔blob resolution, the rejection-class grammar), this spec pins it.

**R-ID numbering note.** The global series continues from R-0077 (coordination wedge, `main@8f27cc5`). This spec claims **R-0078–R-0092, skipping R-0082**: that identifier is textually held as a drop-tombstone in [2026-07-03-reporting-engine.md](2026-07-03-reporting-engine.md) ("R-0082 drop, SC4 map"); assigning it a live requirement would make the identifier resolve to two different things across the corpus (AP1 unambiguous-resolution discipline). R-0092 is the last-assigned slot — the maintainer's 2026-07-11 install-atomicity ruling, folded post-r2 at the spec-exit gate (see the Changelog); it names a determinacy property the locked structure already delivers rather than mapping a further Frame OC.

### Substrate base pin

This layer designs against **`design/plugin-distribution@3b4efd8`** plus assumed-shipped surfaces: the V0 substrate spec as amended by the signing-to-runnable spec — a runnable host with synchronous fail-closed Ed25519 verification (P-0005), the BLAKE3 single-read content-hash gate (R-0021), the P-0003 signed TOML manifest (with the 2026-06-30 `[component]` amendment), the P-0019 D6 lifecycle, and the P-0012 Wasmtime pin (`=45.0.2`) — and the existing `core: true` plugin set (`plugins/mnemra-echo/`, filesystem shape). Staleness of this pin is checked at the designed → committed transition; the plan author re-verifies the assumed-shipped set against then-current `main`. **Deliberately excluded from the assumed-shipped set:** the retrieval cluster, the reporting engine, and the coordination cluster — this layer touches none of them.

**Backfill rule check: N/A.** No requirement re-derives values from an existing data source. The hard cutover (R-0089) re-packages existing plugins from their build inputs and re-signs them through the normal build pipeline — a rebuild, not a backfill; no post-backfill invariant or unrecoverable-outcome analysis applies.

## Requirements

RFC-2119 keywords throughout. `SHALL`/`MUST` = mandatory; `SHALL NOT`/`MUST NOT` = prohibited; `SHOULD` = preferred; `MAY` = optional. Prohibitions are as load-bearing as requirements — this layer's headline risks are an over-helpful implementation that adds a legacy accept path "for migration," a best-effort load on a verification error, an unbounded read before a bound fires, or trust in registry-supplied metadata.

**Verification-instrument classes** (retrieval-cluster convention): **[acceptance test]** — a runnable black-box scenario (fetch/verify/load through the store contract and loader, both transports); **[construction + review audit]** — a code-shape invariant enforced by construction and verified at code review / CI static check; **[acceptance test via the feature-gated test-hooks seam]** — a runnable unit that exercises an internal check in isolation through the host's existing `test-hooks` feature gate (bare `#[cfg(feature = "test-hooks")]`, guarded by the `no_test_seams` compile-fail check), used only where a property is not black-box constructible (its sole use is R-0087-a's per-anchor isolation).

Numeric calibrations the Frame delegated to spec tier are pinned in [§ Numeric calibrations](#numeric-calibrations-spec-tier-conservative-floors-config-tunable); each is a config-tunable default — the *existence and enforcement* are the lock, the number is a calibrated floor.

---

### R-0078 — Uniform multi-artifact OCI bundle; one flat manifest; no bare load path

*Locks Frame OC-1 (H1) + OC-8 (Q2); load-path invariant #1. Anchors: H1 (maintainer 2026-07-01, LOCKED); Q2 (maintainer 2026-07-07); [P-0019](../src/adrs/P-0019-plugin-contract.md) D6 (one load path); Simplicity (atomic unit, one manifest).*

- **R-0078-a (bundle shape):** A mnemra plugin's distributable unit SHALL be a single **OCI image manifest** (image-spec 1.1) with `artifactType = application/vnd.mnemra.plugin.v1`, whose **config descriptor** references the plugin's **signed inner TOML manifest** blob (media type per R-0079) and whose **layer descriptors** reference the **N≥1 artifact blobs** in order, with **`layers[0]` = the Component Model `.wasm`** (the "wasm = artifact #1" lock). Every blob — config and layers — is digest-addressed by the manifest. *(Binary-observable: a published bundle's manifest parses to exactly this shape — config = inner manifest, layers[0] = wasm — [acceptance test].)*
- **R-0078-b (no bare load path):** The loader SHALL reject any plugin that does not present a signed OCI manifest binding every artifact. No code path SHALL load a bare `.wasm`, a bare TOML manifest, or any non-bundle file layout as a plugin. A wasm-only plugin (N=1) ships as a uniform 1-artifact bundle — the *path* is banned, not N=1 content. *(Binary-observable: a bare `.wasm` + manifest pair in the pre-cutover filesystem shape is rejected with `legacy_layout_rejected`; a 1-layer bundle loads — [acceptance test]; no loader entry point accepts a non-bundle path — [construction + review audit].)*
- **R-0078-c (one flat manifest; index rejected):** The bundle SHALL be one flat image manifest. An OCI **image index** presented as a plugin SHALL be rejected (`index_rejected`) — the image-index shape is the recorded forward path (Frame OC-8), not accepted at this tier. *(Binary-observable: an image index tagged as a plugin is rejected with the designed error — [acceptance test].)*
- **R-0078-d (component-first enforced at load, not narrated):** The loader SHALL reject (`component_not_first`) any bundle whose `layers[0]` / `[[artifacts]]` entry #1 is not the Component Model `.wasm` with `mediaType = application/wasm` — the H1 "wasm = artifact #1" lock is a load-gate rejection with a designed variant, not a builder convention that fails later as an undesigned Wasmtime instantiation error. *(Binary-observable: a signed bundle with `layers[0].mediaType = application/json` is rejected with `component_not_first` at load, before instantiation — [acceptance test].)*

### R-0079 — Media types: the closed Q4 lock set; artifactType pinning; cross-anchor consistency

*Locks Frame OC-14 (Q4, intake SC6 lock target); load-path invariant #6. Anchors: P-AgentPrimarySource AP1 (collision-checked identifiers); Load-path invariant #6 (confused-deputy block).*

- **R-0079-a (the closed media-type set):** The following media types SHALL be the V0 vocabulary, closed — extension is a spec amendment:

  | Surface | Media type |
  |---|---|
  | Outer manifest `artifactType` | `application/vnd.mnemra.plugin.v1` |
  | Config blob (signed inner TOML manifest) | `application/vnd.mnemra.plugin.manifest.v1+toml` |
  | Component layer (`layers[0]`) | `application/wasm` |
  | Data/asset/schema layers | the artifact's declared type (e.g. `application/json`), or `application/vnd.mnemra.plugin.data.v1` where no standard type applies |
  | Signature referrer `artifactType` | `application/vnd.mnemra.signature.v1` |

- **R-0079-b (artifactType pinning, fail-closed):** Fetch SHALL pin the expected outer `artifactType` (`application/vnd.mnemra.plugin.v1`) and reject any manifest that does not match (`artifact_type_mismatch`) — never "load whatever is in the registry" (the confused-deputy block). The config descriptor's media type SHALL equally be pinned; a mismatch rejects with `artifact_type_mismatch` (the same static-pin-failure class as the outer `artifactType` mismatch — both are type-pin failures at the outer manifest, checked against a fixed expected value). *(Binary-observable: a plain container image at the pinned reference is rejected with the designed error — [acceptance test].)*
- **R-0079-c (cross-anchor media-type consistency):** For every artifact, the inner `[[artifacts]]` entry's `media_type` (R-0086) SHALL equal the outer layer descriptor's `mediaType` for the same `id`; a mismatch is rejected (`media_type_mismatch`) at load. *(Binary-observable: a bundle whose outer descriptor and inner entry disagree on media type for one id is rejected — [acceptance test].)*
- **R-0079-d (compression posture — uncompressed-only at V0):** Blob media types carrying a compression suffix (`+gzip`, `+zstd`) SHALL be rejected (`compressed_blob_rejected`); V0 blobs are stored and fetched **uncompressed**, so artifact bytes == blob bytes at both anchors (the inner BLAKE3 and outer sha256 hash the same bytes) and no decompression step exists on the pipeline — the decompression-bomb class is closed **by construction** (the Frame's decompression-ratio bound is satisfied at its conservative floor: no decompression is permitted). Admitting compression later is a spec amendment that SHALL carry the decompression-ratio cap and absolute decompressed-size cap with it. *(Binary-observable: a `+gzip` layer is rejected with the designed error; no decompression code exists on the fetch/unpack path — [acceptance test] + [construction + review audit].)*

### R-0080 — Package signature: keyed-in-tree over the manifest digest, domain-separated, attached as an OCI 1.1 referrer, verified under signer-key-pinning

*Locks Frame OC-5 (Q5) + rationale-chain 1; load-path invariants #2, #9. Anchors: research §5 (keyed-in-tree best-fit); P-StackDiscipline S2 (no Go-CLI signing dependency); Simplicity (reuse the P-0005 root — R-0021's own reuse logic); [P-0005](../src/adrs/P-0005-v0-signing-chain.md) (custody unchanged).*

- **R-0080-a (signing subject + domain separation):** The package signature SHALL be an Ed25519 signature by the P-0005 mnemra root over the **domain-separated message** `"mnemra-oci-manifest-v1:" || <canonical digest string>`, where the canonical digest string is the OCI digest of the outer manifest in `<algorithm>:<lowercase-hex>` ASCII form (e.g. `sha256:ab12…`). The prefix is mandatory: the root SHALL NOT sign a bare digest in this message domain. *(Binary-observable: a signature over the unprefixed digest fails verification — [acceptance test]; the signing call site prepends the prefix constant — [construction + review audit].)*
- **R-0080-b (attachment as referrer):** The signature SHALL be attached as an **OCI 1.1 referrer**: an artifact manifest with `artifactType = application/vnd.mnemra.signature.v1` whose `subject` is the signed manifest's descriptor, carrying the 64-byte raw signature as its single blob and the signer public-key fingerprint as an annotation ([§ Data Model](#data-model)). *(Binary-observable: the published referrer resolves via the store contract's referrer enumeration on both transports — [acceptance test].)*
- **R-0080-c (signer-key-pinning, fail-closed — never TOFU):** Verification SHALL enumerate the subject's signature referrers (within the R-0084 bounds), select only those whose signer public key equals the **pinned mnemra root** (the verification material already embedded in the host binary per P-0005), require **≥1** valid such signature, and **fail closed on zero** (`signature_missing` when no signature referrer exists; `signer_unknown` when referrers exist but none is pinned-root-signed; `signature_invalid` when a pinned-root referrer fails cryptographic verification). There SHALL be no trust-on-first-use path, no unsigned-load fall-through, and no "any referrer that verifies" acceptance. *(Binary-observable: an unsigned bundle, a bundle re-signed by an unknown key, and a bundle with a corrupted signature are each rejected with their distinct designed error — [acceptance test].)*
- **R-0080-d (verification precedes blob trust):** Package-signature verification SHALL complete `Ok` before any layer blob is unpacked, materialized, or handed to any consumer. Blob fetching MAY overlap with signature verification only if no fetched byte is exposed beyond the pipeline before the signature verifies. *(Binary-observable: with signature verification forced to fail, no unpacked file exists after the rejection — [acceptance test].)*
- **R-0080-e (key custody unchanged):** Signing SHALL occur on the build host under P-0005's custody parameters (on-disk mode 600, build-host-only; key material never on the deployment node). This spec adds a second message domain to the existing root, not a second key or custody surface; the single-root compromise-independence exposure and its R-0005-e split are recorded in the Frame (OC-7) and land in the risk register (round 2). *(Binary-observable: the deployment host binary contains verification material only — [construction + review audit].)*

### R-0081 — Store contract: two R-NoExternalHost transports behind one abstraction; digest-pin; digest over received bytes

*Locks Frame OC-2 + OC-4; load-path invariants #3, #4. Anchors: R-NoExternalHost (maintainer 2026-07-01, LOCKED); research §5 (OCI store, direct-authored); P-LockContract (one store contract, transports vary behind it).*

- **R-0081-a (one contract, two transports):** A single store contract SHALL front both transports: **filesystem** (OCI image-layout: `oci-layout` marker + `index.json` + `blobs/<alg>/<digest>`) and **self-hosted registry** (OCI distribution API). The contract's surface is: resolve a reference to a manifest descriptor, fetch a manifest, enumerate signature referrers for a digest, and read a blob by digest — each observable per transport. Loader and pipeline code SHALL NOT branch on transport identity outside the store-contract implementations. *(Binary-observable: the full fetch-verify-load path passes against a local `oci-layout` directory AND against a self-hosted registry fixture with the same pipeline code — [acceptance test]; transport selection is confined to the contract boundary — [construction + review audit].)*
- **R-0081-b (zero external round-trip):** No code path on fetch, verify, unpack, or load SHALL require a network round-trip to a host outside the operator's control. No test in the acceptance suite SHALL require a public registry or external host. *(Binary-observable: the acceptance suite passes with external egress blocked — [acceptance test].)*
- **R-0081-c (digest-pin, not tag):** Bundle resolution for load SHALL be by **digest**. A mutable tag MAY be resolved to a digest as a naming convenience, but every verification and every load decision SHALL be anchored on the digest, and a fetch-for-load that cannot produce a digest-pinned reference SHALL be refused (`tag_unpinned`). *(Binary-observable: repointing a tag between resolve and fetch does not change what is loaded — the digest pin governs — [acceptance test].)*
- **R-0081-d (digest over received bytes):** Every manifest and blob digest SHALL be recomputed over the bytes actually received and compared to the expected digest; the registry's `Docker-Content-Digest` header (or any store-supplied digest claim) SHALL NOT be trusted as verification. *(Binary-observable: a registry fixture that serves tampered bytes under a correct header is rejected with `digest_mismatch_distribution` — [acceptance test].)*
- **R-0081-e (`wkg` is not the bundle path):** The bundle SHALL be authored and fetched as a raw OCI artifact (the ORAS-style arbitrary-artifact path). `wkg` SHALL NOT be used for bundle packaging, publishing, or fetching; its use remains confined to WIT/component build-time dependency pulls. *(Binary-observable: no `wkg`/`wasm-pkg-*` dependency on the distribution code path — [construction + review audit].)*

### R-0083 — Fetch-verify pipeline: bounds-first canonical ordering behind the `PackageVerifier` seam

*Locks Frame OC-6 (Q3) + the canonical pre-phase ordering; discharges the seam-contract spec obligation. Anchors: P-LockContract + the when-to-lock edge (the P-0010 D5 engine-agnostic-seam precedent); P-Defer (TUF adapter deferred behind the seam, fires R-0005-e); Frame D6 disposition (bounds-first is the ADR-destined ordering). (R-0082 is skipped — see the numbering note.)*

- **R-0083-a (canonical ordering):** The distribution pre-phase SHALL execute exactly: **fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds**, fail-closed at every stage, with **every read gated by its bound before the read feeds a verifier** (R-0084). No later stage SHALL be reachable without its predecessor completing `Ok`; no stage SHALL be skippable by configuration. The pre-phase completes before P-0019 D6 state 1 ("Discovered") and hands D6 a verified bundle; D6 states 2–9 run unchanged (R-0088). Within the locked ordering, the pipeline SHOULD verify the package signature over the outer manifest **before committing to the blob-fetch loop** (R-0080-d already permits this overlap discipline), so an unsigned or foreign-signed bundle is rejected at minimum fetch cost rather than after up to a total-bundle-bound of wasted reads. *(Binary-observable: instrumentation shows stage ordering on every load; a failure at each stage prevents every later stage — [acceptance test].)*
- **R-0083-b (the `PackageVerifier` seam):** Package verification SHALL sit behind a `PackageVerifier` contract owned by the host: given a resolved manifest descriptor and the store handle, it yields either a **verified-bundle token** (the digest-pinned manifest + its verified signature identity) or a designed rejection (R-0091). Callers SHALL obtain fetch-side trust decisions only through this seam; the seam's contract SHALL NOT expose transport identity or signature mechanism to callers. The seam is the locked TUF slot: a future TUF adapter (deferred, fires at R-0005-e) composes **behind** it without changing the caller-visible contract. An illustrative (non-normative) shape:

  ```rust
  // Illustrative only — names and exact signatures are implementation-tier.
  trait PackageVerifier {
      fn verify(&self, subject: ManifestDescriptor, store: &dyn BundleStore)
          -> Result<VerifiedBundle, DistributionError>;
  }
  ```

  *(Binary-observable: exactly one implementation ships at V0; the loader consumes the trait, not the implementation — [construction + review audit].)*
- **R-0083-c (single verification chokepoint):** There SHALL be exactly one code path by which a bundle becomes load-eligible — through the seam. No diagnostic, admin, test-support, or cache-warm path SHALL mark a bundle verified without executing the full R-0083-a sequence. *(Binary-observable: the verified-bundle token type is constructible only inside the seam module — [construction + review audit].)*

### R-0084 — Fetch/unpack resource bounds: fail-closed ahead of every read they gate

*Locks Frame OC-12 (the net-new constraint); the metadata dimensions per the Frame's minimum-set extension. Anchors: P-SecurityLayered (availability half of the transport layer); P-GuaranteeByMechanism (bound breach = designed error); [P-0007](../src/adrs/P-0007-plugin-resource-limits.md) (the conservative-floor method; execution-time sibling).*

- **R-0084-a (the bound set):** Bounds SHALL exist and be enforced on, at minimum: **outer-manifest byte size**, **referrers-listing / index-document byte size** (the listing read that enumeration must parse — `index.json` on the fs transport, the Referrers API response on the registry transport — is itself a bounded read; the count bound alone cannot fire until the listing is read), **referrers-enumeration count** and **per-referrer manifest size**, **inner-manifest (config blob) byte size** (parse-time metadata gets a metadata-scale cap, not the data-blob cap), **artifact count N**, **per-blob byte size**, and **total bundle byte size**. **N counts `[[artifacts]]` entries == layers; the config blob is not an artifact and does not count toward N** (it is bounded by its own dimension). (The decompression-ratio dimension is satisfied by construction at V0 — R-0079-d.) Concrete floors are pinned in [§ Numeric calibrations](#numeric-calibrations-spec-tier-conservative-floors-config-tunable); each is config-tunable with the floor as default. *(Binary-observable: each dimension has a rejection test at floor+1 — [acceptance test].)*
- **R-0084-b (bounds fire ahead of the read they gate, on received bytes):** Each bound SHALL be enforced **fail-closed before or during the read it gates, on the received byte count** — a declared size (a descriptor's `size` field) is checked first to fail fast, and the received-byte count is enforced while reading so a store that lies about size cannot exceed the bound. A breach SHALL abort the read, reject the fetch with `bound_exceeded` naming the dimension, and release any partial buffer. No verifier SHALL receive bytes from an unbounded read. *(Binary-observable: a registry fixture streaming more bytes than the descriptor declares is cut off at the bound, not buffered to completion — [acceptance test].)*
- **R-0084-c (streaming-verify preferred; hard cap sufficient):** The implementation SHOULD verify digests incrementally as bytes stream within the bound (bounded-buffer). If the chosen client buffers whole blobs, the per-blob bound caps the buffer and the requirement is still met — the hard-cap existence is the lock (the Frame's consultation on `oci-client`'s streaming surface resolves at implementation). *(Binary-observable: peak pipeline memory during fetch of a floor-sized blob stays within the per-blob bound plus a constant — [acceptance test], tolerance implementation-calibrated.)*

### R-0085 — Unpack safety: identifier-derived materialization, path-traversal guard, missing-blob and set-completeness fail-closed

*Locks load-path invariants #7, #8 + Frame OC-13's bidirectional completeness (F8). Anchors: R-0005-h (identifiers live in the signed body — R-0086); P-GuaranteeByMechanism.*

- **R-0085-a (identifier-derived materialization):** Any materialization of blobs to named files SHALL derive names from the **validated per-artifact `id`** (R-0086-b grammar — no path separators or traversal constructible by construction), never from free-form annotations. The `org.opencontainers.image.title` annotation SHALL NOT name files. *(Binary-observable: a bundle whose title annotation carries `../escape` materializes under its `id`, with no file outside the target directory; an id failing the grammar is rejected `identifier_invalid` before any write — [acceptance test].)*
- **R-0085-b (missing blob fail-closed):** A bundle any of whose manifest-referenced blobs (config or layer) cannot be fetched and verified SHALL be rejected in full (`blob_missing`); a partial bundle SHALL NOT be handed to D6 with an artifact silently absent. *(Binary-observable: an N-1-of-N registry fixture rejects; nothing is materialized as load-eligible — [acceptance test].)*
- **R-0085-c (bidirectional set completeness):** The outer manifest's layer set and the inner `[[artifacts]]` entry set SHALL be mutually complete, joined by `id` (R-0086-c): a layer with no inner entry is rejected (`binding_incomplete_unbound_blob` — a blob that would escape the provenance anchor), and an inner entry with no layer is rejected (`binding_incomplete_missing_blob`). Both checks run at load before any artifact is handed to a consumer. *(Binary-observable: one test per direction, each rejecting with its distinct error — [acceptance test].)*

### R-0086 — The inner `[[artifacts]]` manifest schema (Decision A mechanism; P-0003 amendment shape)

*Locks Frame OC-3 (Decision A) + OC-11 (signed-body residence); load-path invariant #10's schema half; the Q8 headline work. Anchors: Decision A (maintainer 2026-07-01, LOCKED); R-0005-h core-by-provenance; [P-0003](../src/adrs/P-0003-plugin-manifest.md) §Amendment 2026-06-30 (the `[component]` precedent this supersedes — amendment text is round 2); P-GuaranteeByMechanism (parser-enforced presence).*

- **R-0086-a (the schema):** The signed TOML manifest gains an `[[artifacts]]` array of tables **in the signed canonical body** (before the `\n[signature]` marker — the slice produced by the canonical-body splitter in `libs/mnemra-host/signing/verify.rs`), with **N≥1 entries**, **entry #1 = the component `.wasm`**:

  ```toml
  # ... [plugin], [verbs], [content_types], [state_scopes], [host_fns] (unchanged) ...

  [[artifacts]]                      # entry 1 — ALWAYS the component
  id         = "component"           # unique per manifest; grammar per R-0086-b
  media_type = "application/wasm"
  hash_alg   = "blake3"              # blake3-only at V0; every other value rejected (hash_alg_unsupported)
  hash       = "…"                   # mandatory; lowercase-hex digest of the artifact bytes under hash_alg

  [[artifacts]]                      # entries 2..N — data/assets/schemas/secondary components
  id         = "prompt-templates"
  media_type = "application/json"
  hash_alg   = "blake3"
  hash       = "…"

  [signature]                        # unchanged — covers everything above this marker
  ```

  | Field | Type | Constraints |
  |---|---|---|
  | `id` | string | REQUIRED; unique within the manifest (duplicate = `identifier_invalid`); grammar per R-0086-b; entry #1's id SHOULD be `component` |
  | `media_type` | string | REQUIRED; SHALL equal the outer layer descriptor's `mediaType` for the joined blob (R-0079-c) |
  | `hash_alg` | string | REQUIRED; **`"blake3"` is the only value accepted at V0** — any other value, including `"sha256"`/`"sha384"`/`"sha512"` (reserved vocabulary for a future amendment that would also extend R-0087-a's verification and acceptance criteria) and the banned-weak `"md5"`/`"sha1"`, is rejected fail-closed with `hash_alg_unsupported`. No non-blake3 verification path is reachable at V0. |
  | `hash` | string | **Mandatory** (absence = fail-closed rejection, `hash_missing`); lowercase-hex digest of the artifact bytes under `hash_alg` |

- **R-0086-b (identifier grammar):** `id` SHALL match `^[a-z0-9][a-z0-9._-]{0,63}$` and SHALL NOT contain `/`, `\`, or any sequence resolvable as path traversal — traversal is unconstructible by grammar, which is the invariant-#7 guard at the schema layer. *(Binary-observable: ids with separators, leading dots, or length 65 are each rejected at parse — [acceptance test].)*
- **R-0086-c (entry↔blob resolution):** Each `[[artifacts]]` entry SHALL join to exactly one outer layer descriptor via the outer annotation **`vnd.mnemra.artifact.id`** carrying the entry's `id`. Annotation values SHALL be unique within a manifest (a duplicate outer annotation id = `annotation_duplicate` — distinct from the inner duplicate-`id` case, which is `identifier_invalid`); the join SHALL be total in both directions (R-0085-c). The authoritative identifier is the **inner signed `id`** (provenance anchor, R-0005-h); the outer annotation is its distribution-anchor projection, itself covered by the package signature via the manifest digest. *(Binary-observable: a duplicate annotation id rejects; a fixture bundle's join resolves 1:1 — [acceptance test].)*
- **R-0086-d (parser-enforced presence; signed-slice-only):** A signed manifest body lacking `[[artifacts]]` (or with zero entries) SHALL be rejected fail-closed at load (`artifacts_missing`). The array SHALL be parsed **only from the signed canonical body**; an `[[artifacts]]` array in the unsigned `[signature]`-adjacent region SHALL NOT satisfy presence and SHALL cause the same `artifacts_missing` rejection (an array outside the signed body is, for presence purposes, no array at all — the variant is `artifacts_missing`, not a distinct one). `schema_version` stays `1` — presence is parser-enforced, not version-signaled (the P-0003 amendment's one-decision rule, carried; the amendment text lands round 2). *(Binary-observable: a manifest with `[[artifacts]]` only after the signature marker is rejected with `artifacts_missing` — [acceptance test].)*
- **R-0086-e (`[component]` superseded — strict):** Post-cutover, the manifest schema SHALL NOT carry a `[component]` section: entry #1 of `[[artifacts]]` is the component binding. A signed body carrying `[component]` SHALL be rejected (`legacy_manifest_rejected`) — one binding schema, no ambiguity about which table binds (a dual carrier would let the two disagree). *(Binary-observable: a re-signed manifest carrying both tables is rejected; the re-packaged `core: true` set carries only `[[artifacts]]` — [acceptance test].)*

### R-0087 — Load-time complete-mediation gate over all artifacts (inner primary; outer complementary; one read)

*Locks Frame OC-3's runtime half; load-path invariants #10 + #5; the S2 mediated-gate rule made normative. Anchors: Decision A (LOCKED); R-0021 (the single-read discipline, extended in coverage not in kind); the Frame's N2 weighing (keep #5 on algorithm-diversity grounds).*

- **R-0087-a (single-read dual-digest mediation):** At load, for every `[[artifacts]]` entry, the gate SHALL read the artifact bytes **exactly once** and, over those same bytes, verify **both** digests: the inner entry's hash under `hash_alg` (BLAKE3 at V0 — the provenance anchor, primary) and the blob's OCI content-address (sha256 — the distribution anchor, the invariant-#5 complementary outer check), then hand the verified bytes to the consumer. Fail-closed on absence or mismatch at either anchor, with the two anchors' mismatches as distinct errors — and because both digests are functions of the same bytes, a content tamper fails both; **the reported variant on a dual mismatch SHALL be `digest_mismatch_provenance`** (the primary anchor reports; deterministic, so the per-variant test has one right answer). No re-open, no verify-then-return-a-path.

  **Verification instruments (per-anchor independence is not black-box constructible — both digests are deterministic functions of one byte string, so no crafted input trips one while passing the other):**
  - *Black-box [acceptance test]:* the post-fetch cache tamper is caught at load (intake SC4) — proving load always re-verifies and never trusts a fetch-time flag; the reported variant is `digest_mismatch_provenance` per the precedence rule.
  - *Per-anchor isolation [acceptance test via the feature-gated test-hooks seam]:* the two digest checks SHALL be independently-invokable units under the host's existing test-seam convention — the `test-hooks` cargo feature, gated with bare `#[cfg(feature = "test-hooks")]`. The feature name is LOCKED (`libs/mnemra-host/Cargo.toml`; the "Shared tokens (do not rename)" contract in `libs/mnemra-host/tests/no_test_seams.rs`); the compound `#[cfg(any(test, feature = "…"))]` form is not this codebase's convention and SHALL NOT be introduced. The seams are guarded by the `no_test_seams` compile-fail check — never an always-compiled bypass — so each anchor's rejection is exercised with the other check absent by construction. Because `no_test_seams.rs` hand-enumerates its `trybuild` compile-fail fixtures (six today), extending the guard to the two new digest-check seams is **explicit red-phase work**: two new fixtures (e.g. `provenance_digest_check_reachable.rs` and `distribution_digest_check_reachable.rs`) under `tests/ui/no_test_seams/` plus their two `t.compile_fail(...)` lines added to that gate file — a red-phase edit to `no_test_seams.rs` itself (its "green does not edit this file" contract makes the fixture-list edit red-phase touch_scope), not coverage that falls out incidentally. This realizes intake SC5's "with the other anchor disabled" at the seam tier; a shipped-runtime disable toggle would be exactly the bypass R-0083-c forbids and is NOT built. The seam invokes checks separately; it cannot mark a bundle verified (R-0083-c unaffected).
  - *[construction + review audit]:* both checks are unconditionally invoked over the single read; neither is reachable via a code path that skips the other (the algorithm-diversity property the Frame's N2 weighing names as the live ground).
- **R-0087-b (mediated-gate-only access):** Consumers SHALL obtain artifact bytes only through the mediation gate; direct reads of the unpacked layout or blob cache by any consumer are out of contract and SHALL NOT exist in the shipped system's runtime code. *(Binary-observable: the verified-bytes handle is the only artifact-access type exported to consumers; no runtime code path opens cache/unpack paths directly — [construction + review audit].)*
- **R-0087-c (wasm gate unchanged in primitive):** For the component (entry #1), this gate IS the R-0021 gate with its binding source moved from `[component].hash` to `[[artifacts]]` entry #1 — same primitive (BLAKE3, single-read-then-`from_binary`, fail-closed, distinct error variant), extended in coverage to every artifact. R-0021's discipline SHALL NOT be weakened for any artifact class. *(Binary-observable: the component's load path satisfies R-0021's existing acceptance criteria unchanged — [acceptance test].)*

### R-0088 — Distribution pre-phase feeds unchanged D6; existing gates unchanged in primitive

*Locks Frame OC-10 + the D6 disposition. Anchors: [P-0005](../src/adrs/P-0005-v0-signing-chain.md) V0 invariants; [P-0019](../src/adrs/P-0019-plugin-contract.md) D6 (ordering single-sourced; gains one cross-reference line — round 2); [P-0012](../src/adrs/P-0012-plugin-runtime-and-mcp-sdk.md) (Wasmtime pin untouched).*

- **R-0088-a (pre-phase position):** The R-0083 pipeline SHALL complete before D6 state 1 ("Discovered"); its output — a verified, digest-pinned bundle — is what D6 discovers. D6 states 2–9 (signature-verify → content-hash-verify → manifest-parse/allowlist → instantiate → … ) SHALL run unchanged in ordering and gate semantics; verify-before-instantiate is preserved end-to-end. *(Binary-observable: D6's existing acceptance criteria pass unchanged over a bundle-delivered plugin — [acceptance test].)*
- **R-0088-b (P-0005 invariants untouched):** Synchronous fail-closed verification, no verify-async path, `core: true` honored only by signature provenance, and the file-mode startup checks SHALL hold unchanged. This spec adds no key, no custody surface, and no async verification anywhere. *(Binary-observable: P-0005's invariant tests pass unchanged — [acceptance test].)*
- **R-0088-c (runtime pin untouched):** The distribution layer SHALL NOT alter the P-0012 Wasmtime pin or the plugin ABI; no WIT-surface change ships with this layer. *(Binary-observable: the diff touches no WIT file and no Wasmtime version — [construction + review audit].)*

### R-0089 — Hard cutover: one atomic flip, no dual-accept window, pre-OCI tests retired

*Locks Frame OC-9 (Q7). Anchors: Q7 (maintainer 2026-07-07, LOCKED); H1 (a dual-accept window is the bare path reborn); intake SC1.*

- **R-0089-a (atomic flip):** The uniform-packaging invariant SHALL land as one change: the existing `core: true` plugin set (`plugins/mnemra-echo/`) is re-packaged as uniform OCI bundles and re-signed with `[[artifacts]]` manifests **in the same change** that activates the bundle-only loader. There SHALL be no commit range in which the loader accepts both shapes. *(Binary-observable: at the flip commit, the loader rejects the pre-OCI shape and loads the re-packaged set — [acceptance test].)*
- **R-0089-b (no legacy accept path — not even flagged):** No configuration flag, environment variable, build feature, or code path SHALL re-enable a pre-OCI or bare load. An implementation SHALL NOT add a "migration compatibility" accept path helpfully. *(Binary-observable: no such flag or path exists in the diff — [construction + review audit].)*
- **R-0089-c (pre-OCI tests retired; one frozen fixture retained):** Tests exercising the pre-OCI load shape SHALL be retired or converted to **rejection** tests in the same change; post-flip, no test exercises a pre-OCI load as a success path. **One frozen pre-OCI fixture snapshot (bare `.wasm` + TOML pair) SHALL be retained as the rejection-test input** — "retire" applies to success-path *tests*, never to the fixture the `legacy_layout_rejected` scenario needs to construct its rejection. *(Binary-observable: the test corpus post-flip contains no passing pre-OCI load — [construction + review audit]; the bare-path rejection scenario runs against the retained frozen fixture — [acceptance test].)*

### R-0090 — Bundle builder/publisher (build-host side)

*Locks Frame component ①. Anchors: [P-0005](../src/adrs/P-0005-v0-signing-chain.md) (custody); OC-4 (direct OCI authoring); intake use-cases (a) and (b).*

- **R-0090-a (assembly):** A build-host tool SHALL assemble the bundle from the built artifacts: compute each blob's digest, author the outer manifest (R-0078-a shape, R-0079 media types, R-0086-c annotations), and emit a valid **OCI image-layout** directory as the canonical build output. *(Binary-observable: the emitted layout round-trips through the store contract's fs transport and passes the full R-0083 pipeline — [acceptance test].)*
- **R-0090-b (sign at build):** The tool SHALL sign the manifest digest per R-0080-a/-b on the build host and include the signature referrer in the layout. The inner manifest is signed by the existing P-0005 chain (with `[[artifacts]]`, R-0086) before bundle assembly — inner signing precedes outer packaging by construction (the inner manifest blob is content-addressed into the outer manifest). *(Binary-observable: the emitted layout contains the referrer and verifies under R-0080-c — [acceptance test].)*
- **R-0090-c (publish):** The tool SHALL publish a layout to a self-hosted registry via the OCI distribution API (manifest, blobs, and referrer), such that fetch from the registry and fetch from the layout verify identically. *(Binary-observable: intake use-case (a) — publish to a LAN registry fixture, then fetch-verify-load — and use-case (b) — the same layout read from a directory — both pass with identical verification outcomes — [acceptance test].)*
- **R-0090-d (dependency gates are acceptance evidence — intake SC7):** The implementation change that first adds the OCI dependency set (`oci-client`, `oci-spec`, and `ocidir` if adopted) SHALL carry, as this spec's acceptance evidence: **(a)** a `cargo audit` / RUSTSEC screen against the pinned versions, with every finding dispositioned (the L5 gate; L5/L3 refer to intake SC7's dependency-gate severity numbering), wired as a CI recipe in the same change so the screen re-runs thereafter; **(b)** either the focused audit of `ocidir`'s layout-write path or the narrow image-layout read/write written directly against `oci-spec` types, with the choice and its evidence recorded (the L3 gate). These are spec acceptance gates carried from intake SC7 — symmetric with the lockfile-hash-pin item in [§ Out of Scope](#out-of-scope) — not optional follow-ups; silence on either at the implementation PR is a spec violation. *(Binary-observable: the dependency-adding PR carries the audit output + dispositions and the CI recipe exists — [construction + review audit].)*

### R-0091 — Designed rejection grammar + instrumentation

*Locks Frame OC-13 + intake SC8. Anchors: P-GuaranteeByMechanism (loud-failing mechanism per class); P-SecurityLayered (fail-closed structural); P-InstrumentBefore + [P-0011](../src/adrs/P-0011-logging-facade.md) (ships instrumented, tracing facade); the P-0007 structured-attribution precedent.*

- **R-0091-a (closed error enum):** Distribution rejections SHALL be a closed, structured error type with one distinct variant per rejection class — at minimum (25 variants): `manifest_malformed` (**scope: the OUTER OCI image manifest** — fails JSON parse or image-manifest schema validation; fabrication: truncated/garbage bytes at the manifest reference), `inner_manifest_malformed` (the config blob fails TOML parse or canonical-body slicing; fabrication: garbage bytes as the config blob), `artifacts_missing` (R-0086-d), `hash_alg_unsupported` (R-0086-a), `hash_missing` (R-0086-a), `annotation_duplicate` (R-0086-c), `component_not_first` (R-0078-d), `bound_exceeded { dimension }`, `artifact_type_mismatch`, `media_type_mismatch`, `compressed_blob_rejected`, `index_rejected`, `tag_unpinned`, `signature_missing`, `signer_unknown`, `signature_invalid`, `digest_mismatch_distribution`, `digest_mismatch_provenance`, `blob_missing`, `binding_incomplete_unbound_blob`, `binding_incomplete_missing_blob`, `identifier_invalid` (inner `id` grammar violations and inner duplicate ids), `legacy_manifest_rejected`, `legacy_layout_rejected`, `store_unavailable`. Every rejection is fail-closed; there is no warning-and-proceed severity on this surface. **Variant precedence (one crafted input violating several rules):** the reported variant is the first check to fire in the R-0083-a canonical stage order, and within a stage the check whose inputs are available earliest — so a bundle that is *both* non-wasm-first *and* cross-anchor-media-type-inconsistent (outer `layers[0].mediaType = application/json`, inner entry #1 `media_type = application/wasm`) reports `component_not_first` (an outer-manifest-only check, evaluable before the inner TOML is parsed) ahead of `media_type_mismatch` (which needs the parsed inner manifest). The one case of two checks unavoidably co-triggered over a single byte-string — the dual-digest mismatch — is pinned separately in R-0087-a (`digest_mismatch_provenance`). **Variant scope:** `artifact_type_mismatch` covers both the outer `artifactType` pin failure and the config-descriptor `mediaType` pin failure (R-0079-b) — one static-pin-failure class, not two variants. *(Binary-observable: an error-path acceptance test exists per variant (intake SC8); the enum is closed — no catch-all `Other(String)` variant — [acceptance test] + [construction + review audit].)*
- **R-0091-b (instrumentation):** Every pipeline outcome — success and each rejection — SHALL emit a structured event on the P-0011 `tracing` facade carrying the bundle reference (digest or ref), transport, stage, and (for rejections) the error variant; no new log store is stood up. **Success-path events SHALL additionally carry the measured bound-dimension values** (artifact count N, total bundle bytes, largest per-blob bytes, outer-manifest bytes at minimum — values the bound checks already compute), so an operator can watch a bundle trend toward a floor rather than discovering it at the rejection wall. *(Binary-observable: each rejection class's test asserts its structured event; the happy-path scenarios assert the success events carry the dimension fields — [acceptance test].)*
- **R-0091-c (rejection is not retry-masked):** The pipeline SHALL NOT silently retry a failed verification stage. A transport-level read failure (`store_unavailable`) MAY be retried within the **pinned bounded policy** ([§ Numeric calibrations](#numeric-calibrations-spec-tier-conservative-floors-config-tunable): retry attempts, backoff, total wall-clock ceiling — config-tunable like every other bound, so tests calibrate it small) before the pipeline fails closed with `store_unavailable`, but a **verification** failure (signature, digest, binding, bounds, schema) is terminal for that fetch attempt — never retried against the same bytes, never downgraded. The pinned retry ceiling is scoped **per read** (per blob / manifest / referrer fetch) and is deliberately **not aggregated into a whole-install wall-clock ceiling** at V0: under *intermittent* (not permanently-down) store flakiness across a multi-artifact bundle the per-read ceilings do not compose into a bound on total install wall-clock, but at V0's `mnemra-echo` scale (N=1, ~2–3 reads) the headroom makes an install-level bound premature. A composed whole-install wall-clock ceiling — a single cap over the bundle's full read set — is therefore **parked** (not silently omitted): it re-examines at the first operator-observed install-appears-hung event under intermittent flakiness, or at the R-0005-e deployment widening (where N and read-count grow past dogfood single-digit scale), whichever fires first. *(Binary-observable: a digest-mismatch fixture observes exactly one verification attempt; an unreachable-store fixture receives `store_unavailable` within the pinned ceiling — [acceptance test].)*

### R-0092 — Install atomicity: the system moves working-state → working-state; no failure or crash point is indeterminate

*Locks the maintainer's 2026-07-11 spec-exit-gate install-atomicity ruling — an explicit naming of a system-state determinacy property the locked fail-closed structure already delivers (Frame OC-6 canonical fail-closed ordering + the single-chokepoint discipline), raised to a named requirement at the gate. Anchors: maintainer ruling (2026-07-11, spec-exit gate, LOCKED); P-GuaranteeByMechanism (determinacy is a structural property of load-eligibility, not a best-effort transaction); P-SecurityLayered (fail-closed at every stage). **Delivered by, and citing — not duplicating —** R-0083-a (fail-closed canonical ordering), R-0083-c (single verification chokepoint: the sole path to load-eligibility), R-0080-d (verification precedes any blob trust), R-0085-b (missing-blob fail-closed), R-0087-b (mediated-gate-only artifact access). The general "working-state → working-state" principle system-wide is routed to canon separately (#2248) and is NOT authored here — this requirement is the install-scoped commitment only. **Disambiguation:** "install atomicity" here is a runtime **system-state** determinacy guarantee, distinct from R-0078's "atomic *unit*" (the bundle's packaging shape) and R-0089's "atomic *flip*" (the one-change code cutover).*

- **R-0092-a (system-state determinacy invariant):** At every observation point of an install — mid-fetch, mid-verify, mid-unpack, post-crash, post-failed-retry, and restart-after-kill — the host's serving state and its on-disk load-eligible state SHALL be a determinate, well-defined **working** state: either the **prior working state** (the load-eligible plugin set that held before the install attempt — for a first install of an absent plugin, the plugin-absent state) or the **new working state** (the verified bundle handed to D6, the plugin Ready). An install SHALL transition working-state → working-state; no failure, verification rejection, crash, or kill point SHALL leave the serving or load-eligible state indeterminate, nor expose a partially-installed plugin as load-eligible. *(Binary-observable: for each failure/crash fixture — a forced verification failure, a partial (N-1-of-N) bundle, a terminal `store_unavailable`, and a SIGKILL during fetch/unpack — the host's load-eligible plugin set after the event equals exactly one determinate working state (here the prior state, since none complete), never a partial or indeterminate set — [acceptance test].)*
- **R-0092-b (load-eligibility flips only at the chokepoint; residue is inert):** A bundle becomes load-eligible **only** by passing the full R-0083-a pipeline through the single R-0083-c chokepoint, and artifact bytes reach a consumer **only** through the R-0087-b mediation gate — so the flip from prior to new working state is the pipeline's `Ok`, and no intermediate state exists in which a partially-fetched, partially-verified, or partially-unpacked bundle is load-eligible. Staging or fetch residue left by a failed, crashed, or killed attempt is therefore **inert**: on the next host start it is either cleaned or safely ignored (whichever the cache-write substrate does — this requirement mandates *no* specific cleanup or commit step, only that the residue cannot make the load decision ambiguous), it SHALL NOT make the host's load decision ambiguous, and it can never become load-eligible outside R-0083 (a partial residue somehow presented as a plugin simply fails the pipeline via an existing R-0091-a variant — **no new rejection variant is introduced; the enum stays 25**). *(Binary-observable: the verified-bundle token is constructible only inside the seam (R-0083-c, already audited) and no consumer path reads the cache directly (R-0087-b, already audited) — [construction + review audit]; after SIGKILL during fetch/unpack and restart, the host serves exactly the prior working state, the residue is inert, and a subsequent install of the same bundle runs the full pipeline and reaches Ready — [acceptance test]. The cache-write recovery discipline the "subsequent install succeeds" leg depends on is named as a plan-tier carry, [§ Out of Scope](#out-of-scope), not locked as a requirement here.)*
- **R-0092-c (retries operate inside the atomicity envelope; determinacy is not duration):** The R-0091-c transport-read retry policy (the pinned ⊕ row — 3 attempts per read · exponential backoff from 100 ms · 10 s per-fetch wall-clock ceiling — **unchanged** by this requirement) operates **inside** the atomicity envelope: however a retry resolves — a later attempt succeeding, or the terminal `store_unavailable` firing — the operation's outcome is determinate and the R-0092-a invariant holds throughout the retry/backoff window (the host continues to serve its prior working state; no partial bundle is load-eligible at any point in the window). Atomicity here is a **determinacy** guarantee, not a **duration** guarantee: the R-0091-c parked whole-install wall-clock non-decision (a bound on total install *time* across a multi-read bundle) is a separate question and remains **parked, untouched** — a bounded-determinate outcome does not require a bounded total duration. *(Binary-observable: throughout an unreachable-store fixture's retry/backoff window and after the terminal `store_unavailable`, the host's load-eligible set equals the prior working state and no partial bundle is load-eligible — [acceptance test], extending the R-0091-c unreachable-store scenario.)*

## Out of Scope

The agent SHALL NOT build any of the following, even where adjacent (each carries its trip-wire per the Frame's deferral table — decision content and canon anchors live there):

- **TUF mechanism** (`tough`, timestamp/snapshot freshness, delegated roles, offline root) — the `PackageVerifier` seam is its slot; fires at R-0005-e. Rollback/downgrade protection is an accepted residual until then; the implementation SHALL NOT add a bespoke version-monotonicity check "meanwhile."
- **Third-party publisher support** — non-`core` distribution, distribution-key split, delegated roles; R-0005-e territory.
- **Update / upgrade flow** — the operation that replaces an already-installed plugin's bundle with a newer version is outside this spec's locked intake scope (the two install use-cases only — LAN-registry fetch and air-gapped-filesystem install — re-verified 2026-07-11; scope does **not** widen here). **The R-0092 system-state determinacy invariant is forward-binding on it:** when the update flow is authored at a later tier, the old version SHALL keep serving until the new bundle is verified-ready, the swap SHALL be atomic (working-state → working-state), and a failed or crashed update SHALL leave the old version serving. Binding invariant, deferred flow — the invariant is locked **now** (R-0092), the flow is authored when the update use-case enters scope. This is not a deferral of the determinacy *decision* (locked) but of the *flow* it will bind. Tripwire: the first update/upgrade use-case entering intake scope (the R-0005-e multi-deployment widening is the likely carrier).
- **Sigstore keyless / transparency log** — forward-allowance on R-0005-e; no cosign CLI, no Fulcio/Rekor dependency.
- **SLSA / in-toto provenance attestations** — fires at #1942 (reproducible builds). The interim lockfile-hash-pin weighing is a named spec-gate item resolved at this spec's review, not silently built.
- **Registry product selection / deployment story** — any OCI-compliant registry satisfies R-0081; picking and operating one is the deployment brief's scope (intake Non-goal 7). Acceptance fixtures use a self-hostable registry without endorsing it.
- **Tier-C custody hardening** (`{{P-SigningKeyCustodyHardening}}`) — stays unauthored; W2-opt remains parked.
- **Image-index bundle shape** — recorded forward path (OC-8); rejected at this tier (R-0078-c).
- **Compressed blobs** — rejected at V0 (R-0079-d); admitting compression is a spec amendment carrying the decompression caps.
- **Bundle-cache retention/eviction policy** — the local layout cache grows per fetched bundle; unbounded at dogfood scale (single-digit bundles). Named non-control; re-examines at the first operator-observed cache-pressure event or the R-0005-e deployment widening, whichever fires first.
- **The companion ADR and amendment texts** — `{{P-PluginDistribution}}`, the P-0003 `[[artifacts]]` amendment, the P-0019 DEF-2 disposition line, risk-register entries, and the architecture-overview DFD landing are **round 2** of this Stage-3 package.

**Named plan-tier carries** (anchored here so none evaporates — DF1; each is a committed-tier item, not a spec gap): (a) a **runbook for the R-0090 builder tool** alongside the existing signing-ceremony runbook, same What/Blast-radius/Pre-checks/Steps/Post-checks/Rollback shape; (b) **cache directory location + file mode** — confirm the existing plugin-loading substrate's convention pins them, else pin at plan (the cache holds signed public content, not secrets; convention, not a security gate); (c) **deploy sequencing** — confirm the new binary + re-packaged bundle set arrive together unambiguously on the single dogfood node at the flip; (d) a **mechanized retirement check** backing R-0089-c's audit (e.g. a CI grep over the test corpus for bare-`.wasm`-load success assertions); (e) **cache-write recovery discipline** — the digest-idempotent / temp-then-rename write posture that R-0092-b's "a subsequent install of the same bundle succeeds after a killed attempt" leg depends on (a re-fetch must complete or overwrite a partial blob at its digest path, not skip it as already-present); confirm the plugin-loading substrate's cache-write path already provides it, else pin at plan. Distinct from the parked cache *retention/eviction* non-control above (that is about growth; this is about partial-write recovery) — the R-0092 determinacy invariant itself does not depend on it (residue is inert regardless), only the crash fixture's re-install-liveness leg does.

## Scenarios

### Scenario: LAN registry install, end-to-end (R-0080/R-0081/R-0083/R-0090; intake use-case a; SC2, SC3)

**Given** a bundle published to a self-hosted registry fixture by the R-0090 tool, and a host with the pinned root
**When** the operator points the host at the registry reference and triggers install
**Then** the pipeline runs fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds, hands the verified bundle to D6, and the plugin reaches Ready — with no network egress beyond the LAN fixture — **and** the R-0091-b success events exist for each stage (signature-verify, digest-verify) carrying the expected bundle digest, transport = registry, and the measured dimension values (a `PackageVerifier` stub returning unconditional `Ok` cannot green this scenario: the stage events it never emitted are asserted).

### Scenario: air-gapped filesystem install (R-0081-a/-b; intake use-case b; SC3)

**Given** the same bundle as an `oci-layout` directory on removable media, and no network available
**When** the host is pointed at the directory
**Then** the identical pipeline verifies and loads it, every verification outcome equals the registry case, **and** the R-0091-b per-stage success events match the registry run's (transport = filesystem) — stage-for-stage evidence of real verification, not an end-state-only assert.

### Scenario: unsigned bundle fails closed at fetch (R-0080-c; SC2)

**Given** a bundle in the store with no signature referrer
**When** fetch runs
**Then** the pipeline rejects with `signature_missing` before any blob is unpacked, and the structured event carries the stage and variant.

### Scenario: re-signed by an unknown key (R-0080-c; SC2)

**Given** a bundle whose signature referrer verifies cryptographically but whose signer key is not the pinned root
**When** fetch runs
**Then** the pipeline rejects with `signer_unknown` — a valid-but-foreign signature is not a fall-through.

### Scenario: substitution at the distribution anchor, fetch-time (R-0081-d; SC5 distribution half)

**Given** a registry fixture serving a tampered blob whose `Docker-Content-Digest` header claims the expected digest
**When** fetch runs — at this stage the provenance gate has not yet run *by the R-0083-a ordering* (it is a later, separate gate at load; nothing is "disabled")
**Then** the received-bytes digest recomputation rejects with `digest_mismatch_distribution`.

### Scenario: post-fetch cache tamper caught at load (R-0087-a; SC4)

**Given** a bundle fetched and verified clean, then one non-wasm blob modified in the local cache (the distribution-layer fetch check already passed)
**When** load runs
**Then** the load gate rejects — both digests are recomputed over the tampered bytes and both mismatch; the reported variant is `digest_mismatch_provenance` per R-0087-a's precedence rule — and no consumer sees the tampered bytes. (Per-anchor isolation — each anchor rejecting with the other absent — is exercised at the R-0087-a feature-gated seam tier, not constructible black-box; see the gate-flag in §Intent self-report.)

### Scenario: every bound dimension rejects at floor+1 (R-0084-a/-b — table-driven, all dimensions)

**Given** for **each** dimension in the [§ Numeric calibrations](#numeric-calibrations-spec-tier-conservative-floors-config-tunable) table — outer-manifest bytes, referrers-listing bytes, referrers count, per-referrer bytes, inner-manifest (config) bytes, artifact count N, per-blob bytes, total bundle bytes — a fixture constructed at floor+1 for that dimension (one parametrized scenario over the table, so no dimension is silently skipped)
**When** fetch/unpack reaches the read that dimension gates
**Then** the read aborts at the bound with `bound_exceeded { dimension }` naming that dimension — no parse, no verifier sees the bytes (for the oversized-manifest case: cut off before parse; for oversized-N: rejected at manifest validation before any blob read). The referrers-listing / index-document dimension is transport-specific — `index.json` on the fs transport, the Referrers API response on the registry transport — so its floor+1 fixture is realized once per transport surface under R-0081-a's both-transports mandate; the other seven dimensions are transport-invariant and one fixture each suffices.

### Scenario: partial bundle rejected (R-0085-b; invariant #8)

**Given** a bundle with one of N layer blobs absent from the store
**When** fetch runs
**Then** the pipeline rejects with `blob_missing`; nothing is handed to D6 — **and** the host's load-eligible set is unchanged: the prior working state holds (R-0092-a), the failed attempt leaving nothing load-eligible.

### Scenario: unbound blob rejected — under-binding direction (R-0085-c)

**Given** a bundle whose outer manifest carries a layer with no matching inner `[[artifacts]]` entry
**When** load runs
**Then** the gate rejects with `binding_incomplete_unbound_blob` — no blob escapes the provenance anchor.

### Scenario: entry with no layer rejected — over-binding direction (R-0085-c)

**Given** a bundle whose inner `[[artifacts]]` manifest declares an entry (`id = "missing-data"`) with no outer layer descriptor carrying a matching `vnd.mnemra.artifact.id` annotation
**When** load runs
**Then** the gate rejects with `binding_incomplete_missing_blob` — the other half of R-0085-c's one-test-per-direction mandate.

### Scenario: malformed outer manifest rejected before any trust decision (R-0091-a)

**Given** a store serving truncated or non-JSON bytes (or JSON failing OCI image-manifest schema validation) at the manifest reference, within the size bound
**When** fetch parses the manifest
**Then** rejection with `manifest_malformed` (outer scope); a config blob of garbage bytes on an otherwise well-formed bundle instead rejects with `inner_manifest_malformed` at load.

### Scenario: unreachable store fails closed within the pinned ceiling (R-0091-c)

**Given** a store fixture that refuses connections (or hangs past the per-read timeout), and the calibration-table retry policy at its test-calibrated values
**When** fetch runs
**Then** the pipeline retries within the pinned attempts/backoff, then rejects with `store_unavailable` inside the total wall-clock ceiling — no indefinite hang, no silent success — **and** throughout the retry/backoff window and after the terminal `store_unavailable`, the host serves its prior working state with no partial bundle load-eligible (R-0092-c: retries operate inside the atomicity envelope).

### Scenario: bare load path is gone post-cutover (R-0078-b/R-0089; SC1)

**Given** the pre-cutover filesystem shape (bare `.wasm` + TOML manifest) present on disk post-flip
**When** the loader scans for plugins
**Then** it rejects with `legacy_layout_rejected`; the re-packaged bundle set is the only thing that loads.

### Scenario: legacy `[component]` manifest rejected (R-0086-e)

**Given** a signed manifest carrying a `[component]` section (with or without `[[artifacts]]`)
**When** the manifest parses at load
**Then** rejection with `legacy_manifest_rejected` — one binding schema is in force.

### Scenario: mutable tag cannot swap the loaded bundle — pinned branch (R-0081-c)

**Given** a tag resolved to digest D, then repointed by a registry-write attacker to digest E before fetch — where D and E carry **genuinely distinct artifact bytes**
**When** the pipeline fetches for load
**Then** everything verified and loaded is anchored on D; the repoint is inert; no error fires — and the loaded artifact content equals D's bytes, not E's (a stub that ignored the digest pin and followed the tag to E would load E's distinct bytes and fail this assertion — the anti-vacuity check, decidable from artifact content without extra instrumentation).

### Scenario: bare tag with no digest resolution refused — unpinned branch (R-0081-c)

**Given** a fetch-for-load attempted against a bare mutable tag where no digest-resolution step was (or could be) performed
**When** the pipeline evaluates the reference
**Then** the fetch refuses with `tag_unpinned` — the two branches are distinct behaviors with distinct outcomes, not alternatives.

### Scenario: remaining rejection variants fire from their requirement-prose fabrications (uniformity sweep)

**Given** one fixture per remaining variant, built exactly from its requirement's stated fabrication: a cross-anchor media-type disagreement (`media_type_mismatch`, R-0079-c) · a `+gzip` layer (`compressed_blob_rejected`, R-0079-d) · an image index as plugin (`index_rejected`, R-0078-c) · a corrupted pinned-root signature (`signature_invalid`, R-0080-c) · an id with a separator / leading dot / length 65 (`identifier_invalid`, R-0086-b) · a zero-entry `[[artifacts]]` body (`artifacts_missing`, R-0086-d) · `hash_alg = "sha256"` (`hash_alg_unsupported`, R-0086-a) · an entry with no `hash` key (`hash_missing`, R-0086-a) · two layers sharing one annotation id (`annotation_duplicate`, R-0086-c) · `layers[0] = application/json` (`component_not_first`, R-0078-d)
**When** the pipeline (or load gate) reaches the violated rule
**Then** each rejects with exactly its named variant — closing SC8's test-per-class over the full 25-variant enum.

### Scenario: confused deputy blocked on artifactType (R-0079-b; invariant #6)

**Given** a plain container image (no mnemra artifactType) at the configured reference
**When** fetch runs
**Then** rejection with `artifact_type_mismatch` before signature work begins.

### Scenario: kill mid-install leaves a determinate working state (R-0092-a/-b; system-state determinacy)

**Given** a host serving its prior working state (the load-eligible plugin set that held before the attempt) and an install of a new bundle in progress
**When** the host process receives SIGKILL during fetch or unpack, then restarts
**Then** the host serves exactly the prior working state; any staging/fetch residue is inert (cleaned or safely ignored — never load-eligible outside the R-0083 pipeline, R-0083-c); and a subsequent install of the same bundle runs the full pipeline and reaches Ready — no observation point (mid-install, post-kill, post-restart) exposed a partial or indeterminate state. (Residue can never be load-eligible because load-eligibility requires passing R-0083 through its single chokepoint; the "subsequent install succeeds" leg depends on the cache-write recovery discipline named as plan-tier carry (e), [§ Out of Scope](#out-of-scope).)

## Data Model

No database tables — this layer's persistent shapes are file/wire formats.

**Outer bundle manifest (OCI image manifest, image-spec 1.1):**

| Element | Value / constraint |
|---|---|
| `artifactType` | `application/vnd.mnemra.plugin.v1` (pinned, R-0079-b) |
| `config` | descriptor → signed inner TOML manifest blob; `mediaType = application/vnd.mnemra.plugin.manifest.v1+toml` |
| `layers[0]` | descriptor → component `.wasm`; `mediaType = application/wasm`; annotation `vnd.mnemra.artifact.id` = inner entry #1 id |
| `layers[1..N-1]` | descriptors → artifact blobs; declared media types (R-0079-a); annotation `vnd.mnemra.artifact.id` = joined inner id (unique, R-0086-c) |
| digests | `sha256` content addresses; verified over received bytes (R-0081-d) |

**Signature referrer (OCI 1.1 referrer manifest):**

| Element | Value / constraint |
|---|---|
| `artifactType` | `application/vnd.mnemra.signature.v1` |
| `subject` | the signed bundle manifest's descriptor |
| blob | 64-byte raw Ed25519 signature over `"mnemra-oci-manifest-v1:" || <alg>:<hex>` (R-0080-a) |
| annotation `vnd.mnemra.signer.fingerprint` | the signer public-key fingerprint (selection hint only — pinning verifies the key itself, R-0080-c) |

**Inner `[[artifacts]]` schema:** per R-0086-a's table (the P-0003 amendment shape; amendment text round 2).

## API Contract

No MCP tool, no HTTP endpoint — this layer's contracts are host-internal seams plus the build-host CLI surface.

- **`PackageVerifier`** (R-0083-b): manifest descriptor + store → verified-bundle token | `DistributionError`. One implementation at V0; the TUF adapter composes behind it at R-0005-e. Token constructible only inside the seam (R-0083-c).
- **Store contract** (R-0081-a): resolve-ref → descriptor; fetch-manifest; enumerate-signature-referrers(digest) — bounded per R-0084; read-blob(digest) — bounded, received-bytes-verified. Two implementations at V0 (image-layout, distribution API); transport identity confined behind the contract. Credential injection (should a future registry require auth — bearer token, mTLS) composes at the transport-implementation layer *behind* the contract, never through its surface — a conscious non-decision recorded now (the TUF-seam shape), not an unexamined gap.
- **Mediated artifact access** (R-0087-b): the load-side gate yields verified artifact bytes per `[[artifacts]]` entry; the only consumer-facing artifact-access surface.
- **Builder CLI** (R-0090): build-host tool — assemble layout, sign, publish. Exact flags are implementation-tier; the three operations and their custody boundary (signing only on the build host) are the contract.
- **Error taxonomy:** the closed R-0091-a enum, one grammar across fetch, unpack, and load-gate surfaces.

## Threat-model DFD (typed anchors)

Typed element IDs for the new surface, proposed here per the sibling convention (P-0003/P-0005/P-0019 cite typed elements); the architecture-overview landing and the new ADR's Threat-references section consume these in round 2:

| ID | Type | Element |
|---|---|---|
| `EE-plugin-store` | external entity / zone | the store (registry or removable-media layout) — untrusted by design; trust derives from signatures + digests, never from the store |
| `P-bundle-builder` | process | build-host assemble/sign/publish tool (R-0090); lives inside `TB-build-pipeline` |
| `P-fetch-verify` | process | the R-0083 pipeline behind `PackageVerifier`; lives inside `TB-mnemra-host` |
| `DS-oci-store` | data store | the bundle content (manifest, blobs, referrer) at rest in the store zone |
| `DS-bundle-cache` | data store | the local fetched-layout cache the load path reads (invariant #5's re-validation surface) |
| `DF-publish` | data flow | builder → store (crosses `TB-build-pipeline` → store zone) |
| `DF-fetch` | data flow | store → host pipeline (crosses store zone → `TB-mnemra-host`; the bounds-first surface) |
| `DF-referrers` | data flow | signature-referrer enumeration (bounded, R-0084-a) |
| TB extension | trust boundary | the store zone sits between `TB-build-pipeline` and `TB-mnemra-host`; both crossings are verification surfaces |

## Numeric calibrations (spec-tier; conservative floors; config-tunable)

Per the P-0007 convention: conservative floors, tune-up-only, the security argument valid at the floor. **The seven r1 values were ratified as-set by the maintainer — 2026-07-07 pre-gate ruling (task #2228, activity 3213). The three rows marked ⊕ were added at the r1 review fold (review findings, same conservative-floor method) and await the same ratification at the gate.** V0 reality: `mnemra-echo` has **N=1** (one artifact: the component; N counts `[[artifacts]]` entries == layers) and its bundle carries 2 blobs — 1 config (inner manifest) + 1 layer — with the component well under 5 MiB.

| Bound (R-0084-a) | V0 floor | Rationale |
|---|---|---|
| Outer-manifest byte size | 1 MiB | descriptors + annotations for N≤64 fit in tens of KiB; 1 MiB is generous headroom |
| ⊕ Referrers-listing / index-document byte size | 4 MiB | the listing read (`index.json` / Referrers API response) parsed to enumerate; must be bounded before the count bound can fire (review fold, W-M4) |
| Referrers enumeration count | 16 referrers considered | one signature expected; 16 tolerates re-signs/rotation debris without unbounded scan |
| Per-referrer manifest size | 64 KiB | a signature referrer is ~1 KiB |
| ⊕ Inner-manifest (config blob) byte size | 1 MiB | parse-time TOML metadata gets a metadata-scale cap, not the 64 MiB data-blob cap (review fold, W-L4) |
| Artifact count N | 64 | an order of magnitude above any foreseen bundle; counts `[[artifacts]]` entries == layers, config excluded |
| Per-blob byte size | 64 MiB | component + generous data artifacts; aligned with P-0007's 64 MiB instance-memory scale |
| Total bundle byte size | 256 MiB | N × typical blob with headroom; caps the whole fetch |
| Decompression ratio | n/a at V0 | closed by construction — uncompressed-only (R-0079-d); the cap requirement travels with any future compression amendment |
| ⊕ Transport read retry (`store_unavailable`) | 3 attempts per read · exponential backoff from 100 ms · 10 s total wall-clock ceiling per fetch attempt | the one runtime dependency this layer adds gets a pinned fail-fast posture like every other bound (review fold, Bolt-M2 + Glitch convergence); config-tunable, so tests calibrate it small |

## Decisions of note

1. **Inner manifest rides as the OCI config descriptor** (R-0078-a) — not as a layer. The config slot is OCI's artifact-metadata position; it keeps "N artifacts" == "N layers" clean (the inner manifest binds the layers and is not itself an `[[artifacts]]` entry — it is vouched by the Ed25519 signature it carries, and its blob digest is bound by the signed outer manifest). *Ratified by the maintainer — 2026-07-07 pre-gate ruling (task #2228, activity 3213).*
2. **Uncompressed-only at V0** (R-0079-d) — closes the decompression-bomb class by construction and makes artifact bytes == blob bytes at both anchors (one read, two digests, R-0087-a). The Frame's decompression-ratio bound is satisfied at its most conservative floor. *Ratified by the maintainer — 2026-07-07 pre-gate ruling (task #2228, activity 3213).*
3. **Entry↔blob join via `vnd.mnemra.artifact.id` annotation** (R-0086-c) — the inner signed `id` is authoritative (R-0005-h residence); the annotation is its projection at the distribution anchor, covered by the package signature. Chosen over positional/order joining (fragile under manifest tooling) and digest-joining (inner blake3 vs outer sha256 hash the same bytes under different algorithms — joinable but opaque; the id also drives materialization naming, R-0085-a). *Ratified by the maintainer — 2026-07-07 pre-gate ruling (task #2228, activity 3213).*
4. **`[component]` strict rejection post-cutover** (R-0086-e) — rejected rather than ignored: a carried-but-unparsed legacy table would let the two binding representations disagree silently. One schema in force. *Ratified by the maintainer — 2026-07-07 pre-gate ruling (task #2228, activity 3213).*
5. **R-0082 skipped** in the series — drop-tombstone in the reporting-engine spec; assigning it would break identifier-resolution uniqueness (AP1).
6. **`store_unavailable` retry carve-out** (R-0091-c) — transport reads may retry bounded; verification failures never. Keeps fail-closed strict without making LAN blips fatal to an install attempt.
7. **Install atomicity — system-state determinacy** (R-0092) — install moves the host working-state → working-state; no failure, crash, or kill point leaves the serving or load-eligible state indeterminate, and a partial/killed attempt's residue is inert (never load-eligible outside R-0083). Delivered by the existing single-chokepoint structure (R-0083-c) + load-time re-verification (R-0087), **named** as a requirement — not new transaction mechanism. Update is forward-bound by the same invariant but out of this spec's install-only intake scope ([§ Out of Scope](#out-of-scope)). *Ratified by the maintainer — 2026-07-11 spec-exit-gate ruling.* The general working-state → working-state principle system-wide is routed to canon separately (#2248), not authored here.

## Intent self-report

**I read the JTBD at spec altitude as:** make the plugin a distributable unit whose whole-bundle provenance and integrity are established before unpacking — mechanically: one bundle shape (R-0078), one signature at the distribution anchor verified under key pinning before anything is trusted (R-0080), two self-hosted transports behind one contract (R-0081), a bounds-first pipeline where no verifier ever receives bytes an attacker could size-attack with (R-0083/R-0084), and every blob re-verified at the provenance anchor at load with the same discipline the wasm always had (R-0086/R-0087). The requirements change the existing gates' *coverage*, never their *primitive* (R-0087-c, R-0088).

**Strains, named:** (1) R-0079-d (uncompressed-only) realizes the Frame's decompression-ratio bound by eliminating decompression rather than capping it — within OC-12's fail-closed posture but a posture choice the Frame did not pre-decide; ratified by the maintainer (2026-07-07 pre-gate ruling), not silently locked. (2) R-0091-c's bounded transport-retry carve-out touches "fail-closed everywhere" (intake SC8) — scoped strictly to transport reads (never verification), named rather than buried. (3) **GATE-FLAG — intake SC5's "substitution tests at each anchor, with the other anchor disabled" is realized at the feature-gated test-hooks seam tier, not as a shipped-runtime toggle** (r1 review fold, Glitch H1): both load-time digests are deterministic functions of the same single-read bytes, so no black-box input trips one anchor while passing the other, and a runtime disable switch would be exactly the verification bypass R-0083-c forbids (a fail-open hazard). The faithful reading implemented in R-0087-a: per-anchor isolation via independently-invokable digest-check units under the existing `test-hooks` feature (bare `#[cfg(feature = "test-hooks")]`) + `no_test_seams` pattern — with two new red-phase compile-fail fixtures added to that gate file for the two digest-check seams — plus the black-box cache-tamper test and the construction audit that neither check can short-circuit the other. The maintainer confirms or re-directs this reading at the gate. No Non-goal is crossed; the seven distribution Non-goals hold as scoped, and no requirement re-opens a maintainer lock.

**Late-arriving gate ruling folded (2026-07-11).** After the three-lens r2 verification closed, at his spec-exit-gate read the maintainer ruled that install (and, forward, update) must be **atomic** — the system moves working-state → working-state, operations never leaving the state indeterminate. This landed post-r2 as **R-0092** (install-scoped system-state determinacy — an explicit naming of a property the fail-closed single-chokepoint structure already delivered, not new mechanism), with the **update flow carried forward-binding** in [§ Out of Scope](#out-of-scope) (binding invariant, deferred flow — the install-only intake scope did not widen) and the retry policy left numerically unchanged (R-0092-c only frames the existing ⊕ row as operating inside the atomicity envelope; the R-0091-c parked whole-install wall-clock non-decision is untouched — atomicity is determinacy, not duration). The general system-wide "working-state → working-state" principle is routed to canon separately (#2248), not authored here. Honesty note: this input arrived at the gate, *after* r2 — a maintainer novel-judgment lock folded at the exit, not a Frame mapping.

## Provenance

- Frame: `docs/intent/plugin-distribution-frame.md`, blob `60c437c5e569eacbf00329e92cae2e7c42cebba6` (LOCKED 2026-07-08Z, internal_conformance=pass).
- Intake: `docs/intent/plugin-distribution.md`, blob `9c8e1577ed345cbcef546ba51d252a0df4db1144` (LOCKED 2026-07-07).
- Research: the plugin artifact-repository + package-signing survey (locked 2026-07-01, r3) — carried through the Frame; §4 invariants #1–#10 land here per the Frame's landing map.
- Base pin: `design/plugin-distribution@3b4efd8`; R-ID series continues from `main@8f27cc5` (R-0077).

## Changelog

- **2026-07-11 (e)** — Maintainer install-atomicity ruling folded at the spec-exit gate (post-r2). The maintainer ruled (verbatim, four messages): *"i think the install of a plugin should be atomic"* · *"and the retries should be at that atomic level … the backoffs and number are fine, its the atomicity we may want to adjust"* · *"both install and update should be atomic, in general where possible the system should move from working state to working state, operations should not leave the state indeterminate"* · *"the state of the system to be even more clear, i hope."* Landed as new **R-0092** (install-scoped system-state determinacy: at every observation point — mid-install, post-crash, post-failed-retry, restart-after-kill — the host is in a determinate prior-or-new working state; residue from a failed/killed attempt is inert and never load-eligible outside R-0083; an explicit **naming** of a property R-0083-c + R-0087 already deliver, not new mechanism). **Update carried forward-binding** in § Out of Scope (old version serves until the new bundle is verified-ready, swap atomic, failure leaves old serving) — binding invariant, deferred flow; install-only intake scope unchanged. **Retry policy numerically unchanged** (the ⊕ 3 × 100 ms × 10 s row stays exactly as-is; R-0092-c only frames it as operating inside the atomicity envelope — per the maintainer's own "backoffs and number are fine"). **R-0091-c parked whole-install wall-clock non-decision untouched** (atomicity = determinacy, not duration). New crash-determinacy scenario added; partial-bundle and unreachable-store scenarios gain the system-state assertion. New plan-tier carry (e): the cache-write recovery discipline the crash fixture's re-install leg depends on. Enum **stays 25 variants** (no new rejection class). General working-state → working-state principle routed to canon separately (#2248), not authored here. §Intent self-report records the gate-arrival. R-ID range R-0078–R-0091 → **R-0078–R-0092** (R-0082 still tombstone). Edits confined to `docs/specs/2026-07-07-plugin-distribution.md`; no `docs/src/**` page touched. Flagged for Puck (not edited — translate-gate regen is Puck's): P-0023's stale R-range citations (Context, More Information) + a D4/D5 consequence line — proposed text in the dispatch report.
- **2026-07-07 (d)** — r2 review fold (three-lens r2 verify: Warden CLEAN + 2 nit · Bolt CLEAN + 1 low 1 nit · Glitch CONCERNS 1 HIGH + 4 low/nit; r1 resolutions re-confirmed 10/10 · 8/8 · 8/8). Every union finding folded; nothing dropped. **G-HIGH (Glitch NF-1, Puck-adjudicated CONFIRMED — overrides Bolt's contrary TS2 disposition):** R-0087-a's per-anchor-isolation instrument cited a non-existent test seam (`test-support`, `cfg(any(test, …))`); re-cited to the codebase's sole real convention — the `test-hooks` cargo feature with bare `#[cfg(feature = "test-hooks")]` (name LOCKED per `Cargo.toml` + `no_test_seams.rs` "Shared tokens — do not rename"), and named the two new `no_test_seams.rs` `compile_fail` fixtures (six exist today) as explicit red-phase work to add to that gate file (its "green does not edit this file" contract makes the fixture-list edit red-phase touch_scope). §Intent self-report gate-flag citation corrected to `test-hooks` (the SC5 gate-flag itself preserved — maintainer rules at the gate, INTENTIONAL GAP #2). **Glitch NF-5:** the third instrument label added to the Verification-instrument-classes taxonomy. **Bolt N-1 (LOW):** the per-read retry ceiling does not compose into a whole-install wall-clock bound under intermittent flakiness — recorded as an explicit **parked** non-decision in R-0091-c (V0 N=1 headroom; tripwire: first operator-observed install-appears-hung, or R-0005-e deployment widening), no new ⊕ number (the three ⊕ rows stay three, INTENTIONAL GAP #1). **Bolt N-2 (NIT):** L5/L3 gloss added to R-0090-d ("intake SC7's dependency-gate severity numbering"). **Warden S-1 (NIT):** `component_not_first` ↔ `media_type_mismatch` precedence stated in R-0091-a (first-failing in stage/input-availability order → `component_not_first`, an outer-only check, precedes the inner-parse-dependent `media_type_mismatch`; dual-digest co-trigger carved out to R-0087-a). **Warden S-2 (NIT):** config-descriptor media-type mismatch dispositioned into `artifact_type_mismatch` (one static-pin-failure class) — named in R-0079-b, scope-noted in R-0091-a; enum stays 25 variants. **Glitch NF-2 (LOW):** `artifacts_missing` named for the signed-slice-only trigger in R-0086-d. **Glitch NF-3 (LOW/nit):** Scenario 7 notes the referrers-listing dimension is transport-specific (fixture per transport under R-0081-a). **Glitch NF-4 (nit):** Scenario "mutable tag … pinned branch" strengthened with the D≠E distinct-bytes anti-vacuity assertion. Edits confined to `docs/specs/2026-07-07-plugin-distribution.md`; no `docs/src/**` page touched (translate gate untouched).
- **2026-07-07 (c)** — r1 review fold (Warden 5M/3L/2N + Bolt 2H/2M/4L + Glitch 3H/4M, all folded, zero dismissed-without-rationale). Error enum 19 → **25 variants** (`inner_manifest_malformed`, `artifacts_missing`, `hash_alg_unsupported`, `hash_missing`, `annotation_duplicate`, `component_not_first`) with `manifest_malformed` scoped to the outer document; `hash_alg` narrowed to blake3-only-at-V0 (sha256/384/512 reserved, rejected on sight); R-0078-d component-first load rejection; R-0084-a gains the referrers-listing and inner-manifest byte dimensions + the N-counts-artifacts-not-blobs clarification (calibration "N=2" corrected to N=1/2-blobs); R-0087-a gains dual-mismatch variant precedence + the per-anchor-isolation instrument split (feature-gated seam + construction audit — **SC5 reading gate-flagged**, §Intent self-report); R-0089-c retains one frozen pre-OCI fixture; R-0090-d makes the SC7 L5/L3 dependency gates explicit acceptance evidence; R-0091-b success events carry measured dimensions; transport-retry policy pinned (3 × exp-backoff-100 ms × 10 s, ⊕); scenarios 13 → **18** (over-binding direction, malformed-manifest, store-unavailable, tag branches split, uniformity sweep; S1/S2 backed by stage events; S7 table-driven over all dimensions); plan-tier carries named (runbook, cache loc/mode, deploy sequencing, mechanized retirement check); store-contract auth non-decision recorded; three ⊕ calibration rows await gate ratification. Docs-translate regen run as batch companion.
- **2026-07-07 (b)** — Pre-gate ratifications folded (maintainer ruling, task #2228, activity 3213): bound floors as-set; uncompressed-only V0; config-descriptor placement + `vnd.mnemra.artifact.id` join key; strict reject of legacy `[component]`. Marker-only sweep — no requirement text changed.
- **2026-07-07** — Initial draft (Stage 3, single-pass designer session). R-0078–R-0091 (R-0082 skipped — tombstone). Round 2 of this package: `{{P-PluginDistribution}}` ADR, P-0003 `[[artifacts]]` amendment, P-0019 DEF-2/D6 lines, risk-register entries, overview DFD landing.
