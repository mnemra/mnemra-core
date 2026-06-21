//! Host-fn ABI binding skeleton.
//!
//! Mirrors the WIT interfaces declared in `wit/host.wit` as Rust types and
//! dispatch functions.  Bodies are unimplemented stubs; behaviour is wired in
//! as later tasks add storage and runtime layers.
//!
//! # Dispatch wrapper seam (R-0012-e)
//!
//! Every host-fn invocation routes through [`DispatchWrapper::invoke`].  The
//! wrapper inspects the function's [`Stability`] annotation:
//!
//! - `Stability::Stable` — passes through unchanged.
//! - `Stability::Unstable` — emits a host-side `tracing` WARN event (R-0012-e:
//!   "@unstable emits a deprecation warning to the log") AND returns a
//!   [`DispatchWarning`] value, then passes through.  The *caller* owns the
//!   returned warning; the log fires regardless so nothing is silently swallowed.
//! - `Stability::Deprecated` — returns [`DispatchError::Deprecated`] without
//!   invoking the body.  A structured error, not a panic or a log line.
//!
//! This is the surface the reviewer will write the R-0012-e behavioural test
//! bodies against — see `Reviewer handoff` section in the completion report.

pub mod host_fns;

/// Stability annotation from the WIT model, mirrored on the Rust side so the
/// dispatch wrapper can branch without re-parsing WIT at runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum Stability {
    /// `@since(version = …)` — function is stable.
    Stable,
    /// `@unstable(feature = …)` — function is experimental; callers receive a
    /// warning value.
    Unstable { feature: &'static str },
    /// Function has been deprecated; calls return a structured error.
    Deprecated {
        since: &'static str,
        reason: &'static str,
    },
}

/// Warning emitted when an `@unstable` function is invoked.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchWarning {
    pub feature: &'static str,
    pub fn_name: &'static str,
}

/// Structured error returned when a deprecated function is invoked.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchError {
    /// The invoked function has been deprecated.
    Deprecated {
        since: &'static str,
        reason: &'static str,
        fn_name: &'static str,
    },
}

impl core::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DispatchError::Deprecated {
                since,
                reason,
                fn_name,
            } => {
                write!(f, "host-fn `{fn_name}` deprecated since {since}: {reason}")
            }
        }
    }
}

/// Dispatch outcome returned for every host-fn invocation.
///
/// On `Unstable`, the warning is returned alongside the result so the caller
/// can decide how to surface it (log, propagate, collect).  This avoids
/// coupling the ABI skeleton to a specific logging framework.
#[derive(Debug)]
pub struct DispatchOutcome<T> {
    pub value: T,
    /// Present when the function was annotated `@unstable`.
    pub warning: Option<DispatchWarning>,
}

/// Dispatch wrapper.  Stateless — all stability metadata is passed at call site
/// via the `stability` parameter so no runtime WIT parsing is needed.
pub struct DispatchWrapper;

impl DispatchWrapper {
    /// Invoke `f` through the stability dispatch layer.
    ///
    /// - Stable: calls `f`, wraps result in [`DispatchOutcome`] with no warning.
    /// - Unstable: calls `f`, wraps result with a [`DispatchWarning`].
    /// - Deprecated: returns [`DispatchError::Deprecated`] without calling `f`.
    pub fn invoke<T, F>(
        stability: &Stability,
        fn_name: &'static str,
        f: F,
    ) -> Result<DispatchOutcome<T>, DispatchError>
    where
        F: FnOnce() -> T,
    {
        match stability {
            Stability::Stable => Ok(DispatchOutcome {
                value: f(),
                warning: None,
            }),
            Stability::Unstable { feature } => {
                // R-0012-e: "@unstable emits a deprecation warning to the log."
                // Emit a host-side WARN event in addition to returning the
                // `DispatchWarning` value — the returned value lets the caller
                // forward/collect; the log fires unconditionally so unstable
                // usage is recorded even when the caller drops the warning.
                tracing::warn!(
                    feature,
                    fn_name,
                    "unstable host-fn invoked — feature is experimental and may change"
                );
                Ok(DispatchOutcome {
                    value: f(),
                    warning: Some(DispatchWarning { feature, fn_name }),
                })
            }
            Stability::Deprecated { since, reason } => Err(DispatchError::Deprecated {
                since,
                reason,
                fn_name,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_dispatch_passes_through() {
        let outcome = DispatchWrapper::invoke(&Stability::Stable, "test-fn", || 42u32)
            .expect("stable dispatch must succeed");
        assert_eq!(outcome.value, 42);
        assert!(outcome.warning.is_none());
    }

    #[test]
    fn unstable_dispatch_returns_warning() {
        let outcome = DispatchWrapper::invoke(
            &Stability::Unstable {
                feature: "test-feature",
            },
            "test-unstable-fn",
            || "result",
        )
        .expect("unstable dispatch must succeed");
        let warn = outcome
            .warning
            .expect("unstable dispatch must emit warning");
        assert_eq!(warn.feature, "test-feature");
        assert_eq!(warn.fn_name, "test-unstable-fn");
    }

    #[test]
    fn deprecated_dispatch_returns_structured_error() {
        let err = DispatchWrapper::invoke(
            &Stability::Deprecated {
                since: "0.0.1",
                reason: "replaced by new-fn",
            },
            "old-fn",
            || (),
        )
        .expect_err("deprecated dispatch must return error");
        match err {
            DispatchError::Deprecated {
                since,
                reason,
                fn_name,
            } => {
                assert_eq!(since, "0.0.1");
                assert_eq!(reason, "replaced by new-fn");
                assert_eq!(fn_name, "old-fn");
            }
        }
    }
}
