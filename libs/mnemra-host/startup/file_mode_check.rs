//! Startup file-permission check.
//!
//! # R-0005-f / R-0014-d
//!
//! "On host startup, the system SHALL check that the admin-token file and the
//! signing-verification-material file are both mode 600 and not world-readable;
//! if either check fails, the host SHALL refuse to start." (R-0005-f)
//!
//! "The LLM-API-key SHALL be stored in a file at mode 600; the startup
//! file-mode invariant check SHALL cover the LLM-key file as well." (R-0014-d)
//!
//! This module provides the synchronous `check()` function called during host
//! startup, before any plugin is loaded. A non-Ok return MUST cause the host
//! to exit without loading any plugin.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Returned when a required file does not have exactly mode 600.
#[derive(Debug)]
pub struct FileModeError {
    /// The path that failed the check.
    pub path: PathBuf,
    /// The observed Unix permission bits (lower 12 bits of `st_mode`).
    pub actual_mode: u32,
    /// The required Unix permission bits (always `0o600`).
    pub required_mode: u32,
}

impl std::fmt::Display for FileModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "file mode check failed for {}: required mode {:#o}, got {:#o}",
            self.path.display(),
            self.required_mode,
            self.actual_mode,
        )
    }
}

impl std::error::Error for FileModeError {}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Check that ALL THREE secret files are mode 600.
///
/// Covers:
///   1. The admin-token file (`token_path`) — R-0005-f
///   2. The signing-material file (`signing_material_path`) — R-0005-f
///   3. The LLM-key file (`llm_key_path`) — R-0014-d
///
/// Returns `Err(FileModeError)` if ANY file:
///   - has any bit set in the world-permission mask (`0o007`), OR
///   - has a mode that is not exactly `0o600`.
///
/// Note: `PermissionsExt::mode()` returns the full `st_mode` word including
/// file-type bits (e.g. a regular file at mode 600 reads as `0o100600`).
/// This function masks to the lower 12 bits (`& 0o7777`) before comparing.
///
/// UID/GID ownership is NOT checked at V0 (advisory only per the spec).
///
/// Called on host startup before any plugin is loaded or any embedding call
/// is made (R-0005-f, R-0014-d). If this returns `Err`, the host MUST refuse
/// to start.
///
/// There is ONE `check()` — no partial 2-arg form remains (SF2: complete
/// mediation over all secret files).
pub fn check(
    token_path: &Path,
    signing_material_path: &Path,
    llm_key_path: &Path,
) -> Result<(), FileModeError> {
    check_single(token_path)?;
    check_single(signing_material_path)?;
    check_single(llm_key_path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Check that a single file is exactly mode 600.
fn check_single(path: &Path) -> Result<(), FileModeError> {
    let metadata = std::fs::metadata(path).map_err(|_| FileModeError {
        path: path.to_owned(),
        actual_mode: 0,
        required_mode: 0o600,
    })?;

    // Mask to lower 12 bits to strip file-type bits from st_mode.
    let mode = metadata.permissions().mode() & 0o7777;

    if mode != 0o600 {
        return Err(FileModeError {
            path: path.to_owned(),
            actual_mode: mode,
            required_mode: 0o600,
        });
    }

    Ok(())
}
