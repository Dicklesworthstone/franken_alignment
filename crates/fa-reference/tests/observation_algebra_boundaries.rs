//! Public-boundary tests for the pure FA-068 observation-transfer algebra.
//!
//! These exercise declared transfer laws only. They do not execute operators,
//! invoke helpers, authenticate sources, or grant effect authority.

use fa_reference::Error;
use fa_reference::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart,
};
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, CancellationObligations, CapturedInput, CapturedObservation, ClaimClass,
    DecodeLaw, DecodeRequirement, EncodedObject, Knowledge, LawDeclaration, Obligation,
    ObligationKind, ObservationRequirement, OperatorContext, PrivacyLabel, PrivacyRestrictions,
    ProjectionSpec, ProofStates, ReadWitness, RequirementClass, ResourceEnvelope, ScopedProof,
    Transfer, UncertaintyClass, UnknownReason, VerifiedObject, authorize_projection,
    await_closed_scope, decode, discover_similar, emit_judgment, join_evidence, judge_independent,
    lookup_exact, probe, require_prefix, tap, verify_object,
};
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, WitnessSnapshot};

fn basis(claim_class: ClaimClass) -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 1,
            question_id: 2,
            profile_id: 3,
            semantic_generation: 4,
            policy_epoch: 5,
            model_epoch: 6,
            tokenizer_epoch: 0,
            codec_epoch: 7,
            control_seq: 8,
        },
        claim_class,
        ProofStates {
            authentic_origin: ScopedProof::Declared {
                scope_id: 11,
                generation: 4,
            },
            complete_for_contract: ScopedProof::Declared {
                scope_id: 13,
                generation: 4,
            },
            semantically_valid: ScopedProof::Declared {
                scope_id: 17,
                generation: 4,
            },
            available_for_replay: ScopedProof::Declared {
                scope_id: 19,
                generation: 4,
            },
        },
    )
}

fn resources() -> ResourceEnvelope {
    ResourceEnvelope::new(8, 8, 8, 8, 4096, 4096, 8).unwrap()
}

fn context_with(
    class: RequirementClass,
    uncertainty: UncertaintyClass,
    basis: Basis,
    privacy: Vec<PrivacyLabel>,
    obligations: Vec<Obligation>,
) -> OperatorContext {
    OperatorContext::new(
        ObservationRequirement {
            class,
            uncertainty,
            basis,
        },
        PrivacyRestrictions::new(privacy).unwrap(),
        CancellationObligations::new(obligations).unwrap(),
        resources(),
    )
}

fn context(class: RequirementClass, uncertainty: UncertaintyClass) -> OperatorContext {
    context_with(
        class,
        uncertainty,
        basis(ClaimClass::BoundedModel),
        vec![PrivacyLabel {
            purpose_id: 23,
            transform_id: 29,
        }],
        Vec::new(),
    )
}

fn key() -> ProjectionKey {
    ProjectionKey {
        source: 17,
        branch: 19,
        projection: 31,
        source_epoch: 29,
    }
}

fn captured() -> Transfer<CapturedObservation> {
    tap(
        CapturedInput::new(17, 29, b"source".to_vec()).unwrap(),
        context(RequirementClass::Capture, UncertaintyClass::Exact),
    )
    .unwrap()
}

fn prefixed()
-> fa_reference::observation_algebra::Transfer<fa_reference::observation_algebra::PrefixObservation>
{
    let capture = captured();
    let projection = authorize_projection(
        &capture,
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context(RequirementClass::Projection, UncertaintyClass::Exact),
    )
    .unwrap();
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key(), FrontierStage::Captured, 1).unwrap();
    require_prefix(
        &projection,
        &frontiers,
        FrontierRequirement {
            key: key(),
            stage: FrontierStage::Captured,
            through: 1,
            closure: None,
        },
        context(RequirementClass::Prefix, UncertaintyClass::Exact),
    )
    .unwrap()
}

fn verified_object() -> Transfer<VerifiedObject> {
    let prefix = prefixed();
    verify_object(
        &prefix,
        fa_reference::observation_algebra::ObjectDeclaration {
            object_id: 17,
            generation: 29,
            profile_id: 3,
        },
        LawDeclaration::new(41, 29, 4, 3).unwrap(),
        context(RequirementClass::ExactObject, UncertaintyClass::Exact),
    )
    .unwrap()
}

fn encoded() -> EncodedObject {
    EncodedObject {
        object_id: 17,
        bytes: b"encoded".to_vec(),
        profile_id: 3,
        codec_epoch: 7,
    }
}

fn helper_input(bytes: &[u8]) -> ActualHelperInput {
    let parts = if bytes.len() == 2 {
        vec![
            SubmittedPart {
                span: ByteSpan { start: 0, end: 1 },
                kind: PartKind::Question,
            },
            SubmittedPart {
                span: ByteSpan { start: 1, end: 2 },
                kind: PartKind::Prompt,
            },
        ]
    } else {
        vec![SubmittedPart {
            span: ByteSpan { start: 0, end: 1 },
            kind: PartKind::Question,
        }]
    };
    ActualHelperInput::new(
        bytes.to_vec(),
        InputProfileBinding {
            profile_id: 3,
            profile_bytes: b"profile".to_vec(),
            tokenizer_epoch: 0,
            policy_epoch: 5,
            model_epoch: 6,
        },
        parts,
        Vec::new(),
    )
    .unwrap()
}

fn closed_snapshot(
    satisfy_frontier: bool,
) -> (WitnessSnapshot, ProductFrontiers, FrontierRequirement) {
    let marker = TrustedClosingMarker {
        key: key(),
        final_sequence: 1,
        marker_generation: 43,
    };
    let snapshot = WitnessSnapshot::new(
        47,
        8,
        4,
        AdapterDomainInput::new(
            DomainProjection::new(53, 59, key()),
            DomainClosure::Closed(marker),
        ),
        Vec::new(),
    )
    .unwrap();
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    if satisfy_frontier {
        frontiers
            .accept(key(), FrontierStage::Authenticated, 1)
            .unwrap();
        frontiers.record_close(marker).unwrap();
    }
    (
        snapshot,
        frontiers,
        FrontierRequirement {
            key: key(),
            stage: FrontierStage::Authenticated,
            through: 1,
            closure: Some(43),
        },
    )
}

#[test]
fn runnable_public_composition_retains_typed_evidence_without_operator_execution() {
    let verified = verified_object();
    let decoded = decode(
        &verified,
        encoded(),
        b"decoded".to_vec(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(LawDeclaration::new(61, 7, 4, 3).unwrap()),
        context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
    )
    .unwrap();
    assert!(decoded.witnesses().entries().iter().any(|entry| {
        matches!(entry, ReadWitness::EncodedBytes {
            object_id: 17,
            profile_id: 3,
            codec_epoch: 7,
            bytes,
        } if bytes == b"encoded")
    }));
    assert!(decoded.witnesses().entries().iter().any(|entry| {
        matches!(entry, ReadWitness::DecodedBytes {
            object_id: 17,
            profile_id: 3,
            codec_epoch: 7,
            bytes,
        } if bytes.as_ref() == b"decoded")
    }));
    match decoded.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), b"decoded"),
        value => panic!("expected decoded bytes, got {value:?}"),
    }
    let probe = probe(
        &decoded,
        67,
        5,
        1,
        71,
        context(
            RequirementClass::Probe,
            UncertaintyClass::Bounded { bound_id: 71 },
        ),
    )
    .unwrap();
    assert!(matches!(probe.value(), Knowledge::Known { .. }));
    assert_eq!(probe.value().basis(), Some(basis(ClaimClass::BoundedModel)));
    assert!(matches!(
        probe
            .value()
            .basis()
            .unwrap()
            .proofs()
            .complete_for_contract,
        ScopedProof::Declared {
            scope_id: 13,
            generation: 4
        }
    ));
    assert_eq!(probe.resources().max_bytes(), 4096);

    let left = captured();
    let right = tap(
        CapturedInput::new(73, 29, b"second".to_vec()).unwrap(),
        context(RequirementClass::Capture, UncertaintyClass::Exact),
    )
    .unwrap();
    let evidence = join_evidence(
        &[left, right],
        context(RequirementClass::JoinedEvidence, UncertaintyClass::Exact),
    )
    .unwrap();
    let judgment = judge_independent(
        &helper_input(b"QP"),
        context(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque { calibration_id: 79 },
        ),
    )
    .unwrap();
    let emitted = emit_judgment(
        &evidence,
        &judgment,
        context(
            RequirementClass::EmittedJudgment,
            UncertaintyClass::Opaque { calibration_id: 79 },
        ),
    )
    .unwrap();
    assert!(matches!(emitted.value(), Knowledge::Known { .. }));
}

#[test]
fn captured_bytes_are_retained_as_exact_witnesses_after_cancellation() {
    let capture = captured();
    assert!(capture.witnesses().entries().iter().any(|entry| {
        matches!(entry, ReadWitness::CapturedBytes {
            source_id: 17,
            source_generation: 29,
            profile_id: 3,
            bytes,
        } if bytes.as_ref() == b"source")
    }));
    match capture.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), b"source"),
        value => panic!("expected captured bytes, got {value:?}"),
    }
    let cancelled = capture.cancel();
    assert!(cancelled.is_cancelled());
    assert!(cancelled.witnesses().entries().iter().any(|entry| {
        matches!(entry, ReadWitness::CapturedBytes {
            source_id: 17,
            source_generation: 29,
            profile_id: 3,
            bytes,
        } if bytes.as_ref() == b"source")
    }));
}

#[test]
fn top_k_and_lossy_restart_cannot_satisfy_stronger_requirements() {
    let universal_input = verified_object();
    assert_eq!(
        discover_similar(
            &universal_input,
            83,
            89,
            vec![97, 101],
            context(RequirementClass::Universal, UncertaintyClass::Exact),
        ),
        Err(Error::Binding)
    );
    assert!(matches!(universal_input.value(), Knowledge::Known { .. }));

    let approximate_input = verified_object();
    let approximate = discover_similar(
        &approximate_input,
        83,
        89,
        vec![97, 101],
        context(
            RequirementClass::ApproximateDiscovery,
            UncertaintyClass::Approximate { profile_id: 89 },
        ),
    )
    .unwrap();
    assert!(matches!(approximate.value(), Knowledge::Known { .. }));

    let lossy_input = verified_object();
    assert_eq!(
        decode(
            &lossy_input,
            encoded(),
            b"lossy".to_vec(),
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::Lossy {
                profile_id: 3,
                error_bound_id: 103
            },
            context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
        ),
        Err(Error::InvalidInput)
    );
    assert!(matches!(lossy_input.value(), Knowledge::Known { .. }));

    let exact_input = verified_object();
    let exact = decode(
        &exact_input,
        encoded(),
        b"decoded".to_vec(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(LawDeclaration::new(181, 7, 4, 3).unwrap()),
        context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
    )
    .unwrap();
    assert!(matches!(exact.value(), Knowledge::Known { .. }));
}

#[test]
fn homogeneous_derived_probes_deduplicate_one_inherited_frontier_on_join() {
    let verified = verified_object();
    let decoded = decode(
        &verified,
        encoded(),
        b"decoded".to_vec(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(LawDeclaration::new(191, 7, 4, 3).unwrap()),
        context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
    )
    .unwrap();
    let first = probe(
        &decoded,
        193,
        5,
        1,
        197,
        context(
            RequirementClass::Probe,
            UncertaintyClass::Bounded { bound_id: 197 },
        ),
    )
    .unwrap();
    let second = probe(
        &decoded,
        199,
        5,
        1,
        197,
        context(
            RequirementClass::Probe,
            UncertaintyClass::Bounded { bound_id: 197 },
        ),
    )
    .unwrap();
    let inputs = [first, second];
    assert_eq!(inputs[0].witnesses().entries().len(), 8);
    assert_eq!(inputs[1].witnesses().entries().len(), 8);
    let small = context(
        RequirementClass::JoinedEvidence,
        UncertaintyClass::Bounded { bound_id: 197 },
    );
    let requirement = small.requirement();
    assert_eq!(join_evidence(&inputs, small), Err(Error::Limit));
    let enough_witnesses = OperatorContext::new(
        requirement,
        PrivacyRestrictions::new(vec![PrivacyLabel {
            purpose_id: 23,
            transform_id: 29,
        }])
        .unwrap(),
        CancellationObligations::new(Vec::new()).unwrap(),
        ResourceEnvelope::new(8, 8, 16, 8, 4096, 4096, 8).unwrap(),
    );
    let joined = join_evidence(&inputs, enough_witnesses).unwrap();
    assert_eq!(joined.witnesses().entries().len(), 16);
    let inherited = FrontierRequirement {
        key: key(),
        stage: FrontierStage::Captured,
        through: 1,
        closure: None,
    };
    assert_eq!(joined.frontiers(), &[inherited]);
    assert!(matches!(joined.value(), Knowledge::Known { .. }));
}

#[test]
fn retained_judgment_and_identity_mismatches_refuse_causally() {
    let full = helper_input(b"QP");
    let judgment = judge_independent(
        &full,
        context(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 107,
            },
        ),
    )
    .unwrap();
    match judgment.value() {
        Knowledge::Known { value, .. } => {
            assert!(value.judgment().valid_at(&full));
            assert!(!value.judgment().valid_at(&helper_input(b"Q")));
        }
        value => panic!("expected retained judgment, got {value:?}"),
    }

    for bad_key in [
        ProjectionKey {
            source: 109,
            ..key()
        },
        ProjectionKey {
            projection: 113,
            ..key()
        },
        ProjectionKey {
            source_epoch: 127,
            ..key()
        },
    ] {
        let capture = captured();
        assert_eq!(
            authorize_projection(
                &capture,
                ProjectionSpec {
                    key: bad_key,
                    projection_id: 31
                },
                context(RequirementClass::Projection, UncertaintyClass::Exact),
            ),
            Err(Error::Binding)
        );
        assert!(matches!(capture.value(), Knowledge::Known { .. }));
    }

    let accepted_prefix = prefixed();
    assert!(
        verify_object(
            &accepted_prefix,
            fa_reference::observation_algebra::ObjectDeclaration {
                object_id: 17,
                generation: 29,
                profile_id: 3,
            },
            LawDeclaration::new(131, 29, 4, 3).unwrap(),
            context(RequirementClass::ExactObject, UncertaintyClass::Exact),
        )
        .is_ok()
    );

    let prefix = prefixed();
    assert_eq!(
        verify_object(
            &prefix,
            fa_reference::observation_algebra::ObjectDeclaration {
                object_id: 131,
                generation: 29,
                profile_id: 3,
            },
            LawDeclaration::new(137, 29, 4, 3).unwrap(),
            context(RequirementClass::ExactObject, UncertaintyClass::Exact),
        ),
        Err(Error::Binding)
    );
    let prefix = prefixed();
    assert_eq!(
        verify_object(
            &prefix,
            fa_reference::observation_algebra::ObjectDeclaration {
                object_id: 17,
                generation: 139,
                profile_id: 3,
            },
            LawDeclaration::new(149, 139, 4, 3).unwrap(),
            context(RequirementClass::ExactObject, UncertaintyClass::Exact),
        ),
        Err(Error::Binding)
    );
    let prefix = prefixed();
    assert_eq!(
        verify_object(
            &prefix,
            fa_reference::observation_algebra::ObjectDeclaration {
                object_id: 17,
                generation: 29,
                profile_id: 3,
            },
            LawDeclaration::new(151, 29, 157, 3).unwrap(),
            context(RequirementClass::ExactObject, UncertaintyClass::Exact),
        ),
        Err(Error::Binding)
    );

    for invalid in [
        EncodedObject {
            object_id: 163,
            ..encoded()
        },
        EncodedObject {
            profile_id: 167,
            ..encoded()
        },
        EncodedObject {
            codec_epoch: 173,
            ..encoded()
        },
    ] {
        let verified = verified_object();
        let control = decode(
            &verified,
            encoded(),
            b"decoded".to_vec(),
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::RegisteredLossless(LawDeclaration::new(179, 7, 4, 3).unwrap()),
            context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
        )
        .expect("unchanged identity and registered generations must decode");
        assert!(matches!(control.value(), Knowledge::Known { .. }));
        assert_eq!(
            decode(
                &verified,
                invalid,
                b"decoded".to_vec(),
                DecodeRequirement::ExactCheckpoint,
                DecodeLaw::RegisteredLossless(LawDeclaration::new(179, 7, 4, 3).unwrap()),
                context(RequirementClass::ExactCheckpoint, UncertaintyClass::Exact),
            ),
            Err(Error::Binding)
        );
        assert!(matches!(verified.value(), Knowledge::Known { .. }));
    }
}

#[test]
fn closed_scope_absence_binds_key_domain_cut_marker_and_inherited_obligations() {
    let (snapshot, frontiers, requirement) = closed_snapshot(true);
    assert_eq!(
        await_closed_scope(
            &snapshot,
            &frontiers,
            requirement,
            context(
                RequirementClass::ClosedScope,
                UncertaintyClass::Conservative
            ),
        ),
        Err(Error::Binding)
    );
    let scope = await_closed_scope(
        &snapshot,
        &frontiers,
        requirement,
        context_with(
            RequirementClass::ClosedScope,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            vec![PrivacyLabel {
                purpose_id: 181,
                transform_id: 191,
            }],
            vec![Obligation {
                id: 193,
                kind: ObligationKind::PreserveFrontier,
            }],
        ),
    )
    .unwrap();
    let absent = lookup_exact(
        &snapshot,
        197,
        Some(&scope),
        context_with(
            RequirementClass::ExactLookup,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            vec![PrivacyLabel {
                purpose_id: 199,
                transform_id: 211,
            }],
            vec![Obligation {
                id: 223,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    assert_eq!(absent.frontiers(), &[requirement]);
    for purpose_id in [181, 199] {
        assert!(
            absent
                .privacy()
                .labels()
                .iter()
                .any(|label| label.purpose_id == purpose_id)
        );
    }
    for id in [193, 223] {
        assert!(
            absent
                .cancellation()
                .entries()
                .iter()
                .any(|entry| entry.id == id)
        );
    }
    match absent.value() {
        Knowledge::Absent { domain } => {
            assert_eq!(domain.domain_id(), 53);
            assert_eq!(domain.revision(), 47);
            assert_eq!(domain.control_cut(), 8);
            assert_eq!(domain.semantic_epoch(), 4);
            assert_eq!(domain.marker().marker_generation, 43);
        }
        value => panic!("expected typed absence, got {value:?}"),
    }
    assert!(absent.witnesses().entries().iter().any(|entry| {
        matches!(entry, ReadWitness::AbsentKey { key: 197, domain } if domain.domain_id() == 53)
    }));

    let unrelated = WitnessSnapshot::new(
        47,
        8,
        4,
        AdapterDomainInput::new(
            DomainProjection::new(283, 59, key()),
            DomainClosure::Closed(TrustedClosingMarker {
                key: key(),
                final_sequence: 1,
                marker_generation: 43,
            }),
        ),
        Vec::new(),
    )
    .unwrap();
    let unrelated_lookup = lookup_exact(
        &unrelated,
        197,
        Some(&scope),
        context(RequirementClass::ExactLookup, UncertaintyClass::Exact),
    )
    .unwrap();
    assert!(matches!(
        unrelated_lookup.value(),
        Knowledge::Unknown {
            reason: UnknownReason::IncompatibleGeneration
        }
    ));
}

#[test]
fn incompatible_join_and_cancelled_projection_preserve_inherited_obligations() {
    let left = tap(
        CapturedInput::new(17, 29, b"left".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            vec![PrivacyLabel {
                purpose_id: 227,
                transform_id: 229,
            }],
            vec![Obligation {
                id: 233,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .unwrap();
    let right = tap(
        CapturedInput::new(239, 29, b"right".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            // Deliberately different declared claim strength: this is the
            // causal incompatible-basis input, not a value-role substitute.
            basis(ClaimClass::Proof),
            vec![PrivacyLabel {
                purpose_id: 241,
                transform_id: 251,
            }],
            vec![Obligation {
                id: 257,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    assert_eq!(
        join_evidence(
            &[left.clone(), right.clone()],
            context_with(
                RequirementClass::JoinedEvidence,
                UncertaintyClass::Conservative,
                basis(ClaimClass::BoundedModel),
                Vec::new(),
                Vec::new(),
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(left.cancellation().entries()[0].id, 233);
    assert_eq!(right.cancellation().entries()[0].id, 257);
    let joined = join_evidence(
        &[left, right],
        context_with(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            vec![PrivacyLabel {
                purpose_id: 263,
                transform_id: 269,
            }],
            vec![Obligation {
                id: 271,
                kind: ObligationKind::ResolveBeforeRelease,
            }],
        ),
    )
    .unwrap();
    assert!(matches!(
        joined.value(),
        Knowledge::Unknown {
            reason: UnknownReason::ConflictingEvidence
        }
    ));
    for purpose_id in [227, 241, 263] {
        assert!(
            joined
                .privacy()
                .labels()
                .iter()
                .any(|label| label.purpose_id == purpose_id)
        );
    }
    for id in [233, 257, 271] {
        assert!(
            joined
                .cancellation()
                .entries()
                .iter()
                .any(|entry| entry.id == id)
        );
    }

    let cancelled_capture = tap(
        CapturedInput::new(17, 29, b"source".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 277,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .unwrap()
    .cancel();
    let projection = authorize_projection(
        &cancelled_capture,
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context_with(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 281,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    assert!(projection.is_cancelled());
    assert!(matches!(
        projection.value(),
        Knowledge::Unknown {
            reason: UnknownReason::Cancelled
        }
    ));
    for id in [277, 281] {
        assert!(
            projection
                .cancellation()
                .entries()
                .iter()
                .any(|entry| entry.id == id)
        );
    }

    let evidence_left = tap(
        CapturedInput::new(17, 29, b"emit-left".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 283,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .unwrap();
    let evidence_right = tap(
        CapturedInput::new(293, 29, b"emit-right".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 307,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    let emit_evidence = join_evidence(
        &[evidence_left, evidence_right],
        context_with(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 311,
                kind: ObligationKind::PreserveFrontier,
            }],
        ),
    )
    .unwrap();
    let mismatched_judgment = judge_independent(
        &helper_input(b"QP"),
        context_with(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 313,
            },
            basis(ClaimClass::Proof),
            Vec::new(),
            vec![Obligation {
                id: 317,
                kind: ObligationKind::ResolveBeforeRelease,
            }],
        ),
    )
    .unwrap()
    .cancel();
    let emitted = emit_judgment(
        &emit_evidence,
        &mismatched_judgment,
        context_with(
            RequirementClass::EmittedJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 313,
            },
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 331,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    assert!(matches!(
        emitted.value(),
        Knowledge::Unknown {
            reason: UnknownReason::IncompatibleGeneration
        }
    ));
    assert!(emitted.is_cancelled());
    for id in [283, 307, 311, 317, 331] {
        assert!(
            emitted
                .cancellation()
                .entries()
                .iter()
                .any(|entry| entry.id == id)
        );
    }
}

#[test]
fn cancellation_obligation_union_deduplicates_only_identical_entries() {
    let identical = Obligation {
        id: 283,
        kind: ObligationKind::RetainInput,
    };
    let first = tap(
        CapturedInput::new(17, 29, b"first".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![identical],
        ),
    )
    .unwrap();
    let second = tap(
        CapturedInput::new(293, 29, b"second".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![identical],
        ),
    )
    .unwrap();
    let joined = join_evidence(
        &[first, second],
        context_with(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![identical],
        ),
    )
    .unwrap();
    assert_eq!(joined.cancellation().entries(), &[identical]);

    let conflicting_left = tap(
        CapturedInput::new(17, 29, b"left".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 307,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .unwrap();
    let conflicting_right = tap(
        CapturedInput::new(311, 29, b"right".to_vec()).unwrap(),
        context_with(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            basis(ClaimClass::BoundedModel),
            Vec::new(),
            vec![Obligation {
                id: 307,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .unwrap();
    assert_eq!(
        join_evidence(
            &[conflicting_left.clone(), conflicting_right.clone()],
            context(RequirementClass::JoinedEvidence, UncertaintyClass::Exact),
        ),
        Err(Error::Binding)
    );
    assert_eq!(conflicting_left.cancellation().entries()[0].id, 307);
    assert_eq!(conflicting_right.cancellation().entries()[0].id, 307);
}

#[test]
fn pending_closed_scope_and_unknown_lookup_survive_cancellation() {
    let (snapshot, frontiers, requirement) = closed_snapshot(false);
    let pending = await_closed_scope(
        &snapshot,
        &frontiers,
        requirement,
        context(RequirementClass::ClosedScope, UncertaintyClass::Exact),
    )
    .unwrap();
    let unknown = lookup_exact(
        &snapshot,
        283,
        None,
        context(RequirementClass::ExactLookup, UncertaintyClass::Exact),
    )
    .unwrap();
    fn assert_preserved<T: Clone + std::fmt::Debug + PartialEq>(transfer: Transfer<T>) {
        let value = transfer.value().clone();
        let cancelled = transfer.cancel();
        assert!(cancelled.is_cancelled());
        assert_eq!(cancelled.value(), &value);
    }
    assert_preserved(pending);
    assert_preserved(unknown);
}
