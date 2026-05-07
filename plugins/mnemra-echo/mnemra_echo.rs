//! mnemra-echo — first `core: true` plugin.
//!
//! Demonstrates the round-trip and state-persistence shape that mnemra
//! plugins use: imports a host interface, calls into it, returns a
//! formatted string. The host's V0.01 spike asserts the counter
//! advances across two `run` calls on the same component instance.

// Generates the guest-side bindings for `world plugin` defined in
// `wit/echo.wit`. `export!` below registers our struct as the world's
// `run` exporter.
wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

struct EchoPlugin;

impl Guest for EchoPlugin {
    fn run(input: String) -> String {
        // Round-trip: ask the host to echo the input.
        let echoed = mnemra::host::echo::echo(&input);
        // State: ask the host for its current counter (it auto-increments).
        let counter = mnemra::host::echo::increment_counter();
        format!("{echoed} | counter: {counter}")
    }
}

export!(EchoPlugin);
