//! mnemra — thin binary entry point.
//!
//! Delegates all startup logic to `mnemra_host::run`. The spike existence-test
//! assertions that previously lived here are removed; the plugin-runtime test
//! surface lands in Tasks 19/20.

fn main() {
    if let Err(e) = mnemra_host::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
