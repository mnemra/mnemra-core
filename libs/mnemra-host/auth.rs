//! Auth subsystem.
//!
//! Exposes the admin-token generation, hashing, constant-time verification,
//! rotation, and file-mode bootstrap path (R-0008, R-0009).
//!
//! # Module layout
//!
//! - `token` — token generation, hashing, verification, rotation (R-0008).
//! - `role` — binary `Role` enum (`Admin` / `ReadObserver`) (R-0009-a).
//! - `workspace_ctx` — `WorkspaceCtx` struct with private field + accessor
//!   (R-0006-b/c/f, R-0009-b).
//! - `resolve` — single production `WorkspaceCtx` construction site after
//!   token validation (R-0006-b).

pub mod permissions;
pub mod resolve;
pub mod role;
pub mod token;
pub mod workspace_ctx;
