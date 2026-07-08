# Frame: Plugin distribution — uniform OCI packaging · package signing · verified fetch

**Locked: 2026-07-07**

**Date:** 2026-07-07
**Status:** locked (Frame-exit gate accepted by the maintainer 2026-07-07; Stage 2 review: security reviewer r1 needs-revision — 1 high + 3 medium + 2 low + 2 nit, all folded, none dismissed — then r2 GATE-CLEAR, 8/8 resolved-as-claimed, 0 new findings; the two sub-bar S1/S2 hygiene notes kept by maintainer ruling at the gate)
**Modulation:** cold-start
**Intake:** `docs/intent/plugin-distribution.md` (locked 2026-07-07; blob `9c8e1577`, committed `33f4a56`)
**Baseline:** `docs/src/adrs/` P-0002 · P-0003 (+2026-06-30 amendment) · P-0005 (+2026-05-24 amendment) · P-0007 · P-0012 · P-0013 · P-0019 (D6 + DEF-2); workspace architecture values / principles / constraint-edges
**Research:** the plugin artifact-repository + package-signing standards survey (locked 2026-07-01, r3 final)

> This Frame **is** the architecture for the distribution layer. It decides *which constraints operate and why* at architectural altitude. The `[[artifacts]]` schema field-by-field design, exact API signatures, resource-bound numeric values, and crate-level code layout are Stage-3 / spec work and are deferred here with named tripwires, not filled.

---

## Stage 2a — Elicitation

Modulation **cold-start**: a new architectural surface (the distribution layer — packaging, registry/filesystem transport, fetch-time verification) with no prior ADR home. The elicitation substrate is the locked intake, the locked 2026-07-01 research survey, and the maintainer's confirmations below.

### Input record (maintainer, 2026-07-07 — elicitation walk)

- **Architectural shape CONFIRMED:** boundary = build-host (package + sign; P-0005 key custody) → store (filesystem `oci-layout` AND self-hosted registry, behind one contract) → host (fetch → verify → unpack → existing D6 load chain; nothing trusted until signature + digest pass). Components: ① bundle builder/publisher (build-host side); ② store transports behind one contract; ③ fetch-verify pipeline behind the `PackageVerifier` seam; ④ loader integration as a distribution pre-phase feeding the unchanged inner gates. ADR slots: new `{{P-PluginDistribution}}`, P-0003 `[[artifacts]]` amendment, P-0019 DEF-2 disposition.
- **D6 fork RESOLVED:** the distribution lifecycle pre-phase (fetch → verify-package-signature → verify-blob-digests → unpack) is recorded in the **new distribution ADR**; P-0019 D6 gains **one cross-reference line** — the canonical ordering stays single-sourced, no two live framings.

### LOCKED — verbatim, not re-litigable (transcribed per the locked-subject rule)

Maintainer-locked 2026-07-01: **(H1)** "Every plugin is a uniform multi-artifact OCI bundle (N≥1, wasm = artifact #1); one packaging path, one load path; the loader rejects any plugin not presenting a signed OCI manifest binding every artifact. A wasm-only plugin ships as a uniform 1-artifact bundle — the *path* is banned, not N=1 *content*." **(R-NoExternalHost)** "Self-hostability on a filesystem AND a self-hosted server (internal-LAN-only), with zero required round-trip to a host we do not control." **(Decision A)** "The inner signed manifest binds every blob via an `[[artifacts]]`-style N≥1 list, and each blob is verified with R-0021's single-read / complete-mediation / fail-closed discipline at the provenance anchor."

Maintainer-ratified 2026-07-07: **(Q7)** hard cutover — atomic flip, existing plugins re-packaged in the same change, no dual-accept window; **(Q5)** single-root custody — the P-0005 root signs both layers at this tier, exposure recorded in the ADR, split fires at R-0005-e; **(Q2)** one flat OCI manifest, image-index noted as forward shape only; **(Q3)** `PackageVerifier` seam locks now, TUF mechanism deferred behind R-0005-e. Store = OCI artifacts; signing = manifest digest keyed-in-tree as OCI 1.1 referrer; `spec_type=code`.

### Intentional gaps (design at Frame altitude; do not fill)

The `[[artifacts]]` TOML schema field-by-field design, exact API signatures, and crate-level code layout are Stage-3 / spec work. `{{P-SigningKeyCustodyHardening}}` stays an unauthored placeholder. The Frame decides *which constraints operate and why* — mechanisms at architectural altitude.

---

## Intent self-report

**I read the JTBD as this.** Today a mnemra plugin is only ever a set of files loaded from known local paths, trusted after the fact by two inner gates — the Ed25519 manifest signature (P-0005) and the R-0021 BLAKE3 content-hash over the `.wasm` component. There is no *distributable unit*: no packaging shape, the package as a whole is unsigned, and non-`wasm` artifacts (data, assets, schemas, secondary components) carry no individual integrity coverage — a live per-artifact substitution hole. The job is to make the plugin a distributable unit whose **whole-bundle provenance and integrity a consumer can establish before unpacking**, rather than re-deriving trust artifact-by-artifact from the inner manifest after the fact. Within the locked frame that means: one uniform packaging shape, signed end-to-end at the distribution anchor, fetchable from a filesystem or a self-hosted registry with zero required round-trip to a host we do not control, verified on fetch *and* re-verified blob-by-blob at load. The design is a **pre-phase** in front of the existing inner gates — it extends the trust boundary earlier (to the distribution anchor) and widens the inner gates' coverage (to every blob), and it changes those gates' *coverage*, never their *primitive*.

**Decisions that strain a Non-goal or Success criterion — named (obligation 5b).**

1. **Q5 single-root custody strains "each layer independently load-bearing" (Security value; P-SecurityLayered) and P-LeastAuthority.** Reusing the P-0005 root to sign both the package (distribution anchor) and the inner manifest (provenance anchor) means one root-key theft forges both — the four layers stay *scope-independent* but are **not compromise-independent**, and the single key concentrates authority the P-LeastAuthority default would split. The axis is **Security ↔ Simplicity** (a known-tension row in `constraint-edges.md`): one custody story vs. compromise-independence. The maintainer resolved it (Q5) for single-publisher `core: true` scope, with the exposure recorded and the split fired at R-0005-e. Recorded here, not escalated — the trade-off is ratified, not open.

2. **Pulling P-0019 DEF-2 ahead of its stated tripwire strains P-Defer's "fire at the tripwire" discipline.** DEF-2 named the third-party-publisher surface as its firing condition; this bundle pulls the deferral forward at *single-publisher* scope before that condition fires. This is a deliberate board-placement decision (2026-07-05), not a silent tripwire jump — the third-party-publisher *scope* stays deferred behind R-0005-e (see Prior-decision dispositions). No Success criterion is weakened; SC6 explicitly requires the disposition.

No decision strains a Non-goal beyond these; the seven Non-goals hold as scoped.

---

## Architectural shape (the boundary and its four components)

The boundary the Frame locks (maintainer-confirmed 2026-07-07):

```
build-host                          store (one contract, two transports)         host
──────────                          ────────────────────────────────────         ────
① builder/publisher   ──package──▶  ┌ filesystem  (OCI image-layout)  ┐  ──fetch──▶  ③ fetch-verify pipeline
   package + sign                    └ self-hosted registry (dist API) ┘             (behind PackageVerifier seam)
   (P-0005 key custody)                                                                    │
                                                                                           ▼
                                                                              ④ loader integration
                                                                              (distribution pre-phase) ──▶ existing D6 gates
                                                                                                            (P-0019, unchanged in kind)
```

- **① Bundle builder / publisher (build-host side).** Assembles the uniform OCI bundle (one manifest binding N≥1 digest-addressed blobs, `.wasm` = artifact #1), signs the OCI manifest digest keyed-in-tree with the P-0005 root (domain-separated), and attaches the signature as an OCI 1.1 referrer. Key custody is unchanged from P-0005 (build-host-on-disk, mode 600, off the deployment node).
- **② Store transports behind one contract.** The OCI **image-layout** on disk (`oci-layout` + `index.json` + `blobs/<alg>/<digest>`) and a self-hosted OCI **distribution API** registry on the LAN are two transports over the *same* content-addressed structure — one abstraction, both satisfying R-NoExternalHost.
- **③ Fetch-verify pipeline behind the `PackageVerifier` seam.** **Fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds** — every read is gated by its bound *before* it feeds a verifier (metadata caps ahead of the outer-manifest/referrers reads, size/N caps ahead of blob reads, decompression-ratio cap at unpack; fail-closed, per OC-12); signature verification is signer-key-pinned; blob digests are computed over received bytes. The `PackageVerifier` is a locked seam; the TUF adapter slots behind it later without rework.
- **④ Loader integration as a distribution pre-phase.** The pipeline output feeds the **unchanged** inner gates (P-0019 D6 states 2–9). The pre-phase is recorded in the new distribution ADR; D6 gains one cross-reference line.

---

## Operating constraints

The constraints that bear on this work. Each cites its anchor (workspace value / principle / constraint-edge / project ADR; most-specific-wins). `[LOCKED — maintainer]` = transcribed, not re-litigable. `[Research-locked]` = fixed by the 2026-07-01 survey. `[Applied]` = a DaVinci lock against canon in this Frame.

**OC-1 — Uniform packaging, no bare load path (H1). [LOCKED — maintainer]**
Every plugin is a uniform OCI bundle whose signed manifest binds N≥1 artifacts; the loader rejects any plugin not presenting one; there is no bare/`wasm`-only load path (N=1 ships as a 1-artifact bundle — the *path* is banned, not N=1 content). *Anchors: H1 (maintainer 2026-07-01); P-0019 D6 (one load path — the uniform-packaging invariant is a plugin-contract lock, reaching P-0003 + the P-0019 plugin contract); Load-path invariant #1.*

**OC-2 — R-NoExternalHost: filesystem AND self-hosted server, zero external round-trip. [LOCKED — maintainer]**
The store is self-hostable on a filesystem (true air-gap) and a self-hosted LAN registry (restricted-egress), with no mandatory round-trip to a host we do not control. *Anchors: R-NoExternalHost (maintainer 2026-07-01); Reliability (availability without an external dependency, `architecture-values.md`); Simplicity (OCI image-layout ≡ registry structure — one abstraction serves both transports).*

**OC-3 — Decision A: inner `[[artifacts]]` complete-mediation over all N≥1 blobs. [LOCKED — maintainer]**
The inner signed manifest binds every blob via an `[[artifacts]]`-style N≥1 list at the *provenance* anchor; each blob is verified with R-0021's single-read / complete-mediation / fail-closed discipline before its consumer sees it. This is the **primary** load-time integrity gate for all blobs. *Anchors: Decision A (maintainer 2026-07-01); P-SecurityLayered (provenance-at-load layer); R-0021 (`docs/specs/2026-06-30-signing-to-runnable.md` — the single-read discipline, extended in coverage not in kind); Load-path invariant #10.*

**OC-4 — Store standard = OCI artifacts, authored directly. [Research-locked]**
The store is raw OCI artifacts (image-spec 1.1 manifest + distribution/image-layout), authored directly (the ORAS/arbitrary-artifact path), **not** via `wkg`'s one-component-per-package convention; `wkg` is retained only for WIT/component dependency pulls. *Anchors: research §5; P-StackDiscipline (in-stack Rust clients `oci-client` / `oci-spec` over a bespoke registry or a foreign ecosystem); Simplicity (adopt the established standard's least-standing-mechanism over a hand-rolled CAS + signing + layout scheme).* **Ecosystem-finding carry (intake Evidence, research Honesty note):** the two dated findings — "OCI is the Wasm-component ecosystem default" and "warg is no longer actively developed" — are **carried as current on their 2026-07-01 verification** (six days old; not load-bearing to any lock this Frame makes: the store pick is maintainer/research-locked and would not flip on them), with re-verification routed to the spec-stage crate evaluation alongside the L3/L5 screens.

**OC-5 — Package signing = OCI manifest digest, keyed-in-tree, OCI 1.1 referrer, signer-key-pinned. [Research-locked + LOCKED — maintainer (Q5)]**
Sign the OCI manifest digest by reusing the P-0005 `ed25519-dalek` root (domain-separated prefix, Load-path invariant #9); attach as an OCI 1.1 referrer; verify in-Rust on fetch under signer-key-pinning (enumerate referrers, accept only the pinned P-0005 root, ≥1 required, zero = fail-closed, no fall-through). *Anchors: research §5; P-StackDiscipline S2 (keyed-in-tree avoids the Go-CLI `cosign` foreign-ecosystem signing cost — license tier is a separate additive gate); Simplicity (zero new signing dependency — the same reuse logic R-0021 used for in-tree BLAKE3); P-0005 (the root and its custody); Load-path invariants #2, #9.*

**OC-6 — `PackageVerifier` seam locks now; TUF mechanism deferred behind R-0005-e (Q3). [LOCKED — maintainer]**
Lock a `PackageVerifier` seam now; defer the TUF rollback/rotation mechanism behind the seam, fired at R-0005-e. *Anchors: P-LockContract (lock the intrinsic seam, vary the implementation); the P-LockContract ⇄ P-PreserveDecisionSpace **when-to-lock** edge (`constraint-edges.md` — lock what is intrinsic to the artifact's identity, preserve the space on a separable future option), whose canonical worked instance is the P-0010 D5 engine-agnostic-seam precedent the intake cites; P-Defer (the TUF adapter is deferred, not omitted).*

**OC-7 — Single-root custody at this tier; split fires at R-0005-e (Q5). [LOCKED — maintainer]**
The package signature and the inner manifest signature share the P-0005 root at this tier — scope-independent, **not** compromise-independent; the ADR records this exposure explicitly; the split to a distinct distribution key (or TUF delegated roles) fires at R-0005-e. *Anchors: Q5 (maintainer 2026-07-07); P-SecurityLayered (the "each layer independently load-bearing" floor is met on scope but not on compromise — stated, not hidden); P-LeastAuthority (the concentration is the recorded exposure); P-0005 (custody + the R-0005-e trip-wire).*

**OC-8 — One flat OCI manifest at this tier; image-index is forward shape only (Q2). [LOCKED — maintainer]**
The plugin is an atomic unit: one manifest binding N blobs. The image-index (manifest-of-manifests) shape is recorded as the forward path if secondary components ever need independent addressing/signing — noted, not designed. *Anchors: Q2 (maintainer 2026-07-07); Simplicity (atomic unit, one manifest); P-PreserveDecisionSpace (the index path is recorded as the live forward alternative, not erased).*

**OC-9 — Hard cutover; no dual-accept window (Q7). [LOCKED — maintainer]**
The uniform-packaging invariant lands as one atomic flip: existing pre-OCI plugins are re-packaged in the same change; no legacy-accept path, no sunset mechanism to design. *Anchors: Q7 (maintainer 2026-07-07); H1 (a dual-accept window would be the bare path reborn by deployment — it would defeat H1 through migration compatibility); Reversibility (the mitigation path is the re-package-and-re-sign of the finite `core: true` set, not a live legacy window).*

**OC-10 — Existing inner gates unchanged in primitive; extended in coverage. [LOCKED — maintainer, hard constraint]**
Inner Ed25519 (P-0005) + BLAKE3 single-read (R-0021) extend to all blobs, not in kind. P-0005's V0 invariants hold untouched: synchronous fail-closed verification, no verify-async path, `core: true` by signature provenance, file-mode startup checks. The design slots into P-0019 D6's ordering (signature-verify → content-hash-verify → manifest-parse/allowlist → instantiate, fail-closed at each gate) and targets P-0012's pinned raw-Wasmtime runtime. *Anchors: P-0005 (signing invariants); R-0021 (content-hash gate); P-0019 D6 (lifecycle ordering); P-0012 (Wasmtime pin `=45.0.2`, `>= 44.0.x` floor).*

**OC-11 — Signed-body residence of per-artifact identifiers (R-0005-h). [Applied — Frame security constraint]**
Decision A's `[[artifacts]]` binding needs a per-artifact identifier for entry↔blob resolution; this **supersedes** P-0003's 2026-06-30 amendment scope-out ("binds bytes, not a path"). Every per-artifact identifier SHALL live in the **signed canonical body** (before the `\n[signature]` marker — the slice at `libs/mnemra-host/signing/verify.rs:337-344`), and entry↔blob resolution SHALL be unambiguous and collision-free. *Anchors: R-0005-h core-by-provenance (P-0005 2026-05-24 amendment — an identifier in the unsigned region would let an attacker swap both the artifact and its declared identity); P-0003 §Amendment 2026-06-30 (the `[component]` section this extends); P-GuaranteeByMechanism (the residence rule is a mechanism, not a convention).* This is a Frame security constraint the schema design must honor, **not** schema-field discovery.

**OC-12 — Fetch/unpack resource bounds, fail-closed ahead of the integrity gate. [Applied — the Frame's net-new constraint]**
The fetch/unpack surface is new attack surface P-0007 does not cover (P-0007 bounds *execution-time* per-instance limits — fuel/epoch/memory — only; it says nothing about fetch/unpack-time cost). Because digest verification must read the bytes first, an availability-DoS (oversized blob, excessive artifact count N, decompression bomb, unbounded buffering) lands *before* the integrity gates. So bounds SHALL exist — as a **minimum set, not a closed one** — on **{ outer-manifest byte size, referrers-enumeration count and per-referrer size, total bundle size, artifact count N, per-blob size, decompression ratio }** and SHALL fire **fail-closed ahead of every read they gate** — the bound is the first gate, extending D6's fail-closed ordering into the distribution pre-phase. The first two dimensions bound the **pre-manifest metadata reads**: the blob-level bounds (N, per-blob sizes) are enforceable only after the outer manifest is fetched and parsed, so the outer-manifest fetch itself and the referrers enumeration that signer-key-pinning (invariant #2) requires are otherwise unbounded reads from an untrusted store before any verification — the same threat class, one hop earlier. **Streaming-verify** (bounded-buffer, verify-as-you-read) is the preferred *realization*; the **hard-cap existence** is the locked constraint and is requirement-clean even if the chosen OCI client buffers whole blobs. Exact numeric values are spec-level (the P-0007 conservative-floor precedent applies — see Routine decisions). *Anchors: P-SecurityLayered (the availability half of the new network-transport layer; LAN TLS/auth is defense-in-depth for confidentiality, not the availability answer); P-GuaranteeByMechanism (each breach a distinct designed error, fail-closed — see OC-13); P-0019 D6 (the pre-phase inherits D6's fail-closed ordering); P-0007 (the execution-time sibling this constraint is distinct from, not a duplicate of).*

**OC-13 — Fail-closed everywhere, with a distinct designed error per rejection class. [Applied]**
Malformed outer manifest · unknown signer key · missing referrer signature · digest mismatch · media-type mismatch · incomplete `[[artifacts]]` binding · missing blob · each fetch/unpack resource-bound breach — each rejects the fetch/load with a **distinct designed error**; no best-effort path. **"Incomplete `[[artifacts]]` binding" rejects in both directions:** the outer-manifest artifact set and the inner `[[artifacts]]` set SHALL be mutually complete — a bundle blob with no inner entry (under-binding: a blob that would escape the provenance anchor) and an inner entry with no blob are each their own designed rejection (the latter overlaps "missing blob"; the former is its own class). *Anchors: P-GuaranteeByMechanism (the guarantee is a loud-failing mechanism per class, mirroring P-0007's structured attribution event on trap); P-SecurityLayered (fail-closed is structural, not advisory); P-0005 (fail-closed-on-load floor).*

**OC-14 — Q4 media-type locks are named lock targets. [Intake-carried lock target (SC6, r1-folded) + Applied]**
The outer `artifactType` = `application/vnd.mnemra.plugin.v1` (Load-path invariant #6 — the confused-deputy block) and per-blob media types lock **with the schema**, not by assumption. Named as lock targets here; the field-level values land at spec. *Anchors: Q4 (intake SC6 named lock target, carried from the r1-folded intake — not among the 2026-07-07 maintainer ratifications in the 2a record); P-AgentPrimarySource (a stable, collision-checked media-type identifier — AP1 collision check applies); Load-path invariant #6.*

**OC-15 — Rust-ecosystem, Green-tier dependencies; screens are spec gates. [LOCKED — maintainer, hard constraint]**
Candidate crates (`oci-client`, `oci-spec`, `ocidir` if adopted, `tough` if TUF fires) are all Green (`Apache-2.0 WITH LLVM-exception` maps Green). License tier is a separate additive gate from stack-fit and from the vulnerability screen. *Anchors: Rust-ecosystem alignment; P-StackDiscipline S2; `dependency-approval` Green tier.* The `cargo audit`/RUSTSEC screen (L5) and the `ocidir` layout-write-path audit (L3) are **spec acceptance gates** (intake SC7), not Frame-blocking.

---

## Rationale chain

Intent → constraints → decisions, traceable.

1. **The job is whole-bundle provenance-and-integrity before unpack (JTBD).** → The trust boundary must extend earlier than the inner gates, to a *distribution anchor* the consumer verifies before it trusts anything. → A **package signature over the whole bundle** (OC-5), verified on fetch before unpack (OC-13), is the distribution anchor.

2. **The bundle must be one uniform shape with no bare path (OC-1, H1).** → A single packaging path whose signed manifest binds N≥1 blobs closes per-artifact substitution *at the distribution layer* unconditionally (no weaker path to downgrade to). → **OCI artifacts** (OC-4) natively carry this (one manifest → N content-addressed blobs). → The image-layout ≡ registry-structure identity satisfies **R-NoExternalHost** (OC-2) from one abstraction.

3. **Substitution must also close at load, for every blob, not just the `wasm` (Decision A, OC-3).** → The inner manifest binds all N≥1 blobs (`[[artifacts]]`) and complete-mediates each at the *provenance* anchor — the primary load-time gate. → Per-artifact identifiers become necessary, so they must reside in the signed body (OC-11, R-0005-h) and the P-0003 `[component]` amendment is superseded in scope.

4. **Rollback/rotation/revocation are the residual OCI+keyed-sig cannot close.** → Defer the **mechanism** (TUF) behind a `PackageVerifier` **seam locked now** (OC-6, Q3) — the when-to-lock edge: the seam is intrinsic, the adapter is separable. → The seam-now/mechanism-later split reuses the P-0010 D5 engine-agnostic-seam precedent.

5. **Single-publisher `core: true` is this tier's scope.** → Reuse the P-0005 root for both signatures (OC-5, OC-7, Q5) — one custody story, zero new dependency — and **record the compromise-independence exposure explicitly**; the distribution-key split fires at R-0005-e when third-party publishers arrive.

6. **The uniform invariant must land without reopening the bare path.** → **Hard cutover** (OC-9, Q7): re-package the finite `core: true` set in the same change; no dual-accept window (which would be the bare path reborn by deployment).

7. **Fetching over a network is new attack surface P-0007 does not cover.** → **Fetch/unpack resource bounds fire fail-closed ahead of the integrity gate** (OC-12) — availability-DoS lands before the digest read, so the bound is the first gate, extending D6's fail-closed ordering into the pre-phase.

8. **The distribution layer must integrate with the existing gates without disturbing them (OC-10).** → A **distribution pre-phase** feeds the unchanged D6 states 2–9; the pre-phase is recorded in the new ADR and D6 gains one cross-reference line (see Prior-decision dispositions).

---

## Consultations

DaVinci cannot spawn agents. The item below is recorded for the orchestrator as a named, spec-stage consultation — it does **not** block the Frame (the constraint holds under either outcome).

- **Streaming-verify feasibility against the chosen OCI client crate's fetch API (feasibility / ops).** OC-12 prefers streaming-verify (bounded-buffer, verify-as-you-read). Whether `oci-client` (or the adopted fetch crate) exposes an incremental/streaming read that lets the digest be computed and the size-cap enforced *without* first buffering the whole blob is an API-surface question that needs someone to check the crate at spec time. **Why it does not block:** the locked constraint is the *hard-cap existence*, fail-closed ahead of the integrity gate (OC-12) — if the client buffers whole blobs, the per-blob size cap bounds the buffer and the requirement is still met; streaming is an optimization of *how*, not a change to *what*. Route to the spec-stage crate evaluation alongside the L3 `ocidir` audit and the L5 `cargo audit` screen.

No Frame-blocking consultation fires. The locked research survey covers the landscape; no principle conflict, intent ambiguity, novel cross-project precedent, or substrate pivot arose.

---

## Routine decisions (batched)

Within-principle decisions taken inline; reported at the spec-exit gate, not escalated.

1. **Distribution pre-phase sits ahead of D6 state 1 ("Discovered"), not interleaved with states 2–9.** The pre-phase (fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds; bounds fail-closed ahead of every read they gate, per OC-12) completes and hands a verified, unpacked, on-disk bundle to the existing "Discovered" state. *Within P-0019 D6 (no later state reachable without its predecessor) + P-MinBlastRadius (the pre-phase lands behind one seam; D6's interior is untouched).*

2. **Fetch/unpack resource-bound numeric values follow the P-0007 conservative-floor method at spec.** The *dimensions* and the fail-closed-ahead-of-integrity ordering lock here (OC-12); the concrete numbers (a max bundle size, a max N, a max per-blob size, a max decompression ratio) are set at spec by the same "conservative floor, tune-up-only, security valid at the floor" reasoning P-0007 used for fuel/epoch/memory. *Within P-Defer (last-responsible-moment — the number needs the real bundle-size distribution the spec will have) + P-0007 precedent.*

3. **`PackageVerifier` seam shape is a spec-level trait design; the Frame locks only that the seam exists and what it must gate.** The trait's exact methods/signatures are Stage-3 work behind the locked seam. *Within P-LockContract (the contract is locked; the signature detail varies) + the intake's intentional-gap list.*

4. **The `wkg` retention is dependency-scoped, not bundle-scoped.** `wkg` is kept only for pulling WIT/component build-time dependencies; the bundle is authored as a raw OCI artifact directly. *Within OC-4 / research §2(a).*

5. **Load-path invariants #1–#10 are carried wholesale as fail-closed locks and mapped to their landing sites** (see the map in Risk profile). Each is locked at Frame/ADR, not re-discovered at implementation. *Within P-SecurityLayered + P-GuaranteeByMechanism.*

---

## Escalated decisions

**None.** Every decision in this Frame is either (a) maintainer-locked (H1, R-NoExternalHost, Decision A, Q2, Q3, Q5, Q7, the hard constraints) and transcribed, or (b) an intake-carried obligation (Q4/SC6) or a within-principle application anchored to canon (the `[Applied]` OCs and the Routine decisions above). The one genuine trade-off — Q5 single-root custody on the **Security ↔ Simplicity** axis — was **resolved by the maintainer** (Q5 ratification, 2026-07-07) and is therefore recorded in the rationale chain and the risk profile with its axis named, not escalated. No pause-and-escalate trigger fired: no principle conflict without precedent, no intent ambiguity unresolvable from inputs, no novel cross-project precedent, no substrate pivot. (Empty by reasoning, not by omission.)

---

## Prior-decision dispositions

The intake (SC6) requires three prior-decision dispositions carried in the Frame as the text the ADR/amendment will land. The plugin-contract reach the research cited "as given by the maintainer" is **verified against source** in this Frame: P-0003 (`[component]` binds bytes-not-path in the signed body) and P-0019 (D6 states 1–9; DEF-2) were opened and confirmed to accommodate the reach.

### DEF-2 disposition (P-0019) — supersede-in-shape + pull single-publisher scope; third-party scope stays deferred

P-0019 **DEF-2** deferred "third-party plugin distribution + signature verification" over an OCI/`wkg` registry with a `application/vnd.mnemra.plugin.v1` artifact type, verify-before-instantiate, recording a **cosign-on-`wkg`** shape as *recommended, not locked*, behind the **third-party-publisher** tripwire. This Frame disposes DEF-2 as **two moves, one partial** (do not over-close):

- **(a) Supersede the SHAPE.** The cosign-on-`wkg` recommendation is superseded by the locked research: **keyed-in-tree** signing (reuse the P-0005 root, direct-OCI) over cosign-by-default (`cosign` signs as a Go CLI — a P-StackDiscipline S2 foreign-ecosystem cost), and **direct-OCI** authoring over `wkg`'s one-component-per-package convention. The `application/vnd.mnemra.plugin.v1` artifactType and the verify-before-instantiate ordering are **retained** (Q4, OC-14; D6 pre-phase).
- **(b) Pull the SINGLE-PUBLISHER scope ahead of the tripwire.** The `core: true` single-publisher distribution story is pulled deliberately forward (maintainer board placement 2026-07-05) ahead of DEF-2's third-party tripwire — this bundle designs it now.
- **What STAYS deferred behind R-0005-e:** the **third-party-publisher** scope — non-`core` plugin distribution, key splitting for external publishers, delegated roles, the distribution-key/TUF split. DEF-2 is **not fully retired**; its third-party half remains a live deferral (see the anti-hedge test below). The new distribution ADR (`{{P-PluginDistribution}}`) records the superseded shape and pulled scope; P-0019 carries a disposition line pointing to it.

### D6 disposition (P-0019) — pre-phase in the new ADR; D6 gains one cross-reference line

The distribution lifecycle pre-phase (**fetch-within-bounds → verify-package-signature → verify-blob-digests → unpack-within-bounds**, bounds fail-closed ahead of every read they gate per OC-12 — this bounds-first sequence is the canonical ordering string the new ADR lands; the shorter sequence in the 2a input record above is the verbatim transcription of the elicitation walk and is not the ADR-destined text) is recorded in the **new distribution ADR**, ahead of D6 state 1 ("Discovered"). P-0019 **D6 gains one cross-reference line** at state 1 pointing to the pre-phase — the canonical ordering stays single-sourced in P-0019; there are **not** two live framings. *Anchors: P-0019 D6 (the ordering invariant is single-source); P-AgentPrimarySource (one authority per fact); the "capstone that cites, not a parent that overrides" posture P-0019 already holds.*

### P-0003 amendment reach — `[[artifacts]]` extension supersedes the binds-bytes-not-path scope-out

The P-0003 2026-06-30 amendment scoped out a component path/name ("binds bytes, not a path … a manifest-declared component path/name is an out-of-scope multi-plugin concern"). Decision A's N≥1 `[[artifacts]]` binding makes a per-artifact identifier **necessary** for entry↔blob resolution — so the amendment's scope-out is **superseded**. The P-0003 amendment this bundle lands extends the single `[component]` hash to an N≥1 `[[artifacts]]` binding, with every per-artifact identifier residing in the signed canonical body (OC-11, R-0005-h) and unambiguous, collision-free entry↔blob resolution. The field-by-field schema (per-blob media types, ordering, the fail-closed complete-mediation loop) is spec/headline work; this Frame locks the *contract reach and the security constraints on it*, not the fields.

---

## Risk profile (resolved)

**Trust boundary: YES** (supply chain, signing, plugin load path — build-pipeline ↔ host boundaries). The threat anchors the intake carried forward, resolved against the now-known mechanism:

| Threat anchor | Resolution against the known mechanism |
|---|---|
| **Root-key compromise (Critical — single root signs both layers, Q5)** | The four layers are *scope-independent*, **not** compromise-independent under keyed-in-tree reuse: one theft of the P-0005 root forges both the package signature and the inner manifest signature. Recorded as an explicit ADR exposure (OC-7). Detection of a stolen-key forgery needs a transparency-log / TUF witness (deferred, R-0005-e). Restoring compromise-independence = split to a distinct distribution key (or delegated roles) at R-0005-e. |
| **Per-artifact substitution — at BOTH anchors** | Closed at the **distribution** anchor (the signed OCI manifest digest-pins every blob; given invariants #2/#3/#4) and independently at the **provenance** anchor (Decision A inner `[[artifacts]]` complete-mediation, OC-3). SC5 requires substitution tests at each anchor with the other disabled to both reject. |
| **Fetch↔load TOCTOU (all blobs, under Decision A)** | Closed for every blob: the inner gate complete-mediates each blob single-read at the provenance anchor (OC-3), so a post-fetch cache tamper fails at load even if the distribution-layer check is bypassed (SC4). |
| **Rollback / downgrade (serve an older validly-signed bundle)** | **Accepted residual** until R-0005-e. OCI + keyed-sig cannot close it; TUF is the answer, deferred behind the `PackageVerifier` seam (OC-6). The trip-wire is named and fireable, not silent. |
| **Build-time dependency confusion (`wkg`-pulled deps composed pre-sign; #1942 residual)** | **Named residual**, unchanged: R-0021 faithfully binds *whatever was built* (bytes-run == bytes-signed; integrity, not provenance-of-inputs). SLSA/#1942 is the honest answer (payoff point for reproducible builds); an interim lockfile-hash pin may be weighed at spec without pulling #1942 in. |
| **Fetch/unpack resource exhaustion (NEW surface — oversized manifest/referrer metadata, oversized blob, excessive N, decompression bomb, unbounded buffering)** | Closed by OC-12: fail-closed bounds on the pre-manifest metadata reads (outer-manifest size, referrers-enumeration count and per-referrer size) and the blob reads (total bundle size, N, per-blob size, decompression ratio) fire ahead of every read they gate (availability-DoS lands before the digest read). Streaming-verify preferred; hard-cap existence locked. LAN TLS/auth is defense-in-depth for confidentiality, not the availability answer. |

### Load-path invariants #1–#10 — landing-site map

Each research §4 invariant is carried wholesale and mapped to where it locks. `[NewADR]` = the new distribution ADR (`{{P-PluginDistribution}}`); `[P-0003]` = the `[[artifacts]]` amendment; `[Spec]` = Stage-3.

| # | Invariant | Lands at |
|---|-----------|----------|
| #1 | Uniform-packaging (loader rejects any plugin without a signed manifest binding every artifact) | `[NewADR]` + P-0019 D6 cross-ref + plugin-contract |
| #2 | Signer-key-pinning, fail-closed (accept only the pinned P-0005 root; ≥1; zero = fail-closed) | `[NewADR]` (fetch-verify pipeline) |
| #3 | Digest-pin, not tag (verify by digest, never a mutable tag) | `[NewADR]` |
| #4 | Digest over RECEIVED bytes, not the registry `Docker-Content-Digest` header | `[NewADR]` |
| #5 | Cache-read re-validation — **complementary outer** check at the distribution anchor | `[NewADR]` (see the N2 weighing below) |
| #6 | `artifactType` pinning (`application/vnd.mnemra.plugin.v1`; confused-deputy block) | `[NewADR]` + `[P-0003]`/schema (Q4 media-type lock, OC-14) |
| #7 | Path-traversal / zip-slip guard on any annotation→filename mapping | `[NewADR]` (unpack phase) |
| #8 | Missing-blob fail-closed (partial N-1-of-N bundle rejected) | `[NewADR]` |
| #9 | Domain-separation on the reused key (`"mnemra-oci-manifest-v1:" \|\| digest`) | `[NewADR]` (signing) |
| #10 | Inner complete-mediation over ALL artifacts (Decision A) — the **primary** load-time gate | `[P-0003]` (`[[artifacts]]` schema) + `[Spec]` (fail-closed loop) + P-0019 D6 cross-ref |

### N2 layer-cost weighing (informational, per intake) — does invariant #5 earn its place now?

The intake parked N2 for the Frame to weigh under P-SecurityLayered's **ceiling clause** (marginal cost vs marginal risk; *"defense-in-breadth is not depth"* — a layer whose marginal cost exceeds its marginal risk reduction is not added). Invariant #5 (the complementary **outer** cache-read re-validation at the distribution anchor) sits *behind* invariant #10 (the primary inner complete-mediation over all blobs, Decision A). Its two claimed grounds:

- **Anchor independence** (inner *provenance* vs outer *distribution*) — **dormant** at this tier: both anchors are vouched by the same P-0005 root under Q5 single-root reuse, so this leg is latent, becoming load-bearing only once the distribution key splits at R-0005-e.
- **Algorithm / implementation diversity** (inner BLAKE3 code path vs the OCI client's outer sha256 path — a bug in one verification path is caught by the other) — **currently live**: it holds regardless of key custody.

**Conclusion: keep #5** — the currently-live algorithm/impl-diversity ground alone clears the ceiling (a verification-path bug in a security-critical load gate is exactly the marginal risk defense-in-depth exists to reduce), and the anchor-independence ground pre-positions the check to become fully load-bearing at R-0005-e without a later retrofit. The **lens applied is the P-SecurityLayered ceiling clause**, and the honest accounting (anchor-independence dormant, algorithm-diversity live) is recorded so the ADR states why #5 is depth, not breadth. #5 should mirror #10's single-read discipline (validate-then-hand-to-consumer) so the outer check is itself TOCTOU-tight.

### Deferrals — anti-hedge three-part test (decision-content · canon-anchor · fireable/parked trip-wire)

Each deferral is tested, not blanket-deferred behind "R-0005-e." **R-0005-e run through DF1's three-outcome test:** it is **self-announcing** — the firing events (a deployment beyond the maintainer's single dogfood instance; a third-party publisher onboarding) cannot arise without someone explicitly initiating them, so no detector mechanism is required and it is a valid deferral, not a parked item. (P-0005 defines it as "the moment mnemra-core is deployed beyond the maintainer's single dogfood instance"; the research adds the co-condition "third-party publishers arrive.")

| Deferral | Decision content (what fires when it resolves) | Canon anchor | Trip-wire (fireable/self-announcing) |
|---|---|---|---|
| **TUF rollback/rotation mechanism** | Adopt `tough` behind the locked `PackageVerifier` seam; add timestamp/snapshot freshness + delegated roles + offline root (co-requires P-0005 Tier-C custody hardening for the rotation half) | P-Defer + P-LockContract (seam locked now, OC-6) | **R-0005-e** — self-announcing (multi-deployment / third-party publisher) |
| **Distribution-key split (compromise-independence)** | Split the package signer to a distinct distribution key (or TUF delegated roles), restoring compromise-independence | P-LeastAuthority + P-SecurityLayered (OC-7) | **R-0005-e** — self-announcing; cheap to exercise because the seam is locked now (last-responsible-moment preserved) |
| **Sigstore keyless signing** | Adopt Fulcio/Rekor keyless (self-hosted to stay R-NoExternalHost-clean; a heavy lift), or public keyless if a deployment tolerates an external transparency-log round-trip | P-Defer (Non-goal 3) | **R-0005-e** — self-announcing (third-party publishers, or a deployment tolerating external round-trip) |
| **SLSA / in-toto provenance attestations** | Attach SLSA provenance + SBOM as OCI referrers; addresses the build-time dependency-confusion residual | P-Defer (Non-goal 5) | **#1942** (reproducible `wasm32-wasip2` builds land) — self-announcing (the build capability either exists or does not) |
| **Tier-C custody hardening (`{{P-SigningKeyCustodyHardening}}`)** | Offline-root / HSM / never-on-node + transparency log; the P-0005 slot stays unauthored | P-Defer (P-0005; Non-goal 8) | **R-0005-e** self-announcing for the firing; the **paired W2-opt custody-ADR pre-author is PARKED** (intake Non-goal 8) with the named cadence = the strangler-program board's per-bundle review |

Every deferral carries all three anti-hedge elements. The one **parked** (not deferred) item is the W2-opt custody-ADR pre-author, correctly labelled parked with its human cadence named (the strangler board), per DF1.

---

## ADR landing map

Where this Frame's locks land (agent-primary source artifacts; views derivative):

- **New `{{P-PluginDistribution}}` (the distribution ADR).** The distribution pre-phase (D6-ahead), OCI store + two transports (OC-2, OC-4), keyed-in-tree package signing + signer-key-pinning + domain separation (OC-5), the `PackageVerifier` seam (OC-6), single-root exposure + R-0005-e split (OC-7), one flat manifest + image-index forward shape (OC-8), hard cutover (OC-9), fetch/unpack resource bounds (OC-12), fail-closed designed errors (OC-13), load-path invariants #1–#9, the N2 #5 weighing, and the deferral table. Two recorded hygiene notes travel with it: the **one-sided domain-separation rationale** (invariant #9 prefixes only the new OCI-digest message domain; the existing inner TOML domain stays unprefixed, its separation resting on format structure — record why that is accepted, or the spec adds an inner prefix at the next re-sign boundary), and the **mediated-gate-only access rule** (consumers obtain artifact bytes only via the verified gate; direct reads of the unpacked layout are out of contract).
- **Threat-model typing + DFD extension (architecture overview, cited by the new ADR).** The new trust topology — the store zone between `TB-build-pipeline` and `TB-mnemra-host`, the builder/publisher process, the two store transports, the fetch-verify pipeline, the unpacked-bundle cache, and the publish/fetch/referrers crossing flows — lands as **typed elements** in the architecture overview's trust-boundary model (element IDs of the sibling convention's shape, e.g. a store data-store, a fetch-verify process, fetch/publish data-flows, the TB extension; exact IDs are the ADR author's to assign), so the new ADR's Threat-references section can cite typed elements per the sibling-ADR convention (P-0003/P-0005/P-0007/P-0019 all do).
- **Risk-register entries for the three accepted residuals (project register, R-NNNN per the R-0004/R-0007 convention).** One entry each for: rollback/downgrade (retires at R-0005-e), single-root compromise-independence / Q5 exposure (retires at R-0005-e), and build-time dependency confusion (retires at #1942) — each with severity, rationale, compensating controls, and its trip-wire as the retirement condition. ADR-text exposure alone is not the queryable register the convention exists for.
- **P-0003 `[[artifacts]]` amendment.** Single `[component]` hash → N≥1 `[[artifacts]]` binding; per-artifact identifiers in the signed canonical body (OC-11, R-0005-h); Q4 per-blob media types (OC-14); invariant #10 (the fail-closed complete-mediation loop is spec-level).
- **P-0019 DEF-2 disposition + D6 cross-reference line.** DEF-2 superseded-in-shape + single-publisher scope pulled forward (third-party scope stays deferred); D6 state-1 cross-reference to the pre-phase (single-source ordering preserved).
- **`{{P-SigningKeyCustodyHardening}}` — unchanged, unauthored placeholder.** Its R-0005-e trip-wire is untouched; the W2-opt pre-author stays parked.

Spec acceptance gates carried from this Frame: security re-verification of the finished `[[artifacts]]` mechanism (intake SC6); the `cargo audit`/RUSTSEC (L5) and `ocidir` layout-write-path (L3) screens (intake SC7); the ecosystem-finding re-verification carried in OC-4; and the **interim lockfile-hash-pin weighing** (the build-time dependency-confusion stopgap named in the risk profile — weighed at spec, not silently dropped).
