//! Built-in subsystem bootstraps.
//!
//! Builtins are subsystems that initialize during `mnemra init`:
//! - `authentication`: admin-token bootstrap path and RFC 9728 config surface.

pub mod authentication;
pub mod permissions;
