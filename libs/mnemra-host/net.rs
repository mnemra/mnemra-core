//! Network-layer controls for the mnemra host.
//!
//! Provides outbound hostname allow-listing for embedding-call pathways
//! (R-0014-b). All outbound calls from the host MUST pass through the
//! allowlist before a connection is established.

pub mod hostname_allowlist;
