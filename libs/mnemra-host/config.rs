//! Per-deployment configuration for the mnemra host.
//!
//! Provides the LLM-key configuration surface (R-0014-a) used by the
//! embedding-batch pathway (DF-embed-call). Configuration is loaded from
//! deploy-time files, never from compiled-in defaults.

pub mod llm_key;
