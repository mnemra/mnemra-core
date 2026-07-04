# Verify Verdict — 2026-07-03-reporting-engine

**Date:** 2026-07-04T19:59:47Z
**Spec:** docs/specs/2026-07-03-reporting-engine.md:932fe23a6c5ceb0fe3ffa298a0284f967291605f
**Verdict:** passed_with_concerns (designed-tier — impl NOT verified)
**Covers:** intent + frame + spec (designed-tier)
**Full-run required:** yes — designed-tier run on a code spec; post-impl /verify remains mandatory
**Aggregate:** All pre-signal checks pass; Charon Pass with concerns (one Low ADR-precision finding, non-gating); spot-check approved as-is.
**requires_upstream_change_at:** n/a

## Audit-chain re-validation
- intent: eab0553635904e025f6e97ce3242a409d902f0e7 @ docs/intent/reporting-engine.md — match
- frame:  871441da32d9cbbb799ac7ea96c3b0806b40f196 @ docs/intent/reporting-engine-frame.md — match
- spec:   932fe23a6c5ceb0fe3ffa298a0284f967291605f @ docs/specs/2026-07-03-reporting-engine.md — match
- impl:   absent (designed-tier — exempt by mode)
- consumed contract (informational, R-0049-f/OC-11 instrument): retrieval-cluster 0b948ea22ef77ffd122a9aec3700b5bdeb2098a5 @ main:docs/specs/2026-07-02-retrieval-cluster.md — match (pin scope R-0025 + R-0026 + R-0025-g intact; D4 re-derive not fired)

## Pre-signal checks
- chain_freshness: pass (worktree clean; all three blob SHAs match BOM; Charon's independent re-check agrees)
- chain_internal_conformance: pass (frame: pass; spec: pass)
- stuck_detector: skipped (designed-tier)

## Signal array
- auditor (Charon, T1 Opus, dispatch 1345): pass_with_concerns

### /verify Canon Conformance — Reporting Engine, designed-tier chain (intent + frame + spec + companion ADRs P-0020/P-0021)

**Verdict:** Pass with concerns — one Low-severity ADR-conformance finding: spec R-0044-b and P-0020 D-GRANT label `agents` a "core-entity table … not sensitive-class," which is imprecise against locked canon (P-0018 closes the core entity set at four, with the unified `actors`; P-0002 names `P-builtin-agents` a distinct identity builtin, sibling to the sensitive-class `users`). The imprecision resolves two ways with different fixes — surfaced, not silently picked. Upstream-change layer: **spec** (spec-local text fix + companion ADR P-0020; no P-0018/P-0002 amendment, no re-lock of the security model). Nothing blocks the designed→committed transition.

**Audit-chain freshness re-check:** intent eab0553… match; frame 871441d… match; spec 932fe23… match; impl n/a (designed-tier); consumed-contract retrieval-cluster 0b948ea… match (unmoved).

**Values:** No findings. Security exemplary (two independent write-blocks, scoped RLS keyed per-execution, function-EXECUTE allow-list, session-key tamper-resistance proven not assumed, fail-closed OC-10 reconciliation with standing migration-apply trigger, 11-item threat suite, three prior security-lensed rounds — no layer conceded silently). Simplicity's RLS-early exception traded off with the `Security ⇄ Simplicity` axis named, resolved to Security. Quality/Observability/Maintainability/Honesty/Decomposition/Composition hold.

**Principles:** No findings. P-Defer/DF1 (every deferral carries a named firing instrument), P-SecurityLayered, P-TrustworthySignal TS2 (reconciliation ships with each execution-gating enumeration; PG-test-binary wiring note pre-empts the silent-narrowing class), P-MinBlastRadius (one fragment module, N consumers), P-LockContract, P-InstrumentBefore, P-ShiftLeft D1+D2, P-AgentPrimarySource, P-PreserveDecisionSpace — all honored.

**ADRs:** One finding.

- [spec R-0044-b:L87; also P-0020 §D-GRANT] (severity: Low, confidence: 88) "The core-entity tables (`projects`, `agents`, and kin) are simply *not granted* at V0 — not sensitive-class" is imprecise against locked canon and resolves two ways: **Reading A** — if `agents` denotes a core entity, the name is wrong (P-0018 D-ENT closes the core FK-target set at `projects`/`actors`/`tags`/`attachments`; no `agents` core entity exists); **Reading B** — if `agents` denotes the `P-builtin-agents` identity store (P-0002 names it a distinct identity builtin, sibling to `P-builtin-users`), it is not a core-entity table and is labeled "not sensitive-class" while its sibling `users` is excluded as sensitive in the same clause. Net V0 grant effect identical under either reading (unreadable by default-deny — no live cross-tenant leak); the hazard is a latent classification asymmetry a future P-0020 grant-set amendment could lean on to expose agent-principal identity where `users` is protected. Canon ref: P-0018 D-ENT/D-ACTOR; P-0002 §Builtins. Upstream-change layer: spec (+ P-0020 D-GRANT mirror).

All other ADR cross-references conform: the scoped RLS exception reconciles against P-0006/P-0009's R-0001 with the three OR-condition trip-wire and V0.1 `mnemra.workspace_id`/`mnemra.role` keying carried verbatim; P-0015 PE-2/PE-4/PE-6 dispositions consumed faithfully; `visibility`-first precedence correctly derives from R-0025-g; P-0017 `reference` classification gate-accepted; P-0018 D-BOUNDARY honored; P-0002/P-0007/P-0010/P-0014-RA-4/P-0019/P-0001 cited consistently.

**Intent conformance:** No findings. JTBD delivered; all eight Non-goals honored; SC1–SC4 render to requirements + scenarios; SC4's R80–R99+C-D disposition map complete with no silent drop; SC5 correctly treated as a pipeline marker; all hard constraints traced.

**Frame conformance:** No findings. Directions 2a-1/2a-2/2a-3 elaborated; D1–D8 → R-0040–R-0062; OC-1..OC-11 → clauses; TB1–TB5/S1–S4 → scenarios + threat tests. Open ADR slots resolve as authorized: P-0020, P-0021, `{{P-ReportRegistrySurface}}` folded (no P-0022) under the Frame's own fold-vs-lock discriminator — deliberate, correctly recorded.

**Canon ambiguity:** None. Both candidate tensions (scoped RLS exception vs deferred-RLS posture; one definition table vs Non-goal 2) are gate-adjudicated strains with the axis named and canon-anchored resolutions recorded.

**Suppressed findings (confidence < 80):** Values 1 found / 1 suppressed (RLS-early-vs-Simplicity — named, gate-accepted trade-off, below violation bar); Principles 0/0; ADRs 2 found / 1 suppressed (measurement-family grants vs P-0018 D-BOUNDARY — dismissed: measurement tables ≠ workflow-primitive content tables); Intent 0/0; Frame 0/0.

**Charon summary:** Happy path, one exception. The designed-tier chain is fresh, internally coherent, and conforms across all five dimensions; every load-bearing claim spot-checked against source matched verbatim. The single finding is a Low ADR-precision inconsistency with no live V0 consequence. The gate that should have caught it: the spec-authoring internal-conformance / ADR-anchoring pass (secondarily the three review rounds + maintainer gate) — a cross-reference-precision check against cited P-0018/P-0002 entity-vs-builtin names, which load-bearing-tuned gates let slip because the parenthetical carries no functional weight.

- intent_conformance (named signal, surfaced non-gating per 2026-06-29 a2): pass — zero intent findings, zero suppressed. Excluded from the AND-aggregate by design.
- spot_check: pass
  Depth: review (BOM-configured). Maintainer reviewed the Stage B summary + full Charon report (scratch/dispatch-1345-report.md) 2026-07-04 and dispositioned: (1) verdict **approved as-is** — finding stays Low, no elevation to Medium; (2) fix shape ratified as **both** readings' correction — rename the core-entity example `agents`→`actors` AND explicitly add the `P-builtin-agents` identity store to the R-0044-b identity/auth exclusion enumeration beside `users`/`sessions`, mirrored into P-0020 D-GRANT; (3) fix timing — **at designed→committed pickup** (tracked task; rides the committed-tier freshness re-check), no fourth lock commit now.

## Summary
Second live /verify run, first on a W1-C-lane spec (designed-tier dogfood, W1-A precedent). All mechanical checks pass; Charon surfaced one Low ADR-precision finding (the `agents` naming/classification imprecision in R-0044-b + P-0020 D-GRANT) that the spec-authoring ADR-anchoring pass should have caught. Maintainer approved the verdict as-is and ratified the combined fix (rename → `actors` + regroup the identity store into the sensitive exclusion set), deferred to the designed→committed pickup as a tracked task. Advisory verdict only — the post-impl full run remains mandatory.

## Loop-back recommendation
No re-run. Designed-tier verdict stands; the R-0044-b/P-0020 text fix lands at committed-tier pickup (tracked in puck.db), where this verdict file is overwritten by the full post-impl run.
