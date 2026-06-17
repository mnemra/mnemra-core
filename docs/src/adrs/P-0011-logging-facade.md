---
title: "P-0011: Logging Facade (`tracing`, facade-from-binary)"
summary: "The logging facade is the `tracing` + `tracing-subscriber` crate pair, wired facade-from-binary: the binary crate owns the global subscriber (JSON → stdout + EnvFilter); library crates use `tracing` macros only and carry zero subscriber knowledge. Foundation only, impl-crate-only; the full emission layer (semantics, per-verb metrics, OTel export, redaction) is deferred to Task 25 / R-0004."
primary-audience: agent
---

---
status: "accepted"
date: "2026-06-17"
decision-makers: ["the maintainer"]
consulted: ["the orchestrator"]
informed: []
supersedes: null
superseded_by: null
---

# P-0011: Logging Facade (`tracing`, facade-from-binary)

## Status

`accepted`

The decision recorded here was made by the maintainer and implemented + merged in commit `b2830da` ("build(mnemra): tracing logging scaffold (facade-from-binary)") this same increment. This ADR records the merged decision and its crate-topology contract; it is `accepted` on landing rather than `proposed` because the choice is already in the tree, not awaiting a review gate. It does not override a [`DEFAULTS.md`](DEFAULTS.md) baseline entry.

This is a **dependency-selection + crate-topology** decision (which crate provides the logging plumbing, and where in the crate graph the subscriber lives). It is **not** an observability-*shape* decision — it locks no metric, no event schema, no telemetry sink, and no emission semantic. The observability *shape* (emission semantics, per-verb metrics, sink/storage) was re-altituded out of the project-ADR layer to the [observability baseline](../architecture/overview.md#observability) by the 2026-06-09 E1 disposition and is deferred here to Task 25 / R-0004 (see [Scope — foundation only, impl-crate-only](#scope--foundation-only-impl-crate-only) below). The home rationale — why this is an ADR while observability shape is not — is recorded under [More Information](#more-information).

## Context and Problem Statement

mnemra-core ships observability from `0.1.0` per the workspace-wide instrument-first principle (`P-InstrumentBefore`; Observability value: "every production surface ships instrumented … in place at first user-touch, not added after the first incident"). Before any emission semantics can be designed, the foundation must exist: a logging plumbing crate, a single place where the global subscriber is installed, and a discipline for *where* in the crate graph that subscriber lives so that library crates do not each reach for their own logging implementation.

Two structural questions had to be settled to lay that foundation:

1. **Which crate provides the logging facade?** The library code was emitting diagnostics via ad-hoc `eprintln!` to stderr — unstructured, unfilterable, and unroutable to a real sink.
2. **Where does the subscriber live, and what do library crates depend on?** A logging facade can be wired so that every crate configures its own subscriber, or so that exactly one crate (the binary) installs the global subscriber and every library crate emits through macros against the global dispatcher. The second is a deliberate crate-topology lock; the first invites duplicate or conflicting subscriber init and couples libraries to a concrete logging implementation.

This ADR settles both. It is foundation only — the emission layer that rides on top is explicitly out of scope (see below).

## Decision Drivers

- **Instrument-first foundation.** Observability is a `0.1.0` deliverable, not a later add-on (`P-InstrumentBefore`; Observability value). The plumbing has to be in place before the emission layer can be built on it; the foundation is the prerequisite for the instrument-first commitment, not the commitment itself.
- **Lock the contract, vary the implementation.** The seam between "library code emits a diagnostic" and "a concrete subscriber formats and routes it" is intrinsic to the logging layer's identity and should be locked even though the routing details are deferred (`P-LockContract`: lock what is intrinsic, even when not fully exposed until later). The `tracing` macros are that contract; the subscriber is the implementation behind it.
- **Library crates carry no implementation knowledge.** A library that installs or configures a subscriber cannot be embedded cleanly (double-init, conflicting filters, format coupling). Libraries must depend on the facade macros only — never on a subscriber implementation (`P-PerRepoFirst` decoupling; the standard `tracing` ecosystem idiom).
- **Ecosystem-standard, license-clean plumbing.** The facade should be the de-facto Rust standard for structured, leveled, span-aware logging so the emission layer (OTel export, JSON formatting) composes from existing layers rather than bespoke code, and so the dependency clears the license gate.
- **Smallest mature foundation now; emission semantics deferred on a named tripwire.** Lay only the plumbing and the topology at V0; defer the emission layer to its own work item rather than over-specifying it here (`P-Defer` / DF1; Simplicity).

## Considered Options

The plumbing-crate choice and the topology were settled by the maintainer; they are recorded here per `P-PreserveDecisionSpace`, not re-derived.

### Logging plumbing crate

1. **`tracing` + `tracing-subscriber` (chosen).** The de-facto Rust standard for structured, span-aware, leveled diagnostics; subscriber composed from layers (JSON formatting + `EnvFilter`); OTel export available as an additional layer when the emission layer lands. Green-tier (MIT).
2. **`log` + `env_logger`.** The older facade/impl pair. Leveled and filterable but record-oriented, not span-aware; weaker structured-output and OTel-bridge story.
3. **Bespoke `eprintln!`/custom macros.** No structure, no level filtering, no routing, no sink path; the status quo this decision replaces.

### Subscriber topology

1. **Facade-from-binary (chosen).** The binary crate (`cmd/mnemra`) installs the one global subscriber; library crates depend on `tracing` (macros) only and never on `tracing-subscriber`.
2. **Each crate configures its own subscriber.** Every crate (including libraries) sets up logging. Risks duplicate/conflicting global-subscriber init and couples libraries to a concrete implementation.

## Decision Outcome

**Plumbing crate: Option 1** — `tracing` + `tracing-subscriber` are the logging facade (merged as `tracing@0.1.44` + `tracing-subscriber@0.3.23`, both MIT / Green-tier). The standard Rust facade/subscriber split gives structured, span-aware, level-filtered diagnostics with a composable subscriber and a first-class OTel bridge for the deferred emission layer.

**Topology: Option 1 — facade-from-binary (the maintainer's term).** Concretely:

- **The binary crate `cmd/mnemra` owns the global subscriber.** It installs, as the first action in `main`, a `tracing-subscriber` registry with a JSON formatting layer writing to **stdout** and an `EnvFilter` (reads `MNEMRA_LOG`, then `RUST_LOG`, then falls back to `info`). The install uses `try_init()` so a repeated call (e.g. tests sharing a process) is a no-op rather than a panic. (`cmd/mnemra/logging.rs`.)
- **Library crates use `tracing` macros ONLY.** `libs/mnemra-host` depends on `tracing` (the macros) and performs **no** library-level logging init. The macros **are** the facade over the global dispatcher; the library has zero knowledge of which subscriber is installed, how it formats, or where it routes.
- **`tracing-subscriber` is NOT a library dependency.** Only the binary depends on `tracing-subscriber`. This is the structural invariant that keeps the implementation knowledge in the binary and out of every library crate.

The macros are the locked **contract**; the subscriber is the **implementation**, owned solely by the binary — the logging-layer analog of "lock the contract, vary the implementation" (`P-LockContract`). A second binary (a different host, a test harness) installs its own subscriber against the same unchanged library macro call sites.

### Scope — foundation only, impl-crate-only

What this ADR locks is **the foundation and the topology, nothing on top of it.** What landed in `b2830da` is impl-crate-only scaffolding: the global subscriber install (`cmd/mnemra/logging.rs`), the `tracing` dependency added to the binary and to `libs/mnemra-host`, the two prior `eprintln!` diagnostic sites in the libraries swapped to `tracing` macros (`libs/mnemra-host/storage/postgres/engine.rs`, `libs/mnemra-host/projection/worker.rs`), and a few judicious `debug!` calls added at seams. Those call-site swaps are **illustrative of the foundation, not the subject of this record** — this ADR is about the facade choice and the crate topology, not about which specific lines log today.

**Explicitly OUT OF SCOPE here (deferred — see the named tripwire below):** the full emission layer — emission semantics, per-verb metrics, event schemas, OTel / OTLP export wiring, telemetry redaction, and host-function-body logging. None of those is specified by this ADR. The observability *shape* those concern lives in the [observability baseline](../architecture/overview.md#observability) (a theory-trait baseline, not an ADR), and the V0 work that builds the emission layer on this foundation is its own work item.

> **Deferral tripwire (named instrument, `P-Defer` / DF1).** The full emission layer — emission semantics, per-verb metrics, events, OTel/OTLP export, redaction, host-fn-body logging — is deferred to **Task 25 / R-0004**. That work item is the named instrument that fires the deferred decisions back; it builds the emission layer on the foundation locked here. Until it lands, this ADR's scope is the plumbing + topology only, and the emission surface is the documented-invariant generation baseline in the overview.

### Consequences

**Good:**
- The instrument-first foundation exists from `0.1.0`: structured, level-filtered, stdout-routable diagnostics, with the emission layer able to build on top rather than starting from `eprintln!`.
- Library crates are cleanly embeddable: no double-init, no subscriber coupling, no format/filter ownership — they emit through macros and inherit whatever the host binary installs.
- The OTel/OTLP export path for the deferred emission layer composes as an additional `tracing-subscriber` layer in the binary, with no change to any library call site.
- The contract (macros) is locked while the implementation (subscriber) stays free to vary per binary, satisfying `P-LockContract` without over-committing the deferred emission details.

**Bad / Trade-offs:**
- A library crate that genuinely needs to *configure* logging (rather than emit) cannot — by design. The topology forbids library-level subscriber init. Accepted: that capability belongs to the binary, and no V0 library needs it.
- The emission semantics are not locked here, so this ADR alone does not satisfy any observability quality-attribute scenario; it is the prerequisite foundation, and the scenarios are met only once Task 25 / R-0004 builds the emission layer on it. This is the intended altitude split, recorded so the gap is named rather than silent.

## Pros and Cons of the Options

### Plumbing — `tracing` + `tracing-subscriber` (chosen)

- Pro: De-facto Rust standard for structured, span-aware, leveled diagnostics; the emission layer composes from existing subscriber layers (JSON, `EnvFilter`, OTel bridge) rather than bespoke code.
- Pro: Clean facade/implementation split — the macros are usable by libraries with zero subscriber coupling, which is exactly what the topology requires.
- Pro: Green-tier (MIT), clears the dependency license gate for a redistributed Apache-2.0 product.
- Con: `tracing-subscriber` carries more surface than a minimal logger; mitigated by confining it to the binary.

### Plumbing — `log` + `env_logger`

- Con: Record-oriented, not span-aware; the structured-context and OTel-bridge story is weaker, which the deferred emission layer (per-verb metrics, OTel export) leans on.

### Plumbing — bespoke `eprintln!` / custom macros

- Con: No structure, no level filtering, no routing, no sink path; this is the status quo the decision replaces. Cannot satisfy the instrument-first foundation.

### Topology — facade-from-binary (chosen)

- Pro: Exactly one global subscriber, installed in one place; no duplicate or conflicting init.
- Pro: Libraries depend on macros only and are cleanly embeddable in any host binary.
- Con: Libraries cannot self-configure logging; accepted as the intended constraint.

### Topology — each crate configures its own subscriber

- Con: Duplicate/conflicting global-subscriber init across crates; couples every library to a concrete subscriber implementation, breaking clean embedding.

## More Information

**Home rationale — why this is a project ADR (and observability *shape* is not).** The 2026-06-09 E1 disposition ([P-0010](P-0010-storage-substrate-engine.md) D8; [ADR README](README.md) "Current ADRs") re-altituded observability **shape** — emission semantics, the metric/event/health generation decisions, and the storage/sink question — out of the project-ADR layer to the [observability baseline](../architecture/overview.md#observability), and `deprecated` the observability-shape ADR [P-0004](P-0004-observability-shape.md) (no successor ADR). That disposition's scope is the observability *shape*. This decision is categorically different: it is a **dependency-selection + crate-topology** decision (which crate provides the plumbing; where the subscriber lives), structurally the same kind of decision as [P-0010](P-0010-storage-substrate-engine.md)'s concrete engine choice behind a swappable trait (D5: "the storage surface is an engine-agnostic trait; the implementation varies behind it"). The ADR-vs-design-note criterion ([ADR README](README.md); agent-first: a reversal that forces downstream agent rework or external-consumer reference) is met independently: reversing the facade or the topology forces every library-crate macro site and the binary's subscriber install to change. So the facade decision belongs in the ADR layer; the observability *shape* it is the plumbing for does not.

**Relationship to the observability baseline.** This ADR is the *plumbing* foundation; the [observability baseline](../architecture/overview.md#observability) is the *generation* shape that rides on it (emitted metrics/events/logs, the stdout + OTel emission surface, the health endpoint). The baseline's "generation mechanism … is a documented invariant on the build" until the chassis lands; this ADR records the concrete crate choice and topology underneath that invariant.

**References (paths confirmed to resolve in-repo):**
- Subscriber install site: `cmd/mnemra/logging.rs` (binary owns the global subscriber).
- Binary dependency: `cmd/mnemra/Cargo.toml` (`tracing` + `tracing-subscriber`).
- Library dependency: `libs/mnemra-host/Cargo.toml` (`tracing` only; no `tracing-subscriber`).
- Illustrative library macro sites: `libs/mnemra-host/storage/postgres/engine.rs`, `libs/mnemra-host/projection/worker.rs`.
- Merged in commit `b2830da` ("build(mnemra): tracing logging scaffold (facade-from-binary)").

**Canon anchors:**
- `P-InstrumentBefore` and the Observability value (workspace `architecture-principles.md` / `architecture-values.md`): observability ships from `0.1.0`; this is the foundation that makes that achievable.
- `P-LockContract`: lock what is intrinsic (the macro facade) even when the implementation detail (subscriber routing) is deferred.
- `P-Defer` / DF1: the emission layer is deferred behind a named tripwire (Task 25 / R-0004).
- `P-PreserveDecisionSpace`: the considered options are recorded as the surfaced alternatives the lock chose among.

**Follow-up:** the full emission layer is Task 25 / R-0004 (per the deferral tripwire above).
