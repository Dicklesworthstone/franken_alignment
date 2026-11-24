//! Original native inference, replay and Store barriers; synthetic model weights.
use super::*;
use crate::Snapshot;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::delivery::persistent::{JournalIo, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{
    BaseEvent, Event, Machine, journal, storage,
    credibility::{FileIndependentEvaluator, andon::FileAndonConfig},
    decoder::text::FileTextGenerationCommand,
    guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements},
    source::FileSourcePolicy,
    stream::generated::checked::{
        GeneratedPublicationProfile, source::evaluation::FileGeneratedEvaluationConfig,
    },
};
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{
    credibility::Fraction,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    joint_credibility::{JointPromotionPolicy, JointReplayBudget},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use crate::round::review::andon::{AndonBudget, LedgerIdentity};
use crate::witness::refinement::RefinementBudget;
use std::collections::BTreeMap;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"
    ));
    pub(super) fn stopped_config() -> FileDecoderConfig {
        configured(3.0, 65, Some(256))
    }
}
use fixtures::{Directory, command, request, tokenizer};

fn profile() -> FileOversightProfile {
    let mut profile = fixtures::host_profile();
    profile.delivery.initial_payload.clear();
    profile
}

fn selection(checked: bool) -> GeneratedSourceProfile {
    let stream = StreamProfile::new(9, 1, 4, 64, 256).unwrap();
    let reserve = Some(RecoveryReserve::terminal());
    GeneratedSourceProfile {
        source: FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits {
                events: 64,
                retained_bytes: 65536,
            },
            freshness: StateFreshness::new(50).unwrap(),
        },
        publication: if checked {
            GeneratedSourcePublication::Checked(GeneratedPublicationProfile {
                stream,
                reserve,
                limits: PublicationLimits {
                    bindings: 4,
                    validation: RefinementBudget {
                        steps: 1000,
                        value_bytes: 10000,
                    },
                },
                feed: None,
            })
        } else {
            GeneratedSourcePublication::Stream { stream, reserve }
        },
    }
}

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol {
        domain: 1,
        stratum: 2,
        period: 3,
        minimum_violation_origins: 1,
        minimum_benign_origins: 1,
        precision_floor: Fraction {
            numerator: 1,
            denominator: 2,
        },
        recall_floor: Fraction {
            numerator: 1,
            denominator: 2,
        },
        false_positive_ceiling: Fraction {
            numerator: 1,
            denominator: 2,
        },
        false_stop_budget: 10,
    }
}

fn andon() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 5,
            false_stops_allowed: 2,
            cost_per_stop: 3,
        }],
    )
    .unwrap()
}

fn joint() -> JointPromotionPolicy {
    JointPromotionPolicy::new(
        50,
        1,
        Fraction {
            numerator: 0,
            denominator: 1,
        },
        Fraction {
            numerator: 1,
            denominator: 1,
        },
        JointReplayBudget::default(),
    )
    .unwrap()
}

fn evidence(root: &Directory) -> FileEvidenceSource {
    let path = root.store().parent().unwrap().join("evidence.json");
    let scope = profile().delivery.scope;
    let snapshot = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::new(),
        },
        BTreeMap::from([("reviewer".into(), b"actual file context".to_vec())]),
    )
    .unwrap();
    std::fs::write(&path, snapshot.encode()).unwrap();
    FileEvidenceSource::new(path, 51, scope, 1048576).unwrap()
}

fn seeded(
    root: &Directory,
    checked: bool,
    jointly_evaluated: bool,
) -> (FileOversight, FileIndependentEvaluator) {
    let mut evaluation = FileGeneratedEvaluationConfig::new(protocol())
        .with_andon(andon())
        .unwrap();
    if jointly_evaluated {
        evaluation = evaluation.with_joint(joint()).unwrap();
    }
    let (mut host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
        root.store(),
        profile(),
        fixtures::stopped_config(),
        tokenizer(false),
        selection(checked),
        evaluation,
    )
    .unwrap();
    host.register_andon_window(
        host.revision(),
        AndonBudget {
            window: 2,
            start: 5,
            end: 100,
            false_stops_allowed: 4,
            cost_per_stop: 3,
        },
    )
    .unwrap();
    host.refresh_file_source(host.revision(), &mut evidence(root), ElapsedTick(1))
        .unwrap();
    let command = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), command).unwrap();
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    (host, roles.evaluator)
}

fn requirements(
    host: &FileOversight,
    checked: bool,
    jointly_evaluated: bool,
) -> FileAndonRecoveryRequirements {
    let selected = selection(checked);
    let stream = match selected.publication {
        GeneratedSourcePublication::Stream { stream, .. } => stream,
        GeneratedSourcePublication::Checked(p) => p.stream,
        GeneratedSourcePublication::Joint { .. } => unreachable!(),
    };
    let control = host.inspect().control;
    let mut expected = FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(stream),
                decoder: Some(fixtures::stopped_config()),
                decoder_stop: None,
                source: Some(selected.source),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: host.current_policy().unwrap().clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: host.revision(),
                control_sequence: control.sequence,
                authority_epoch: control.ledger.epoch,
            },
        },
        andon(),
        host.andon_ledger()
            .unwrap()
            .windows()
            .values()
            .map(|w| w.budget)
            .collect(),
        host.andon_source().unwrap().revision(),
    )
    .unwrap();
    if let GeneratedSourcePublication::Checked(p) = selected.publication {
        expected = expected.require_publication(p.limits).unwrap();
    }
    if jointly_evaluated {
        expected = expected.require_joint(joint()).unwrap();
    }
    expected
}

fn begin(
    root: &Directory,
    checked: bool,
    expected: &FileAndonRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<FileAndonGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
        root.store(),
        profile(),
        expected,
        &fixtures::stopped_config(),
        &tokenizer(false),
        selection(checked),
        &protocol(),
        anchor,
    )
}

fn ready(run: &mut FileAndonGeneratedSourceRecovery) {
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1).unwrap();
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(run.progress().status, FileGeneratedSourceRecoveryStatus::Ready);
}

fn bytes(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join(storage::CANONICAL)).unwrap()
}

#[test]
fn complete_native_replay_preserves_numerical_spend_and_reissues_separate_custody() {
    for checked in [false, true] {
        for jointly_evaluated in [false, true] {
            let root = Directory::new();
            let (host, old_evaluator) = seeded(&root, checked, jointly_evaluated);
            let expected = requirements(&host, checked, jointly_evaluated);
            let anchor = host.history_anchor().unwrap();
            let numerical = host.decoder_inspection().unwrap().numerical;
            let original = bytes(&root);
            let report = host.credibility_report().unwrap();
            let accounting = host.andon_ledger().unwrap().clone();
            let capacity = host.journal_capacity().unwrap();
            let mut oracle = Machine::replay(&profile(), &host.events).unwrap();
            oracle.apply(&Event::Core(BaseEvent::Fence)).unwrap();
            let oracle_cut = oracle.snapshot(host.revision() as usize + 1);
            drop(host);
            let mut run = begin(&root, checked, &expected, &anchor).unwrap();
            assert_eq!(run.progress().attempted_events, 0);
            assert!(matches!(
                storage::Store::open(&root.store()),
                Err(JournalError::Busy)
            ));
            while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
                run.advance(run.progress().replayed_events, 1).unwrap();
                assert_eq!(bytes(&root), original);
            }
            let (mut host, roles) = run.finish().unwrap();
            assert_eq!(host.inspect(), oracle_cut);
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            assert_eq!(
                host.decoder_text_progress(7).unwrap().generation_revision(),
                1
            );
            assert_eq!(host.credibility_report().unwrap(), report);
            assert_eq!(host.andon_ledger().unwrap(), &accounting);
            assert_eq!(host.journal_capacity().unwrap().reserve(), capacity.reserve());
            assert_eq!(
                host.journal_capacity().unwrap().remaining().events + 1,
                capacity.remaining().events
            );
            assert!(!host.clock_ready());
            assert!(host.capture_file_policy_state().is_err());
            assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
            assert_eq!(old_evaluator.check_owner(&host), Err(Error::Binding.into()));
            let fenced = bytes(&root);
            assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
            assert_eq!(bytes(&root), fenced);
            host.refresh_file_source(host.revision(), &mut evidence(&root), ElapsedTick(2))
                .unwrap();
            host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position)
                .unwrap();
            for revision in 1..4 {
                host.advance_decoder_text(host.revision(), 7, revision)
                    .unwrap();
            }
            let progress = host.decoder_text_progress(7).unwrap();
            assert_eq!(progress.bytes().unwrap(), b"A");
            assert_eq!(progress.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn cancellation_at_every_boundary_retains_canonical_and_pending_bytes() {
    let root = Directory::new();
    let (host, _) = seeded(&root, false, false);
    let expected = requirements(&host, false, false);
    let anchor = host.history_anchor().unwrap();
    let total = host.revision() as usize;
    let original = bytes(&root);
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged-original-staging").unwrap();
    for boundary in 0..=total {
        let mut run = begin(&root, false, &expected, &anchor).unwrap();
        for event in 0..boundary {
            run.advance(event, 1).unwrap();
        }
        let progress = run.cancel();
        assert_eq!(progress.status, FileGeneratedSourceRecoveryStatus::Cancelled);
        assert_eq!(progress.replayed_events, boundary);
        assert_eq!(bytes(&root), original);
        assert_eq!(
            std::fs::read(&pending).unwrap(),
            b"unacknowledged-original-staging"
        );
        drop(storage::Store::open(&root.store()).unwrap());
    }
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    let (host, _) = run.finish().unwrap();
    assert_eq!(host.revision(), total as u64 + 1);
    assert!(!pending.exists());
}

#[test]
fn protocol_inventory_joint_and_publication_mismatches_refuse_before_replay() {
    let root = Directory::new();
    let (host, _) = seeded(&root, true, true);
    let expected = requirements(&host, true, true);
    let anchor = host.history_anchor().unwrap();
    let original = bytes(&root);
    drop(host);
    for which in 0..5 {
        let mut bad_protocol = protocol();
        let mut config = andon();
        let mut windows = expected.windows().to_vec();
        let mut base = expected.base().clone();
        match which {
            0 => bad_protocol.domain += 1,
            1 => {
                config = FileAndonConfig::new(
                    LedgerIdentity {
                        deployment: 101,
                        evaluator: 203,
                    },
                    config.windows().to_vec(),
                )
                .unwrap();
            }
            2 => windows.truncate(1),
            3 => base.guards.source = None,
            _ => {}
        }
        let mut bad = FileAndonRecoveryRequirements::new(
            base,
            config,
            windows,
            expected.minimum_evaluation_revision(),
        )
        .unwrap();
        bad = bad.require_joint(joint()).unwrap();
        if which != 4 {
            bad = bad.require_publication(expected.publication().unwrap()).unwrap();
        }
        assert!(
            matches!(
                FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
                    root.store(),
                    profile(),
                    &bad,
                    &fixtures::stopped_config(),
                    &tokenizer(false),
                    selection(true),
                    &bad_protocol,
                    &anchor,
                ),
                Err(JournalError::Contract(Error::Binding))
            ),
            "mismatch {which}"
        );
        assert_eq!(bytes(&root), original);
    }
    let marginal = FileAndonRecoveryRequirements::new(
        expected.base().clone(),
        andon(),
        expected.windows().to_vec(),
        0,
    )
    .unwrap()
    .require_publication(expected.publication().unwrap())
    .unwrap();
    assert!(matches!(
        begin(&root, true, &marginal, &anchor),
        Err(JournalError::Contract(Error::Binding))
    ));
    let mut run = begin(&root, true, &expected, &anchor).unwrap();
    ready(&mut run);
    assert!(run.finish().is_ok());
}

#[test]
fn final_counter_floors_latch_failure_and_cannot_escape_through_stop_or_finish() {
    let root = Directory::new();
    let (host, _) = seeded(&root, false, false);
    let expected = requirements(&host, false, false);
    let anchor = host.history_anchor().unwrap();
    let total = host.revision() as usize;
    let original = bytes(&root);
    drop(host);
    for which in 0..4 {
        let mut base = expected.base().clone();
        let mut evaluation_floor = expected.minimum_evaluation_revision();
        match which {
            0 => base.minimum.journal_revision += 1,
            1 => base.minimum.control_sequence += 1,
            2 => base.minimum.authority_epoch += 1,
            _ => evaluation_floor += 1,
        }
        let bad = FileAndonRecoveryRequirements::new(
            base,
            andon(),
            expected.windows().to_vec(),
            evaluation_floor,
        )
        .unwrap();
        let mut run = begin(&root, false, &bad, &anchor).unwrap();
        assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS), Err(Error::Stale.into()));
        assert_eq!(run.progress().replayed_events, total);
        assert_eq!(
            run.progress().status,
            FileGeneratedSourceRecoveryStatus::Failed(Error::Stale)
        );
        assert_eq!(run.advance(total, 1), Err(Error::Stale.into()));
        assert_eq!(run.stop_request(701), Err(Error::Stale.into()));
        assert!(matches!(
            run.finish(),
            Err(JournalError::Contract(Error::Stale))
        ));
        assert_eq!(bytes(&root), original);
    }
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    assert!(run.finish().is_ok());
}

#[test]
fn stale_quantum_incomplete_finish_and_foreign_history_anchor_cannot_mutate() {
    let root = Directory::new();
    let foreign = Directory::new();
    let (host, _) = seeded(&root, false, false);
    let (other, _) = seeded(&foreign, false, false);
    let expected = requirements(&host, false, false);
    let anchor = host.history_anchor().unwrap();
    let foreign_anchor = other.history_anchor().unwrap();
    let original = bytes(&root);
    drop(host);
    assert!(matches!(
        begin(&root, false, &expected, &foreign_anchor),
        Err(JournalError::Contract(Error::Binding))
    ));
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    let initial = run.progress();
    for (predecessor, quantum, error) in [
        (1, 1, Error::Stale),
        (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit),
    ] {
        assert_eq!(run.advance(predecessor, quantum), Err(error.into()));
        assert_eq!(run.progress(), initial);
    }
    run.advance(0, 1).unwrap();
    assert!(matches!(
        run.finish(),
        Err(JournalError::Contract(Error::Incomplete))
    ));
    assert_eq!(bytes(&root), original);
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    assert!(run.finish().is_ok());
}

#[test]
fn changed_canonical_image_after_ready_refuses_before_staging_cleanup() {
    let root = Directory::new();
    let (host, _) = seeded(&root, false, false);
    let expected = requirements(&host, false, false);
    let anchor = host.history_anchor().unwrap();
    let original = bytes(&root);
    let mut events = host.events.clone();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(2))));
    Machine::replay(&profile(), &events).unwrap();
    let alternate = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"retain-staging").unwrap();
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    std::fs::write(root.store().join(storage::CANONICAL), &alternate).unwrap();
    assert!(matches!(
        run.finish(),
        Err(JournalError::Contract(Error::Binding))
    ));
    assert_eq!(bytes(&root), alternate);
    assert_eq!(std::fs::read(&pending).unwrap(), b"retain-staging");
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    assert!(run.finish().is_ok());
}

#[test]
fn every_original_store_barrier_withholds_roles_and_preserves_native_accounting() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Directory::new();
        let (host, _) = seeded(&root, false, false);
        let expected = requirements(&host, false, false);
        let anchor = host.history_anchor().unwrap();
        let revision = host.revision();
        let report = host.credibility_report().unwrap();
        let accounting = host.andon_ledger().unwrap().clone();
        drop(host);
        let mut run = begin(&root, false, &expected, &anchor).unwrap();
        ready(&mut run);
        run.inner.fail_storage_once(barrier);
        assert!(matches!(
            run.finish(),
            Err(JournalError::Io(failure)) if failure.operation == barrier
        ));
        let saved = FileOversight::read_andon_guarded(
            root.store(),
            &profile(),
            &expected,
            &protocol(),
            &anchor,
        )
        .unwrap();
        assert_eq!(saved.credibility.report, report);
        assert_eq!(saved.accounting, accounting);
        assert_eq!(
            saved.credibility.journal.revision,
            revision + u64::from(barrier == JournalIo::DirectorySync)
        );
    }
}

#[test]
fn terminal_stop_keeps_original_reconstruction_and_exact_retries_without_ordinary_fence() {
    let root = Directory::new();
    let (host, _) = seeded(&root, false, false);
    let expected = requirements(&host, false, false);
    let anchor = host.history_anchor().unwrap();
    let original = host.inspect();
    drop(host);
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    let request = run.stop_request(701).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, original.revision + 2);
    assert_eq!(stopped.snapshot().executions, original.executions);
    assert_eq!(
        stopped.snapshot().control.ledger.charged,
        original.control.ledger.charged
    );
    let canonical = bytes(&root);
    let mut run = begin(&root, false, &expected, &anchor).unwrap();
    ready(&mut run);
    assert_eq!(run.stop_request(702), Err(Error::Duplicate.into()));
    assert_eq!(run.stop_request(701).unwrap(), request);
    let retried = run.finish_admission_stop(request).unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), stopped.snapshot());
    assert_eq!(bytes(&root), canonical);
}

#[test]
fn recorded_selectors_require_ready_and_exact_original_native_admission_without_writes() {
    use crate::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageRequest;

    for recorded in [false, true] {
        let root = Directory::new();
        let (mut host, _) = seeded(&root, false, false);
        let captured = host
            .refresh_file_source(host.revision(), &mut evidence(&root), ElapsedTick(2))
            .unwrap();
        for revision in 1..4 {
            host.advance_decoder_text(host.revision(), 7, revision).unwrap();
        }
        let original_request = request(b"ab", 2);
        assert_eq!(
            host.decoder_text_progress(7).unwrap().finish(),
            Some(Ok(GenerationFinish::StopToken))
        );
        if recorded {
            let source = FileTextMessageRequest {
                request: 91,
                generation: 7,
                generation_revision: 4,
                target: host.inspect().target,
                policy_epoch: host.inspect().control.ledger.epoch,
                deadline: ElapsedTick(100),
            };
            host.submit_decoder_text_message(
                host.revision(),
                source,
                captured.snapshot().clone(),
            )
            .unwrap();
        }
        let expected = requirements(&host, false, false);
        let anchor = host.history_anchor().unwrap();
        let canonical = bytes(&root);
        drop(host);
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"original unacknowledged staging").unwrap();
        let mut run = begin(&root, false, &expected, &anchor).unwrap();
        assert_eq!(
            run.recorded_message(91, 7, &original_request),
            Err(Error::Incomplete.into())
        );
        assert_eq!(
            run.recorded_finish(91, 92, 7, &original_request),
            Err(Error::Incomplete.into())
        );
        ready(&mut run);
        let progress = run.progress();
        assert_eq!(
            run.recorded_message(91, 7, &original_request),
            if recorded { Ok(()) } else { Err(Error::Missing.into()) }
        );
        assert!(run.recorded_message(92, 7, &original_request).is_err());
        assert!(run.recorded_message(91, 8, &original_request).is_err());
        assert!(run.recorded_message(91, 7, &request(b"changed", 2)).is_err());
        assert!(run.recorded_finish(91, 91, 7, &original_request).is_err());
        assert!(run.recorded_finish(91, 92, 7, &original_request).is_err());
        assert_eq!(run.progress(), progress);
        assert_eq!(bytes(&root), canonical);
        assert_eq!(
            std::fs::read(&pending).unwrap(),
            b"original unacknowledged staging"
        );
        assert_eq!(run.cancel().status, FileGeneratedSourceRecoveryStatus::Cancelled);
        assert_eq!(bytes(&root), canonical);
        assert!(pending.exists());
    }
}
