//! Independent verification of the bounded FARJ rights-journal profile.
//!
//! No journal, policy, authority, or broker implementation is imported here.
//! Conclusions concern supplied bytes and independently selected context. CRCs
//! and exact prefix agreement do not authenticate either input, establish
//! freshness, or prove that an external effect occurred.

mod ledger;
mod stream;
mod wire;

use std::fmt;
use std::io::Read;

/// This executable profile bounds configured capacity, not just current length.
pub const MAX_INPUT_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_FRAME_BYTES: u32 = 1_048_576;
pub const MAX_RECORDS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_frame_bytes: u32,
    pub max_records: u64,
    pub max_total_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryReserve {
    pub records: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedContext {
    pub run: [u8; 32],
    pub policy: [u8; 32],
    pub total_units: u64,
    pub limits: Limits,
    pub recovery: Option<RecoveryReserve>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub sequence: u64,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accounting {
    pub total_units: u64,
    pub available_units: u64,
    pub spent_units: u64,
    pub reserved_units: u64,
    pub dispatched_units: u64,
    pub unknown_units: u64,
    pub epoch: u64,
    pub incident_count: u64,
    pub reservations: u64,
    pub reserved_count: u64,
    pub dispatched_count: u64,
    pub unknown_count: u64,
    pub committed_count: u64,
    pub aborted_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssuranceStatus {
    Known,
    Unknown,
}

impl AssuranceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Known => "Known",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assurance {
    pub property: &'static str,
    pub status: AssuranceStatus,
    pub explanation: &'static str,
}

const ASSURANCES: &[Assurance] = &[
    Assurance {
        property: "canonical_framing",
        status: AssuranceStatus::Known,
        explanation: "Every supplied journal byte belongs to the selected bounded FARJ encoding; every header and record CRC was checked.",
    },
    Assurance {
        property: "context_binding",
        status: AssuranceStatus::Known,
        explanation: "The header matches the independently supplied run, policy, total and selected format. Selected lifetime limits and recovery capacity were enforced.",
    },
    Assurance {
        property: "sequence_continuity",
        status: AssuranceStatus::Known,
        explanation: "The entire supplied history has consecutive sequence numbers and matching predecessor CRCs under the journal-local format.",
    },
    Assurance {
        property: "exact_prefix_agreement",
        status: AssuranceStatus::Known,
        explanation: "The supplied anchor is a complete canonical prefix and every prefix byte equals the journal. Its origin and freshness are not established.",
    },
    Assurance {
        property: "rights_accounting",
        status: AssuranceStatus::Known,
        explanation: "An independent bounded reducer checked reservation uniqueness, exact dispatch effects, epochs, settlement transitions, persistent unknown liabilities and unit conservation.",
    },
    Assurance {
        property: "authentication",
        status: AssuranceStatus::Unknown,
        explanation: "FARJ CRCs are not signatures; this profile authenticates neither the writer nor the anchor supplier.",
    },
    Assurance {
        property: "latest_acknowledged_history",
        status: AssuranceStatus::Unknown,
        explanation: "The supplied prefix cannot prove it is the latest acknowledged history. Joint rollback of journal and anchor remains undetectable.",
    },
    Assurance {
        property: "external_occurrence",
        status: AssuranceStatus::Unknown,
        explanation: "Dispatch and reconciliation records are journal inputs, not independently verified remote receipts or proof of an external effect.",
    },
    Assurance {
        property: "full_permit_law",
        status: AssuranceStatus::Unknown,
        explanation: "This journal lacks the authenticated decision closure, evidence, congress and authority material needed to verify the complete permit law.",
    },
    Assurance {
        property: "capture_completeness",
        status: AssuranceStatus::Unknown,
        explanation: "A valid supplied suffix does not prove that all actual effects or later acknowledged records were captured.",
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationReport {
    pub format_version: u8,
    pub prefix_head: Head,
    pub prefix_bytes: u64,
    pub final_head: Head,
    pub final_bytes: u64,
    pub accounting: Accounting,
}

impl VerificationReport {
    pub const fn assurances(&self) -> &'static [Assurance] {
        ASSURANCES
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Context,
    Journal,
    Anchor,
}

impl Input {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Journal => "journal",
            Self::Anchor => "anchor",
        }
    }
}

/// Byte offsets are zero based. Sequence is absent for context/header errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub input: Input,
    pub offset: u64,
    pub sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidContext,
    UnsupportedProfile,
    LimitExceeded,
    TruncatedInput,
    InvalidMagic,
    ContextMismatch,
    ChecksumMismatch,
    InvalidSequence,
    InvalidPreviousChecksum,
    NoncanonicalEvent,
    InvalidTransition,
    EffectMismatch,
    StaleEpoch,
    DuplicateReservation,
    UnknownReservation,
    InsufficientUnits,
    ArithmeticOverflow,
    ConservationFailure,
    AnchorMismatch,
    AnchorTooShort,
    AnchorNotRecordBoundary,
    AnchorBeyondJournal,
    Io,
}

impl ErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidContext => "invalid_context",
            Self::UnsupportedProfile => "unsupported_profile",
            Self::LimitExceeded => "limit_exceeded",
            Self::TruncatedInput => "truncated_input",
            Self::InvalidMagic => "invalid_magic",
            Self::ContextMismatch => "context_mismatch",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::InvalidSequence => "invalid_sequence",
            Self::InvalidPreviousChecksum => "invalid_previous_checksum",
            Self::NoncanonicalEvent => "noncanonical_event",
            Self::InvalidTransition => "invalid_transition",
            Self::EffectMismatch => "effect_mismatch",
            Self::StaleEpoch => "stale_epoch",
            Self::DuplicateReservation => "duplicate_reservation",
            Self::UnknownReservation => "unknown_reservation",
            Self::InsufficientUnits => "insufficient_units",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::ConservationFailure => "conservation_failure",
            Self::AnchorMismatch => "anchor_mismatch",
            Self::AnchorTooShort => "anchor_too_short",
            Self::AnchorNotRecordBoundary => "anchor_not_record_boundary",
            Self::AnchorBeyondJournal => "anchor_beyond_journal",
            Self::Io => "io",
        }
    }
}

#[derive(Debug)]
pub struct VerifyError {
    pub kind: ErrorKind,
    pub location: Location,
    pub io_kind: Option<std::io::ErrorKind>,
    explanation: &'static str,
}

impl VerifyError {
    pub const fn kind_name(&self) -> &'static str {
        self.kind.as_str()
    }

    pub const fn explanation(&self) -> &'static str {
        self.explanation
    }

    fn at(kind: ErrorKind, location: Location, explanation: &'static str) -> Self {
        Self { kind, location, io_kind: None, explanation }
    }

    fn context(kind: ErrorKind, explanation: &'static str) -> Self {
        Self::at(kind, Location { input: Input::Context, offset: 0, sequence: None }, explanation)
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {} byte {}", self.kind_name(), self.location.input.as_str(), self.location.offset)?;
        if let Some(sequence) = self.location.sequence {
            write!(f, " (record {sequence})")?;
        }
        write!(f, ": {}", self.explanation)
    }
}

impl std::error::Error for VerifyError {}

/// Verify a whole history against an independently supplied canonical prefix.
/// No partial report is returned on failure. No input is written or executed.
/// Repeated verification of the same inputs is intentionally idempotent.
pub fn verify<J: Read, A: Read>(
    journal: J,
    anchor: A,
    expected: ExpectedContext,
) -> Result<VerificationReport, VerifyError> {
    stream::verify(journal, anchor, expected)
}

#[cfg(test)]
mod tests;
