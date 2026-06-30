# mnemra-core

mnemra-core is the core context layer for Mnemra, an MCP-native system for persistent, structured memory that integrates with AI development workflows. Model Context Protocol (MCP) is the agent-facing interface; the substrate underneath it is built to be queried by coding agents and the people working alongside them.

Teams that run coding agents (Claude Code, Cursor, Copilot, and others) need those agents to carry structured, queryable context across sessions, covering the codebase, the decisions, the tickets, the docs, and prior agent runs. Today that context is hand-loaded into each session and rebuilt the next time, so it drifts and does not scale. An agent preparing to act on a task sits inside a graph of a parent spec, related decisions, sibling tasks, prior reviews, and recent commits, and that graph is reconstructed by hand every time. mnemra-core's job is to make it a durable, agent-addressable substrate instead.

## Architecture

What follows describes the locked V0 design. For what is actually wired today versus designed, see [Status and roadmap](#status-and-roadmap).

mnemra-core is a single-process server binary. It ships the storage substrate, the host, the plugin runtime, and the builtin components in one process, with self-hosted-first deployment as the default posture.

- **Storage.** Embedded PostgreSQL with the `pgvector` extension, bundled into the binary and managed by the build, behind an engine-agnostic `Storage` trait. PostgreSQL is the only implementation; the trait keeps the contract swappable. Recursive CTEs, JSONB, native full-text search, and `pgvector` cover the V0 query surface, so no extra extensions are required to start.

- **Plugin runtime.** Plugins are WebAssembly Component Model modules hosted in-process on Wasmtime. Plugin core logic is IO-free: every side effect (storage, network, clock, secrets) routes through host functions defined in WIT. Plugins are leaves with no sideways linkage, and each instance runs under Wasmtime fuel and epoch limits, so a runaway plugin is killed and replaced from the pool rather than taking the host down with it.

- **MCP server.** One MCP server over stdio, built on the official Rust SDK (`rmcp`) and conforming to the MCP 2025-06-18 specification. Plugin verbs are namespaced under a single catalog rather than each plugin running as its own server. The server is the auth and dispatch boundary: it checks the request token, constructs the workspace context once, and applies a per-verb capability check before dispatching.

- **Content model.** Artifacts are the unit of content. The host exposes a fixed CRUD surface (create, get, list, update, delete) over the WIT contract; a plugin implements the typed content methods and calls back into the host to persist and read. Storage is content-first with CQRS read-side projections that rebuild reactively from content. A single artifact lives in one row: JSONB frontmatter, body, and system-generated fields.

- **Tenancy.** Every artifact table carries `workspace_id`, NOT NULL from the first migration, so multi-tenancy is structural even though a solo deployment collapses to a single `default` workspace. The host derives a typed workspace context from the validated token and threads it through every host function, which makes a read path that forgets to scope by workspace impossible to write. Row-Level Security column shape ships now; policy enforcement is a later addition on top of the same key, with no substrate migration required.

- **Builtins and plugins.** Workspaces, users, agents, authentication, sessions, per-plugin permissions, and projects are builtin to the host, not plugins (plugins are scoped per project, so a project cannot itself be a plugin without a bootstrap cycle). Additional capability families arrive either as further builtins or as signed `core: true` plugins that the host cannot uninstall.

- **Authentication.** mnemra-core is a Resource Server only. A static admin token, filesystem-stored at mode 600, bootstraps first-run and solo deployments. Per-deployment OpenID Connect, advertised through RFC 9728 protected-resource-metadata, is the path for federated authorization.

- **Observability.** The host emits its own telemetry (structured logs to stdout, OpenTelemetry metrics and events, and a health endpoint) and does not own where that telemetry lands. Generation is separated from storage, so the binary ships instrumented without bundling an observability store; the operator chooses the sink.

- **Boundaries.** The system never hosts a language model. Embeddings call out to an external provider over a per-deployment configured key, and an immutable image or appliance is a valid way to package the single binary.

## Status and roadmap

mnemra-core is pre-1.0 and under active development. The agent-facing API is not yet stable.

The checklist below is the project's living status. A box is checked only when the item is wired and working, not when it is merely designed, and it gets ticked as work lands. Read the boxes accordingly.

Substrate and host core:

- [x] Embedded PostgreSQL substrate with schema bootstrap (`mnemra init`)
- [x] WIT host-function ABI and the WASM Component Model plugin runtime on Wasmtime (fuel and epoch limits, kill-and-replace recovery)
- [x] Host startup: plugin pool population and MCP server wiring
- [x] `rmcp`-based stdio MCP server handler (MCP 2025-06-18): token check, single workspace-context construction, per-verb capability check
- [x] PostgreSQL persistence
- [x] Walking skeleton end to end: an artifact `create` verb round-trips through a plugin to PostgreSQL and back, exercised by the integration test suite
- [x] The remaining agent-facing artifact CRUD verbs (get, list, update, delete), each round-tripping through a plugin to PostgreSQL and exercised by the integration test suite
- [x] Artifact-list paging (keyset cursor over a workspace-scoped page)

In flight:

- [ ] Long-running stdio MCP server launched from the `mnemra` binary

Toward the V0 milestone (the public API is defined and a full reference workload runs on mnemra-core with no fallback to prior tooling):

- [ ] Additional artifact types and capability families on the substrate
- [ ] One-shot migration of an existing content corpus

Post-V0 direction (not yet designed in detail):

- [ ] One-call cross-session context retrieval for a given artifact (the core product promise)
- [ ] Ongoing ingest beyond one-shot migration (watchers, scheduled polls, webhooks)
- [ ] Full-text and vector search activation
- [ ] First-class graph edges and traversal

## Repository layout

```
mnemra-core/
├── Cargo.toml            # workspace root
├── rust-toolchain.toml   # adds the wasm32-wasip2 target
├── justfile              # build / run / check / verify recipes
├── deny.toml             # dependency and license policy (cargo-deny)
├── wit/                  # WIT package source (the host-plugin contract)
│   ├── host.wit          # mnemra:host package (artifact, metrics, log, event, projection, sampling, secrets)
│   └── echo.wit          # the echo world used by the first plugin
├── cmd/
│   └── mnemra/           # host binary crate (thin entry point + subcommand dispatch)
├── libs/
│   └── mnemra-host/      # host runtime library (storage, schema, MCP, plugin runtime, auth, signing)
├── plugins/
│   └── mnemra-echo/      # first plugin crate (compiled to a wasm32-wasip2 component)
└── docs/                 # mdBook documentation site (architecture, ADRs, glossary)
```

The flat layout (`cmd/`, `libs/`, `plugins/`, no `src/` subdirectories, entry files named after the crate) is the convention this workspace uses.

## Building and running

### Prerequisites

- A recent stable Rust toolchain. The workspace is on the 2024 edition, which needs Rust 1.85 or newer.
- The `wasm32-wasip2` target. The `rust-toolchain.toml` adds it automatically; otherwise `rustup target add wasm32-wasip2`.
- [`just`](https://github.com/casey/just) for the recipes.
- No external PostgreSQL is required. The embedded engine is downloaded and managed by the build.
- Optional: [`wasm-tools`](https://github.com/bytecodealliance/wasm-tools) to inspect a compiled component.

### Build

```sh
just plugin    # build the echo plugin as a wasm32-wasip2 component
just release   # build the host binary
just check     # fmt, clippy, host + plugin tests, and docs gates
```

### Bootstrap a deployment

```sh
mnemra init
```

`mnemra init` starts the embedded PostgreSQL engine, enables `pgvector`, creates the substrate tables and indexes, creates the `default` workspace and the least-privilege database roles, and confirms the bootstrap is healthy before returning.

### Run the agent-facing server

The agent-facing entry point is the stdio MCP server. Launching the long-running server from the `mnemra` binary is still being wired; the bare binary does not yet start a live stdio server. The server handler and a content round-trip through a plugin are already exercised end to end by the integration test suite, which is the path that proves the wiring today:

```sh
just verify-test    # or: cargo test
```

## Documentation

Developer documentation is an mdBook site under [`docs/`](docs/). The architecture decision records under [`docs/src/adrs/`](docs/src/adrs/) carry the locked decisions, including the storage substrate and engine, the plugin runtime and MCP SDK choice, the plugin invocation model, tenant enforcement, and the admin token shape. The architecture overview under `docs/src/architecture/` holds the constraint inventory and the data-flow diagram, and `docs/src/glossary.md` defines the vocabulary the docs use.

## License

Apache-2.0. See [`LICENSE`](LICENSE).

---

mnemra-core is early and moving. The ADRs are the best place to understand why the design is shaped the way it is, and the roadmap above is the honest read on where it is today.
