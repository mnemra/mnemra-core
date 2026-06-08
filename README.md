# mnemra-core

The runtime engine for [mnemra](https://github.com/mnemra) — a single host
binary that hosts WebAssembly Component Model plugins via [Wasmtime].

This is **V0.01** — an existence-test scaffold that proves the substrate
(Wasmtime + WIT + `wasm32-wasip2`) can host long-running, stateful, in-process
plugins with host-fn round-trips. It contains:

- `cmd/mnemra/` — the host binary
- `plugins/mnemra-echo/` — the first plugin (compiled to a CM component)
- `wit/echo.wit` — the host-plugin contract for V0.01

## Layout

```
mnemra-core/
├── Cargo.toml              # workspace root
├── rust-toolchain.toml     # pins wasm32-wasip2 target
├── justfile                # build / run / check recipes
├── wit/                    # WIT package source
│   └── echo.wit            # mnemra:host@0.0.1 (interfaces: echo)
├── cmd/
│   └── mnemra/             # host binary crate
└── plugins/
    └── mnemra-echo/        # first plugin crate (cdylib → wasm32-wasip2)
```

The flat layout (`cmd/`, `plugins/`, no `src/` subdirs, library entrypoints
named after the crate) is the convention these workspaces use.

## Prerequisites

- Rust 1.85+ (the toolchain file pins this)
- The `wasm32-wasip2` target — installed automatically by the toolchain file,
  or manually via `rustup target add wasm32-wasip2`
- [`wasm-tools`] for inspecting the compiled component (optional):
  `cargo install wasm-tools`

[Wasmtime]: https://wasmtime.dev
[`wasm-tools`]: https://github.com/bytecodealliance/wasm-tools

## Build

Build the plugin (a CM component, not a plain wasm module):

```sh
cargo build --release -p mnemra-echo --target wasm32-wasip2
```

Build the host:

```sh
cargo build --release -p mnemra
```

Or use the `justfile`:

```sh
just plugin   # build the plugin
just release  # build the host
just run      # build plugin + run host
```

## Run the V0.01 spike

```sh
cargo run --release -p mnemra
```

Expected output:

```
Loading plugin from target/wasm32-wasip2/release/mnemra_echo.wasm
Plugin instantiated.
run("hello") -> "echo: hello | counter: 1"
run("world") -> "echo: world | counter: 2"
Spike PASS - round-trip works, counter persisted across invocations.
```

The host instantiates the plugin component once via
`Linker::instantiate_async`, then calls the plugin's exported `run`
function twice. Each call invokes two host functions through the WIT
contract — `echo.echo(s)` (round-trip) and `echo.increment-counter()`
(host-side state). The counter advancing from 1 to 2 across calls
proves plugin state persists between invocations on the same component
instance.

## Inspect the plugin

To verify the build artifact is a valid CM component (not a core
module):

```sh
wasm-tools component wit target/wasm32-wasip2/release/mnemra_echo.wasm
```

This should print the WIT package and world that the component
exports.

## License

Apache-2.0 — see [`LICENSE`](LICENSE).
