//! Public-API boundary tests for the FA-081 evidence-view commitment.

use fa_reference::{
    Error,
    evidence_view::{
        AuthorizationProjection, EvidencePartView, EvidenceViewManifest, EvidenceViewWitness,
        OriginalIdentity, RedactionMetadata, WindowMetadata,
    },
    full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    },
};

fn identity(object_id: u64, generation: u64) -> OriginalIdentity {
    OriginalIdentity {
        tenant_id: 44,
        object_id,
        generation,
    }
}

fn profile(model_epoch: u64) -> InputProfileBinding {
    InputProfileBinding {
        profile_id: 17,
        profile_bytes: b"review-profile-v2\0".to_vec(),
        tokenizer_epoch: 3,
        policy_epoch: 29,
        model_epoch,
    }
}

fn actual_input(
    bytes: Vec<u8>,
    evidence_order: &[(u64, u64)],
    profile: InputProfileBinding,
    omissions: Vec<Omission>,
) -> ActualHelperInput {
    let mut ordered_parts = vec![SubmittedPart {
        span: ByteSpan { start: 0, end: 1 },
        kind: PartKind::Question,
    }];
    for (offset, (source_id, transform_id)) in evidence_order.iter().enumerate() {
        ordered_parts.push(SubmittedPart {
            span: ByteSpan {
                start: offset + 1,
                end: offset + 2,
            },
            kind: PartKind::Evidence {
                source_id: *source_id,
                transform_id: *transform_id,
            },
        });
    }
    ActualHelperInput::new(bytes, profile, ordered_parts, omissions).expect("valid complete input")
}

fn manifest(
    bytes: Vec<u8>,
    profile: InputProfileBinding,
    evidence_order: &[(u64, u64)],
    originals: &[OriginalIdentity],
    windows: &[WindowMetadata],
    redaction: RedactionMetadata,
    omissions: Vec<Omission>,
) -> EvidenceViewManifest {
    assert_eq!(evidence_order.len(), originals.len());
    assert_eq!(evidence_order.len(), windows.len());
    let bindings = evidence_order
        .iter()
        .zip(originals.iter().copied())
        .zip(windows.iter().copied())
        .enumerate()
        .map(
            |(offset, (((_, transform_id), original), window))| EvidencePartView {
                input_part_index: offset + 1,
                original,
                transform_id: *transform_id,
                redaction,
                window,
            },
        )
        .collect();

    let mut projected_originals = originals.to_vec();
    projected_originals.sort_unstable();
    projected_originals.dedup();
    EvidenceViewManifest::new(
        actual_input(bytes, evidence_order, profile.clone(), omissions),
        AuthorizationProjection {
            projection_id: 73,
            policy_epoch: profile.policy_epoch,
            projected_originals,
        },
        bindings,
    )
    .expect("valid evidence manifest")
}

fn windows() -> [WindowMetadata; 2] {
    [
        WindowMetadata {
            original_byte_len: 8,
            window_start: 0,
            window_len: 8,
            truncated: false,
        },
        WindowMetadata {
            original_byte_len: 24,
            window_start: 8,
            window_len: 8,
            truncated: true,
        },
    ]
}

#[test]
fn captured_view_exposes_exact_complete_bytes_profile_and_omissions() {
    let submitted = b"Q\0\xff".to_vec();
    let input_profile = profile(5);
    let omissions = vec![
        Omission::ClosedAbsent {
            domain_id: 7,
            trusted_closure_marker_id: 99,
        },
        Omission::Redacted {
            domain_id: 8,
            transform_id: 501,
        },
    ];
    let manifest = manifest(
        submitted.clone(),
        input_profile.clone(),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        omissions.clone(),
    );
    let witness = EvidenceViewWitness::capture(&manifest);

    assert!(witness.valid_at(&manifest));
    assert_eq!(manifest.submitted_bytes(), submitted);
    assert_eq!(manifest.input_profile(), &input_profile);
    assert_eq!(manifest.omissions(), omissions.as_slice());
    assert_eq!(manifest.actual_input().part_bytes(1), Ok(&b"\0"[..]));
    assert_eq!(manifest.actual_input().part_bytes(2), Ok(&b"\xff"[..]));
    assert!(
        manifest
            .evidence_parts()
            .iter()
            .all(|part| part.redaction == RedactionMetadata::None)
    );
    assert_eq!(witness.manifest(), &manifest);
}

#[test]
fn repeated_windows_are_bound_in_submitted_order_with_exact_projection_coverage() {
    let repeated = identity(300, 2);
    let repeated_view = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (300, 502)],
        &[repeated, repeated],
        &windows(),
        RedactionMetadata::None,
        vec![],
    );

    assert_eq!(
        repeated_view.authorization().projected_originals,
        vec![repeated]
    );
    assert_eq!(repeated_view.evidence_parts().len(), 2);
    assert_eq!(repeated_view.evidence_parts()[0].input_part_index, 1);
    assert_eq!(repeated_view.evidence_parts()[1].input_part_index, 2);
    assert_eq!(repeated_view.evidence_parts()[0].window.window_start, 0);
    assert_eq!(repeated_view.evidence_parts()[1].window.window_start, 8);
    assert!(!repeated_view.evidence_parts()[0].window.truncated);
    assert!(repeated_view.evidence_parts()[1].window.truncated);

    let prefix_only = [
        WindowMetadata {
            original_byte_len: 8,
            window_start: 0,
            window_len: 7,
            truncated: true,
        },
        windows()[1],
    ];
    let prefix_manifest = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (300, 502)],
        &[repeated, repeated],
        &prefix_only,
        RedactionMetadata::None,
        vec![],
    );
    assert!(prefix_manifest.evidence_parts()[0].window.truncated);
    assert_eq!(prefix_manifest.evidence_parts()[0].window.window_start, 0);
    assert_eq!(prefix_manifest.evidence_parts()[0].window.window_len, 7);

    let actual = repeated_view.actual_input().clone();
    let mut missing = repeated_view.authorization().clone();
    missing.projected_originals.clear();
    assert_eq!(
        EvidenceViewManifest::new(
            actual.clone(),
            missing,
            repeated_view.evidence_parts().to_vec()
        ),
        Err(Error::Binding)
    );
    let mut overbroad = repeated_view.authorization().clone();
    overbroad.projected_originals.push(identity(301, 4));
    assert_eq!(
        EvidenceViewManifest::new(actual, overbroad, repeated_view.evidence_parts().to_vec()),
        Err(Error::Binding)
    );
}

#[test]
fn raw_constructor_refuses_desynchronized_provenance_and_duplicate_projection() {
    let valid = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        vec![],
    );
    let actual = valid.actual_input().clone();
    let authorization = valid.authorization().clone();
    let bindings = valid.evidence_parts().to_vec();

    let mut generation_mismatch = bindings.clone();
    generation_mismatch[0].original.generation = 3;
    assert_eq!(
        EvidenceViewManifest::new(actual.clone(), authorization.clone(), generation_mismatch,),
        Err(Error::Binding)
    );

    let mut tenant_mismatch = bindings.clone();
    tenant_mismatch[0].original.tenant_id = 45;
    assert_eq!(
        EvidenceViewManifest::new(actual.clone(), authorization.clone(), tenant_mismatch),
        Err(Error::Binding)
    );

    let mut transform_mismatch = bindings.clone();
    transform_mismatch[0].transform_id = 503;
    assert_eq!(
        EvidenceViewManifest::new(actual.clone(), authorization.clone(), transform_mismatch),
        Err(Error::Binding)
    );

    let mut duplicate_projection = authorization;
    duplicate_projection
        .projected_originals
        .push(identity(301, 4));
    assert_eq!(
        EvidenceViewManifest::new(actual, duplicate_projection, bindings),
        Err(Error::InvalidInput)
    );
}

#[test]
fn whole_manifest_differences_do_not_replay_as_the_captured_view() {
    let baseline_omissions = vec![Omission::Gapped {
        domain_id: 7,
        first_missing: 11,
    }];
    let baseline = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        baseline_omissions.clone(),
    );
    let witness = EvidenceViewWitness::capture(&baseline);
    assert!(witness.valid_at(&baseline));

    let generation_changed = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 3), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        baseline_omissions.clone(),
    );
    let tenant_changed = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[
            OriginalIdentity {
                tenant_id: 45,
                ..identity(300, 2)
            },
            identity(301, 4),
        ],
        &windows(),
        RedactionMetadata::None,
        baseline_omissions.clone(),
    );
    for candidate in [&generation_changed, &tenant_changed] {
        assert_eq!(candidate.actual_input(), baseline.actual_input());
        assert_ne!(candidate.authorization(), baseline.authorization());
        assert!(!witness.valid_at(candidate));
    }

    let mut shifted_windows = windows();
    shifted_windows[1].window_start = 9;
    shifted_windows[1].window_len = 7;
    let candidates = vec![
        manifest(
            b"Qzb".to_vec(),
            profile(5),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::None,
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(6),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::None,
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(5),
            &[(300, 503), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::None,
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(5),
            &[(301, 502), (300, 501)],
            &[identity(301, 4), identity(300, 2)],
            &windows(),
            RedactionMetadata::None,
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(5),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::DeclaredTransform { transform_id: 501 },
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(5),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &shifted_windows,
            RedactionMetadata::None,
            baseline_omissions.clone(),
        ),
        manifest(
            b"Qab".to_vec(),
            profile(5),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::None,
            vec![Omission::Gapped {
                domain_id: 7,
                first_missing: 12,
            }],
        ),
    ];

    for candidate in candidates {
        assert!(
            !witness.valid_at(&candidate),
            "a one-family mutation replayed as the captured view"
        );
    }
}

#[test]
fn contradictory_truncation_flags_refuse_instead_of_creating_a_replay_candidate() {
    let valid = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        vec![],
    );
    let actual = valid.actual_input().clone();
    let authorization = valid.authorization().clone();
    let bindings = valid.evidence_parts().to_vec();
    assert!(
        EvidenceViewManifest::new(actual.clone(), authorization.clone(), bindings.clone()).is_ok()
    );

    let mut full_window_claims_truncation = bindings.clone();
    full_window_claims_truncation[0].window.truncated = true;
    assert_eq!(
        EvidenceViewManifest::new(
            actual.clone(),
            authorization.clone(),
            full_window_claims_truncation,
        ),
        Err(Error::InvalidInput)
    );

    let mut partial_window_claims_completeness = bindings;
    partial_window_claims_completeness[1].window.truncated = false;
    assert_eq!(
        EvidenceViewManifest::new(actual, authorization, partial_window_claims_completeness),
        Err(Error::InvalidInput)
    );
}

#[test]
fn unsupported_omission_is_a_valid_but_distinct_replay_candidate() {
    let baseline = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        vec![Omission::Gapped {
            domain_id: 7,
            first_missing: 11,
        }],
    );
    let witness = EvidenceViewWitness::capture(&baseline);
    let unsupported = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        vec![Omission::Unsupported { domain_id: 7 }],
    );

    assert!(matches!(
        unsupported.omissions(),
        [Omission::Unsupported { domain_id: 7 }]
    ));
    assert!(!witness.valid_at(&unsupported));
}

#[test]
fn every_remaining_profile_component_changes_a_valid_captured_view() {
    let baseline = manifest(
        b"Qab".to_vec(),
        profile(5),
        &[(300, 501), (301, 502)],
        &[identity(300, 2), identity(301, 4)],
        &windows(),
        RedactionMetadata::None,
        vec![],
    );
    let witness = EvidenceViewWitness::capture(&baseline);

    let mut changed_profile_id = profile(5);
    changed_profile_id.profile_id = 18;
    let mut changed_profile_bytes = profile(5);
    changed_profile_bytes.profile_bytes[0] ^= 1;
    let mut changed_tokenizer_epoch = profile(5);
    changed_tokenizer_epoch.tokenizer_epoch = 4;
    let mut changed_policy_epoch = profile(5);
    changed_policy_epoch.policy_epoch = 30;

    for changed_profile in [
        changed_profile_id,
        changed_profile_bytes,
        changed_tokenizer_epoch,
        changed_policy_epoch,
    ] {
        let candidate = manifest(
            b"Qab".to_vec(),
            changed_profile.clone(),
            &[(300, 501), (301, 502)],
            &[identity(300, 2), identity(301, 4)],
            &windows(),
            RedactionMetadata::None,
            vec![],
        );
        assert_eq!(
            candidate.authorization().policy_epoch,
            changed_profile.policy_epoch
        );
        assert_ne!(candidate.input_profile(), baseline.input_profile());
        assert!(!witness.valid_at(&candidate));
    }
}
