//! `sign-ceremony` CLI — thin clap shim over the `sign_ceremony` lib.
//!
//! Subcommands:
//! - `keygen <key-out-path>` — generate a keypair, write the 32-byte private
//!   seed mode-600, print the public key hex.
//! - `sign <key-path> <wasm-path> <manifest-path>` — run the signing ceremony.
//!
//! All logic lives in the lib (`ceremony.rs`) so the round-trip tests in
//! `tests/` can drive it and check output against the real mnemra-host verifier.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use sign_ceremony::{cmd_keygen, cmd_sign};

/// mnemra root signing-ceremony producer (keygen + manifest signing).
#[derive(Parser)]
#[command(name = "sign-ceremony", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a fresh Ed25519 root keypair: write the 32-byte private seed
    /// (mode 600) and print the public key hex.
    Keygen {
        /// Path to write the private key seed to (must not already exist).
        key_out: PathBuf,
    },
    /// Sign a manifest: hash the committed wasm, embed [component], sign the
    /// body, and populate [signature] in place.
    Sign {
        /// Path to the custody private key (32-byte Ed25519 seed).
        key: PathBuf,
        /// Path to the committed signed .wasm artifact to hash.
        wasm: PathBuf,
        /// Path to the manifest to sign (rewritten in place).
        manifest: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Keygen { key_out } => cmd_keygen(&key_out),
        Command::Sign {
            key,
            wasm,
            manifest,
        } => cmd_sign(&key, &wasm, &manifest),
    };
    if let Err(e) = result {
        eprintln!("sign-ceremony: error: {e}");
        std::process::exit(1);
    }
}
