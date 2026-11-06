//! Bounded, versioned replay of the reference rights state machine.
//!
//! Replay reconstructs accounting only: it never dispatches an external effect
//! or verifies a remote receipt. Run/policy identifiers and recovery anchors must
//! come from the caller's trusted context. CRC-32 detects accidental damage; it
//! is NOT authentication, a signature, or protection against a malicious writer.
#![forbid(unsafe_code)]

#[cfg(unix)]
pub mod evaluated_andon;
#[cfg(unix)]
pub mod file;

include!("journal_core.rs");
