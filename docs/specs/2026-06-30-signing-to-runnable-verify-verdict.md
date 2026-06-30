# Verify Verdict — signing-to-runnable (M1 real signing + supply-chain binding; M2 runnable host)

**Date:** 2026-06-30
**Spec:** docs/specs/2026-06-30-signing-to-runnable.md:479852a28124727ca3eafdb6cd29f0ad95d76f76
**Verdict:** passed
**Aggregate:** all signals pass; two round-1 Low Frame findings (frame⊥spec, spec correct) reconciled and confirmed closed on round-2 re-verify
**requires_upstream_change_at:** n/a

## The arc
1. **Round-1 Charon (dispatch 1192):** Pass with concerns (requires_upstream_change_at: frame) — two Low findings where the locked Frame contradicted the already-correct locked Spec: R-0005-f startup file-mode check named the vacuous *signing-verification-material file* (the verification material is the embedded `ROOT` constant, not a file); QA-2 asserted `GATE verify-build` vs the justfile's `GATE build`. Spec needed no change.
2. **Spot-check (Peter, review depth):** directed a targeted Frame reconciliation (not a re-derivation), matching the v0-substrate pattern.
3. **DaVinci (dispatch 1193):** reconciled the Frame — 5 hunks (R-0005-f target → admin-token file across D4 / QA-6 m3 / §5 amended-in-place table; `GATE build` across QA-2 m1-2), swept all references (zero residuals), spec untouched. Puck committed `325ebeb`; BOM `frame.version` + `spec.generated_against.frame_version` re-locked to `83bba6a`.
4. **Round-2 Charon (dispatch 1194):** Pass — both findings closed against primary ground truth (`root_material.rs` `ROOT` is a compile-time `&[u8]` constant, not a runtime file; parent R-0008-e binds the admin-token file to the R-0005-f check; `justfile:175/177` emits `GATE build PASS/FAIL`), zero findings ≥80, no collateral drift.
5. **Round-2 spot-check (Peter):** one sub-80 provenance-pointer divergence (locked spec frontmatter/body cite frame blob `946d786`; BOM locks `83bba6a`) — dispositioned **accept-as-residue** (P-LockContract / P-MinBlastRadius: do not touch the BOM-locked spec for a cosmetic pointer; the BOM is the authoritative /verify surface and is consistent; the old blob still resolves in git). Not blocking.

## Audit-chain re-validation (at HEAD 325ebeb)
- intent: `1c6820e726720e76f60aa5c76735af0223de5207` @ docs/intent/signing-to-runnable.md — match
- frame:  `83bba6a5ba76e6dab97b74799212107332e0198d` @ docs/intent/signing-to-runnable-frame.md — match (reconciled)
- spec:   `479852a28124727ca3eafdb6cd29f0ad95d76f76` @ docs/specs/2026-06-30-signing-to-runnable.md — match (unchanged)
- impl:   n/a (architecture spec)

## Pre-signal checks
- chain_freshness: pass (all three blobs match the re-locked BOM at HEAD 325ebeb)
- chain_internal_conformance: pass (frame + spec both "pass")
- stuck_detector: n/a (architecture spec). **BOM nit:** `pre_signal_checks.stuck_detector.applicable = true` should be `false` for a non-code spec — no-op here (no impl change set), surfaced to Peter, non-blocking.

## Signal array
- **auditor (Charon, dispatch 1194 / round 2): pass** — happy path, zero findings ≥80 across Values / Principles / ADRs / Intent / Frame; both round-1 Frame findings closed against ground truth; one sub-80 provenance-pointer divergence surfaced to spot-check.
- **spot_check (Peter, review depth): passed** — round-1 directed the targeted Frame reconciliation; round-2 dispositioned the provenance divergence as accept-as-residue. Push pre-authorized, conditional on the round-2 pass now confirmed.

## Summary
Passed. The shift-left backstop caught two real Low frame⊥spec inconsistencies — a Frame-exit gate let through a freshly-authored Frame with a vacuous file-mode-check target and a `GATE` emission string that contradicted the repo's own justfile. The locked spec (the implementation driver) was already correct, so nothing shipped broken. A targeted Frame reconciliation (not a re-derivation) closed both; round-2 confirmed closure against primary ground truth; the only residue (a stranded frame-blob pointer in the locked spec) was dispositioned accept-as-residue under P-LockContract. Gate that should have caught the originals: the Frame-exit internal-conformance check — rule candidate logged (QA-measure literals checked vs repo ground truth + can-fail discipline applied to QA measures themselves).

## Loop-back recommendation
**Ship** — squash-merge `signing-to-runnable` to `main` (push pre-authorized by Peter, conditional on the round-2 pass now confirmed). Design-artifacts only, no code. Next: committed-tier plan (Wyrd) sequencing M1 → M2, then implementation.
