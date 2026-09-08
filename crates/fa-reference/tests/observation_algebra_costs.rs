//! Descriptive fixed-case costs for the FA-068 observation algebra.
//!
//! These pure reference transfers do not execute capture, verification,
//! decoding, helper inference, scheduling, authentication, or permit issuance.

use std::time::Instant;

use fa_reference::{
    Error,
    observation_algebra::{
        Basis, BasisIdentity, CancellationObligations, CapturedInput, ClaimClass, DecodeLaw,
        DecodeRequirement, EncodedObject, JoinedEvidence, Knowledge, LawDeclaration,
        MAX_OPERATOR_BYTES, MAX_OPERATOR_CANDIDATES, MAX_OPERATOR_DECODED_BYTES,
        MAX_OPERATOR_FRONTIERS, MAX_OPERATOR_INPUTS, MAX_OPERATOR_OUTPUTS, MAX_OPERATOR_WITNESSES,
        Obligation, ObligationKind, ObservationRequirement, OperatorContext, PrivacyRestrictions,
        ProjectionSpec, ProofStates, ReadWitness, RequirementClass, ResourceEnvelope, ScopedProof,
        Transfer, UncertaintyClass, authorize_projection, await_closed_scope, decode,
        join_evidence, probe, require_prefix, tap, verify_object,
    },
    product_frontier::{
        FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    },
    witness::{AdapterDomainInput, DomainClosure, DomainProjection, WitnessSnapshot},
};

const SAMPLES: usize = 4;
const SOURCE_ID: u64 = 211;
const SOURCE_GENERATION: u64 = 13;
const PROFILE_ID: u64 = 17;
const SEMANTIC_GENERATION: u64 = 19;
const CODEC_EPOCH: u64 = 23;
const CAPTURED_BYTES: &[u8] = b"tap-bytes";
const RETAIN_OBLIGATION: Obligation = Obligation {
    id: 29,
    kind: ObligationKind::RetainInput,
};

fn key() -> ProjectionKey {
    ProjectionKey {
        source: SOURCE_ID,
        branch: 31,
        projection: 37,
        source_epoch: SOURCE_GENERATION,
    }
}

fn basis(profile_id: u64, semantic_generation: u64) -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 41,
            question_id: 43,
            profile_id,
            semantic_generation,
            policy_epoch: 47,
            model_epoch: 53,
            tokenizer_epoch: 59,
            codec_epoch: CODEC_EPOCH,
            control_seq: 61,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: ScopedProof::Declared {
                scope_id: 67,
                generation: semantic_generation,
            },
            complete_for_contract: ScopedProof::Declared {
                scope_id: 71,
                generation: semantic_generation,
            },
            semantically_valid: ScopedProof::Declared {
                scope_id: 73,
                generation: semantic_generation,
            },
            available_for_replay: ScopedProof::Declared {
                scope_id: 79,
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
    .expect("fixed reference resource profile")
}

fn context(
    class: RequirementClass,
    uncertainty: UncertaintyClass,
    profile_id: u64,
    semantic_generation: u64,
    obligations: Vec<Obligation>,
) -> OperatorContext {
    OperatorContext::new(
        ObservationRequirement {
            class,
            uncertainty,
            basis: basis(profile_id, semantic_generation),
        },
        PrivacyRestrictions::new(vec![]).expect("empty privacy restriction set"),
        CancellationObligations::new(obligations).expect("fixed cancellation obligations"),
        resources(),
    )
}

fn prefix_frontiers() -> (ProductFrontiers, FrontierRequirement) {
    let mut frontiers = ProductFrontiers::new(1, 4).expect("fixed frontier bounds");
    frontiers
        .accept(key(), FrontierStage::Captured, 1)
        .expect("captured prefix evidence");
    (
        frontiers,
        FrontierRequirement {
            key: key(),
            stage: FrontierStage::Captured,
            through: 1,
            closure: None,
        },
    )
}

fn verified_control() -> Transfer<fa_reference::observation_algebra::VerifiedObject> {
    let captured = tap(
        CapturedInput::new(SOURCE_ID, SOURCE_GENERATION, CAPTURED_BYTES.to_vec())
            .expect("bounded fixed capture input"),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![RETAIN_OBLIGATION],
        ),
    )
    .expect("fixed capture");
    let projection = authorize_projection(
        &captured,
        ProjectionSpec {
            key: key(),
            projection_id: key().projection,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![],
        ),
    )
    .expect("fixed projection");
    let (frontiers, requirement) = prefix_frontiers();
    let prefix = require_prefix(
        &projection,
        &frontiers,
        requirement,
        context(
            RequirementClass::Prefix,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![],
        ),
    )
    .expect("fixed prefix");
    verify_object(
        &prefix,
        fa_reference::observation_algebra::ObjectDeclaration {
            object_id: SOURCE_ID,
            generation: SOURCE_GENERATION,
            profile_id: PROFILE_ID,
        },
        LawDeclaration::new(83, SOURCE_GENERATION, SEMANTIC_GENERATION, PROFILE_ID)
            .expect("fixed verifier law"),
        context(
            RequirementClass::ExactObject,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![],
        ),
    )
    .expect("fixed verified object")
}

fn full_chain() -> Result<(Transfer<JoinedEvidence>, u128), Error> {
    let input = CapturedInput::new(SOURCE_ID, SOURCE_GENERATION, CAPTURED_BYTES.to_vec())?;
    let projection_spec = ProjectionSpec {
        key: key(),
        projection_id: key().projection,
    };
    let (frontiers, prefix_requirement) = prefix_frontiers();
    let declaration = fa_reference::observation_algebra::ObjectDeclaration {
        object_id: SOURCE_ID,
        generation: SOURCE_GENERATION,
        profile_id: PROFILE_ID,
    };
    let verifier_law = LawDeclaration::new(83, SOURCE_GENERATION, SEMANTIC_GENERATION, PROFILE_ID)?;
    let decode_law = DecodeLaw::RegisteredLossless(LawDeclaration::new(
        89,
        CODEC_EPOCH,
        SEMANTIC_GENERATION,
        PROFILE_ID,
    )?);
    let encoded = EncodedObject {
        object_id: SOURCE_ID,
        bytes: b"encoded-chain".to_vec(),
        profile_id: PROFILE_ID,
        codec_epoch: CODEC_EPOCH,
    };
    let decoded_bytes = b"decoded-chain".to_vec();
    let capture_context = context(
        RequirementClass::Capture,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![RETAIN_OBLIGATION],
    );
    let projection_context = context(
        RequirementClass::Projection,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let prefix_context = context(
        RequirementClass::Prefix,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let verify_context = context(
        RequirementClass::ExactObject,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let decode_context = context(
        RequirementClass::ExactCheckpoint,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let probe_context = context(
        RequirementClass::Probe,
        UncertaintyClass::Bounded { bound_id: 97 },
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let join_context = context(
        RequirementClass::JoinedEvidence,
        UncertaintyClass::Bounded { bound_id: 97 },
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );

    let started = Instant::now();
    let captured = tap(input, capture_context)?;
    let projection = authorize_projection(&captured, projection_spec, projection_context)?;
    let prefix = require_prefix(&projection, &frontiers, prefix_requirement, prefix_context)?;
    let verified = verify_object(&prefix, declaration, verifier_law, verify_context)?;
    let decoded = decode(
        &verified,
        encoded,
        decoded_bytes,
        DecodeRequirement::ExactCheckpoint,
        decode_law,
        decode_context,
    )?;
    let positive = probe(&decoded, 101, 5, 1, 97, probe_context.clone())?;
    let negative = probe(&decoded, 103, -5, 1, 97, probe_context)?;
    let joined = join_evidence(&[positive, negative], join_context)?;
    Ok((joined, started.elapsed().as_nanos()))
}

fn logical_retained_bytes(entries: &[ReadWitness]) -> usize {
    entries
        .iter()
        .map(|entry| match entry {
            ReadWitness::ExactValue { value, .. }
            | ReadWitness::EncodedBytes { bytes: value, .. } => value.len(),
            ReadWitness::CapturedBytes { bytes, .. } | ReadWitness::DecodedBytes { bytes, .. } => {
                bytes.len()
            }
            ReadWitness::FullInput(input) => {
                input.submitted_bytes().len() + input.input_profile().profile_bytes.len()
            }
            ReadWitness::ExactObject { .. }
            | ReadWitness::AbsentKey { .. }
            | ReadWitness::WitnessSnapshot { .. }
            | ReadWitness::ClosedDomain(_)
            | ReadWitness::Derived { .. }
            | ReadWitness::Epoch(_)
            | ReadWitness::Frontier(_) => 0,
        })
        .sum()
}

#[test]
fn fixed_public_operator_chain_costs_retain_exact_obligations_and_bytes() {
    let mut timings = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let (joined, elapsed) = full_chain().expect("fixed public operator chain");
        timings.push(elapsed);
        match joined.value() {
            Knowledge::Known { value, .. } => assert_eq!(value.member_count(), 2),
            value => panic!("joined probes must retain known evidence, got {value:?}"),
        }
        assert_eq!(joined.frontiers().len(), 1);
        assert_eq!(joined.frontiers()[0].stage, FrontierStage::Captured);
        assert_eq!(
            joined
                .cancellation()
                .entries()
                .iter()
                .filter(|obligation| **obligation == RETAIN_OBLIGATION)
                .count(),
            1,
            "identical retained obligation must survive the join exactly once"
        );
        assert_eq!(
            logical_retained_bytes(joined.witnesses().entries()),
            2 * (CAPTURED_BYTES.len() + b"encoded-chain".len() + b"decoded-chain".len()),
            "logical retained bytes are the actual byte-bearing public witnesses"
        );
    }
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "FA068_OBSERVATION_ALGEBRA case=tap_projection_prefix_verify_decode_probe_join samples={SAMPLES} successes={SAMPLES} refusals=0 observed_errors=0 logical_retained_bytes={} elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} timing_scope=public_operator_chain_only_input_frontier_context_law_setup_and_output_assertions_excluded memory_measurement=unavailable_reference_profile no_claim=capture_verification_decode_helper_or_runtime_execution",
        2 * (CAPTURED_BYTES.len() + b"encoded-chain".len() + b"decoded-chain".len()),
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
    );
}

#[test]
fn changed_profile_or_generation_refuses_without_consuming_the_control_transfer() {
    let captured = tap(
        CapturedInput::new(SOURCE_ID, SOURCE_GENERATION, CAPTURED_BYTES.to_vec()).unwrap(),
        context(
            RequirementClass::Capture,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![RETAIN_OBLIGATION],
        ),
    )
    .unwrap();
    let accepted_projection = authorize_projection(
        &captured,
        ProjectionSpec {
            key: key(),
            projection_id: key().projection,
        },
        context(
            RequirementClass::Projection,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![],
        ),
    )
    .expect("same captured fixture accepts its declared profile");
    assert!(matches!(
        accepted_projection.value(),
        Knowledge::Known { .. }
    ));
    let changed_profile_spec = ProjectionSpec {
        key: key(),
        projection_id: key().projection,
    };
    let changed_profile_context = context(
        RequirementClass::Projection,
        UncertaintyClass::Exact,
        PROFILE_ID + 1,
        SEMANTIC_GENERATION,
        vec![],
    );
    let started = Instant::now();
    let changed_profile =
        authorize_projection(&captured, changed_profile_spec, changed_profile_context);
    let profile_elapsed = started.elapsed().as_nanos();
    assert_eq!(changed_profile, Err(Error::Binding));
    assert_eq!(captured.cancellation().entries(), &[RETAIN_OBLIGATION]);
    println!(
        "FA068_OBSERVATION_ALGEBRA case=changed_profile_refusal samples=1 successes=0 refusals=1 observed_errors=1 elapsed_ns={profile_elapsed} timing_scope=authorize_projection_only_same_fixture_valid_control_and_negative_context_setup_and_assertions_excluded memory_measurement=unavailable_reference_profile no_claim=policy_or_runtime_execution"
    );

    let verified = verified_control();
    let encoded = EncodedObject {
        object_id: SOURCE_ID,
        bytes: b"encoded".to_vec(),
        profile_id: PROFILE_ID,
        codec_epoch: CODEC_EPOCH,
    };
    let decoded_bytes = b"decoded".to_vec();
    let accepted_decode = decode(
        &verified,
        encoded.clone(),
        decoded_bytes.clone(),
        DecodeRequirement::ExactCheckpoint,
        DecodeLaw::RegisteredLossless(
            LawDeclaration::new(107, CODEC_EPOCH, SEMANTIC_GENERATION, PROFILE_ID).unwrap(),
        ),
        context(
            RequirementClass::ExactCheckpoint,
            UncertaintyClass::Exact,
            PROFILE_ID,
            SEMANTIC_GENERATION,
            vec![],
        ),
    )
    .expect("same decode fixture accepts its declared semantic generation");
    match accepted_decode.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), decoded_bytes.as_slice()),
        value => panic!("same decode fixture must remain known, got {value:?}"),
    }
    let changed_generation_law = DecodeLaw::RegisteredLossless(
        LawDeclaration::new(107, CODEC_EPOCH, SEMANTIC_GENERATION + 1, PROFILE_ID).unwrap(),
    );
    let changed_generation_context = context(
        RequirementClass::ExactCheckpoint,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let started = Instant::now();
    let changed_generation = decode(
        &verified,
        encoded,
        decoded_bytes,
        DecodeRequirement::ExactCheckpoint,
        changed_generation_law,
        changed_generation_context,
    );
    let generation_elapsed = started.elapsed().as_nanos();
    assert_eq!(changed_generation, Err(Error::Binding));
    assert!(matches!(verified.value(), Knowledge::Known { .. }));
    assert_eq!(verified.cancellation().entries(), &[RETAIN_OBLIGATION]);
    println!(
        "FA068_OBSERVATION_ALGEBRA case=changed_generation_refusal samples=1 successes=0 refusals=1 observed_errors=1 elapsed_ns={generation_elapsed} timing_scope=decode_only_same_fixture_valid_control_and_negative_law_context_setup_and_assertions_excluded memory_measurement=unavailable_reference_profile no_claim=decoder_or_runtime_execution"
    );
}

#[test]
fn near_decode_limit_and_failed_closed_scope_keep_exact_outcomes_and_obligations() {
    let verified = verified_control();
    let near_limit = vec![0xA5; MAX_OPERATOR_DECODED_BYTES - 1];
    let near_limit_encoded = EncodedObject {
        object_id: SOURCE_ID,
        bytes: b"x".to_vec(),
        profile_id: PROFILE_ID,
        codec_epoch: CODEC_EPOCH,
    };
    let near_limit_law = DecodeLaw::RegisteredLossless(
        LawDeclaration::new(109, CODEC_EPOCH, SEMANTIC_GENERATION, PROFILE_ID).unwrap(),
    );
    let near_limit_context = context(
        RequirementClass::ExactCheckpoint,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![],
    );
    let started = Instant::now();
    let decoded_result = decode(
        &verified,
        near_limit_encoded,
        near_limit,
        DecodeRequirement::ExactCheckpoint,
        near_limit_law,
        near_limit_context,
    );
    let elapsed = started.elapsed().as_nanos();
    let decoded = decoded_result.expect("one byte below decoded limit must remain accepted");
    match decoded.value() {
        Knowledge::Known { value, .. } => {
            assert_eq!(value.bytes().len(), MAX_OPERATOR_DECODED_BYTES - 1)
        }
        value => panic!("near-limit decode must remain known, got {value:?}"),
    }
    assert_eq!(
        logical_retained_bytes(decoded.witnesses().entries()),
        CAPTURED_BYTES.len() + MAX_OPERATOR_DECODED_BYTES
    );
    assert_eq!(
        decode(
            &verified,
            EncodedObject {
                object_id: SOURCE_ID,
                bytes: b"x".to_vec(),
                profile_id: PROFILE_ID,
                codec_epoch: CODEC_EPOCH,
            },
            vec![0xA5; MAX_OPERATOR_DECODED_BYTES + 1],
            DecodeRequirement::ExactCheckpoint,
            DecodeLaw::RegisteredLossless(
                LawDeclaration::new(109, CODEC_EPOCH, SEMANTIC_GENERATION, PROFILE_ID).unwrap(),
            ),
            context(
                RequirementClass::ExactCheckpoint,
                UncertaintyClass::Exact,
                PROFILE_ID,
                SEMANTIC_GENERATION,
                vec![],
            ),
        ),
        Err(Error::Limit),
        "one byte over the public decoded cap must refuse before a result exists"
    );
    assert_eq!(verified.cancellation().entries(), &[RETAIN_OBLIGATION]);
    println!(
        "FA068_OBSERVATION_ALGEBRA case=near_decoded_byte_limit samples=1 successes=1 refusals=0 observed_errors=0 logical_retained_bytes={} elapsed_ns={elapsed} timing_scope=decode_only_verified_input_encoded_object_law_context_setup_and_output_assertions_excluded memory_measurement=unavailable_reference_profile no_claim=decoder_or_runtime_execution",
        logical_retained_bytes(decoded.witnesses().entries()),
    );

    let marker = TrustedClosingMarker {
        key: key(),
        final_sequence: 1,
        marker_generation: 113,
    };
    let snapshot = WitnessSnapshot::new(
        127,
        61,
        SEMANTIC_GENERATION,
        AdapterDomainInput::new(
            DomainProjection::new(131, 137, key()),
            DomainClosure::Closed(marker),
        ),
        vec![],
    )
    .unwrap();
    let unsatisfied_frontiers = ProductFrontiers::new(1, 4).unwrap();
    let requirement = FrontierRequirement {
        key: key(),
        stage: FrontierStage::Authenticated,
        through: 1,
        closure: Some(marker.marker_generation),
    };
    let closure_context = context(
        RequirementClass::ClosedScope,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![Obligation {
            id: 139,
            kind: ObligationKind::PreserveFrontier,
        }],
    );
    let started = Instant::now();
    let pending_result = await_closed_scope(
        &snapshot,
        &unsatisfied_frontiers,
        requirement,
        closure_context,
    );
    let closure_elapsed = started.elapsed().as_nanos();
    let pending =
        pending_result.expect("failed closure is represented as pending, not invented absence");
    assert!(matches!(
        pending.value(),
        Knowledge::Pending {
            frontiers,
            expected_cost: 1
        } if frontiers == &vec![requirement]
    ));
    assert_eq!(pending.frontiers(), &[requirement]);
    assert!(
        pending
            .cancellation()
            .entries()
            .iter()
            .any(|entry| entry.id == 139 && entry.kind == ObligationKind::PreserveFrontier)
    );
    assert!(
        pending
            .witnesses()
            .entries()
            .iter()
            .any(|entry| matches!(entry, ReadWitness::Frontier(found) if *found == requirement))
    );
    println!(
        "FA068_OBSERVATION_ALGEBRA case=unsatisfied_closed_scope samples=1 known=0 pending=1 observed_errors=0 elapsed_ns={closure_elapsed} timing_scope=await_closed_scope_only_snapshot_frontier_requirement_context_setup_and_output_assertions_excluded memory_measurement=unavailable_reference_profile no_claim=closure_cost_or_runtime_execution"
    );
}

/// Select this exact test alone in a fresh release process for a one-sample
/// first-tap observation. Context and input construction are outside timing;
/// no `tap` invocation occurs before the timed call. A normal suite invocation
/// is not OS/page-cache/CPU-cold evidence.
#[test]
fn first_tap_is_selectable_for_fresh_process_measurement() {
    let input = CapturedInput::new(SOURCE_ID, SOURCE_GENERATION, CAPTURED_BYTES.to_vec()).unwrap();
    let context = context(
        RequirementClass::Capture,
        UncertaintyClass::Exact,
        PROFILE_ID,
        SEMANTIC_GENERATION,
        vec![RETAIN_OBLIGATION],
    );
    let started = Instant::now();
    let captured = tap(input, context);
    let elapsed = started.elapsed().as_nanos();
    let captured = captured.expect("first fixed tap must succeed");
    match captured.value() {
        Knowledge::Known { value, .. } => assert_eq!(value.bytes(), CAPTURED_BYTES),
        value => panic!("first tap must remain known, got {value:?}"),
    }
    assert_eq!(captured.cancellation().entries(), &[RETAIN_OBLIGATION]);
    assert_eq!(
        logical_retained_bytes(captured.witnesses().entries()),
        CAPTURED_BYTES.len()
    );
    println!(
        "FA068_OBSERVATION_ALGEBRA case=first_tap samples=1 successes=1 refusals=0 observed_errors=0 logical_retained_bytes={} elapsed_ns={elapsed} timing_scope=fresh_process_exact_release_invocation_only_tap_only_input_context_setup_and_output_assertions_excluded_no_tap_preflight memory_measurement=unavailable_reference_profile no_claim=os_page_cache_cpu_cold_capture_or_runtime_execution",
        CAPTURED_BYTES.len(),
    );
}
