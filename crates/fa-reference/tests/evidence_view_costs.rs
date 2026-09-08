//! Descriptive in-memory costs for the bounded FA-081 evidence-view contract.
//!
//! These fixed cases assert their semantic outcome before emitting any timing
//! record. They are neither OS/process-cold, allocation/RSS, benchmark, SLO,
//! provider-authentication, nor helper-truth evidence. Source-window bytes are
//! declarative metadata, so they are deliberately not counted as submitted
//! logical input bytes.

use std::{hint::black_box, time::Instant};

use fa_reference::{
    Error,
    evidence_view::{
        AuthorizationProjection, EvidencePartView, EvidenceViewManifest, EvidenceViewWitness,
        OriginalIdentity, RedactionMetadata, WindowMetadata,
    },
    full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, MAX_SUBMITTED_BYTES, PartKind,
        SubmittedPart,
    },
};

const SAMPLES: usize = 12;
const PROFILE_BYTES: &[u8] = b"fa081-cost-profile-v1";

fn profile(policy_epoch: u64) -> InputProfileBinding {
    InputProfileBinding {
        profile_id: 91,
        profile_bytes: PROFILE_BYTES.to_vec(),
        tokenizer_epoch: 3,
        policy_epoch,
        model_epoch: 5,
    }
}

fn identity() -> OriginalIdentity {
    OriginalIdentity {
        tenant_id: 7,
        object_id: 41,
        generation: 2,
    }
}

fn actual_input(bytes: Vec<u8>, policy_epoch: u64) -> ActualHelperInput {
    assert!(
        bytes.len() >= 2,
        "fixed evidence-view input needs question and evidence bytes"
    );
    let end = bytes.len();
    ActualHelperInput::new(
        bytes,
        profile(policy_epoch),
        vec![
            SubmittedPart {
                span: ByteSpan { start: 0, end: 1 },
                kind: PartKind::Question,
            },
            SubmittedPart {
                span: ByteSpan { start: 1, end },
                kind: PartKind::Evidence {
                    source_id: 41,
                    transform_id: 13,
                },
            },
        ],
        vec![],
    )
    .expect("fixed complete input must remain valid")
}

fn manifest(
    bytes: Vec<u8>,
    policy_epoch: u64,
    window: WindowMetadata,
) -> Result<EvidenceViewManifest, Error> {
    EvidenceViewManifest::new(
        actual_input(bytes, policy_epoch),
        AuthorizationProjection {
            projection_id: 29,
            policy_epoch,
            projected_originals: vec![identity()],
        },
        vec![EvidencePartView {
            input_part_index: 1,
            original: identity(),
            transform_id: 13,
            redaction: RedactionMetadata::None,
            window,
        }],
    )
}

fn full_window(original_byte_len: u64) -> WindowMetadata {
    WindowMetadata {
        original_byte_len,
        window_start: 0,
        window_len: original_byte_len,
        truncated: false,
    }
}

fn logical_input_bytes(submitted_bytes: usize) -> usize {
    submitted_bytes + PROFILE_BYTES.len()
}

fn run_case(
    case: &str,
    timing_scope: &str,
    submitted_bytes: usize,
    expected_successes: usize,
    expected_refusals: usize,
    mut sample: impl FnMut() -> (usize, usize),
) {
    let mut timings = Vec::with_capacity(SAMPLES);
    let mut successes = 0;
    let mut refusals = 0;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let (sample_successes, sample_refusals) = sample();
        let elapsed = started.elapsed().as_nanos();
        successes += sample_successes;
        refusals += sample_refusals;
        timings.push(elapsed);
    }
    assert_eq!(
        successes,
        expected_successes * SAMPLES,
        "{case} dropped a success"
    );
    assert_eq!(
        refusals,
        expected_refusals * SAMPLES,
        "{case} dropped a refusal"
    );
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "case={case} samples={SAMPLES} successes={successes} refusals={refusals} observed_errors=0 submitted_bytes={submitted_bytes} profile_bytes={} logical_input_bytes={} elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} timing_scope={timing_scope} memory_measurement=unavailable_in_memory_profile",
        PROFILE_BYTES.len(),
        logical_input_bytes(submitted_bytes),
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
    );
}

#[test]
fn fixed_evidence_view_costs_are_descriptive_and_assert_all_outcomes() {
    run_case(
        "fresh_construct_capture_reuse",
        "fresh_manifest_construction_capture_reuse_assertions_drop",
        2,
        1,
        0,
        || {
            let manifest = manifest(b"QE".to_vec(), 17, full_window(8))
                .expect("fresh fixed manifest must construct");
            let witness = EvidenceViewWitness::capture(&manifest);
            assert!(witness.valid_at(&manifest));
            assert_eq!(witness.manifest().submitted_bytes(), b"QE");
            black_box(witness);
            (1, 0)
        },
    );

    let reuse_manifest = manifest(b"QE".to_vec(), 17, full_window(8))
        .expect("setup-excluded fixed manifest must construct");
    run_case(
        "setup_excluded_capture_reuse",
        "setup_excluded_manifest_capture_reuse_assertions_drop",
        2,
        1,
        0,
        || {
            let witness = EvidenceViewWitness::capture(&reuse_manifest);
            assert!(witness.valid_at(&reuse_manifest));
            black_box(witness);
            (1, 0)
        },
    );

    run_case(
        "fresh_policy_churn_invalidates_prior_witness",
        "fresh_baseline_capture_candidate_construction_validation_assertions_drop",
        2,
        1,
        0,
        || {
            let baseline =
                manifest(b"QE".to_vec(), 17, full_window(8)).expect("baseline must remain valid");
            let witness = EvidenceViewWitness::capture(&baseline);
            let churned = manifest(b"QE".to_vec(), 18, full_window(8))
                .expect("policy-churn candidate itself must remain valid");
            assert_eq!(churned.input_profile().policy_epoch, 18);
            assert_eq!(churned.authorization().policy_epoch, 18);
            assert!(
                !witness.valid_at(&churned),
                "a valid policy-churn candidate must not replay an old witness"
            );
            black_box(churned);
            (1, 0)
        },
    );

    run_case(
        "fresh_malformed_window_refusal",
        "fresh_actual_input_manifest_refusal_assertions_drop",
        2,
        0,
        1,
        || {
            assert_eq!(
                manifest(
                    b"QE".to_vec(),
                    17,
                    WindowMetadata {
                        original_byte_len: 8,
                        window_start: 2,
                        window_len: 7,
                        truncated: true,
                    },
                ),
                Err(Error::InvalidInput),
            );
            (0, 1)
        },
    );

    run_case(
        "fresh_max_submitted_bytes_construct_capture_reuse",
        "fresh_max_bound_manifest_construction_capture_reuse_assertions_drop",
        MAX_SUBMITTED_BYTES,
        1,
        0,
        || {
            let mut bytes = vec![b'E'; MAX_SUBMITTED_BYTES];
            bytes[0] = b'Q';
            let manifest = manifest(
                bytes,
                17,
                full_window(u64::try_from(MAX_SUBMITTED_BYTES).expect("usize must fit u64")),
            )
            .expect("exact submitted-byte bound must remain valid");
            let witness = EvidenceViewWitness::capture(&manifest);
            assert_eq!(manifest.submitted_bytes().len(), MAX_SUBMITTED_BYTES);
            assert!(witness.valid_at(&manifest));
            black_box(witness);
            (1, 0)
        },
    );
}

/// Select this exact test alone in a fresh release process for a one-sample
/// first-construction observation. A normal suite invocation is not such a
/// process and must not be described as OS/page-cache/CPU-cold evidence.
#[test]
fn first_evidence_view_construct_is_selectable_for_fresh_release_measurement() {
    let started = Instant::now();
    let manifest =
        manifest(b"QE".to_vec(), 17, full_window(8)).expect("fixed manifest must construct");
    let elapsed = started.elapsed().as_nanos();
    assert_eq!(manifest.submitted_bytes(), b"QE");
    assert_eq!(manifest.evidence_parts().len(), 1);
    println!(
        "case=evidence_view_first_construct samples=1 successes=1 refusals=0 observed_errors=0 submitted_bytes=2 profile_bytes={} logical_input_bytes={} elapsed_ns={elapsed} timing_scope=fresh_process_exact_manifest_construction_only_when_sole_selected_test memory_measurement=unavailable_in_memory_profile",
        PROFILE_BYTES.len(),
        logical_input_bytes(2),
    );
}
