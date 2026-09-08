//! Independent public conformance laws for every FA-068 reference operator.
//!
//! These are declared bounded-model transfer laws only: they do not execute an
//! observation plan, authenticate an origin, invoke a helper, or grant effect
//! authority.

use fa_reference::{
    Error,
    full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart},
    observation_algebra::{
        Basis, BasisIdentity, CancellationObligations, CapturedInput, ClaimClass, DecodeLaw,
        DecodeRequirement, EncodedObject, Knowledge, LawDeclaration, MAX_OPERATOR_BYTES,
        MAX_OPERATOR_CANDIDATES, MAX_OPERATOR_DECODED_BYTES, MAX_OPERATOR_FRONTIERS,
        MAX_OPERATOR_INPUTS, MAX_OPERATOR_OBLIGATIONS, MAX_OPERATOR_OUTPUTS,
        MAX_OPERATOR_PRIVACY_LABELS, MAX_OPERATOR_WITNESSES, Obligation, ObligationKind,
        ObservationRequirement, OperatorContext, PrivacyLabel, PrivacyRestrictions, ProjectionSpec,
        ProofStates, ReadWitness, RequirementClass, ResourceEnvelope, ScopedProof,
        UncertaintyClass, UnknownReason, WitnessSet, authorize_projection, await_closed_scope,
        decode, discover_similar, emit_judgment, join_evidence, judge_independent, lookup_exact,
        probe, refine_witness, require_prefix, tap, verify_object,
    },
    product_frontier::{
        FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    },
    witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry, WitnessSnapshot,
    },
};

fn basis(semantic_generation: u64) -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 1,
            question_id: 2,
            profile_id: 3,
            semantic_generation,
            policy_epoch: 5,
            model_epoch: 6,
            tokenizer_epoch: 0,
            codec_epoch: 7,
            control_seq: 8,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: ScopedProof::Declared {
                scope_id: 11,
                generation: semantic_generation,
            },
            complete_for_contract: ScopedProof::Declared {
                scope_id: 13,
                generation: semantic_generation,
            },
            semantically_valid: ScopedProof::Declared {
                scope_id: 17,
                generation: semantic_generation,
            },
            available_for_replay: ScopedProof::Declared {
                scope_id: 19,
                generation: semantic_generation,
            },
        },
    )
}

fn resources() -> ResourceEnvelope {
    ResourceEnvelope::new(
        MAX_OPERATOR_INPUTS,
        MAX_OPERATOR_OUTPUTS,
        MAX_OPERATOR_WITNESSES,
        MAX_OPERATOR_FRONTIERS,
        MAX_OPERATOR_BYTES,
        MAX_OPERATOR_DECODED_BYTES,
        MAX_OPERATOR_CANDIDATES,
    )
    .expect("exact reference resource limits are valid")
}

fn context(
    class: RequirementClass,
    uncertainty: UncertaintyClass,
    semantic_generation: u64,
    obligations: Vec<Obligation>,
) -> OperatorContext {
    OperatorContext::new(
        ObservationRequirement {
            class,
            uncertainty,
            basis: basis(semantic_generation),
        },
        PrivacyRestrictions::new(vec![]).expect("empty privacy set is valid"),
        CancellationObligations::new(obligations).expect("fixed obligations are valid"),
        resources(),
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

fn captured(
    source_id: u64,
    bytes: &[u8],
) -> fa_reference::observation_algebra::Transfer<
    fa_reference::observation_algebra::CapturedObservation,
> {
    tap(
        CapturedInput::new(source_id, 29, bytes.to_vec()).expect("bounded source input"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("exact capture transfer")
}

fn retains_captured_bytes<T>(
    transfer: &fa_reference::observation_algebra::Transfer<T>,
    expected: &[u8],
) -> bool {
    transfer.witnesses().entries().iter().any(|entry| {
        matches!(
            entry,
            ReadWitness::CapturedBytes { bytes, .. } if bytes.as_ref() == expected
        )
    })
}

fn verified()
-> fa_reference::observation_algebra::Transfer<fa_reference::observation_algebra::VerifiedObject> {
    let projection = authorize_projection(
        &captured(17, b"source"),
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("matching projection");
    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontiers");
    frontiers
        .accept(key(), FrontierStage::Captured, 1)
        .expect("captured prefix");
    let prefix = require_prefix(
        &projection,
        &frontiers,
        FrontierRequirement {
            key: key(),
            stage: FrontierStage::Captured,
            through: 1,
            closure: None,
        },
        context(RequirementClass::Prefix, UncertaintyClass::Exact, 4, vec![]),
    )
    .expect("present prefix");
    verify_object(
        &prefix,
        fa_reference::observation_algebra::ObjectDeclaration {
            object_id: 17,
            generation: 29,
            profile_id: 3,
        },
        LawDeclaration::new(41, 29, 4, 3).expect("registered verifier law"),
        context(
            RequirementClass::ExactObject,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("matching exact object declaration")
}

fn helper_input(policy_epoch: u64) -> ActualHelperInput {
    ActualHelperInput::new(
        b"QP".to_vec(),
        InputProfileBinding {
            profile_id: 3,
            profile_bytes: b"profile".to_vec(),
            tokenizer_epoch: 0,
            policy_epoch,
            model_epoch: 6,
        },
        vec![
            SubmittedPart {
                span: ByteSpan { start: 0, end: 1 },
                kind: PartKind::Question,
            },
            SubmittedPart {
                span: ByteSpan { start: 1, end: 2 },
                kind: PartKind::Prompt,
            },
        ],
        vec![],
    )
    .expect("complete helper input")
}

fn closed_snapshot() -> (WitnessSnapshot, ProductFrontiers, FrontierRequirement) {
    closed_snapshot_for(key())
}

fn closed_snapshot_for(
    projection_key: ProjectionKey,
) -> (WitnessSnapshot, ProductFrontiers, FrontierRequirement) {
    let marker = TrustedClosingMarker {
        key: projection_key,
        final_sequence: 1,
        marker_generation: 43,
    };
    let snapshot = WitnessSnapshot::new(
        47,
        8,
        4,
        AdapterDomainInput::new(
            DomainProjection::new(53, 59, projection_key),
            DomainClosure::Closed(marker),
        ),
        vec![SnapshotEntry::new(61, 7, b"exact".to_vec()).expect("exact fixture")],
    )
    .expect("closed snapshot");
    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontiers");
    frontiers
        .accept(projection_key, FrontierStage::Authenticated, 1)
        .expect("authenticated prefix");
    frontiers.record_close(marker).expect("declared close");
    (
        snapshot,
        frontiers,
        FrontierRequirement {
            key: projection_key,
            stage: FrontierStage::Authenticated,
            through: 1,
            closure: Some(43),
        },
    )
}

#[test]
fn tap_cancellation_retains_exact_captured_bytes_witness() {
    let captured = tap(
        CapturedInput::new(17, 29, b"captured-before-cancellation".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("Tap control");
    match captured.value() {
        Knowledge::Known { value, .. } => {
            assert_eq!(value.bytes(), b"captured-before-cancellation")
        }
        value => panic!("Tap control must expose supplied bytes, got {value:?}"),
    }
    assert!(retains_captured_bytes(
        &captured,
        b"captured-before-cancellation"
    ));
    let cancelled = captured.cancel();
    assert!(matches!(
        cancelled.value(),
        Knowledge::Unknown {
            reason: UnknownReason::Cancelled
        }
    ));
    assert!(retains_captured_bytes(
        &cancelled,
        b"captured-before-cancellation"
    ));
}

#[test]
fn every_operator_has_a_declared_legal_bounded_model_law() {
    let captured = captured(17, b"source");
    match captured.value() {
        Knowledge::Known {
            value,
            basis: value_basis,
        } => {
            assert_eq!(value.bytes(), b"source");
            assert_eq!(*value_basis, basis(4));
        }
        value => panic!("tap must retain known exact bytes, got {value:?}"),
    }

    let projection = authorize_projection(
        &captured,
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("AuthorizeProjection legal control");
    assert_eq!(
        projection
            .value()
            .basis()
            .expect("projection has a known basis"),
        basis(4)
    );

    let mut prefix_frontiers = ProductFrontiers::new(1, 4).expect("bounded frontiers");
    prefix_frontiers
        .accept(key(), FrontierStage::Captured, 1)
        .expect("captured prefix");
    let prefix_requirement = FrontierRequirement {
        key: key(),
        stage: FrontierStage::Captured,
        through: 1,
        closure: None,
    };
    let prefix = require_prefix(
        &projection,
        &prefix_frontiers,
        prefix_requirement,
        context(RequirementClass::Prefix, UncertaintyClass::Exact, 4, vec![]),
    )
    .expect("RequirePrefix legal control");
    assert_eq!(prefix.frontiers(), &[prefix_requirement]);

    let verified = verify_object(
        &prefix,
        fa_reference::observation_algebra::ObjectDeclaration {
            object_id: 17,
            generation: 29,
            profile_id: 3,
        },
        LawDeclaration::new(41, 29, 4, 3).expect("registered verifier law"),
        context(
            RequirementClass::ExactObject,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("VerifyObject legal control");
    assert!(retains_captured_bytes(&verified, b"source"));
    let similar = discover_similar(
        &verified,
        67,
        71,
        vec![73, 79],
        context(
            RequirementClass::ApproximateDiscovery,
            UncertaintyClass::Approximate { profile_id: 71 },
            4,
            vec![],
        ),
    )
    .expect("DiscoverSimilar legal approximate control");
    match similar.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.candidates(), &[73, 79]),
        value => panic!("discovery control must remain known, got {value:?}"),
    }

    let identity_witnesses =
        WitnessSet::new(vec![ReadWitness::Epoch(basis(4))], resources()).expect("witness set");
    let refinement_law = LawDeclaration::new(83, 4, 4, 3).expect("registered identity law");
    let refined = refine_witness(
        identity_witnesses.clone(),
        identity_witnesses,
        refinement_law,
        context(RequirementClass::Refine, UncertaintyClass::Exact, 4, vec![]),
    )
    .expect("identity refinement is valid under its registered law");
    match refined.value() {
        Knowledge::Known { value, .. } => {
            assert_eq!(value.parent(), value.refined());
            assert_eq!(value.law(), refinement_law);
        }
        value => panic!("RefineWitness legal control must remain known, got {value:?}"),
    }

    let decoded = decode(
        &verified,
        EncodedObject {
            object_id: 17,
            bytes: b"encoded".to_vec(),
            profile_id: 3,
            codec_epoch: 7,
        },
        b"decoded".to_vec(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(
            LawDeclaration::new(89, 7, 4, 3).expect("registered lossless law"),
        ),
        context(
            RequirementClass::ExactCheckpoint,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("Decode legal exact checkpoint");
    match decoded.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), b"decoded"),
        value => panic!("Decode legal control must retain decoded bytes, got {value:?}"),
    }
    assert!(decoded.witnesses().entries().iter().any(|entry| matches!(
        entry,
        ReadWitness::EncodedBytes { bytes, .. } if bytes == b"encoded"
    )));
    assert!(decoded.witnesses().entries().iter().any(|entry| matches!(
        entry,
        ReadWitness::DecodedBytes { bytes, .. } if bytes.as_ref() == b"decoded"
    )));
    let first_probe = probe(
        &decoded,
        97,
        5,
        1,
        101,
        context(
            RequirementClass::Probe,
            UncertaintyClass::Bounded { bound_id: 101 },
            4,
            vec![],
        ),
    )
    .expect("Probe legal bounded margin");
    assert!(matches!(first_probe.value(), Knowledge::Known { .. }));
    assert!(retains_captured_bytes(&first_probe, b"source"));
    let second_probe = probe(
        &decoded,
        103,
        -5,
        1,
        101,
        context(
            RequirementClass::Probe,
            UncertaintyClass::Bounded { bound_id: 101 },
            4,
            vec![],
        ),
    )
    .expect("second Probe legal bounded margin");
    assert!(matches!(second_probe.value(), Knowledge::Known { .. }));

    let (snapshot, closed_frontiers, closed_requirement) = closed_snapshot();
    let exact = lookup_exact(
        &snapshot,
        61,
        None,
        context(
            RequirementClass::ExactLookup,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("LookupExact known key");
    match exact.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.value(), b"exact"),
        value => panic!("exact lookup must retain value, got {value:?}"),
    }
    let closed_scope = await_closed_scope(
        &snapshot,
        &closed_frontiers,
        closed_requirement,
        context(
            RequirementClass::ClosedScope,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("AwaitClosedScope legal close");
    let absent = lookup_exact(
        &snapshot,
        103,
        Some(&closed_scope),
        context(
            RequirementClass::ExactLookup,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("closed-domain absent lookup");
    assert!(matches!(absent.value(), Knowledge::Absent { .. }));
    assert_eq!(
        lookup_exact(
            &snapshot,
            103,
            Some(&closed_scope),
            context(
                RequirementClass::ExactLookup,
                UncertaintyClass::Approximate { profile_id: 3 },
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "absence promotion must not elevate an approximate lookup context"
    );

    let joined_inputs = [first_probe, second_probe];
    let evidence = join_evidence(
        &joined_inputs,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Bounded { bound_id: 101 },
            4,
            vec![],
        ),
    )
    .expect("JoinEvidence legal homogeneous probe inputs");
    assert_eq!(
        evidence.frontiers(),
        &[prefix_requirement],
        "shared inherited frontier must be retained once across joined probes"
    );
    assert!(retains_captured_bytes(&evidence, b"source"));
    let judgment = judge_independent(
        &helper_input(5),
        context(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 109,
            },
            4,
            vec![],
        ),
    )
    .expect("JudgeIndependent exact whole input");
    let emitted = emit_judgment(
        &evidence,
        &judgment,
        context(
            RequirementClass::EmittedJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 109,
            },
            4,
            vec![],
        ),
    )
    .expect("EmitJudgment legal compatible inputs");
    match emitted.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.evidence_members(), 2),
        value => panic!("EmitJudgment must retain known compatible evidence, got {value:?}"),
    }
}

#[test]
fn uncertainty_profile_generation_and_registered_law_mismatches_refuse() {
    assert_eq!(
        tap(
            CapturedInput::new(17, 29, b"source".to_vec()).expect("fixture"),
            context(
                RequirementClass::Capture,
                UncertaintyClass::Conservative,
                4,
                vec![]
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        authorize_projection(
            &captured(17, b"source"),
            ProjectionSpec {
                key: key(),
                projection_id: 31,
            },
            context(
                RequirementClass::Projection,
                UncertaintyClass::Conservative,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );

    let projection = authorize_projection(
        &captured(17, b"source"),
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("unchanged projection control");
    let mut frontiers = ProductFrontiers::new(1, 4).expect("bounded frontiers");
    assert_eq!(
        require_prefix(
            &projection,
            &frontiers,
            FrontierRequirement {
                key: key(),
                stage: FrontierStage::Captured,
                through: 1,
                closure: None,
            },
            context(RequirementClass::Prefix, UncertaintyClass::Exact, 5, vec![]),
        ),
        Err(Error::Binding)
    );

    frontiers
        .accept(key(), FrontierStage::Captured, 1)
        .expect("valid prefix fixture");
    let verification_requirement = FrontierRequirement {
        key: key(),
        stage: FrontierStage::Captured,
        through: 1,
        closure: None,
    };
    let valid_prefix = require_prefix(
        &projection,
        &frontiers,
        verification_requirement,
        context(RequirementClass::Prefix, UncertaintyClass::Exact, 4, vec![]),
    )
    .expect("same-fixture valid prefix");
    let declaration = fa_reference::observation_algebra::ObjectDeclaration {
        object_id: 17,
        generation: 29,
        profile_id: 3,
    };
    assert!(
        verify_object(
            &valid_prefix,
            declaration,
            LawDeclaration::new(109, 29, 4, 3).expect("valid law declaration"),
            context(
                RequirementClass::ExactObject,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        )
        .is_ok(),
        "the mismatched-law fixture must have a valid verification control"
    );
    assert_eq!(
        verify_object(
            &valid_prefix,
            declaration,
            LawDeclaration::new(113, 29, 5, 3).expect("law declaration"),
            context(
                RequirementClass::ExactObject,
                UncertaintyClass::Exact,
                4,
                vec![]
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        discover_similar(
            &verified(),
            127,
            131,
            vec![137],
            context(
                RequirementClass::ApproximateDiscovery,
                UncertaintyClass::Approximate { profile_id: 139 },
                4,
                vec![],
            ),
        ),
        Err(Error::Limit)
    );

    let witnesses =
        WitnessSet::new(vec![ReadWitness::Epoch(basis(4))], resources()).expect("witnesses");
    assert_eq!(
        refine_witness(
            witnesses.clone(),
            witnesses,
            LawDeclaration::new(149, 4, 5, 3).expect("law declaration"),
            context(RequirementClass::Refine, UncertaintyClass::Exact, 4, vec![]),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        decode(
            &verified(),
            EncodedObject {
                object_id: 17,
                bytes: b"encoded".to_vec(),
                profile_id: 3,
                codec_epoch: 7,
            },
            b"decoded".to_vec(),
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::RegisteredLossless(
                LawDeclaration::new(151, 7, 4, 23).expect("law declaration"),
            ),
            context(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        decode(
            &verified(),
            EncodedObject {
                object_id: 17,
                bytes: b"encoded".to_vec(),
                profile_id: 3,
                codec_epoch: 7,
            },
            b"decoded".to_vec(),
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::RegisteredLossless(
                LawDeclaration::new(153, 7, 5, 3).expect("law declaration"),
            ),
            context(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        decode(
            &verified(),
            EncodedObject {
                object_id: 17,
                bytes: b"encoded".to_vec(),
                profile_id: 3,
                codec_epoch: 23,
            },
            b"decoded".to_vec(),
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::RegisteredLossless(
                LawDeclaration::new(157, 23, 4, 3).expect("law declaration"),
            ),
            context(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );

    let decoded = decode(
        &verified(),
        EncodedObject {
            object_id: 17,
            bytes: b"encoded".to_vec(),
            profile_id: 3,
            codec_epoch: 7,
        },
        b"decoded".to_vec(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(LawDeclaration::new(163, 7, 4, 3).expect("law declaration")),
        context(
            RequirementClass::ExactCheckpoint,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("unchanged decode control");
    assert_eq!(
        probe(
            &decoded,
            167,
            5,
            1,
            173,
            context(
                RequirementClass::Probe,
                UncertaintyClass::Bounded { bound_id: 179 },
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        judge_independent(
            &helper_input(9),
            context(
                RequirementClass::IndependentJudgment,
                UncertaintyClass::Opaque {
                    calibration_id: 179,
                },
                4,
                vec![],
            ),
        ),
        Err(Error::Binding)
    );

    let (snapshot, closed_frontiers, requirement) = closed_snapshot();
    let exact_scope = await_closed_scope(
        &snapshot,
        &closed_frontiers,
        requirement,
        context(
            RequirementClass::ClosedScope,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("same closed fixture admits the exact request");
    assert!(matches!(exact_scope.value(), Knowledge::Known { .. }));
    assert_eq!(
        await_closed_scope(
            &snapshot,
            &closed_frontiers,
            requirement,
            context(
                RequirementClass::ClosedScope,
                UncertaintyClass::Exact,
                5,
                vec![]
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        await_closed_scope(
            &snapshot,
            &closed_frontiers,
            requirement,
            context(
                RequirementClass::ClosedScope,
                UncertaintyClass::Approximate { profile_id: 3 },
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "a closed scope must not become exact from an approximate context"
    );
    let stale = lookup_exact(
        &WitnessSnapshot::new(
            47,
            8,
            5,
            snapshot.domain_input(),
            vec![SnapshotEntry::new(61, 7, b"exact".to_vec()).expect("fixture")],
        )
        .expect("generation-mutated snapshot"),
        61,
        None,
        context(
            RequirementClass::ExactLookup,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("stale value is typed, not upgraded");
    assert!(matches!(stale.value(), Knowledge::Stale { .. }));

    let exact_join_inputs = [captured(211, b"exact-a"), captured(223, b"exact-b")];
    assert_eq!(
        join_evidence(
            &exact_join_inputs,
            context(
                RequirementClass::JoinedEvidence,
                UncertaintyClass::Conservative,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "JoinEvidence requires its context uncertainty to match every input"
    );
    let refinement_parent =
        WitnessSet::new(vec![ReadWitness::Epoch(basis(4))], resources()).expect("fixture");
    let exact_refinement = refine_witness(
        refinement_parent.clone(),
        refinement_parent.clone(),
        LawDeclaration::new(227, 4, 4, 3).expect("law declaration"),
        context(RequirementClass::Refine, UncertaintyClass::Exact, 4, vec![]),
    )
    .expect("exact refinement fixture");
    let conservative_refinement = refine_witness(
        refinement_parent.clone(),
        refinement_parent,
        LawDeclaration::new(229, 4, 4, 3).expect("law declaration"),
        context(
            RequirementClass::Refine,
            UncertaintyClass::Conservative,
            4,
            vec![],
        ),
    )
    .expect("conservative refinement fixture");
    let heterogeneous_join_inputs = [exact_refinement, conservative_refinement];
    assert_eq!(
        join_evidence(
            &heterogeneous_join_inputs,
            context(
                RequirementClass::JoinedEvidence,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "JoinEvidence requires exact uncertainty agreement across all inputs"
    );

    let conflicting_inputs = [
        captured(227, b"basis-four"),
        tap(
            CapturedInput::new(229, 29, b"basis-five".to_vec()).expect("fixture"),
            context(
                RequirementClass::Capture,
                UncertaintyClass::Exact,
                5,
                vec![],
            ),
        )
        .expect("mismatched-basis fixture"),
    ];
    let conflicting_join = join_evidence(
        &conflicting_inputs,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("basis conflict is modeled as uncertainty, not an authority upgrade");
    assert!(matches!(
        conflicting_join.value(),
        Knowledge::Unknown {
            reason: UnknownReason::ConflictingEvidence
        }
    ));

    let compatible_inputs = [captured(233, b"evidence-a"), captured(239, b"evidence-b")];
    let compatible_evidence = join_evidence(
        &compatible_inputs,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("compatible evidence fixture");
    let later_judgment = judge_independent(
        &helper_input(5),
        context(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 241,
            },
            5,
            vec![Obligation {
                id: 243,
                kind: ObligationKind::PreserveFrontier,
            }],
        ),
    )
    .expect("later-generation judgment fixture");
    let same_generation_judgment = judge_independent(
        &helper_input(5),
        context(
            RequirementClass::IndependentJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 251,
            },
            4,
            vec![],
        ),
    )
    .expect("same-generation opaque judgment fixture");
    assert_eq!(
        emit_judgment(
            &compatible_evidence,
            &same_generation_judgment,
            context(
                RequirementClass::EmittedJudgment,
                UncertaintyClass::Opaque {
                    calibration_id: 257,
                },
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "EmitJudgment must bind its opaque calibration to the judgment context"
    );
    let incompatible_emit = emit_judgment(
        &compatible_evidence,
        &later_judgment,
        context(
            RequirementClass::EmittedJudgment,
            UncertaintyClass::Opaque {
                calibration_id: 241,
            },
            4,
            vec![],
        ),
    )
    .expect("generation conflict is modeled as uncertainty, not a synthetic judgment");
    assert!(matches!(
        incompatible_emit.value(),
        Knowledge::Unknown {
            reason: UnknownReason::IncompatibleGeneration
        }
    ));
    assert!(
        incompatible_emit
            .cancellation()
            .entries()
            .iter()
            .any(|entry| {
                *entry
                    == Obligation {
                        id: 243,
                        kind: ObligationKind::PreserveFrontier,
                    }
            }),
        "judgment-origin mismatch must retain active cancellation work"
    );
}

#[test]
fn cancellation_retains_bytes_and_obligations_without_restoring_knowledge() {
    let transfer = tap(
        CapturedInput::new(17, 29, b"retained".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![Obligation {
                id: 181,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .expect("capture control");
    match transfer.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), b"retained"),
        value => panic!("capture must retain source bytes, got {value:?}"),
    }
    let projection = authorize_projection(
        &transfer.cancel(),
        ProjectionSpec {
            key: key(),
            projection_id: 31,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            4,
            vec![Obligation {
                id: 191,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .expect("cancelled transfer remains composable");
    assert!(projection.is_cancelled());
    assert!(matches!(
        projection.value(),
        Knowledge::Unknown {
            reason: UnknownReason::Cancelled
        }
    ));
    for id in [181, 191] {
        assert!(
            projection
                .cancellation()
                .entries()
                .iter()
                .any(|entry| entry.id == id),
            "cancellation erased declared obligation {id}"
        );
    }
}

#[test]
fn rejected_borrowed_transfers_preserve_their_declared_cancellation_work() {
    let original = tap(
        CapturedInput::new(17, 29, b"preserve-after-refusal".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![Obligation {
                id: 197,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .expect("capture control");
    assert_eq!(
        authorize_projection(
            &original,
            ProjectionSpec {
                key: ProjectionKey {
                    source: 199,
                    ..key()
                },
                projection_id: 31,
            },
            context(
                RequirementClass::Projection,
                UncertaintyClass::Exact,
                4,
                vec![]
            ),
        ),
        Err(Error::Binding)
    );
    assert_eq!(
        original.cancellation().entries(),
        &[Obligation {
            id: 197,
            kind: ObligationKind::RetainInput,
        }],
        "a rejected borrowed transfer must not erase retained cancellation work"
    );
    assert!(!original.is_cancelled());
}

#[test]
fn cancellation_composition_deduplicates_identical_work_and_refuses_kind_conflicts() {
    let same = Obligation {
        id: 211,
        kind: ObligationKind::RetainWitness,
    };
    let first = tap(
        CapturedInput::new(17, 29, b"first".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![same],
        ),
    )
    .expect("first capture");
    let second = tap(
        CapturedInput::new(107, 29, b"second".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![same],
        ),
    )
    .expect("second capture");
    let composed_inputs = [first, second];
    let composed = join_evidence(
        &composed_inputs,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("identical cancellation work composes idempotently");
    assert_eq!(composed.cancellation().entries(), &[same]);

    let capacity_obligations = (1..=MAX_OPERATOR_OBLIGATIONS)
        .map(|id| Obligation {
            id: id as u64 + 300,
            kind: ObligationKind::PreserveFrontier,
        })
        .collect::<Vec<_>>();
    let capacity_left = tap(
        CapturedInput::new(17, 29, b"capacity-left".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            capacity_obligations.clone(),
        ),
    )
    .expect("capacity-left capture");
    let capacity_right = tap(
        CapturedInput::new(107, 29, b"capacity-right".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            capacity_obligations,
        ),
    )
    .expect("capacity-right capture");
    let capacity_inputs = [capacity_left, capacity_right];
    let capacity_join = join_evidence(
        &capacity_inputs,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("identical obligations deduplicate before the unique-cap check");
    assert_eq!(
        capacity_join.cancellation().entries().len(),
        MAX_OPERATOR_OBLIGATIONS
    );

    let conflicting_left = tap(
        CapturedInput::new(17, 29, b"left".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![Obligation {
                id: 223,
                kind: ObligationKind::RetainInput,
            }],
        ),
    )
    .expect("left capture");
    let conflicting_right = tap(
        CapturedInput::new(107, 29, b"right".to_vec()).expect("fixture"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            4,
            vec![Obligation {
                id: 223,
                kind: ObligationKind::RetainWitness,
            }],
        ),
    )
    .expect("right capture");
    let conflicting_inputs = [conflicting_left, conflicting_right];
    assert_eq!(
        join_evidence(
            &conflicting_inputs,
            context(
                RequirementClass::JoinedEvidence,
                UncertaintyClass::Exact,
                4,
                vec![],
            ),
        ),
        Err(Error::Binding),
        "one obligation id with incompatible kinds cannot be silently merged"
    );
    assert_eq!(
        conflicting_inputs[0].cancellation().entries(),
        &[Obligation {
            id: 223,
            kind: ObligationKind::RetainInput,
        }]
    );
    assert_eq!(
        conflicting_inputs[1].cancellation().entries(),
        &[Obligation {
            id: 223,
            kind: ObligationKind::RetainWitness,
        }]
    );
}

#[test]
fn join_accepts_the_public_exact_cap_of_distinct_inherited_frontiers() {
    let mut distinct_scopes = Vec::with_capacity(MAX_OPERATOR_FRONTIERS);
    for index in 0..MAX_OPERATOR_FRONTIERS {
        let frontier_key = ProjectionKey {
            source: 10_000 + index as u64,
            ..key()
        };
        let (snapshot, frontiers, requirement) = closed_snapshot_for(frontier_key);
        distinct_scopes.push(
            await_closed_scope(
                &snapshot,
                &frontiers,
                requirement,
                context(
                    RequirementClass::ClosedScope,
                    UncertaintyClass::Exact,
                    4,
                    vec![],
                ),
            )
            .expect("distinct closed-scope fixture"),
        );
    }
    let joined = join_evidence(
        &distinct_scopes,
        context(
            RequirementClass::JoinedEvidence,
            UncertaintyClass::Exact,
            4,
            vec![],
        ),
    )
    .expect("exact distinct-frontier cap is admitted");
    assert_eq!(joined.frontiers().len(), MAX_OPERATOR_FRONTIERS);
}

#[test]
fn join_unions_identical_privacy_sets_and_refuses_one_distinct_label_over_cap() {
    let common_labels = (1..=MAX_OPERATOR_PRIVACY_LABELS)
        .map(|id| PrivacyLabel {
            purpose_id: id as u64,
            transform_id: 10_000 + id as u64,
        })
        .collect::<Vec<_>>();
    let capture_context = |privacy| {
        OperatorContext::new(
            ObservationRequirement {
                class: RequirementClass::Capture,
                uncertainty: UncertaintyClass::Exact,
                basis: basis(4),
            },
            privacy,
            CancellationObligations::new(vec![]).expect("empty cancellation fixture"),
            resources(),
        )
    };
    let join_context = || {
        OperatorContext::new(
            ObservationRequirement {
                class: RequirementClass::JoinedEvidence,
                uncertainty: UncertaintyClass::Exact,
                basis: basis(4),
            },
            PrivacyRestrictions::new(vec![]).expect("empty privacy fixture"),
            CancellationObligations::new(vec![]).expect("empty cancellation fixture"),
            resources(),
        )
    };

    let first = tap(
        CapturedInput::new(401, 29, b"privacy-a".to_vec()).expect("fixture"),
        capture_context(
            PrivacyRestrictions::new(common_labels.clone()).expect("exact privacy cap"),
        ),
    )
    .expect("first privacy capture");
    let second = tap(
        CapturedInput::new(403, 29, b"privacy-b".to_vec()).expect("fixture"),
        capture_context(
            PrivacyRestrictions::new(common_labels.clone()).expect("exact privacy cap"),
        ),
    )
    .expect("second privacy capture");
    let identical_inputs = [first, second];
    let identical_join =
        join_evidence(&identical_inputs, join_context()).expect("identical sets deduplicate");
    assert!(matches!(identical_join.value(), Knowledge::Known { .. }));
    assert_eq!(identical_join.privacy().labels(), common_labels.as_slice());

    let mut one_new_label = common_labels.clone();
    one_new_label[MAX_OPERATOR_PRIVACY_LABELS - 1] = PrivacyLabel {
        purpose_id: MAX_OPERATOR_PRIVACY_LABELS as u64 + 1,
        transform_id: 10_000 + MAX_OPERATOR_PRIVACY_LABELS as u64 + 1,
    };
    let changed_second = tap(
        CapturedInput::new(403, 29, b"privacy-b".to_vec()).expect("fixture"),
        capture_context(
            PrivacyRestrictions::new(one_new_label.clone()).expect("changed privacy fixture"),
        ),
    )
    .expect("changed second privacy capture");
    let over_cap_inputs = [identical_inputs[0].clone(), changed_second];
    assert_eq!(
        join_evidence(&over_cap_inputs, join_context()),
        Err(Error::Limit),
        "one new privacy label above the unique cap must be refused"
    );
    assert_eq!(
        over_cap_inputs[0].privacy().labels(),
        common_labels.as_slice(),
        "borrowed first input must survive a refused privacy union"
    );
    assert_eq!(
        over_cap_inputs[1].privacy().labels(),
        one_new_label.as_slice(),
        "borrowed changed input must survive a refused privacy union"
    );
}

#[test]
fn exact_and_one_over_node_witness_frontier_and_byte_bounds_use_real_constructors() {
    assert!(
        ResourceEnvelope::new(
            MAX_OPERATOR_INPUTS,
            MAX_OPERATOR_OUTPUTS,
            MAX_OPERATOR_WITNESSES,
            MAX_OPERATOR_FRONTIERS,
            MAX_OPERATOR_BYTES,
            MAX_OPERATOR_DECODED_BYTES,
            MAX_OPERATOR_CANDIDATES,
        )
        .is_ok()
    );
    assert_eq!(
        ResourceEnvelope::new(
            MAX_OPERATOR_INPUTS + 1,
            MAX_OPERATOR_OUTPUTS,
            MAX_OPERATOR_WITNESSES,
            MAX_OPERATOR_FRONTIERS,
            MAX_OPERATOR_BYTES,
            MAX_OPERATOR_DECODED_BYTES,
            MAX_OPERATOR_CANDIDATES,
        ),
        Err(Error::Limit)
    );

    let exact_witnesses = vec![ReadWitness::Epoch(basis(4)); MAX_OPERATOR_WITNESSES];
    assert!(WitnessSet::new(exact_witnesses.clone(), resources()).is_ok());
    let mut one_over_witnesses = exact_witnesses;
    one_over_witnesses.push(ReadWitness::Epoch(basis(4)));
    assert_eq!(
        WitnessSet::new(one_over_witnesses, resources()),
        Err(Error::Limit)
    );

    let exact_frontiers = (0..MAX_OPERATOR_FRONTIERS)
        .map(|index| FrontierRequirement {
            key: ProjectionKey {
                source: index as u64 + 1,
                ..key()
            },
            stage: FrontierStage::Captured,
            through: 1,
            closure: None,
        })
        .collect::<Vec<_>>();
    assert!(Knowledge::<()>::pending(exact_frontiers.clone(), 1).is_ok());
    let mut one_over_frontiers = exact_frontiers;
    one_over_frontiers.push(FrontierRequirement {
        key: ProjectionKey {
            source: 10_001,
            ..key()
        },
        stage: FrontierStage::Captured,
        through: 1,
        closure: None,
    });
    assert_eq!(
        Knowledge::<()>::pending(one_over_frontiers, 1),
        Err(Error::Limit)
    );

    assert!(CapturedInput::new(17, 29, vec![0; MAX_OPERATOR_BYTES]).is_ok());
    assert_eq!(
        CapturedInput::new(17, 29, vec![0; MAX_OPERATOR_BYTES + 1]),
        Err(Error::Limit)
    );
}
