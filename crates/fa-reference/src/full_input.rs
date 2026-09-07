//! Bounded reference model for an opaque helper's complete actual input view.
//!
//! `submitted_bytes` are supplied by the caller as the exact bytes sent to the
//! helper. This module neither reconstructs those bytes from metadata nor claims
//! to capture a provider request. Its metadata only partitions and describes the
//! supplied byte stream.

use std::collections::BTreeSet;

use crate::Error;

/// Reference-profile bounds for hostile or malformed input.
pub const MAX_SUBMITTED_BYTES: usize = 64 * 1024;
pub const MAX_PROFILE_BYTES: usize = 8 * 1024;
pub const MAX_SUBMITTED_PARTS: usize = 128;
pub const MAX_OMISSIONS: usize = 64;

/// A half-open range in `ActualHelperInput::submitted_bytes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

/// The role and provenance metadata for bytes already present in the submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartKind {
    Question,
    Prompt,
    Instruction,
    ToolSchema { schema_id: u64 },
    Evidence { source_id: u64, transform_id: u64 },
    Delimiter,
    Other,
}

/// A contiguous piece of the exact byte stream submitted to a helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmittedPart {
    pub span: ByteSpan,
    pub kind: PartKind,
}

/// The profile that shaped the submitted input, retained as bytes and identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputProfileBinding {
    pub profile_id: u64,
    pub profile_bytes: Vec<u8>,
}

/// A domain omitted from the helper's actual input view.
///
/// A closure marker is only an identity in this reference model. The trusted
/// boundary that authenticates it is intentionally outside this module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Omission {
    ClosedAbsent {
        domain_id: u64,
        trusted_closure_marker_id: u64,
    },
    Gapped {
        domain_id: u64,
        first_missing: u64,
    },
    Unsupported {
        domain_id: u64,
    },
    Redacted {
        domain_id: u64,
        transform_id: u64,
    },
}

impl Omission {
    fn domain_id(&self) -> u64 {
        match self {
            Self::ClosedAbsent { domain_id, .. }
            | Self::Gapped { domain_id, .. }
            | Self::Unsupported { domain_id }
            | Self::Redacted { domain_id, .. } => *domain_id,
        }
    }
}

/// The entire bounded actual input view of an opaque helper judgment.
///
/// ```
/// use fa_reference::full_input::{
///     ActualHelperInput, InputProfileBinding, PartKind, SubmittedPart, ByteSpan,
/// };
///
/// let input = ActualHelperInput::new(
///     b"Q".to_vec(),
///     InputProfileBinding {
///         profile_id: 1,
///         profile_bytes: vec![],
///     },
///     vec![SubmittedPart {
///         span: ByteSpan { start: 0, end: 1 },
///         kind: PartKind::Question,
///     }],
///     vec![],
/// )
/// .unwrap();
/// assert_eq!(input.submitted_bytes(), b"Q");
/// ```
///
/// ```compile_fail,E0451
/// use fa_reference::full_input::{
///     ActualHelperInput, InputProfileBinding, PartKind, SubmittedPart, ByteSpan,
/// };
///
/// let _ = ActualHelperInput {
///     submitted_bytes: b"Q".to_vec(),
///     input_profile: InputProfileBinding {
///         profile_id: 1,
///         profile_bytes: vec![],
///     },
///     ordered_parts: vec![SubmittedPart {
///         span: ByteSpan { start: 0, end: 1 },
///         kind: PartKind::Question,
///     }],
///     omissions: vec![],
/// };
/// ```
///
/// Construction must go through [`ActualHelperInput::new`], which checks the
/// bounded full-view contract before an opaque judgment can capture it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActualHelperInput {
    submitted_bytes: Vec<u8>,
    input_profile: InputProfileBinding,
    ordered_parts: Vec<SubmittedPart>,
    omissions: Vec<Omission>,
}

impl ActualHelperInput {
    /// Validates metadata against bytes supplied by the caller without changing
    /// their encoding or assigning semantics to an untrusted explanation.
    pub fn new(
        submitted_bytes: Vec<u8>,
        input_profile: InputProfileBinding,
        ordered_parts: Vec<SubmittedPart>,
        omissions: Vec<Omission>,
    ) -> Result<Self, Error> {
        if submitted_bytes.len() > MAX_SUBMITTED_BYTES
            || input_profile.profile_bytes.len() > MAX_PROFILE_BYTES
            || ordered_parts.len() > MAX_SUBMITTED_PARTS
            || omissions.len() > MAX_OMISSIONS
        {
            return Err(Error::Limit);
        }

        let mut expected_start = 0;
        let mut question_count = 0;
        for part in &ordered_parts {
            if part.span.start != expected_start
                || part.span.end <= part.span.start
                || part.span.end > submitted_bytes.len()
            {
                return Err(Error::InvalidInput);
            }
            if matches!(&part.kind, PartKind::Question) {
                question_count += 1;
            }
            expected_start = part.span.end;
        }
        if question_count != 1 || expected_start != submitted_bytes.len() {
            return Err(Error::InvalidInput);
        }

        let mut domains = BTreeSet::new();
        for omission in &omissions {
            if matches!(
                omission,
                Omission::ClosedAbsent {
                    trusted_closure_marker_id: 0,
                    ..
                }
            ) || !domains.insert(omission.domain_id())
            {
                return Err(Error::InvalidInput);
            }
        }

        Ok(Self {
            submitted_bytes,
            input_profile,
            ordered_parts,
            omissions,
        })
    }

    pub fn submitted_bytes(&self) -> &[u8] {
        &self.submitted_bytes
    }

    pub fn input_profile(&self) -> &InputProfileBinding {
        &self.input_profile
    }

    pub fn ordered_parts(&self) -> &[SubmittedPart] {
        &self.ordered_parts
    }

    pub fn omissions(&self) -> &[Omission] {
        &self.omissions
    }

    pub fn part_bytes(&self, part_index: usize) -> Result<&[u8], Error> {
        let part = self.ordered_parts.get(part_index).ok_or(Error::Missing)?;
        self.submitted_bytes
            .get(part.span.start..part.span.end)
            .ok_or(Error::Binding)
    }
}

/// The indivisible dependency for an opaque helper judgment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueInputWitness {
    actual_input: ActualHelperInput,
}

impl OpaqueInputWitness {
    pub fn actual_input(&self) -> &ActualHelperInput {
        &self.actual_input
    }
}

/// A historical opaque judgment whose helper explanation cannot narrow inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueJudgment {
    witness: OpaqueInputWitness,
}

impl OpaqueJudgment {
    /// The explanation is explicitly ignored: opaque inference depends on every
    /// byte and metadata record in the actual submitted input view.
    pub fn capture(actual_input: &ActualHelperInput, _untrusted_explanation: &[u8]) -> Self {
        Self {
            witness: OpaqueInputWitness {
                actual_input: actual_input.clone(),
            },
        }
    }

    pub fn valid_at(&self, actual_input: &ActualHelperInput) -> bool {
        self.witness.actual_input == *actual_input
    }

    pub fn witness(&self) -> &OpaqueInputWitness {
        &self.witness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mixed_input(omissions: Vec<Omission>) -> ActualHelperInput {
        ActualHelperInput::new(
            b"Q|P|I|S|E".to_vec(),
            InputProfileBinding {
                profile_id: 7,
                profile_bytes: b"profile-v1".to_vec(),
            },
            vec![
                SubmittedPart {
                    span: ByteSpan { start: 0, end: 1 },
                    kind: PartKind::Question,
                },
                SubmittedPart {
                    span: ByteSpan { start: 1, end: 2 },
                    kind: PartKind::Delimiter,
                },
                SubmittedPart {
                    span: ByteSpan { start: 2, end: 3 },
                    kind: PartKind::Prompt,
                },
                SubmittedPart {
                    span: ByteSpan { start: 3, end: 4 },
                    kind: PartKind::Delimiter,
                },
                SubmittedPart {
                    span: ByteSpan { start: 4, end: 5 },
                    kind: PartKind::Instruction,
                },
                SubmittedPart {
                    span: ByteSpan { start: 5, end: 6 },
                    kind: PartKind::Delimiter,
                },
                SubmittedPart {
                    span: ByteSpan { start: 6, end: 7 },
                    kind: PartKind::ToolSchema { schema_id: 3 },
                },
                SubmittedPart {
                    span: ByteSpan { start: 7, end: 8 },
                    kind: PartKind::Delimiter,
                },
                SubmittedPart {
                    span: ByteSpan { start: 8, end: 9 },
                    kind: PartKind::Evidence {
                        source_id: 5,
                        transform_id: 11,
                    },
                },
            ],
            omissions,
        )
        .unwrap()
    }

    #[test]
    fn opaque_witness_binds_the_entire_actual_input_view() {
        let input = mixed_input(vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 9,
        }]);
        let judgment = OpaqueJudgment::capture(&input, b"only the question mattered");

        assert!(judgment.valid_at(&input));
        assert_eq!(judgment.witness().actual_input(), &input);
        assert_eq!(input.submitted_bytes(), b"Q|P|I|S|E");
        assert_eq!(input.input_profile().profile_id, 7);
        assert_eq!(input.ordered_parts().len(), 9);
        assert!(matches!(input.omissions(), [Omission::Gapped { .. }]));
        assert_eq!(input.part_bytes(0), Ok(&b"Q"[..]));
        assert_eq!(input.part_bytes(8), Ok(&b"E"[..]));
    }

    #[test]
    fn question_profile_and_unmentioned_evidence_changes_invalidate() {
        let input = mixed_input(vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 9,
        }]);
        let judgment = OpaqueJudgment::capture(&input, b"I used only part zero");
        assert_eq!(
            judgment,
            OpaqueJudgment::capture(&input, b"a different explanation"),
        );

        let mut question_changed = input.clone();
        question_changed.submitted_bytes[0] = b'X';
        assert!(!judgment.valid_at(&question_changed));

        let mut profile_changed = input.clone();
        profile_changed.input_profile.profile_id = 8;
        assert!(!judgment.valid_at(&profile_changed));

        let mut evidence_changed = input;
        evidence_changed.submitted_bytes[8] = b'X';
        assert!(!judgment.valid_at(&evidence_changed));
    }

    #[test]
    fn transform_and_omission_changes_invalidate() {
        let input = mixed_input(vec![Omission::ClosedAbsent {
            domain_id: 41,
            trusted_closure_marker_id: 13,
        }]);
        let judgment = OpaqueJudgment::capture(&input, b"irrelevant");

        let mut transform_changed = input.clone();
        transform_changed.ordered_parts[8].kind = PartKind::Evidence {
            source_id: 5,
            transform_id: 12,
        };
        assert!(!judgment.valid_at(&transform_changed));

        let mut marker_changed = input.clone();
        marker_changed.omissions[0] = Omission::ClosedAbsent {
            domain_id: 41,
            trusted_closure_marker_id: 14,
        };
        assert!(!judgment.valid_at(&marker_changed));

        let mut coverage_changed = input;
        coverage_changed.omissions[0] = Omission::Gapped {
            domain_id: 41,
            first_missing: 9,
        };
        assert!(!judgment.valid_at(&coverage_changed));
    }

    #[test]
    fn malformed_partitions_omissions_and_bounds_are_rejected() {
        let valid = mixed_input(vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 9,
        }]);

        let mut gapped_parts = valid.ordered_parts.clone();
        gapped_parts[1].span.start = 2;
        assert_eq!(
            ActualHelperInput::new(
                valid.submitted_bytes.clone(),
                valid.input_profile.clone(),
                gapped_parts,
                valid.omissions.clone(),
            ),
            Err(Error::InvalidInput)
        );

        assert_eq!(
            ActualHelperInput::new(
                b"x".to_vec(),
                valid.input_profile.clone(),
                vec![SubmittedPart {
                    span: ByteSpan { start: 0, end: 1 },
                    kind: PartKind::Other,
                }],
                vec![],
            ),
            Err(Error::InvalidInput)
        );

        assert_eq!(
            ActualHelperInput::new(
                valid.submitted_bytes.clone(),
                valid.input_profile.clone(),
                valid.ordered_parts.clone(),
                vec![
                    Omission::Gapped {
                        domain_id: 41,
                        first_missing: 9,
                    },
                    Omission::Unsupported { domain_id: 41 },
                ],
            ),
            Err(Error::InvalidInput)
        );

        assert_eq!(
            ActualHelperInput::new(
                vec![0; MAX_SUBMITTED_BYTES + 1],
                valid.input_profile,
                vec![SubmittedPart {
                    span: ByteSpan {
                        start: 0,
                        end: MAX_SUBMITTED_BYTES + 1,
                    },
                    kind: PartKind::Question,
                }],
                vec![],
            ),
            Err(Error::Limit)
        );
    }
}
