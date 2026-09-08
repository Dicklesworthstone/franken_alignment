//! FA-081 reference binding for the evidence a helper was actually shown.
//!
//! This is an in-memory format contract. It retains caller-supplied submitted
//! bytes through [`ActualHelperInput`] and binds declared provenance, filtering,
//! transforms, redaction, ordering, and source windows to that exact view. It
//! neither authenticates a provider capture nor proves a transform executed.

use std::collections::BTreeSet;

use crate::Error;
use crate::full_input::{
    ActualHelperInput, InputProfileBinding, MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES,
    MAX_SUBMITTED_PARTS, Omission, PartKind,
};

/// Maximum declared distinct original identities in one reference view.
pub const MAX_PROJECTED_ORIGINALS: usize = MAX_SUBMITTED_PARTS;
/// Maximum evidence-part bindings in one reference view.
pub const MAX_EVIDENCE_PART_BINDINGS: usize = MAX_SUBMITTED_PARTS;

/// A declared original object identity. `generation` may be zero for an
/// initial generation; identity validity/authentication belongs to a later
/// boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OriginalIdentity {
    pub tenant_id: u64,
    pub object_id: u64,
    pub generation: u64,
}

/// A declarative selection of originals under one input-policy epoch.
///
/// This is provenance about a view, not authority: it cannot mint a permit,
/// reserve rights, or authorize any effect. Its `policy_epoch` deliberately
/// shares the input-profile policy namespace and must match it exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationProjection {
    pub projection_id: u64,
    pub policy_epoch: u64,
    /// Strictly unique declared identities. Construction requires this set to
    /// equal the distinct originals represented by evidence parts.
    pub projected_originals: Vec<OriginalIdentity>,
}

/// Whether a declared redaction transform was included in a helper view.
///
/// `None` and `DeclaredTransform { transform_id }` are distinct committed
/// values. A declared identifier is not evidence that any provider executed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedactionMetadata {
    None,
    DeclaredTransform { transform_id: u64 },
}

/// A nonempty source-byte window declared for one represented evidence part.
///
/// `truncated` is exactly whether this selected source window is a proper
/// subset of the declared original range. It is not a claim about downstream
/// provider or output truncation. This binds the stated source length and
/// selected window, but does not prove the caller's original object had that
/// length or that a transform preserved source semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowMetadata {
    pub original_byte_len: u64,
    pub window_start: u64,
    pub window_len: u64,
    pub truncated: bool,
}

/// Metadata for exactly one `PartKind::Evidence` part in submitted input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidencePartView {
    /// Index in `ActualHelperInput::ordered_parts`, hence the submitted order.
    pub input_part_index: usize,
    pub original: OriginalIdentity,
    pub transform_id: u64,
    pub redaction: RedactionMetadata,
    pub window: WindowMetadata,
}

/// A fully validated, exact reference manifest for one helper input view.
///
/// Fields are private so callers cannot make a metadata-only manifest or alter
/// it after validation. `actual_input` owns the exact caller-supplied byte
/// sequence; this module never serializes metadata into replacement bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceViewManifest {
    actual_input: ActualHelperInput,
    authorization: AuthorizationProjection,
    evidence_parts: Vec<EvidencePartView>,
}

impl EvidenceViewManifest {
    /// Constructs a manifest only when every submitted evidence part has one
    /// matching, ordered binding and the declared projection is exact.
    pub fn new(
        actual_input: ActualHelperInput,
        authorization: AuthorizationProjection,
        evidence_parts: Vec<EvidencePartView>,
    ) -> Result<Self, Error> {
        Self::validate_input_bounds(&actual_input)?;
        if authorization.projected_originals.len() > MAX_PROJECTED_ORIGINALS
            || evidence_parts.len() > MAX_EVIDENCE_PART_BINDINGS
        {
            return Err(Error::Limit);
        }
        if authorization.policy_epoch != actual_input.input_profile().policy_epoch {
            return Err(Error::Binding);
        }

        let projected_originals = Self::unique_projection(&authorization.projected_originals)?;
        let actual_evidence = actual_input
            .ordered_parts()
            .iter()
            .enumerate()
            .filter_map(|(index, part)| match &part.kind {
                PartKind::Evidence {
                    source_id,
                    transform_id,
                } => Some((index, *source_id, *transform_id)),
                _ => None,
            })
            .collect::<Vec<_>>();

        if evidence_parts.len() != actual_evidence.len() {
            return Err(Error::Binding);
        }

        let mut represented_originals = BTreeSet::new();
        let mut prior_part_index = None;
        for (binding, (expected_index, source_id, transform_id)) in
            evidence_parts.iter().zip(actual_evidence)
        {
            if prior_part_index.is_some_and(|prior| binding.input_part_index <= prior)
                || binding.input_part_index != expected_index
                || binding.original.object_id != source_id
                || binding.transform_id != transform_id
            {
                return Err(Error::Binding);
            }
            Self::validate_window(binding.window)?;
            represented_originals.insert(binding.original);
            prior_part_index = Some(binding.input_part_index);
        }

        if represented_originals != projected_originals {
            return Err(Error::Binding);
        }

        Ok(Self {
            actual_input,
            authorization,
            evidence_parts,
        })
    }

    pub fn actual_input(&self) -> &ActualHelperInput {
        &self.actual_input
    }

    /// The exact bytes supplied to the existing validated whole-input model.
    pub fn submitted_bytes(&self) -> &[u8] {
        self.actual_input.submitted_bytes()
    }

    pub fn input_profile(&self) -> &InputProfileBinding {
        self.actual_input.input_profile()
    }

    /// Omitted ranges and domains remain in the typed whole input view.
    pub fn omissions(&self) -> &[Omission] {
        self.actual_input.omissions()
    }

    pub fn authorization(&self) -> &AuthorizationProjection {
        &self.authorization
    }

    pub fn evidence_parts(&self) -> &[EvidencePartView] {
        &self.evidence_parts
    }

    fn validate_input_bounds(actual_input: &ActualHelperInput) -> Result<(), Error> {
        if actual_input.submitted_bytes().len() > MAX_SUBMITTED_BYTES
            || actual_input.input_profile().profile_bytes.len() > MAX_PROFILE_BYTES
            || actual_input.ordered_parts().len() > MAX_SUBMITTED_PARTS
            || actual_input.omissions().len() > MAX_OMISSIONS
        {
            return Err(Error::Limit);
        }
        Ok(())
    }

    fn unique_projection(
        projected_originals: &[OriginalIdentity],
    ) -> Result<BTreeSet<OriginalIdentity>, Error> {
        let mut originals = BTreeSet::new();
        for identity in projected_originals {
            if !originals.insert(*identity) {
                return Err(Error::InvalidInput);
            }
        }
        Ok(originals)
    }

    fn validate_window(window: WindowMetadata) -> Result<(), Error> {
        if window.original_byte_len == 0 || window.window_len == 0 {
            return Err(Error::InvalidInput);
        }
        let window_end = window
            .window_start
            .checked_add(window.window_len)
            .ok_or(Error::Limit)?;
        if window_end > window.original_byte_len {
            return Err(Error::InvalidInput);
        }
        let expected_truncated = window.window_start != 0 || window_end != window.original_byte_len;
        if window.truncated != expected_truncated {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}

/// A captured in-memory witness for replaying one exact evidence view.
///
/// This equality witness has no hash, signature, provider authentication, or
/// helper-truth claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceViewWitness {
    manifest: EvidenceViewManifest,
}

impl EvidenceViewWitness {
    pub fn capture(manifest: &EvidenceViewManifest) -> Self {
        Self {
            manifest: manifest.clone(),
        }
    }

    pub fn valid_at(&self, candidate: &EvidenceViewManifest) -> bool {
        self.manifest == *candidate
    }

    pub fn manifest(&self) -> &EvidenceViewManifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_input::{ByteSpan, InputProfileBinding, Omission, SubmittedPart};

    fn original(object_id: u64) -> OriginalIdentity {
        OriginalIdentity {
            tenant_id: 1,
            object_id,
            generation: 0,
        }
    }

    fn profile(policy_epoch: u64) -> InputProfileBinding {
        InputProfileBinding {
            profile_id: 2,
            profile_bytes: b"helper-input-v1".to_vec(),
            tokenizer_epoch: 0,
            policy_epoch,
            model_epoch: 0,
        }
    }

    fn input(
        bytes: Vec<u8>,
        evidence_order: &[(u64, u64)],
        profile: InputProfileBinding,
        omissions: Vec<Omission>,
    ) -> ActualHelperInput {
        let mut parts = vec![SubmittedPart {
            span: ByteSpan { start: 0, end: 1 },
            kind: PartKind::Question,
        }];
        for (index, (source_id, transform_id)) in evidence_order.iter().enumerate() {
            parts.push(SubmittedPart {
                span: ByteSpan {
                    start: index + 1,
                    end: index + 2,
                },
                kind: PartKind::Evidence {
                    source_id: *source_id,
                    transform_id: *transform_id,
                },
            });
        }
        ActualHelperInput::new(bytes, profile, parts, omissions).unwrap()
    }

    fn part(
        input_part_index: usize,
        object_id: u64,
        transform_id: u64,
        redaction: RedactionMetadata,
        window: WindowMetadata,
    ) -> EvidencePartView {
        EvidencePartView {
            input_part_index,
            original: original(object_id),
            transform_id,
            redaction,
            window,
        }
    }

    fn window() -> WindowMetadata {
        WindowMetadata {
            original_byte_len: 20,
            window_start: 4,
            window_len: 8,
            truncated: true,
        }
    }

    fn full_window() -> WindowMetadata {
        WindowMetadata {
            original_byte_len: 20,
            window_start: 0,
            window_len: 20,
            truncated: false,
        }
    }

    fn manifest(
        bytes: Vec<u8>,
        evidence_order: &[(u64, u64)],
        projection_id: u64,
        policy_epoch: u64,
        redaction: RedactionMetadata,
        window: WindowMetadata,
        omissions: Vec<Omission>,
    ) -> EvidenceViewManifest {
        let actual = input(bytes, evidence_order, profile(policy_epoch), omissions);
        let evidence_parts = evidence_order
            .iter()
            .enumerate()
            .map(|(index, (object_id, transform_id))| {
                part(index + 1, *object_id, *transform_id, redaction, window)
            })
            .collect::<Vec<_>>();
        let mut originals = evidence_order
            .iter()
            .map(|(object_id, _)| original(*object_id))
            .collect::<Vec<_>>();
        originals.sort_unstable();
        originals.dedup();
        EvidenceViewManifest::new(
            actual,
            AuthorizationProjection {
                projection_id,
                policy_epoch,
                projected_originals: originals,
            },
            evidence_parts,
        )
        .unwrap()
    }

    #[test]
    fn capture_and_replay_bind_exact_input_without_reencoding() {
        let bytes = b"QAB".to_vec();
        let manifest = manifest(
            bytes.clone(),
            &[(7, 11), (7, 12)],
            3,
            0,
            RedactionMetadata::None,
            window(),
            vec![Omission::Gapped {
                domain_id: 41,
                first_missing: 9,
            }],
        );
        let witness = EvidenceViewWitness::capture(&manifest);

        assert!(witness.valid_at(&manifest));
        assert_eq!(manifest.submitted_bytes(), bytes);
        assert_eq!(witness.manifest().actual_input(), manifest.actual_input());
        assert_eq!(manifest.evidence_parts().len(), 2);
        assert_eq!(
            manifest.authorization().projected_originals,
            vec![original(7)]
        );
        assert!(matches!(manifest.omissions(), [Omission::Gapped { .. }]));
    }

    #[test]
    fn source_window_coverage_derives_truncated_and_refuses_mismatches() {
        let actual = input(b"QA".to_vec(), &[(7, 11)], profile(0), vec![]);
        let projection = AuthorizationProjection {
            projection_id: 3,
            policy_epoch: 0,
            projected_originals: vec![original(7)],
        };

        let full = part(1, 7, 11, RedactionMetadata::None, full_window());
        assert!(EvidenceViewManifest::new(actual.clone(), projection.clone(), vec![full]).is_ok());

        let partial = part(1, 7, 11, RedactionMetadata::None, window());
        assert!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), vec![partial]).is_ok()
        );

        let full_marked_partial = part(
            1,
            7,
            11,
            RedactionMetadata::None,
            WindowMetadata {
                truncated: true,
                ..full_window()
            },
        );
        assert_eq!(
            EvidenceViewManifest::new(
                actual.clone(),
                projection.clone(),
                vec![full_marked_partial],
            ),
            Err(Error::InvalidInput)
        );

        let partial_marked_full = part(
            1,
            7,
            11,
            RedactionMetadata::None,
            WindowMetadata {
                truncated: false,
                ..window()
            },
        );
        assert_eq!(
            EvidenceViewManifest::new(actual, projection, vec![partial_marked_full]),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn every_required_manifest_field_causally_invalidates_replay() {
        let baseline = manifest(
            b"QAB".to_vec(),
            &[(7, 11), (8, 12)],
            3,
            0,
            RedactionMetadata::None,
            window(),
            vec![Omission::Gapped {
                domain_id: 41,
                first_missing: 9,
            }],
        );
        let witness = EvidenceViewWitness::capture(&baseline);

        let cases = [
            manifest(
                b"QAB".to_vec(),
                &[(9, 11), (8, 12)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 11), (8, 12)],
                4,
                0,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 13), (8, 12)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 11), (8, 12)],
                3,
                0,
                RedactionMetadata::DeclaredTransform { transform_id: 61 },
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(8, 12), (7, 11)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 11), (8, 12)],
                3,
                0,
                RedactionMetadata::None,
                WindowMetadata {
                    original_byte_len: 20,
                    window_start: 5,
                    window_len: 8,
                    truncated: true,
                },
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QXB".to_vec(),
                &[(7, 11), (8, 12)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 11), (8, 12)],
                3,
                1,
                RedactionMetadata::None,
                window(),
                baseline.omissions().to_vec(),
            ),
            manifest(
                b"QAB".to_vec(),
                &[(7, 11), (8, 12)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                vec![Omission::Gapped {
                    domain_id: 41,
                    first_missing: 10,
                }],
            ),
        ];

        for candidate in cases {
            assert!(
                !witness.valid_at(&candidate),
                "a one-family manifest mutation replayed as the captured view"
            );
        }
    }

    #[test]
    fn repeated_windows_for_one_original_are_valid_but_projection_is_distinct() {
        let manifest = manifest(
            b"QAB".to_vec(),
            &[(7, 11), (7, 12)],
            3,
            0,
            RedactionMetadata::None,
            window(),
            vec![],
        );
        assert_eq!(manifest.evidence_parts().len(), 2);
        assert_eq!(
            manifest.authorization().projected_originals,
            vec![original(7)]
        );
    }

    #[test]
    fn every_input_profile_component_is_a_replay_dependency() {
        let baseline = manifest(
            b"QA".to_vec(),
            &[(7, 11)],
            3,
            0,
            RedactionMetadata::None,
            window(),
            vec![],
        );
        let witness = EvidenceViewWitness::capture(&baseline);

        let mut profile_id = profile(0);
        profile_id.profile_id = 3;
        let mut profile_bytes = profile(0);
        profile_bytes.profile_bytes[0] ^= 1;
        let mut tokenizer_epoch = profile(0);
        tokenizer_epoch.tokenizer_epoch = 1;
        let mut model_epoch = profile(0);
        model_epoch.model_epoch = 1;

        for changed_profile in [profile_id, profile_bytes, tokenizer_epoch, model_epoch] {
            let candidate = EvidenceViewManifest::new(
                input(b"QA".to_vec(), &[(7, 11)], changed_profile, vec![]),
                baseline.authorization().clone(),
                baseline.evidence_parts().to_vec(),
            )
            .unwrap();
            assert!(!witness.valid_at(&candidate));
        }

        // Policy epoch is deliberately one namespace in both records; changing
        // it consistently still produces a different captured manifest.
        let candidate = EvidenceViewManifest::new(
            input(b"QA".to_vec(), &[(7, 11)], profile(1), vec![]),
            AuthorizationProjection {
                policy_epoch: 1,
                ..baseline.authorization().clone()
            },
            baseline.evidence_parts().to_vec(),
        )
        .unwrap();
        assert!(!witness.valid_at(&candidate));
    }

    #[test]
    fn omission_presence_kind_and_declared_range_are_replay_dependencies() {
        let baseline = manifest(
            b"QA".to_vec(),
            &[(7, 11)],
            3,
            0,
            RedactionMetadata::None,
            window(),
            vec![Omission::Gapped {
                domain_id: 41,
                first_missing: 9,
            }],
        );
        let witness = EvidenceViewWitness::capture(&baseline);
        for omissions in [
            vec![],
            vec![Omission::ClosedAbsent {
                domain_id: 41,
                trusted_closure_marker_id: 9,
            }],
            vec![Omission::Unsupported { domain_id: 41 }],
            vec![Omission::Redacted {
                domain_id: 41,
                transform_id: 9,
            }],
            vec![Omission::Gapped {
                domain_id: 41,
                first_missing: 10,
            }],
        ] {
            let candidate = manifest(
                b"QA".to_vec(),
                &[(7, 11)],
                3,
                0,
                RedactionMetadata::None,
                window(),
                omissions,
            );
            assert!(!witness.valid_at(&candidate));
        }
    }

    #[test]
    fn empty_evidence_and_empty_projection_are_explicitly_valid() {
        let actual = input(b"Q".to_vec(), &[], profile(0), vec![]);
        let manifest = EvidenceViewManifest::new(
            actual,
            AuthorizationProjection {
                projection_id: 1,
                policy_epoch: 0,
                projected_originals: vec![],
            },
            vec![],
        )
        .unwrap();
        assert!(EvidenceViewWitness::capture(&manifest).valid_at(&manifest));
    }

    #[test]
    fn coverage_projection_and_window_mismatches_refuse() {
        let actual = input(b"QAB".to_vec(), &[(7, 11), (8, 12)], profile(0), vec![]);
        let bindings = vec![
            part(1, 7, 11, RedactionMetadata::None, window()),
            part(2, 8, 12, RedactionMetadata::None, window()),
        ];
        let projection = AuthorizationProjection {
            projection_id: 1,
            policy_epoch: 0,
            projected_originals: vec![original(7), original(8)],
        };
        assert!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), bindings.clone()).is_ok()
        );

        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), vec![]),
            Err(Error::Binding)
        );

        let mut extra_binding = bindings.clone();
        extra_binding.push(part(2, 8, 12, RedactionMetadata::None, window()));
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), extra_binding),
            Err(Error::Binding)
        );

        let mut duplicate_binding = bindings.clone();
        duplicate_binding[1].input_part_index = 1;
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), duplicate_binding),
            Err(Error::Binding)
        );

        let mut wrong_source = bindings.clone();
        wrong_source[0].original = original(99);
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), wrong_source),
            Err(Error::Binding)
        );

        let mut wrong_transform = bindings.clone();
        wrong_transform[0].transform_id = 99;
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), wrong_transform),
            Err(Error::Binding)
        );

        let mut duplicate_projection = projection.clone();
        duplicate_projection.projected_originals.push(original(8));
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), duplicate_projection, bindings.clone()),
            Err(Error::InvalidInput)
        );

        let mut extra_projection = projection.clone();
        extra_projection.projected_originals.push(original(9));
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), extra_projection, bindings.clone()),
            Err(Error::Binding)
        );

        let mut mismatched_epoch = projection.clone();
        mismatched_epoch.policy_epoch = 1;
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), mismatched_epoch, bindings.clone()),
            Err(Error::Binding)
        );

        let mut malformed_window = bindings;
        malformed_window[0].window.window_len = 0;
        assert_eq!(
            EvidenceViewManifest::new(actual.clone(), projection.clone(), malformed_window),
            Err(Error::InvalidInput)
        );

        let mut overflowing_window = projection;
        let overflowing_binding = vec![
            part(
                1,
                7,
                11,
                RedactionMetadata::None,
                WindowMetadata {
                    original_byte_len: u64::MAX,
                    window_start: u64::MAX,
                    window_len: 1,
                    truncated: true,
                },
            ),
            part(2, 8, 12, RedactionMetadata::None, window()),
        ];
        overflowing_window.projection_id = 2;
        assert_eq!(
            EvidenceViewManifest::new(actual, overflowing_window, overflowing_binding),
            Err(Error::Limit)
        );
    }
}
