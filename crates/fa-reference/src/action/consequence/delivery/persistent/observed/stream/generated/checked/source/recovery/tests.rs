//! Actual native inference and canonical storage, with synthetic model weights.
//! These tests cover recovery contracts, not empirical monitor quality.
mod anchored;
mod governed;
#[cfg(target_os = "linux")]
mod anchored_transport;
use super::*;
use crate::Snapshot;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::delivery::persistent::observed::{
    credibility::held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
    decoder::text::FileTextGenerationCommand,
    source::FileSourcePolicy,
    stream::generated::{
        FileTextMessageRequest,
        checked::{
            GeneratedPublicationFeed, GeneratedPublicationProfile,
            source::GeneratedSourcePublication,
        },
    },
};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::{
    JournalIo, Reconciliation, RecoveryReserve,
};
use crate::action::consequence::delivery::publication_gate::{
    PublicationLimits,
    changes::{PublicationChangePolicy, freshness::PublicationFreshnessPolicy},
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{
    ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use crate::action::{ActionState, ElapsedTick};
use crate::round::{Verdict, commitment};
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};
use std::{
    collections::BTreeMap,
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
};

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
    profile.delivery.total = 4096;
    profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    profile
}

fn selection(mode: usize) -> GeneratedSourceProfile {
    let stream = StreamProfile::new(9, 1, 4, 64, 256).unwrap();
    let reserve = (mode != 0).then_some(RecoveryReserve::terminal());
    let publication = GeneratedPublicationProfile {
        stream,
        reserve,
        limits: PublicationLimits {
            bindings: 4,
            validation: RefinementBudget {
                steps: 1000,
                value_bytes: 10000,
            },
        },
        feed: (mode >= 3).then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy {
                source: 71,
                after: 0,
                lookup: RoutingBudget {
                    steps: 1000,
                    bytes: 10000,
                },
            },
            freshness: PublicationFreshnessPolicy {
                clock_domain: 99,
                max_age_ticks: 50,
            },
            snapshot_fallback: mode >= 4,
        }),
    };
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
        publication: match mode {
            0 | 1 => GeneratedSourcePublication::Stream { stream, reserve },
            2..=4 => GeneratedSourcePublication::Checked(publication),
            _ => GeneratedSourcePublication::Joint {
                publication,
                policy: HeldOutJointPolicy::new(70, 1, 1, 1, 0, 0, HeldOutJointBudget::default())
                    .unwrap(),
            },
        },
    }
}

fn create(
    root: &Directory,
    selected: GeneratedSourceProfile,
) -> (FileOversight, FileHumanReviewer) {
    FileOversight::create_generated_text_stream_from_source(
        root.store(),
        profile(),
        fixtures::stopped_config(),
        tokenizer(false),
        selected,
    )
    .unwrap()
}
fn begin(
    root: &Directory,
    selected: GeneratedSourceProfile,
) -> Result<FileGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_generated_text_stream_from_source(
        root.store(),
        profile(),
        &fixtures::stopped_config(),
        &tokenizer(false),
        selected,
    )
}
fn advance(run: &mut FileGeneratedSourceRecovery) {
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1).unwrap();
        assert_eq!(after.total_events, before.total_events);
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(
        run.progress().status,
        FileGeneratedSourceRecoveryStatus::Ready
    );
}
fn ready(root: &Directory, selected: GeneratedSourceProfile) -> FileGeneratedSourceRecovery {
    let mut run = begin(root, selected).unwrap();
    advance(&mut run);
    run
}
fn canonical(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join(storage::CANONICAL)).unwrap()
}
fn pending(root: &Directory) -> std::path::PathBuf {
    root.store().join("delivery.pending")
}
fn source(root: &Directory) -> FileEvidenceSource {
    let path = root.store().parent().unwrap().join("source.json");
    let scope = profile().delivery.scope;
    let capture = EvidenceSnapshot::new(
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
        BTreeMap::from([("reviewer".into(), b"actual native source".to_vec())]),
    )
    .unwrap();
    fs::write(&path, capture.encode()).unwrap();
    FileEvidenceSource::new(path, 51, scope, 1048576).unwrap()
}
fn seed_pending(root: &Directory, selected: GeneratedSourceProfile) -> FileOversight {
    let (mut host, _) = create(root, selected);
    let mut source = source(root);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let command = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), command).unwrap();
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    host
}

#[test]
fn every_source_profile_replays_in_quanta_and_publishes_one_original_fence() {
    for mode in 0..6 {
        let root = Directory::new();
        let selected = selection(mode);
        let host = seed_pending(&root, selected);
        let numerical = host.decoder_inspection().unwrap().numerical;
        let original = host.decoder_text_progress(7).unwrap();
        let original_events = host.events.clone();
        let expected_bytes = journal::encode_appended(
            &profile(),
            host.store.identity(),
            &original_events,
            &Event::Core(BaseEvent::Fence),
        )
        .unwrap();
        let mut oracle = Machine::replay(&profile(), &original_events).unwrap();
        oracle.apply(&Event::Core(BaseEvent::Fence)).unwrap();
        let expected = oracle.snapshot(original_events.len() + 1);
        drop(host);
        let before = canonical(&root);
        fs::write(pending(&root), b"preserve until acknowledged finish").unwrap();
        let mut run = begin(&root, selected).unwrap();
        assert_eq!(run.progress().attempted_events, 0);
        assert_eq!(run.machine.decoder_contract(), None);
        assert!(begin(&root, selected).is_err()); // exclusive lock survives polling
        advance(&mut run);
        assert_eq!(canonical(&root), before);
        assert!(pending(&root).exists());
        let ready = run.progress();
        assert_eq!(run.advance(ready.replayed_events, 1).unwrap(), ready);
        let (mut host, _) = run.finish().unwrap();
        assert_eq!(canonical(&root), expected_bytes);
        assert_eq!(host.inspect(), expected);
        assert!(!pending(&root).exists());
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert!(host.decoder_inspection().unwrap().paused);
        assert!(!host.clock_ready());
        assert_eq!(
            host.decoder_text_progress(7).unwrap().command(),
            original.command()
        );
        assert_eq!(
            host.decoder_text_progress(7).unwrap().generation_revision(),
            1
        );
        assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
        let mut source = source(&root);
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2))
            .unwrap();
        host.resume_decoder(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
        )
        .unwrap();
        for step in 1..4 {
            host.advance_decoder_text(host.revision(), 7, step).unwrap();
        }
        let complete = host.decoder_text_progress(7).unwrap();
        assert_eq!(complete.bytes().unwrap(), b"A");
        assert_eq!(complete.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn cancelling_or_premature_finish_at_every_replay_cut_preserves_canonical_and_staging() {
    let root = Directory::new();
    let selected = selection(1);
    let host = seed_pending(&root, selected);
    let count = host.events.len();
    drop(host);
    let before = canonical(&root);
    fs::write(pending(&root), b"original staging").unwrap();
    for cut in 0..=count {
        let mut run = begin(&root, selected).unwrap();
        if cut != 0 {
            run.advance(0, cut).unwrap();
        }
        let result = run.cancel();
        assert_eq!(result.replayed_events, cut);
        assert_eq!(result.status, FileGeneratedSourceRecoveryStatus::Cancelled);
        assert_eq!(canonical(&root), before);
        assert_eq!(fs::read(pending(&root)).unwrap(), b"original staging");
        if cut < count {
            let mut run = begin(&root, selected).unwrap();
            if cut != 0 {
                run.advance(0, cut).unwrap();
            }
            assert_eq!(run.stop_request(701), Err(Error::Incomplete.into()));
            assert_eq!(run.finish().err(), Some(Error::Incomplete.into()));
            assert_eq!(canonical(&root), before);
            assert!(pending(&root).exists());
        }
    }
    let (host, _) = ready(&root, selected).finish().unwrap();
    assert_eq!(
        host.decoder_text_progress(7).unwrap().generation_revision(),
        1
    );
}

#[test]
fn invalid_quanta_do_not_spend_original_replay_work() {
    let root = Directory::new();
    drop(seed_pending(&root, selection(1)));
    let mut run = begin(&root, selection(1)).unwrap();
    let initial = run.progress();
    assert_eq!(run.advance(1, 1), Err(Error::Stale.into()));
    assert_eq!(run.advance(0, 0), Err(Error::InvalidInput.into()));
    assert_eq!(
        run.advance(0, MAX_JOURNAL_EVENTS + 1),
        Err(Error::Limit.into())
    );
    assert_eq!(run.progress(), initial);
    run.advance(0, 1).unwrap();
    let progressed = run.progress();
    assert_eq!(run.advance(0, 1), Err(Error::Stale.into()));
    assert_eq!(run.progress(), progressed);
    advance(&mut run);
    assert_eq!(run.stop_request(0), Err(Error::InvalidInput.into()));
    assert!(run.finish().is_ok());
}

#[test]
fn complete_source_model_tokenizer_and_publication_inputs_are_pinned_before_replay() {
    let root = Directory::new();
    let selected = selection(5);
    drop(seed_pending(&root, selected));
    let before = canonical(&root);
    fs::write(pending(&root), b"do not clean").unwrap();
    for changed in 0..12 {
        let mut wrong = selected;
        let decoder = if changed == 0 {
            fixtures::config(0.5, 65)
        } else {
            fixtures::stopped_config()
        };
        let tokenizer = tokenizer(changed == 1);
        match changed {
            2 => wrong.source.source.source += 1,
            3 => wrong.source.source.generation += 1,
            4 => wrong.source.source.scope.principal += 1,
            5 => wrong.source.limits.events -= 1,
            6 => wrong.source.freshness = StateFreshness::new(51).unwrap(),
            7 => wrong.publication = selection(1).publication,
            8 => wrong.publication = selection(4).publication,
            9..=11 => {
                let GeneratedSourcePublication::Joint {
                    publication,
                    policy,
                } = &mut wrong.publication
                else {
                    unreachable!()
                };
                match changed {
                    9 => publication.reserve = None,
                    10 => publication.feed.as_mut().unwrap().snapshot_fallback = false,
                    _ => {
                        *policy = HeldOutJointPolicy::new(
                            70,
                            2,
                            1,
                            1,
                            0,
                            0,
                            HeldOutJointBudget::default(),
                        )
                        .unwrap()
                    }
                }
            }
            _ => {}
        }
        assert_eq!(
            FileOversight::begin_open_generated_text_stream_from_source(
                root.store(),
                profile(),
                &decoder,
                &tokenizer,
                wrong
            )
            .err(),
            Some(Error::Binding.into()),
            "{changed}"
        );
        assert_eq!(canonical(&root), before);
        assert_eq!(fs::read(pending(&root)).unwrap(), b"do not clean");
    }
    assert!(ready(&root, selected).finish().is_ok());
}

#[test]
fn failed_or_unwound_original_application_never_reuses_partial_state() {
    for interrupted in [false, true] {
        let root = Directory::new();
        drop(seed_pending(&root, selection(1)));
        let before = canonical(&root);
        fs::write(pending(&root), b"still unacknowledged").unwrap();
        let mut run = begin(&root, selection(1)).unwrap();
        if interrupted {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = run.advance_with(0, 1, |machine, event| {
                        machine.apply(event)?;
                        panic!("interrupted after original application");
                    });
                }))
                .is_err()
            );
            assert_eq!(
                run.progress().status,
                FileGeneratedSourceRecoveryStatus::Interrupted
            );
            assert_eq!(run.advance(0, 1), Err(JournalError::Unavailable));
            assert_eq!(run.finish().err(), Some(JournalError::Unavailable));
        } else {
            assert_eq!(
                run.advance_with(0, 1, |machine, event| {
                    machine.apply(event)?;
                    Err(Error::Binding)
                }),
                Err(Error::Binding.into())
            );
            assert_eq!(
                run.progress().status,
                FileGeneratedSourceRecoveryStatus::Failed(Error::Binding)
            );
            assert_eq!(run.progress().attempted_events, 1);
            assert_eq!(run.progress().replayed_events, 0);
            assert_eq!(run.advance(0, 1), Err(Error::Binding.into()));
            assert_eq!(run.finish().err(), Some(Error::Binding.into()));
        }
        assert_eq!(canonical(&root), before);
        assert_eq!(fs::read(pending(&root)).unwrap(), b"still unacknowledged");
        assert!(ready(&root, selection(1)).finish().is_ok());
    }
}

#[test]
fn changed_canonical_after_ready_refuses_both_finalizers_before_cleanup() {
    for stop in [false, true] {
        let root = Directory::new();
        drop(seed_pending(&root, selection(1)));
        let before = canonical(&root);
        fs::write(pending(&root), b"keep pending").unwrap();
        let run = ready(&root, selection(1));
        let request = run.stop_request(701).unwrap();
        let mut replaced = before.clone();
        replaced.push(0);
        fs::write(root.store().join(storage::CANONICAL), &replaced).unwrap();
        let error = if stop {
            run.finish_admission_stop(request).err()
        } else {
            run.finish().err()
        };
        assert_eq!(error, Some(Error::Binding.into()));
        assert_eq!(canonical(&root), replaced);
        assert_eq!(fs::read(pending(&root)).unwrap(), b"keep pending");
        fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
        assert!(ready(&root, selection(1)).finish().is_ok());
    }
}

#[test]
fn native_recovery_storage_barriers_return_neither_owner_nor_stop_acknowledgment() {
    for stop in [false, true] {
        for barrier in [
            JournalIo::Stage,
            JournalIo::Write,
            JournalIo::FileSync,
            JournalIo::Rename,
            JournalIo::DirectorySync,
        ] {
            let root = Directory::new();
            drop(seed_pending(&root, selection(1)));
            let before = canonical(&root);
            let run = ready(&root, selection(1));
            let revision = run.progress().total_events;
            let request = run.stop_request(701).unwrap();
            run.store.fail_once(barrier);
            let error = if stop {
                run.finish_admission_stop(request).err()
            } else {
                run.finish().err()
            };
            assert!(
                matches!(error, Some(JournalError::Io(failure)) if failure.operation == barrier)
            );
            if barrier != JournalIo::DirectorySync {
                assert_eq!(canonical(&root), before);
            }
            let recovered = ready(&root, selection(1));
            if stop {
                let result = recovered.finish_admission_stop(request).unwrap();
                assert_eq!(result.snapshot().revision, revision as u64 + 2);
                assert_eq!(
                    result.appended_events(),
                    if barrier == JournalIo::DirectorySync {
                        0
                    } else {
                        2
                    }
                );
            } else {
                let (host, _) = recovered.finish().unwrap();
                assert_eq!(
                    host.revision(),
                    revision as u64
                        + if barrier == JournalIo::DirectorySync {
                            2
                        } else {
                            1
                        }
                );
                assert_eq!(
                    host.decoder_text_progress(7).unwrap().generation_revision(),
                    1
                );
            }
        }
    }
}

fn unknown_message(root: &Directory) -> FileOversight {
    let (mut host, reviewer) = create(root, selection(1));
    let mut source = source(root);
    let captured = host
        .refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let snapshot = captured.snapshot().clone();
    let command = command(&host, 7, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), command)
        .unwrap();
    let input = FileTextMessageRequest {
        request: 91,
        generation: 7,
        generation_revision: host
            .decoder_generation_progress(7)
            .unwrap()
            .generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    };
    let status = host
        .submit_decoder_text_message(host.revision(), input, snapshot.clone())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("original native request was not admitted")
    };
    let action = host.request_action(91).unwrap().clone();
    let inputs = captured.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone())
        .unwrap();
    host.begin_review(
        host.revision(),
        attempt,
        100,
        [9; 32],
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(8),
        },
        snapshot.clone(),
    )
    .unwrap();
    let digest = commitment(100, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 100, "reviewer", digest)
        .unwrap();
    host.open_reveals(host.revision(), 100).unwrap();
    host.reveal_review(
        host.revision(),
        100,
        "reviewer",
        Verdict::Allow,
        b"salt".to_vec(),
    )
    .unwrap();
    host.finish_review(host.revision(), 100, Some(&inputs), snapshot.clone())
        .unwrap()
        .unwrap();
    let automatic = host
        .authorize(host.revision(), attempt, &inputs, snapshot.clone())
        .unwrap();
    let review = host
        .request_human_approval(host.revision(), 101, attempt, &inputs, ElapsedTick(10))
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &review).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        snapshot,
    )
    .unwrap();
    assert_eq!(
        host.reconcile(host.revision(), attempt).unwrap(),
        Reconciliation::AwaitingResolution
    );
    assert_eq!(
        host.inspect().control.ledger.stages.get(&attempt),
        Some(&ActionState::Unknown)
    );
    host
}

#[test]
fn admission_stop_retains_original_unknown_stream_liability_and_exact_retries() {
    let root = Directory::new();
    let host = unknown_message(&root);
    let before = host.inspect();
    assert!(before.control.ledger.charged > 0);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let stream = host.stream_snapshot().unwrap();
    drop(host);
    let run = ready(&root, selection(1));
    assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
    let request = run.stop_request(701).unwrap();
    let result = run.finish_admission_stop(request).unwrap();
    assert_eq!(result.appended_events(), 2);
    assert_eq!(result.snapshot().revision, before.revision + 2);
    assert_eq!(
        result.snapshot().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        result.snapshot().control.ledger.stages,
        before.control.ledger.stages
    );
    assert_eq!(result.snapshot().payload, before.payload);
    assert_eq!(result.snapshot().executions, before.executions);
    assert!(!result.progress().drained());
    let stopped_bytes = canonical(&root);
    let run = ready(&root, selection(1));
    assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
    assert_eq!(
        run.machine.broker.stream_state().unwrap().1,
        &stream.confirmed
    );
    assert_eq!(run.machine.broker.stream_pending(), stream.pending);
    assert_eq!(run.machine.endpoint.stream_view(), Some(&stream.published));
    assert_eq!(run.stop_request(702), Err(Error::Duplicate.into()));
    assert_eq!(run.stop_request(701).unwrap(), request);
    let retried = run.finish_admission_stop(request).unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), result.snapshot());
    assert_eq!(retried.history_anchor(), result.history_anchor());
    assert_eq!(canonical(&root), stopped_bytes);
}

#[cfg(target_os = "linux")]
mod transport;
