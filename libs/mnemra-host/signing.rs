//! Plugin signing-chain verification module.
//!
//! Exposes the synchronous verification path (`verify`) and the embedded root
//! verification material (`root_material`). Entry point for R-0005 requirements.
//!
//! # Trust model
//!
//! The only trusted key is the one embedded at build time in `root_material::ROOT`.
//! No runtime key-fetch path exists (R-0005-d). Every plugin is verified against
//! that root before any instance is created (R-0005-a).

pub mod build_gate;
pub mod root_material;
pub mod verify;
