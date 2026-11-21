//! The original native decoder, source gate, policy corpus and two-key effect
//! survive recovery together. Synthetic weights exercise real inference and
//! persistence; they do not establish detector accuracy or deployment isolation.

#[path = "support/file_oversight_actor.rs"]
mod support;
#[path = "tower_generated_operator/fixture.rs"]
#[allow(dead_code)]
mod fixture;

use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_SAMPLING_ENTRIES,
    text::TextGenerationRequest,
    tokenizer::TokenizationBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        JournalError, Reconciliation, RecoveryReserve,
        governance::PolicyUpdate,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::text::FileTextGenerationCommand,
            governance::campaigns::{FilePolicyCampaignReview, FilePolicyGovernor},
            guarded::{
                FileCampaignRequirement, FileCredentialEpoch, FileGuardSet, FileRecoveryFloor,
                FileRecoveryRequirements, anchored::FileHistoryAnchor,
                generated_recovery::FileGuardedGeneratedSourceRecovery,
            },
            source::FileSourcePolicy,
            stream::generated::{
                FileTextMessageRequest,
                checked::{
                    GeneratedPublicationProfile,
                    source::{
                        FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile,
                        GeneratedSourcePublication,
                    },
                },
            },
        },
        requests::{FileRequestDisposition, actor::FileGeneratedTextActorPort},
    },
    publication_gate::PublicationLimits,
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_governance::CampaignDisposition,
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::witness::refinement::RefinementBudget;
use std::{fs, rc::Rc};
use support::{Directory, Supervisor};

const REQUEST: u64 = 701;

fn profile() -> FileOversightProfile {
    let mut profile = fixture::profile();
    profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    profile
}

fn stream() -> StreamProfile {
    StreamProfile::new(9, 1, 4, 64, 256).unwrap()
}

fn selection() -> GeneratedSourceProfile {
    GeneratedSourceProfile {
        source: FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 7,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(100).unwrap(),
        },
        publication: GeneratedSourcePublication::Stream {
            stream: stream(),
            reserve: Some(RecoveryReserve::terminal()),
        },
    }
}

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits {
            cases: 16,
            input_bytes: 1_048_576,
        },
        max_campaigns: 4,
    }
}

fn create(root: &Directory) -> (FileOversight, FileHumanReviewer, FilePolicyGovernor) {
    FileOversight::create_generated_text_stream_from_source_governed(
        root.store(), profile(), fixture::decoder(), fixture::tokenizer(), selection(), campaigns(),
    )
    .unwrap()
}

fn bytes(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

fn evidence(root: &Directory) -> FileEvidenceSource {
    let scope = profile().delivery.scope;
    let observed = EvidenceSnapshot::new(
        EvidenceIdentity { scope, source: 7, generation: 1 },
        support::snapshot(),
        support::oversight::MEMBERS
            .into_iter()
            .map(|member| (member.to_owned(), b"actual source context".to_vec()))
            .collect(),
    )
    .unwrap();
    let path = root.0.join("evidence.json");
    fs::write(&path, observed.encode()).unwrap();
    FileEvidenceSource::new(path, 7, scope, 1_048_576).unwrap()
}

fn request() -> TextGenerationRequest {
    TextGenerationRequest {
        prompt: b"ab".to_vec(),
        prefix_controls: vec![256],
        max_new_tokens: 2,
        stop_tokens: vec![256],
        tokenization: TokenizationBudget::default(),
        generation: GenerationBudget {
            scalar_products: MAX_DECODER_PRODUCTS,
            sampling_entries: MAX_SAMPLING_ENTRIES,
        },
        max_output_bytes: 4,
    }
}

fn generate(host: &mut FileOversight, source: &mut FileEvidenceSource) -> Rc<EvidenceSnapshot> {
    let captured = host.refresh_file_source(host.revision(), source, ElapsedTick(1)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        7, numerical.actor_revision, numerical.position, request(),
    )
    .unwrap();
    host.generate_decoder_text(host.revision(), command).unwrap();
    assert_eq!(host.decoder_text_progress(7).unwrap().bytes().unwrap(), b"A");
    captured
}

/// Exercise the original full-input review and independent human key. Omitting
/// publication deliberately leaves an admitted, charged Unknown liability.
fn release(
    host: &mut FileOversight,
    human: &FileHumanReviewer,
    old_human: Option<&FileHumanReviewer>,
    captured: &EvidenceSnapshot,
    id: u64,
    generation: u64,
    publish: bool,
) -> u64 {
    let snapshot = captured.snapshot().clone();
    let input = FileTextMessageRequest {
        request: id,
        generation,
        generation_revision: host.decoder_text_progress(generation).unwrap().generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    };
    let status = host.submit_decoder_text_message(host.revision(), input, snapshot.clone()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("actual native text must be admitted");
    };
    let action = host.request_action(id).unwrap().clone();
    let inputs = captured.inputs_for(&action, &profile().committee).unwrap();
    support::oversight::review_existing(host, attempt, id + 100, &inputs);
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot.clone()).unwrap();
    assert!(host.publish_checked(
        host.revision(), attempt, Some(&inputs), snapshot.clone(), ElapsedTick(1),
    ).is_err());
    let human_request = host.request_human_approval(
        host.revision(), id + 1000, attempt, &inputs, ElapsedTick(31),
    ).unwrap();
    let revision = host.revision();
    if let Some(old) = old_human {
        let before = host.inspect();
        assert_eq!(
            old.approve(host, revision, &human_request).err(),
            Some(JournalError::Contract(Error::Binding)),
        );
        assert_eq!(host.inspect(), before);
    }
    let human = human.approve(host, revision, &human_request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
    if publish {
        assert!(matches!(
            host.publish_checked(
                host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(1),
            ).unwrap().outcome,
            EndpointOutcome::Executed { .. },
        ));
        assert!(matches!(
            host.reconcile(host.revision(), attempt).unwrap(),
            Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
        ));
    } else {
        assert_eq!(
            host.reconcile(host.revision(), attempt).unwrap(),
            Reconciliation::AwaitingResolution,
        );
        assert_eq!(host.inspect().control.ledger.stages[&attempt], ActionState::Unknown);
    }
    attempt
}

fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(stream()),
            decoder: Some(fixture::decoder()),
            decoder_stop: None,
            source: Some(selection().source),
            identity: None,
            campaigns: Some(campaigns()),
            credential: None,
        },
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        },
    }
}

fn begin(
    root: &Directory,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<FileGuardedGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
        root.store(), profile(), expected, &fixture::decoder(), &fixture::tokenizer(),
        selection(), anchor,
    )
}

fn ready(
    root: &Directory,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<FileGuardedGeneratedSourceRecovery, JournalError> {
    let mut run = begin(root, expected, anchor)?;
    let canonical = bytes(root);
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.total_events, before.total_events);
        assert_eq!(bytes(root), canonical, "replay must not fence or change storage");
    }
    assert_eq!(run.progress().status, FileGeneratedSourceRecoveryStatus::Ready);
    assert_eq!(run.progress().replayed_events, run.progress().total_events);
    Ok(run)
}

fn campaign(host: &mut FileOversight, id: u64) -> FilePolicyCampaignReview {
    let control = host.inspect().control;
    let update = PolicyUpdate::new(
        id, control.sequence, control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(2048)]).unwrap(),
    ).unwrap();
    host.request_policy_campaign(host.revision(), &update).unwrap()
}

#[test]
fn fresh_governor_promotes_and_publishes_native_continuation_without_reviving_old_keys() {
    let root = Directory::new();
    let (mut host, old_human, old_governor) = create(&root);
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source);
    release(&mut host, &old_human, None, &captured, REQUEST, 7, true);
    let old_review = campaign(&mut host, 900);
    assert_eq!(old_review.report().cases().len(), 2);
    let revision = host.revision();
    let old_permit = old_governor.approve(&mut host, revision, &old_review, false).unwrap();
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let stream_before = host.stream_snapshot().unwrap();
    drop(host);
    // Recovery must reconstruct the historical capture, never re-read a live
    // source or treat that retained snapshot as eligible for new work.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (mut host, roles) = ready(&root, &expected, &anchor).unwrap().finish().unwrap();
    assert_eq!(host.revision(), before.revision + 1);
    assert!(host.inspect().control.ledger.epoch > before.control.ledger.epoch);
    assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
    assert_eq!(host.inspect().executions, before.executions);
    assert_eq!(host.inspect().payload, before.payload);
    assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream_before.confirmed);
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert!(host.capture_file_policy_state().is_err());
    assert!(roles.identity_observer.is_none());
    let governor = roles.policy_governor.expect("fresh original governor role");
    let revoked = host.policy_campaign(900).unwrap();
    assert_eq!(revoked.observed_disposition(), CampaignDisposition::Revoked);
    let fenced = host.inspect();
    let canonical = bytes(&root);
    assert_eq!(
        host.promote_policy_campaign(host.revision(), &old_permit),
        Err(JournalError::Contract(Error::Binding)),
    );
    let revision = host.revision();
    assert_eq!(
        old_governor.approve(&mut host, revision, &old_review, false).err(),
        Some(JournalError::Contract(Error::Binding)),
    );
    assert_eq!(
        old_governor.approve(&mut host, revision, &revoked, false).err(),
        Some(JournalError::Contract(Error::Binding)),
    );
    assert_eq!(
        governor.approve(&mut host, revision, &revoked, false).err(),
        Some(JournalError::Contract(Error::WrongState)),
    );
    assert_eq!(host.inspect(), fenced);
    assert_eq!(bytes(&root), canonical);

    let mut source = evidence(&root);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let review = campaign(&mut host, 901);
    assert_eq!(review.report().cases().len(), 2);
    let revision = host.revision();
    let permit = governor.approve(&mut host, revision, &review, false).unwrap();
    host.promote_policy_campaign(host.revision(), &permit).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
    let captured = host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let command = host.prepare_decoder_text_continuation(REQUEST, 8, request()).unwrap();
    host.begin_decoder_text_continuation(host.revision(), REQUEST, command).unwrap();
    while host.decoder_text_progress(8).unwrap().finish().is_none() {
        let revision = host.decoder_text_progress(8).unwrap().generation_revision();
        host.advance_decoder_text(host.revision(), 8, revision).unwrap();
    }
    assert_eq!(host.decoder_text_progress(8).unwrap().bytes().unwrap(), b"A");
    release(&mut host, &roles.human, Some(&old_human), &captured, REQUEST + 1, 8, true);
    assert_eq!(
        host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(),
        ["A", "A"],
    );
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 4);
    assert!(host.decoder_inspection().unwrap().numerical.position > numerical.position);
    assert!(host.inspect().control.ledger.charged > before.control.ledger.charged);
}

#[test]
fn complete_independent_requirements_fail_closed_and_latch_before_any_fence_or_cleanup() {
    let root = Directory::new();
    let (mut host, _, _) = create(&root);
    generate(&mut host, &mut evidence(&root));
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"unacknowledged staging").unwrap();
    for variation in 0..11 {
        let mut changed = expected.clone();
        match variation {
            0 => changed.guards.campaigns = None,
            1 => changed.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            2 => changed.guards.campaigns.as_mut().unwrap().limits.cases += 1,
            3 => changed.effective_policy = Policy::new(
                expected.effective_policy.generation(), vec![Predicate::PayloadAtMost(4095)],
            ).unwrap(),
            4 => changed.minimum.journal_revision += 1,
            5 => changed.minimum.control_sequence += 1,
            6 => changed.minimum.authority_epoch += 1,
            7 => changed.guards.source = None,
            8 => changed.guards.decoder = None,
            9 => changed.guards.stream = None,
            _ => changed.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }),
        }
        if let Ok(mut run) = begin(&root, &changed, &anchor) {
            loop {
                let progress = run.progress();
                if run.advance(progress.replayed_events, 1).is_err() {
                    break;
                }
                assert_ne!(
                    run.progress().status, FileGeneratedSourceRecoveryStatus::Ready,
                    "incomplete contract reached Ready: variation {variation}",
                );
            }
            assert!(matches!(run.progress().status, FileGeneratedSourceRecoveryStatus::Failed(_)));
            assert!(run.advance(run.progress().replayed_events, 1).is_err());
            assert!(run.stop_request(800).is_err());
            assert!(run.finish().is_err());
        }
        assert_eq!(bytes(&root), canonical, "variation {variation}");
        assert_eq!(fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let (host, roles) = ready(&root, &expected, &anchor).unwrap().finish().unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
    assert!(roles.policy_governor.is_some());
    assert!(!pending.exists());
}

#[test]
fn rollback_and_foreign_anchors_refuse_before_replay_and_preserve_staging() {
    let root = Directory::new();
    let (mut host, _, _) = create(&root);
    let bootstrap = bytes(&root);
    generate(&mut host, &mut evidence(&root));
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"keep staging").unwrap();
    fs::write(root.store().join("delivery.bin"), &bootstrap).unwrap();
    assert!(begin(&root, &expected, &anchor).is_err());
    assert_eq!(bytes(&root), bootstrap);
    assert_eq!(fs::read(&pending).unwrap(), b"keep staging");
    fs::write(root.store().join("delivery.bin"), &canonical).unwrap();
    let foreign_root = Directory::new();
    let (foreign, _, _) = create(&foreign_root);
    let foreign_anchor = foreign.history_anchor().unwrap();
    assert!(begin(&root, &expected, &foreign_anchor).is_err());
    assert_eq!(bytes(&root), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"keep staging");
    assert!(ready(&root, &expected, &anchor).unwrap().finish().is_ok());
}

#[test]
fn cancellation_and_incomplete_finalizers_never_issue_roles_or_modify_storage() {
    let root = Directory::new();
    let (mut host, _, _) = create(&root);
    generate(&mut host, &mut evidence(&root));
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"keep staging").unwrap();
    let mut run = begin(&root, &expected, &anchor).unwrap();
    assert_eq!(run.advance(1, 1), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(run.advance(0, 0), Err(JournalError::Contract(Error::InvalidInput)));
    assert_eq!(run.progress().attempted_events, 0);
    assert!(run.stop_request(800).is_err());
    assert_eq!(run.finish().err(), Some(JournalError::Contract(Error::Incomplete)));
    for phase in 0..3 {
        let mut run = begin(&root, &expected, &anchor).unwrap();
        if phase > 0 {
            run.advance(0, 1).unwrap();
        }
        if phase == 2 {
            while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
                run.advance(run.progress().replayed_events, 1).unwrap();
            }
        }
        assert_eq!(run.cancel().status, FileGeneratedSourceRecoveryStatus::Cancelled);
        assert_eq!(bytes(&root), canonical);
        assert_eq!(fs::read(&pending).unwrap(), b"keep staging");
    }
    assert!(ready(&root, &expected, &anchor).unwrap().finish().is_ok());
}

#[test]
fn changed_canonical_at_ready_refuses_both_guarded_finalizers_before_cleanup() {
    for stop in [false, true] {
        let root = Directory::new();
        let (mut host, _, _) = create(&root);
        generate(&mut host, &mut evidence(&root));
        let expected = requirements(&host);
        let anchor = host.history_anchor().unwrap();
        drop(host);
        let canonical = bytes(&root);
        let pending = root.store().join("delivery.pending");
        fs::write(&pending, b"keep staging").unwrap();
        let run = ready(&root, &expected, &anchor).unwrap();
        let request = run.stop_request(800).unwrap();
        let mut replacement = canonical.clone();
        replacement.push(0);
        fs::write(root.store().join("delivery.bin"), &replacement).unwrap();
        let error = if stop {
            run.finish_admission_stop(request).err()
        } else {
            run.finish().err()
        };
        assert_eq!(error, Some(JournalError::Contract(Error::Binding)));
        assert_eq!(bytes(&root), replacement);
        assert_eq!(fs::read(&pending).unwrap(), b"keep staging");
        fs::write(root.store().join("delivery.bin"), &canonical).unwrap();
        assert!(ready(&root, &expected, &anchor).unwrap().finish().is_ok());
    }
}

#[test]
fn guarded_ready_stop_preserves_unknown_liability_and_exact_retry_without_a_live_owner() {
    let root = Directory::new();
    let (mut host, human, _) = create(&root);
    let captured = generate(&mut host, &mut evidence(&root));
    let attempt = release(&mut host, &human, None, &captured, REQUEST, 7, false);
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    assert!(before.control.ledger.charged > 0);
    drop(host);
    let run = ready(&root, &expected, &anchor).unwrap();
    let request = run.stop_request(800).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2, "only the original Stop/Fence pair");
    assert_eq!(stopped.snapshot().revision, before.revision + 2);
    assert_eq!(stopped.snapshot().control.ledger.charged, before.control.ledger.charged);
    assert_eq!(stopped.snapshot().control.ledger.available, before.control.ledger.available);
    assert_eq!(stopped.snapshot().control.ledger.stages[&attempt], ActionState::Unknown);
    assert_eq!(stopped.snapshot().payload, before.payload);
    assert_eq!(stopped.snapshot().executions, 0);
    assert!(!stopped.progress().drained());
    let canonical = bytes(&root);
    let run = ready(&root, &expected, &anchor).unwrap();
    assert_eq!(run.stop_request(801), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(run.stop_request(800).unwrap(), request);
    let retried = run.finish_admission_stop(request).unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), stopped.snapshot());
    assert_eq!(retried.history_anchor(), stopped.history_anchor());
    assert_eq!(bytes(&root), canonical);
}

#[test]
fn exact_absence_recovers_no_governor_and_checked_publication_cannot_be_weakened() {
    let root = Directory::new();
    let (host, _) = FileOversight::create_generated_text_stream_from_source(
        root.store(), profile(), fixture::decoder(), fixture::tokenizer(), selection(),
    ).unwrap();
    let mut expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let canonical = bytes(&root);
    assert!(ready(&root, &expected, &anchor).is_err());
    assert_eq!(bytes(&root), canonical);
    expected.guards.campaigns = None;
    let (_, roles) = ready(&root, &expected, &anchor).unwrap().finish().unwrap();
    assert!(roles.policy_governor.is_none());
    assert!(roles.identity_observer.is_none());

    let root = Directory::new();
    let selected = GeneratedSourceProfile {
        publication: GeneratedSourcePublication::Checked(GeneratedPublicationProfile {
            stream: stream(),
            reserve: Some(RecoveryReserve::terminal()),
            limits: PublicationLimits {
                bindings: 4,
                validation: RefinementBudget { steps: 1000, value_bytes: 10000 },
            },
            feed: None,
        }),
        ..selection()
    };
    let (host, _, _) = FileOversight::create_generated_text_stream_from_source_governed(
        root.store(), profile(), fixture::decoder(), fixture::tokenizer(), selected, campaigns(),
    ).unwrap();
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"keep checked staging").unwrap();
    assert!(begin(&root, &expected, &anchor).is_err(), "witness guards cannot disappear");
    assert_eq!(bytes(&root), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"keep checked staging");
    let mut run = FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
        root.store(), profile(), &expected, &fixture::decoder(), &fixture::tokenizer(), selected,
        &anchor,
    ).unwrap();
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (host, roles) = run.finish().unwrap();
    assert!(roles.policy_governor.is_some());
    assert_eq!(host.publication_validation_profile().unwrap().unwrap().bindings, 4);
    assert!(!pending.exists());
}
