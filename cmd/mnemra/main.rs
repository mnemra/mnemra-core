//! mnemra — thin binary entry point.
//!
//! Dispatches subcommands to `mnemra_host` via the `cmd/` module tree.
//! Subcommand dispatch is hand-rolled: `mnemra init` is the only verb at V0,
//! so `clap` is not added (avoids ~500ms compile cost; see `cmd/init.rs`).

mod cmd;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let result = match args.get(1).map(String::as_str) {
        Some("init") => cmd::init::run().await,
        Some(unknown) => {
            eprintln!("error: unknown subcommand '{unknown}'");
            eprintln!("usage: mnemra init");
            std::process::exit(1);
        }
        None => {
            // No subcommand: delegate to the legacy `mnemra_host::run` entry
            // (scaffolding anchor for Tasks 23/25).
            mnemra_host::run().map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                // run() returns Box<dyn Error> (not Send+Sync). Wrap for uniformity.
                e.to_string().into()
            })
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
