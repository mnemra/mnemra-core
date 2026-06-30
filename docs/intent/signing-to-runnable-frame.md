---
title: "Frame: real plugin signing + supply-chain binding + runnable host (signing → run())"
summary: "Brownfield-extension Frame for instantiating and hardening the mnemra-core plugin-signing / supply-chain trust boundary (placeholder root → real Ed25519 root; manifest-only verification → manifest + component-byte binding) and assembling the production run() entry point so the host serves a live stdio MCP server end-to-end. Renders the maintainer-locked design as canon-anchored architectural directions, ATAM quality-attribute scenarios, and the open R-ID / ADR-amendment slots Stage 3 will lock. Locks the one open governance item: the content-hash field is routed to a P-0003 amendment (not a spec-local field)."
primary-audience: agent
modulation: brownfield-extension
intake: docs/intent/signing-to-runnable.md
amends-spec: docs/specs/2026-05-24-mnemra-core-v0-substrate.md
amends-spec-version: 3118fbb3f5f81c45f3ea1e7c42bd6448114f123f
amends-spec-type: architecture
---

# Frame — real plugin signing + supply-chain binding + runnable host (signing → run())

**Date:** 2026-06-30 · **Status:** draft (Stage 2b synthesis; Frame-exit gate pending) · **Altitude:** trust-boundary instantiation + host-assembly

> **Modulation note.** This is a `brownfield-extension` Frame. A locked, verified architecture
> spec (`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`) and the product Frame
> (`docs/src/intent/mnemra-core-frame.md`) already govern mnemra-core; Stage 2a elicitation
> collapses because scope and the V0 ceremony approach were locked with the maintainer before this
> run. This Frame is **not** a restatement of the product Frame — it frames the **deltas only**
> (real root + content-hash binding; the `run()` assembly + its `/health` dependency), the closed
> world the Stage-3 Spec amendment draws from. It refines the locked spec; it does not contradict
> it. Where a tension would surface between this work and the locked spec, this body records it
> explicitly rather than silently choosing. (None surfaced — see §2.)

## 1. Purpose / context

The host has a verified-load mechanism and an MCP server handler, but it has **never actually
run**: the production entry point `run()` is an empty `Ok(())` stub
(`libs/mnemra-host/mnemra_host.rs:36-38`), the embedded signing root is a non-production
placeholder (`libs/mnemra-host/signing/root_material.rs:35-40`), and the on-disk plugin manifest
is placeholder-signed (`plugins/mnemra-echo/manifest.toml:77-96` — `public_key = "ROOT"`,
`sig_bytes = "PLACEHOLDER_SIG"`, no content-hash field). This Frame designs the change that makes
the host **start as a live stdio MCP server backed by real plugin signing**, so the V0 reference
workload can be exercised end-to-end with no fallback to prior tooling.

The work is two milestones under one spec, sequenced by the committed-tier plan:

- **M1 — real signing + supply-chain integrity.** Replace the placeholder root with the real
  Ed25519 root and gate the build against a placeholder; add a content-hash of the component bytes
  to the signed manifest body and make the verified-load gate recompute-and-reject on mismatch.
  M1 is the security foundation and merges first.
- **M2 — actually runs.** Assemble `run()` to perform production startup in the locked order, stand
  up the `/health` loopback listener the order requires, source the storage pool, and serve over
  stdio. M2 depends on M1.

- **What this designs.** The trust-boundary instantiation (placeholder → real signing;
  manifest-only → manifest + component-byte binding) and the `run()` assembly + serve path. It does
  **not** re-derive the signing primitive, the custody model, the transport, or the startup
  ordering — all locked (see §LOCKED below).
- **Intake.** [`docs/intent/signing-to-runnable.md`](signing-to-runnable.md) — locked, high-stakes,
  `spec_type: architecture`. This Frame transcribes the maintainer-locked design into
  canon-anchored directions; it does not re-derive it.
- **Spec being amended.** [`docs/specs/2026-05-24-mnemra-core-v0-substrate.md`](../specs/2026-05-24-mnemra-core-v0-substrate.md)
  (`spec_type: architecture`), at blob SHA `3118fbb`. The amendment adds new requirement slots
  (content-hash binding; `run()` assembly) and extends existing ones in place
  (R-0018-f `verify-build`; R-0005-d embedded root). It does not fork a new spec.
- **Code surfaces touched.** `libs/mnemra-host/signing/root_material.rs` (embedded root),
  `libs/mnemra-host/startup/pool_population.rs` (verified-load gate), `libs/mnemra-host/mnemra_host.rs`
  (`run()` assembly), `libs/mnemra-host/mcp/server.rs` (production serve-loop),
  `plugins/mnemra-echo/manifest.toml` (real signing + content-hash), a new `/health` module, and the
  `verify-build` / `verify-smoke` Justfile recipes.

This Frame is the closed world for the Spec amendment: nothing outside the directions locked here
enters the Spec.

## 2. Constraint-graph walk

Walking `brain/about/constraint-edges.md` from the intent (instantiate + harden an existing
signing/supply-chain trust boundary; assemble a stub entry point into a live server). Traversal rule
(G-0013): most-specific applies when no conflict — per-task > project ADR > workspace ADR >
principle. The most-specific layer here is the mnemra-core project ADRs (P-0003, P-0005, P-0010,
P-0012), which apply over the workspace principles they specialize.

### Keystone edge — why a verified manifest is *not* a verified component

**`P-SecurityLayered → Security` (specializes, `constraint-edges.md:42`)**, specifically the value's
**"each layer independently load-bearing"** clause (`architecture-values.md:31`):

> "Security defaults compose by layer, with each layer independently load-bearing … Losing any layer
> weakens the whole."

The prior security review (High confidence; intake Evidence) found that the verified-load path
authenticates the **manifest** but nothing binds the loaded **component bytes** to it: after real
signing, a swapped `.wasm` loads undetected even though the manifest signature still verifies. The
two controls are **distinct, independently load-bearing layers** — manifest-signature verification
(provenance of the *declaration*) and component-byte binding (integrity of the *artifact*) — and one
cannot substitute for the other. Real signing instantiates the first layer; it leaves the second
**absent**. This is the load-bearing anchor for the whole M1 delta: the content-hash binding (D2/D3)
is not a refinement of signing — it is a **second, missing security layer** that must land *with or
before* real signing (D5), because shipping real signing without it would close one layer while
silently leaving the supply-chain-swap surface open. It pre-empts the "real signing already secures
the load" objection: signing secures the manifest; only the content-hash secures the bytes.

### Other edges bearing on the locked decisions

| Edge (type) | Bears on | How it applies |
|---|---|---|
| `P-Defer → Simplicity, Reversibility` (specializes, `:35–36`) | The deferred Tier-C custody hardening; the minimal V0 ceremony | The minimal maintainer-local ceremony (D4) is the smallest mechanism that satisfies the custody constraints; production custody hardening (CI-secret-handling, rotation, HSM-backed keys) is **deferred** behind the R-0005-e multi-deployment trip-wire (the not-yet-authored Tier-C `{{P-SigningKeyCustodyHardening}}` ADR) and is **not designed here**. The `verify-build` fail-if-placeholder gate + CI check is the *structural* enforcement that the V0 placeholder can never silently ship. |
| `P-LockContract → Simplicity, Migration cost` (specializes, `:38–39`) | The manifest schema (content-hash field home); the startup-order invariant; the `Storage` seam; the transport | Three sub-applications: (a) the manifest **schema is the locked contract** every plugin depends on — a new schema field locks into the schema's governing record (P-0003), driving the §5 governance decision; (b) the production startup **order is an intrinsic invariant** ("an invariant intrinsic to what the artifact *is* locks at the stage it is defined") — R-0004-g / R-0002-c / R-0016-a are already locked, and `run()` composes them (D6); (c) the `Storage` seam (P-0010 `start_embedded()`) and the rmcp transport (P-0012) are locked seams `run()` consumes (D8/D9). |
| `G-0003 (merge governance) → P-ShiftLeft` (specializes, `:79`) | The security-mode review this Frame carries forward | Substance is reviewed shift-left on this artifact; a mandatory pre-push security review (security mode) gates the merge per the G-0003 2026-06-25 amendment. §6 makes the security-relevant mechanism explicit so that review can run; a Critical/High finding is fixed in-loop and escalates to Peter only if unresolvable. |
| `P-InstrumentBefore → Observability` (specializes, `:43`) | The host's first real run; the `/health` readiness surface | The host goes from never-run to a live server (M2) — its **first real run** in P-InstrumentBefore's sense. The readiness instrument (`/health`, R-0004-g) and the per-verb metric/log surfaces already exist; M2 **brings the existing instrumentation online and binds `/health` first** (D6/D7). No new instrumentation is designed — the requirement is satisfied by instantiation, not addition. |
| Project ADR `P-0005-v0-signing-chain` (specializes `P-SecurityLayered` + `P-Defer`) | The signing primitive, custody, fail-closed verification, the trip-wire | Most-specific layer. Ed25519 / build-host-on-disk mode-600 / synchronous fail-closed / embedded verification material / multi-deployment trip-wire are all locked here (Option A + the 2026-05-24 core-by-provenance amendment). M1 **instantiates** P-0005; it does not re-decide it. |
| Project ADR `P-0003-plugin-manifest` (specializes `P-LockContract` via the schema contract) | The content-hash field's governance home | Most-specific layer. P-0003 is the **sole authority** for the V0 manifest schema (`schema_version: 1`). A new signed-body field alters that schema → governed by P-0003 (§5 governance decision). |
| Project ADR `P-0012-plugin-runtime-and-mcp-sdk` (specializes `P-StackDiscipline`) | The production serve-loop | Most-specific layer. rmcp v1.7.0, stdio, no HTTP feature — the production stdio serve-loop (D8) is in-stack instantiation of the locked SDK choice. |

### Conflicts-with finding

**None surfaced.** The one candidate tension — `Security ⇄ Simplicity` (`constraint-edges.md:133`,
"each layer load-bearing" vs "smallest mechanism") — does **not** fire here. The supply-chain
binding is simultaneously the Security default **and** a minimal mechanism: a single hash field in
the signed body plus a recompute-and-compare on the load path (no new subsystem, no schema migration
beyond the field, no runtime key-fetch). The minimal V0 ceremony (D4) is likewise both the Simplicity
choice and a Security-preserving one (custody honored even on a single machine). Each serves both
values; it is mutual reinforcement, not a trade-off to escalate. The custody degeneracy "off
deployment node goes degenerate when build host == run host" is a **locked resolution** (intake Hard
constraint + P-0005), not a live conflict: the private key stays out of any runtime-read directory.
No `conflicts-with` edge requires maintainer escalation, no novel cross-project precedent is set, and
no locked artifact is invalidated.

## 3. Locked architectural directions

Each direction is the maintainer-locked decision (or a canon application thereof), rendered with its
canon citation. These are **not** re-opened — if a reader believes one is wrong, that is a
halt-and-escalate to Puck/PA, not a Frame-internal pivot.

### M1 — real signing + supply-chain integrity

#### D1 — Real Ed25519 root replaces the placeholder; `verify-build` fails on a placeholder root, enforced in CI

The embedded root (`libs/mnemra-host/signing/root_material.rs:35-40`) is today a 32-byte non-prod
bootstrap placeholder with **no** fail-if-placeholder check. M1 replaces it with the real root
public key from the V0 ceremony, and the `verify-build` recipe gains a gate that **fails the build**
if the embedded root is still the placeholder; CI runs `verify-build` so a placeholder root can never
ship.

- **Anchor:** R-0005-d (verification material embedded at build time; no runtime key-fetch),
  R-0018-f (`verify-build` SHALL produce the signed binary with the embedded root material), intake
  SC2. Workspace anchor: `P-SecurityLayered` (supply-chain + change-time layers; Security default-on,
  never opt-in — the gate is structural, not advisory).

#### D2 — Content-hash of the component bytes, in the signed canonical body

The manifest gains a **content-hash field of the component bytes**, located in the **signed
canonical body** — the bytes before the `\n[signature]` marker that the verifier slices and the
signature covers (`libs/mnemra-host/signing/verify.rs:337-344`). R-0005-h core-by-provenance forces
this location: a hash in the unsigned `[signature]`-adjacent section would let an attacker swap both
the component and its hash.

- **Anchor:** R-0005-h (the content-hash MUST live in the signed body — core-by-provenance), intake
  Hard constraint + SC3. P-0003 (manifest schema authority — the field's governance home, §5).
  Workspace anchor: `P-LockContract` (the manifest schema is the locked contract; a schema field
  locks into the schema's governing record).
- **Governance:** routed to a **P-0003 amendment** (the dispatch's one open item — locked in §5).

#### D3 — The verified-load gate recomputes the loaded component's hash and rejects on mismatch (fail-closed)

`populate_verified_pool` (`libs/mnemra-host/startup/pool_population.rs:126`) today verifies the
manifest, then loads the `.wasm` from the build target dir with **no hash check**
(`pool_population.rs:141-143`). M1 adds: after manifest verification succeeds, recompute the loaded
component's hash and compare it to the signed content-hash; on mismatch, reject the load with a
structured error **before any pool or instance is constructed** — the same fail-closed discipline as
the signature gate (verify-first, build-nothing-on-failure).

- **Anchor:** R-0005-a (synchronous fail-closed; no instance until verification returns Ok),
  R-0005-b (structured error naming the plugin; no best-effort load), intake SC3 + Evidence (the
  supply-chain-swap finding). Workspace anchor: `P-SecurityLayered` (the keystone — manifest-sig and
  component-byte-binding are distinct independently-load-bearing controls).
- **Note (Stage-3 mechanics, not a Frame decision):** the mismatch error class — reuse the existing
  `StartupError::ComponentLoad` variant (`pool_population.rs:54`) or add a distinct
  hash-mismatch variant — is a Stage-3 / implementation-review concern (Warden F2), not locked here.

#### D4 — Minimal V0 maintainer-local signing ceremony; custody preserved even when build host == run host

DECOMPOSER-LOCKED. The ceremony: generate the keypair locally, store the **private key mode 600
outside any directory the runtime reads**, embed the public key, and sign the manifest (replacing the
`public_key="ROOT"` / `sig_bytes="PLACEHOLDER_SIG"` placeholders at
`plugins/mnemra-echo/manifest.toml:77-96`). "Off deployment node" goes degenerate when the build host
and run host are the same machine (R-0005-c), so the binding constraint is: the private key stays out
of any runtime-read directory. No runtime key-fetch (R-0005-d). The startup file-mode invariant check
(R-0005-f) covers the admin-token file.

- **Anchor:** P-0005 (V0 signing chain, Option A custody — build-host-on-disk, mode 600), R-0005-c /
  R-0005-d / R-0005-f, intake Hard constraint + SC4.
- **Intentional gap (do NOT fill):** Tier-C custody hardening — CI-secret-handling, key rotation,
  hardware-backed custody — is **deferred** behind the R-0005-e multi-deployment trip-wire (fires
  before any non-maintainer deployment). Not designed in this Frame. `P-Defer`.

#### D5 — Content-hash binding lands with or before real signing (sequencing)

The content-hash binding (D2 + D3) lands **with or before** real signing (D1) within M1 — real
signing without component binding verifies the manifest but not the bytes (the keystone). M1 (the full
security foundation) merges before M2.

- **Anchor:** intake Hard constraint ("content-hash binding lands with or before real signing") +
  Sequencing note. Workspace anchor: `P-SecurityLayered` (do not ship a partial control — the layer
  must be whole at merge), `P-MinBlastRadius` (a large change lands as a sequence: M1 foundation
  before M2 assembly).

### M2 — runnable host

#### D6 — `run()` performs production startup in the locked order

`run()` is an empty `Ok(())` stub (`libs/mnemra-host/mnemra_host.rs:36-38`). M2 wires it to perform
production startup, composing the **already-locked** ordering requirements:

- **(5a)** the `/health` loopback listener binds **before config load** and before MCP accept
  (R-0004-g);
- **(5b)** all seven builtins initialize **before any plugin is loaded** (R-0002-c), and the verified
  plugin pool is populated **before** MCP accept (R-0016-a);
- **(5c)** a storage pool is sourced and schema initialization runs (D9), the server is constructed
  (`MnemraMcpServer`, `libs/mnemra-host/mcp/server.rs:89`), and it serves over stdio (D8).

The ordering R-IDs already exist; the **assembly** is the delta — `run()` is a stub today, so the
composition behavior has no existing home.

- **Anchor:** R-0004-g, R-0002-c, R-0016-a, R-0013-a, R-0010-a, intake SC5. Workspace anchor:
  `P-LockContract` (the startup order is an intrinsic invariant — it locks at the stage it is
  defined, and `run()` is its instantiation).

#### D7 — The `/health` loopback listener is stood up (the dependency the locked order requires)

The locked order requires a health listener that does not yet exist (no `/health` module; the
already-planned Task 25 surface). R-0004-g **fully specifies** it (loopback `127.0.0.1` only,
`MNEMRA_HEALTH_PORT` default `8877`, `GET /health` only, the structured detail body, the
non-loopback-bind tripwire). M2 stands it up and binds it **first**.

- **Anchor:** R-0004-g (already specified — this is build + wire, not a text change), intake Hard
  constraint (Health dependency). Workspace anchor: `P-InstrumentBefore` (the readiness instrument
  ships before the first real run).
- **Intentional gap (do NOT fill):** `/health` internals beyond the ordering the assembly imposes
  (it binds first) are the Task 25 surface, not designed here.

#### D8 — Production stdio serve-loop (rmcp v1.7.0, stdio, no HTTP)

`serve_server` exists **only in tests** over an in-process duplex transport (`tests/mcp_server.rs:331`);
there is no production stdio serve-loop. `MnemraMcpServer` already impls `rmcp::ServerHandler`
(`libs/mnemra-host/mcp/server.rs:124`). M2 adds the production **stdio** serve-loop.

- **Anchor:** P-0012 (rmcp v1.7.0), R-0010-a (single MCP server, stdio transport at V0), R-0010-e
  (Streamable-HTTP SHALL NOT be activated at V0), intake Non-goal (no Streamable-HTTP). Workspace
  anchor: `P-StackDiscipline` (in-stack SDK; no foreign transport).

#### D9 — Storage pool from `start_embedded()`; schema init; NOT the LLM-key/config pathway

The production pool is sourced from `PostgresStorage::start_embedded()`
(`libs/mnemra-host/storage/postgres.rs:82`) — the embedded-Postgres engine startup path — and schema
initialization runs (R-0013-a, `mnemra init`: bootstrap on the embedded engine, enable bundled
`pgvector`, create substrate tables + the `default` workspace). This is **not** the LLM-key /
embedding config pathway (R-0014 governs that and does not apply here).

- **Anchor:** P-0010 (storage substrate engine — `start_embedded()`), R-0013-a (schema bootstrap),
  intake Hard constraint (Storage source). Workspace anchor: `P-LockContract` (the `Storage` seam is
  the locked contract `run()` consumes).

## 4. Quality-attribute scenarios (ATAM)

Each scenario is `[stimulus · environment · response · measure]`, the unit of architectural decision.
Each **measure is a conjunction of binary checks**, not a single assertion.

### QA-1 — Supply-chain component swap (adversarial — the keystone scenario)

- **Stimulus:** an attacker swaps the on-disk `.wasm` for a different component **after** the manifest
  is signed, leaving the (validly signed) manifest untouched.
- **Environment:** the verified-load gate `populate_verified_pool` with the content-hash binding active
  (D2/D3); the manifest signature still verifies against the real embedded root.
- **Response:** the load is **rejected** on content-hash mismatch — the manifest's signature verifying
  is **not sufficient** to load a component whose bytes do not match the signed hash.
- **Measure (all must hold):**
  1. With the **correct** component, `populate_verified_pool` returns `Ok` (signature verifies **and**
     recomputed hash == signed hash).
  2. With a **swapped** component (different bytes, same valid manifest), it returns `Err` —
     specifically the hash-mismatch path, **not** the signature path (the signature still verifies).
  3. On the mismatch path, **no pool and no instance are constructed** (fail-closed; the gate fired
     before construction).
  4. The mismatch returns a **structured error naming the plugin** (R-0005-b), with **no** raw
     filesystem/wasmtime internals echoed to the caller.
  5. Moving the content-hash to an **unsigned** position (outside the `\n[signature]` slice) does not
     defeat the binding — such a manifest fails (the signature no longer covers the hash / the
     recompute still fires). (R-0005-h.)

### QA-2 — Placeholder root cannot ship (build gate)

- **Stimulus:** a build is attempted while the embedded root is still the bootstrap placeholder.
- **Environment:** the `verify-build` recipe with the fail-if-placeholder gate (D1); CI invokes
  `verify-build`.
- **Response:** the build **fails**; no binary carrying a placeholder root is produced.
- **Measure (all must hold):**
  1. `verify-build` returns **non-zero** (and emits `GATE build FAIL` per R-0018-f) when the
     embedded root equals the known placeholder bytes.
  2. `verify-build` returns **zero** (`GATE build PASS`) when the embedded root is the real key.
  3. The check runs inside CI's `just ci` path (R-0018-f), so a placeholder root cannot reach main.

### QA-3 — Real-signing round-trip

- **Stimulus:** the host loads the real-signed `mnemra-echo` manifest against the real embedded root.
- **Environment:** `verify_plugin` (`libs/mnemra-host/signing/verify.rs:172`) with the real root
  embedded (D1) and the manifest signed by the ceremony (D4).
- **Response:** verification returns `Ok(CoreStatus::Core)` for the genuine manifest and `Err` for a
  tampered one.
- **Measure (all must hold):**
  1. The genuine real-signed manifest verifies (`Ok(CoreStatus::Core)`) against the **real** embedded
     root — not the bootstrap placeholder.
  2. A manifest with any byte mutated in the signed body fails verification (the byte-exact slice is
     what is signed — `verify.rs:337-344`).
  3. The on-disk `plugins/mnemra-echo/manifest.toml` no longer carries `public_key = "ROOT"` /
     `sig_bytes = "PLACEHOLDER_SIG"`.

### QA-4 — Production startup ordering

- **Stimulus:** the host binary is started.
- **Environment:** `run()` performing production startup (D6).
- **Response:** the locked order holds, independently assertable at each ordering boundary.
- **Measure (all must hold):**
  1. **(5a)** the `/health` loopback listener binds **before** config load **and** before MCP accept
     (R-0004-g) — observable: `/health` answers on `127.0.0.1:8877` before the server accepts MCP.
  2. **(5b-i)** all **seven** builtins initialize **before** any plugin is loaded (R-0002-c).
  3. **(5b-ii)** the verified plugin pool is populated (R-0016-a, 3–5 instances) **before** MCP
     accept.
  4. **(5c)** a storage pool is sourced via `start_embedded()` and schema init runs **before** the
     server is constructed and begins serving.
  5. No MCP request is accepted before steps 1–4 complete (accept is the last step).

### QA-5 — Live serve end-to-end

- **Stimulus:** the host binary is run and an MCP client issues a request over stdio.
- **Environment:** the production stdio serve-loop (D8) with a populated verified pool.
- **Response:** the host answers the MCP request end-to-end.
- **Measure (all must hold):**
  1. The `verify-smoke` recipe (Task 27, named in R-0018-f) **passes** — the binary starts a live
     stdio MCP server that answers an MCP request end-to-end (intake SC6).
  2. The transport is **stdio**; no HTTP MCP transport is opened (R-0010-a/-e) — the only non-stdio
     listener is the loopback `/health` (which is not an MCP transport, R-0004-g).

### QA-6 — Key-custody invariant (degenerate same-machine case)

- **Stimulus:** the host starts on a machine that is also the build host (single-developer dogfood).
- **Environment:** the V0 ceremony's key placement (D4) + the R-0005-f startup file-mode check.
- **Response:** the private key is not reachable on any runtime-read path; only the public key is
  embedded.
- **Measure (all must hold):**
  1. The private signing key file is **mode 600** and resides **outside** every directory the runtime
     reads.
  2. The runtime performs **no** key-fetch at startup or load (R-0005-d) — only the embedded public
     key is used.
  3. The startup file-mode invariant check (R-0005-f) refuses to start if the
     admin-token file is world-readable.

## 5. Open ADR / R-ID slots

What Stage 3 (the Spec amendment) will lock. Names the slots; the only ID asserted is the
confirmed-next R-0021 (the spec runs R-0001..R-0020; R-0020 is the paging requirement).

### Governance decision — content-hash field home: routed to a **P-0003 amendment** (not a spec-local field)

**The dispatch's one open item.** The intake commits to *adding the field in the signed body*; it
carries the **governance home** — P-0003 amendment vs. spec-local field — to this constraint-walk to
resolve. **Decision: the field is governed by a P-0003 amendment.**

**Criterion (intake-stated):** is the content-hash a durable part of the manifest contract *every*
plugin must satisfy (→ P-0003 amendment), or scoped to this work (→ spec-local field)? Both halves
resolve to amendment:

1. **Durable / every-plugin.** The content-hash is recomputed and enforced on *every* plugin load —
   for every present and future `core: true` plugin — by the verified-load gate
   (`libs/mnemra-host/startup/pool_population.rs:126`, D3). It is not a one-workstream artifact.
   P-0003 already frames the manifest as where each plugin "declares its identity, content ownership,
   and required host-fn surface"; component-byte integrity is part of that identity.
2. **Manifest-schema change.** The field adds a new key to the signed canonical body — it alters the
   V0 manifest *schema*, and P-0003 (`docs/src/adrs/P-0003-plugin-manifest.md`) is the **sole
   authority** for that schema. A spec-local field would orphan a schema field from its schema
   authority: a future plugin author reading P-0003 would not see the content-hash requirement, and a
   future manifest-schema change could silently drop it.

*Anchors: P-0003 (manifest schema authority); `P-LockContract` (the manifest schema is the locked
contract every plugin depends on — a schema field locks into the schema's governing record, not into
a downstream spec); R-0005-h (core-by-provenance forces the field into the signed body, which makes it
a manifest-schema change).*

**Steward-not-owner boundary — what this Frame locks vs. what the PA decides.** This Frame locks only
the **routing**: the field's governance *home* is a P-0003 amendment, not a spec-local field. That is
canon **application** — the intake set the criterion, the constraint-walk applied it (the dispatch's
explicit one open item). This Frame does **not** author the amendment text. The P-0003 amendment
itself is the Stage-3 `{{P-ManifestContentHash}}` slot, written as a canon-amendment *shape* for the
Principal Architect at the Frame-exit gate / Stage 3 — DaVinci stewards and shapes canon; the PA owns
whether canon changes (profile `<canon-stewardship>`). Precedent for the shape: P-0005 already carries
an in-place dated amendment block ("Amendment 2026-05-24 — Core-signature binding locked at V0",
`docs/src/adrs/P-0005-v0-signing-chain.md:127`); the content-hash amendment follows that form.

**New-ADR vs. amend-existing-ADR — and the apparent tension with the sibling paging Frame.** The
sibling brownfield-extension Frame (`docs/intent/artifact-list-paging-frame.md` §5) decided **no new
P-ADR** because pagination was *pure application* of existing project canon. This Frame routes the
content-hash to a P-0003 *amendment*. There is **no inconsistency** — these are different questions.
The paging question was "does this novel behavior warrant a *standalone* new `P-NNNN` ADR?" (answer:
no — it is application). This question is "does a change to the manifest *schema* belong in the
schema's governing ADR?" (answer: yes — by amendment, because P-0003 owns the schema). Neither Frame
mints a standalone new ADR; this one extends the existing schema authority in place.

### New requirement / ADR-amendment slots (behavior with no existing home)

| Slot | Locks | Status |
|---|---|---|
| `{{R-ContentHashBinding}}` (R-0021 confirmed next) | The supply-chain binding **behavior**: the manifest carries a content-hash of the component bytes in the **signed canonical body** (D2); the verified-load gate recomputes the loaded component's hash and **rejects on mismatch**, fail-closed with a structured error before any pool/instance (D3); the swapped-component-rejected invariant (QA-1). Anchors R-0005-a/-b (fail-closed structured error) + R-0005-h (signed-body location). Governed by the `{{P-ManifestContentHash}}` P-0003 amendment. | **Open — Stage 3 assigns the R-ID + locks** |
| `{{P-ManifestContentHash}}` (P-0003 amendment) | The **schema** change: a content-hash field added to the V0 manifest schema, in the signed canonical body. Authored as a canon-amendment *shape* for the PA (steward-not-owner, above). **Stage-3 residual — `schema_version` disposition:** P-0003 states "`schema_version: 1` locks the V0 manifest format" and "V0.1+ changes that alter the manifest structure increment this field." Adding a signed-body field *alters the structure*, so there is a real pull either way. **Lean: stays `schema_version: 1`** — V0 is still being *defined* (no external plugin has shipped against a frozen schema; the single `core: true` plugin, `mnemra-echo`, is re-signed with the field as part of M1), so the field is part of the V0 schema definition rather than a post-freeze evolution. Counter-pull: P-0003's literal format-lock text. Stage 3 pins this inside the amendment. | **Open — PA reviews the amendment shape; Stage 3 pins schema_version** |
| `{{R-RunAssembly}}` | The `run()` production-startup **behavior**: `run()` composes the existing ordering R-IDs in the locked order — `/health` binds first (R-0004-g) → seven builtins (R-0002-c) → verified pool before MCP accept (R-0016-a) → storage pool + schema init (P-0010, R-0013-a) → construct server → stdio serve (R-0010-a, R-0010-e) (D6/D8/D9); intake SC5 + SC6 (acceptance: `verify-smoke` passes, QA-5). `run()` is a stub today, so the assembly behavior has no existing home; it **composes** the ordering R-IDs, it does not amend them. | **Open — Stage 3 assigns + locks** |

### Amended-in-place / instantiated requirements (existing R-IDs gain a clause or are stood up)

| R-ID | Amendment / instantiation |
|---|---|
| **R-0018-f** | **Amended** — `verify-build` gains a gate that **FAILS the build** if the embedded root is still the placeholder; CI (`just ci`) runs `verify-build` so a placeholder root cannot ship (D1). The recipe and "produce the signed binary with the embedded root" clause already exist; the fail-if-placeholder clause is the extension. (QA-2.) |
| **R-0005-d** | **Instantiated** — the embedded verification material becomes the **real** root public key (the bootstrap placeholder in `signing/root_material.rs` is retired). Requirement text unchanged; the V0 build-note placeholder is what changes (D1). |
| **R-0005-c** | **Instantiated** — the V0 ceremony stores the private key mode 600 outside any runtime-read directory; custody preserved when build host == run host (D4). Text unchanged. (QA-6.) |
| **R-0005-a / R-0005-b** | **Co-anchors** — the new content-hash-mismatch rejection (D3) reuses the same synchronous fail-closed + structured-error discipline; requirement text unchanged, the new `{{R-ContentHashBinding}}` anchors to them. |
| **R-0005-f** | **Instantiated** — the startup file-mode invariant check is instantiated over the **admin-token file** (R-0008) — the runtime secret that exists on disk; the verification material is the embedded `ROOT` constant, not a file (D4). Text unchanged. (QA-6.) |
| **R-0005-h** | **Co-anchor** — core-by-provenance forces the content-hash into the signed body (D2). Text unchanged; cited as the constraint on the new field's location. |
| **R-0004-g** | **Instantiated** — the `/health` loopback listener is built (the Task 25 surface) and bound **first** by `run()` (D6/D7). Requirement text fully specifies the listener already; this is build + wire, not a text change. (QA-4.) |
| **R-0002-c / R-0016-a / R-0013-a / R-0010-a / R-0010-e** | **Preserved, composed** — `run()` composes these in the locked order (D6/D8/D9). Requirement text unchanged; listed for completeness so the Spec records them as composed-by-assembly. |

**No open binary-observability gaps remain at the Frame altitude.** Every direction has a
binary-observable measure in §4. The Stage-3 residuals are two named *values/dispositions* (the
`schema_version` disposition in `{{P-ManifestContentHash}}`, and the hash-mismatch error-variant
choice noted in D3) — both bounded and testable either way, not open observability gaps.

## 6. Risk profile (security-relevant — carried to the security-mode review)

Intake field 7 resolution, now the mechanism is concrete. **No new trust boundary is introduced** —
the work **instantiates and hardens an existing one**: the plugin-signing / supply-chain boundary
(`TB-build-pipeline` ↔ `TB-mnemra-host`). The mandatory Frame-stage security-mode review applies (the
mechanism is now concrete); the implementation-time pre-push security review gates the merge per the
G-0003 2026-06-25 amendment.

1. **Embedded-root authenticity (placeholder → real).** *Before:* the embedded root is a documented
   non-production placeholder with **no** fail-if-placeholder check (`signing/root_material.rs:35-40`)
   — a build could ship that trusts a key whose private half is effectively non-secret. *After:* the
   real root is embedded; `verify-build` fails on a placeholder; CI enforces (D1). Measure: QA-2, QA-3.
2. **Supply-chain component binding (the gap real signing alone leaves — the keystone).** *Before:*
   `populate_verified_pool` verifies the manifest but loads the `.wasm` with **no hash check**
   (`startup/pool_population.rs:141-143`) — a swapped component loads undetected even though the
   manifest signature verifies (intake Evidence; prior High-confidence finding). *After:* a content-hash
   in the signed body + recompute-and-reject on load (D2/D3). The two controls (manifest-sig +
   component-byte-binding) are each independently load-bearing (`P-SecurityLayered`). Measure: QA-1.
3. **Hash tamper-resistance (location).** The content-hash **MUST** live in the signed canonical body
   (R-0005-h core-by-provenance); a hash in the unsigned section would let an attacker swap both the
   component and its hash. The byte-exact payload slice (`signing/verify.rs:337-344`) is what the
   signature covers. Measure: QA-1 measure 5.
4. **Key custody (degenerate same-machine case).** Private key mode 600, outside any runtime-read
   directory; no runtime key-fetch; startup file-mode check (R-0005-c/-d/-f, D4). Tier-C hardening
   (CI-secret-handling, rotation, HSM-backed custody) is **deferred** behind the R-0005-e
   multi-deployment trip-wire — **not designed here** (`P-Defer`). Measure: QA-6.
5. **Live-serve attack surface (M2).** The host goes from never-run to a live stdio MCP server. **No
   new transport surface** beyond what is already specified — stdio only, no HTTP (R-0010-a/-e); the
   `/health` listener is loopback-only (R-0004-g). The MCP auth / role / verb / health gates are
   already built (`mcp/server.rs:180` `call_tool`); M2 brings them online, it does not design new
   ones. The startup order guarantees no plugin is reachable before the verified pool is populated
   (R-0016-a) and no MCP request is accepted before `/health` binds (R-0004-g). Measure: QA-4, QA-5.

**Security-mode review trigger — carried forward to implementation:** this Frame instantiates and
hardens the plugin-signing / supply-chain trust boundary (embedded root, signature gate,
manifest→component binding). The pre-push security review (security mode) is in scope; a Critical/High
finding is fixed in-loop and escalates to Peter only if unresolvable (G-0003 amendment 2026-06-25).

## 7. Intent self-report (preventive)

- **JTBD reading.** I read the job-to-be-done as: *"The host must start as a live stdio MCP server
  backed by* real *plugin signing, so the V0 reference workload runs end-to-end against the host with
  no fallback to prior tooling — which requires both (M1) instantiating the signing / supply-chain
  trust boundary for real (real embedded root + content-hash binding of the component bytes) and (M2)
  assembling `run()` so the host actually serves."* I do **not** read this as a mandate to design
  production custody hardening, an admin CLI, an HTTP transport, or multi-plugin runtime — all are
  enumerated Non-goals.
- **Strain check against Non-goals / Success criteria — none surfaced:**
  - The governance decision (P-0003 amendment) does **not** strain the "No production /
    multi-deployment key-custody hardening" Non-goal: the amendment governs the manifest *schema
    field*, not key custody; custody hardening stays deferred (D4 + R-0005-e).
  - Standing up `/health` (D7) does **not** strain the "No admin CLI" Non-goal: `/health` is a
    separate already-planned surface (Task 25), not the control-plane CLI (Task 24).
  - The minimal ceremony (D4) does **not** strain the custody Hard constraint: it honors R-0005-c/-d
    explicitly via the same-machine degeneracy resolution (private key out of any runtime-read dir).
  - The serve path (D8) honors the stdio-only / no-HTTP Non-goal (R-0010-a/-e).
- **Escalation status.** No `conflicts-with` edge fired, no novel cross-project precedent is set
  (the governance decision is canon application of the intake's stated criterion), and no locked
  artifact is invalidated. Nothing is escalated to Puck/PA from the constraint-walk. The one item the
  PA must rule on is the **shape** of the `{{P-ManifestContentHash}}` amendment at the Frame-exit gate
  — surfaced as a shape, not fiat-amended (§5).

## 8. Provenance

This is an **in-place amendment** to an existing architecture spec. The BOM chain stays anchored to
the product-tier upstreams (`docs/src/intent/mnemra-core.md` / `mnemra-core-frame.md`, **unchanged**).
The signing-to-runnable intake ([`docs/intent/signing-to-runnable.md`](signing-to-runnable.md)) and
this Frame ([`docs/intent/signing-to-runnable-frame.md`](signing-to-runnable-frame.md)) are the
**localized design records** for the amendment — the localized upstreams the Spec amendment draws
from.

- The product-tier `[audit_chain.intent]` / `[audit_chain.frame]` BOM blocks are **not** touched by
  this Frame.
- The BOM (`docs/specs/2026-06-30-signing-to-runnable.bom.toml`) already carries the locked
  `[audit_chain.intent]` block (intent ref + version `1c6820e`, locked 2026-06-30T16:45:19Z). It is
  **not** edited here. Puck performs the Frame-exit BOM lock (the `[audit_chain.frame]` block) after
  the maintainer checkpoint.

## 9. Consultations

- **none (maintainer).** Scope and the V0 ceremony approach were locked with the maintainer prior to
  this run (full runnable chain; minimal maintainer-local ceremony); Stage 2a elicitation collapsed
  (brownfield-extension). The `advisor()` reviewer pass that sharpened the steward-vs-owner threading
  on the governance decision, the new-R-ID-vs-amend-in-place slot split, the ATAM scenario set, and
  the `schema_version` anti-silent-fill is recorded as authoring-internal, not a maintainer
  consultation.

## Changelog

- **2026-06-30** — Frame authored (Stage 2b, brownfield-extension). Renders the maintainer-locked
  signing → `run()` design as canon-anchored directions (D1–D9), ATAM QA scenarios (QA-1..QA-6), and
  open R-ID / ADR-amendment slots (§5). Keystone anchor: `P-SecurityLayered` "each layer independently
  load-bearing" — manifest-signature verification and component-byte binding are distinct, both
  load-bearing controls; real signing leaves the second absent (`constraint-edges.md:42`,
  `architecture-values.md:31`). **Locked the one open governance item:** the content-hash field is
  routed to a **P-0003 amendment** (not a spec-local field) — durable / every-plugin **and** a
  manifest-schema change, both halves of the intake criterion resolving to amendment; the amendment
  *text* is the Stage-3 `{{P-ManifestContentHash}}` shape for PA review (steward-not-owner). Named the
  confirmed-next requirement id R-0021 for `{{R-ContentHashBinding}}`. No BOM `[audit_chain.frame]`
  edit (Puck locks at the gate); no maintainer consultation (collapsed elicitation). Status: draft —
  Frame-exit gate pending.
