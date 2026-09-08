//! Public-boundary tests for the bounded FA-057 full helper-input reference.
//!
//! These tests bind a caller-supplied frozen byte view to an opaque judgment.
//! They do not model a provider, socket, authenticated capture, authority, or
//! production helper boundary.

use fa_reference::Error;
use fa_reference::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, MAX_OMISSIONS, MAX_PROFILE_BYTES,
    MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS, Omission, OpaqueJudgment, PartKind, SubmittedPart,
};

fn profile(
    profile_id: u64,
    profile_bytes: Vec<u8>,
    tokenizer_epoch: u64,
    policy_epoch: u64,
    model_epoch: u64,
) -> InputProfileBinding {
    InputProfileBinding {
        profile_id,
        profile_bytes,
        tokenizer_epoch,
        policy_epoch,
        model_epoch,
    }
}

fn mixed_parts() -> Vec<SubmittedPart> {
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
            kind: PartKind::Evidence {
                source_id: 17,
                transform_id: 23,
            },
        },
        SubmittedPart {
            span: ByteSpan { start: 5, end: 6 },
            kind: PartKind::Delimiter,
        },
        SubmittedPart {
            span: ByteSpan { start: 6, end: 7 },
            kind: PartKind::Instruction,
        },
        SubmittedPart {
            span: ByteSpan { start: 7, end: 8 },
            kind: PartKind::Delimiter,
        },
        SubmittedPart {
            span: ByteSpan { start: 8, end: 9 },
            kind: PartKind::ToolSchema { schema_id: 29 },
        },
    ]
}

fn view(
    submitted_bytes: Vec<u8>,
    input_profile: InputProfileBinding,
    ordered_parts: Vec<SubmittedPart>,
    omissions: Vec<Omission>,
) -> ActualHelperInput {
    ActualHelperInput::new(submitted_bytes, input_profile, ordered_parts, omissions).unwrap()
}

fn baseline() -> ActualHelperInput {
    // Epoch zero is explicit and valid; no profile default can silently erase it.
    view(
        b"Q|P|U|I|S".to_vec(),
        profile(7, b"profile-v1".to_vec(), 0, 0, 0),
        mixed_parts(),
        vec![Omission::ClosedAbsent {
            domain_id: 41,
            trusted_closure_marker_id: 43,
        }],
    )
}

fn unomitted_baseline() -> ActualHelperInput {
    view(
        b"Q|P|U|I|S".to_vec(),
        profile(7, b"profile-v1".to_vec(), 0, 0, 0),
        mixed_parts(),
        vec![],
    )
}

fn assert_full_partition(input: &ActualHelperInput) {
    let mut next = 0;
    for part in input.ordered_parts() {
        assert_eq!(part.span.start, next);
        assert!(part.span.end > part.span.start);
        next = part.span.end;
    }
    assert_eq!(next, input.submitted_bytes().len());
}

#[test]
fn opaque_judgment_public_boundary_binds_every_submitted_byte_and_epoch() {
    let input = baseline();
    let judgment = OpaqueJudgment::capture(&input, b"the question alone was sufficient");

    assert!(judgment.valid_at(&input));
    assert_eq!(
        judgment,
        OpaqueJudgment::capture(&input, b"an explanation cannot narrow the view"),
    );
    assert_eq!(input.input_profile().tokenizer_epoch, 0);
    assert_eq!(input.input_profile().policy_epoch, 0);
    assert_eq!(input.input_profile().model_epoch, 0);

    // Every byte is changed through a new validated public view. This includes
    // the evidence/context byte `U`, even though the explanation ignores it.
    for byte_index in 0..input.submitted_bytes().len() {
        let mut changed_bytes = input.submitted_bytes().to_vec();
        changed_bytes[byte_index] = if changed_bytes[byte_index] == b'X' {
            b'Y'
        } else {
            b'X'
        };
        let changed = view(
            changed_bytes,
            input.input_profile().clone(),
            input.ordered_parts().to_vec(),
            input.omissions().to_vec(),
        );
        assert!(
            !judgment.valid_at(&changed),
            "accepted submitted byte mutation at {byte_index}"
        );
    }

    let mut tokenizer_changed = input.input_profile().clone();
    tokenizer_changed.tokenizer_epoch = 1;
    let tokenizer_changed = view(
        input.submitted_bytes().to_vec(),
        tokenizer_changed,
        input.ordered_parts().to_vec(),
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&tokenizer_changed));

    let mut policy_changed = input.input_profile().clone();
    policy_changed.policy_epoch = 1;
    let policy_changed = view(
        input.submitted_bytes().to_vec(),
        policy_changed,
        input.ordered_parts().to_vec(),
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&policy_changed));

    let mut model_changed = input.input_profile().clone();
    model_changed.model_epoch = 1;
    let model_changed = view(
        input.submitted_bytes().to_vec(),
        model_changed,
        input.ordered_parts().to_vec(),
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&model_changed));
}

#[test]
fn profile_part_transform_and_omission_metadata_are_full_view_dependencies() {
    let input = baseline();
    let judgment = OpaqueJudgment::capture(&input, b"only the first paragraph was considered");
    assert!(judgment.valid_at(&input));
    assert!(!judgment.valid_at(&unomitted_baseline()));

    let mut profile_id_changed = input.input_profile().clone();
    profile_id_changed.profile_id = 8;
    let profile_id_changed = view(
        input.submitted_bytes().to_vec(),
        profile_id_changed,
        input.ordered_parts().to_vec(),
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&profile_id_changed));

    let mut profile_bytes_changed = input.input_profile().clone();
    profile_bytes_changed.profile_bytes[0] ^= 1;
    let profile_bytes_changed = view(
        input.submitted_bytes().to_vec(),
        profile_bytes_changed,
        input.ordered_parts().to_vec(),
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&profile_bytes_changed));

    let mut transform_changed = input.ordered_parts().to_vec();
    transform_changed[4].kind = PartKind::Evidence {
        source_id: 17,
        transform_id: 24,
    };
    let transform_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        transform_changed,
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&transform_changed));

    let mut kind_changed = input.ordered_parts().to_vec();
    kind_changed[2].kind = PartKind::Instruction;
    let kind_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        kind_changed,
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&kind_changed));

    let mut source_changed = input.ordered_parts().to_vec();
    source_changed[4].kind = PartKind::Evidence {
        source_id: 18,
        transform_id: 23,
    };
    let source_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        source_changed,
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&source_changed));

    let mut schema_changed = input.ordered_parts().to_vec();
    schema_changed[8].kind = PartKind::ToolSchema { schema_id: 30 };
    let schema_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        schema_changed,
        input.omissions().to_vec(),
    );
    assert!(!judgment.valid_at(&schema_changed));

    let marker_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        input.ordered_parts().to_vec(),
        vec![Omission::ClosedAbsent {
            domain_id: 41,
            trusted_closure_marker_id: 44,
        }],
    );
    assert!(!judgment.valid_at(&marker_changed));

    let domain_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        input.ordered_parts().to_vec(),
        vec![Omission::ClosedAbsent {
            domain_id: 42,
            trusted_closure_marker_id: 43,
        }],
    );
    assert!(!judgment.valid_at(&domain_changed));

    let classification_changed = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        input.ordered_parts().to_vec(),
        vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 1,
        }],
    );
    assert!(!judgment.valid_at(&classification_changed));

    let gapped_input = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        input.ordered_parts().to_vec(),
        vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 1,
        }],
    );
    let gapped_judgment = OpaqueJudgment::capture(&gapped_input, b"omission position matters");
    assert!(gapped_judgment.valid_at(&gapped_input));
    let gapped_first_missing_changed = view(
        gapped_input.submitted_bytes().to_vec(),
        gapped_input.input_profile().clone(),
        gapped_input.ordered_parts().to_vec(),
        vec![Omission::Gapped {
            domain_id: 41,
            first_missing: 2,
        }],
    );
    assert!(!gapped_judgment.valid_at(&gapped_first_missing_changed));

    let redacted_input = view(
        input.submitted_bytes().to_vec(),
        input.input_profile().clone(),
        input.ordered_parts().to_vec(),
        vec![Omission::Redacted {
            domain_id: 41,
            transform_id: 61,
        }],
    );
    let redacted_judgment = OpaqueJudgment::capture(&redacted_input, b"redaction identity matters");
    assert!(redacted_judgment.valid_at(&redacted_input));
    let redaction_transform_changed = view(
        redacted_input.submitted_bytes().to_vec(),
        redacted_input.input_profile().clone(),
        redacted_input.ordered_parts().to_vec(),
        vec![Omission::Redacted {
            domain_id: 41,
            transform_id: 62,
        }],
    );
    assert!(!redacted_judgment.valid_at(&redaction_transform_changed));
}

#[test]
fn honestly_repartitioned_omitted_context_still_changes_the_frozen_view() {
    let input = unomitted_baseline();
    let judgment = OpaqueJudgment::capture(&input, b"the unmentioned context was not used");

    // Drop `U|` and rebuild the remaining byte partition rather than mutating
    // private fields or retaining spans that no longer describe the submission.
    let omitted_context = view(
        b"Q|P|I|S".to_vec(),
        input.input_profile().clone(),
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
                kind: PartKind::ToolSchema { schema_id: 29 },
            },
        ],
        vec![Omission::ClosedAbsent {
            // This newly declared omission is the only representation of the
            // removed `U` context in the modified helper view.
            domain_id: 41,
            trusted_closure_marker_id: 43,
        }],
    );
    assert_full_partition(&omitted_context);
    assert!(!judgment.valid_at(&omitted_context));
}

fn one_question_part(length: usize) -> Vec<SubmittedPart> {
    vec![SubmittedPart {
        span: ByteSpan {
            start: 0,
            end: length,
        },
        kind: PartKind::Question,
    }]
}

fn individual_parts(count: usize) -> Vec<SubmittedPart> {
    (0..count)
        .map(|index| SubmittedPart {
            span: ByteSpan {
                start: index,
                end: index + 1,
            },
            kind: if index == 0 {
                PartKind::Question
            } else {
                PartKind::Other
            },
        })
        .collect()
}

fn gapped_omissions(count: usize) -> Vec<Omission> {
    (0..count)
        .map(|index| Omission::Gapped {
            domain_id: index as u64 + 1,
            first_missing: index as u64 + 10,
        })
        .collect()
}

#[test]
fn every_public_full_input_bound_accepts_its_cap_and_refuses_one_over() {
    let zero_epochs = profile(1, vec![], 0, 0, 0);
    assert!(
        ActualHelperInput::new(
            vec![b'Q'; MAX_SUBMITTED_BYTES],
            zero_epochs.clone(),
            one_question_part(MAX_SUBMITTED_BYTES),
            vec![],
        )
        .is_ok()
    );
    assert_eq!(
        ActualHelperInput::new(
            vec![b'Q'; MAX_SUBMITTED_BYTES + 1],
            zero_epochs.clone(),
            one_question_part(MAX_SUBMITTED_BYTES + 1),
            vec![],
        ),
        Err(Error::Limit)
    );

    assert!(
        ActualHelperInput::new(
            b"Q".to_vec(),
            profile(1, vec![b'P'; MAX_PROFILE_BYTES], 0, 0, 0),
            one_question_part(1),
            vec![],
        )
        .is_ok()
    );
    assert_eq!(
        ActualHelperInput::new(
            b"Q".to_vec(),
            profile(1, vec![b'P'; MAX_PROFILE_BYTES + 1], 0, 0, 0),
            one_question_part(1),
            vec![],
        ),
        Err(Error::Limit)
    );

    assert!(
        ActualHelperInput::new(
            vec![b'Q'; MAX_SUBMITTED_PARTS],
            zero_epochs.clone(),
            individual_parts(MAX_SUBMITTED_PARTS),
            vec![],
        )
        .is_ok()
    );
    assert_eq!(
        ActualHelperInput::new(
            vec![b'Q'; MAX_SUBMITTED_PARTS + 1],
            zero_epochs.clone(),
            individual_parts(MAX_SUBMITTED_PARTS + 1),
            vec![],
        ),
        Err(Error::Limit)
    );

    assert!(
        ActualHelperInput::new(
            b"Q".to_vec(),
            zero_epochs.clone(),
            one_question_part(1),
            gapped_omissions(MAX_OMISSIONS),
        )
        .is_ok()
    );
    assert_eq!(
        ActualHelperInput::new(
            b"Q".to_vec(),
            zero_epochs,
            one_question_part(1),
            gapped_omissions(MAX_OMISSIONS + 1),
        ),
        Err(Error::Limit)
    );
}
