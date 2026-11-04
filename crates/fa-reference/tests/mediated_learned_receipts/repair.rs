//! Repair lost independent receipt exports using the original replay and exact
//! source cut. These fixtures perform real native publication/credential work;
//! their synthetic numerical models do not qualify a detector or deployment.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    FileAbandonRequest,
    observed::guarded::{
        FileCredentialEpoch, anchored::MAX_HISTORY_ANCHOR_BYTES,
        mediated::receipt::FileReceiptTopologyRecovery,
    },
};
use std::{
    fs,
    io::{self, Write},
};

fn raw(
    history: &History,
    expected: &FileMediatedRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<Recovery, JournalError> {
    if history.lane == Lane::Plain {
        FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            history.root.store(),
            history.profile.clone(),
            expected,
            &history.config,
            anchor,
        )
        .map(Recovery::Plain)
    } else {
        let expected = FilePredictiveMediatedRequirements {
            oversight: expected.oversight.clone(),
            topology: expected.topology.clone(),
            prediction: prediction(&history.config),
            evaluation: expected.evaluation.clone(),
        };
        FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            history.root.store(),
            history.profile.clone(),
            &expected,
            &history.config,
            anchor,
        )
        .map(Recovery::Predictive)
    }
}
fn restrict(
    run: Recovery,
    saved: &TopologyCheckpoint,
    request: u64,
    after: Option<u64>,
) -> Result<FileReceiptTopologyRecovery, JournalError> {
    match run {
        Recovery::Plain(run) => run.require_receipt_checkpoint(saved, request, after),
        Recovery::Predictive(run) => run.require_receipt_checkpoint(saved, request, after),
    }
}
fn begin_with(
    history: &History,
    expected: &FileMediatedRequirements,
    anchor: &FileHistoryAnchor,
    saved: &TopologyCheckpoint,
    request: u64,
    after: Option<u64>,
) -> Result<FileReceiptTopologyRecovery, JournalError> {
    restrict(raw(history, expected, anchor)?, saved, request, after)
}
fn begin(history: &History, request: u64, after: Option<u64>) -> FileReceiptTopologyRecovery {
    begin_with(
        history,
        &history.expected,
        &history.anchor,
        history.topology.as_ref().unwrap(),
        request,
        after,
    )
    .unwrap()
}
fn drive(run: &mut FileReceiptTopologyRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events;
        let progress = run.advance(before, 1)?;
        assert!(progress.replayed_events - before <= 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    Ok(())
}
fn ready(history: &History, request: u64, after: Option<u64>) -> FileReceiptTopologyRecovery {
    let mut run = begin(history, request, after);
    drive(&mut run).unwrap();
    run
}
fn archive(saved: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    saved
        .write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES)
        .unwrap();
    bytes
}
fn import(bytes: &[u8]) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut &bytes[..], MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}
fn successor(history: &History, receipt: &FileReconciledLearnedReceipt) -> TopologyCheckpoint {
    history
        .topology
        .as_ref()
        .unwrap()
        .after_reconciled_receipt(
            &history.profile,
            &fs::canonicalize(history.root.store()).unwrap(),
            receipt,
        )
        .unwrap()
}
fn stage(history: &History) {
    fs::write(
        history.root.store().join("delivery.pending"),
        b"unexported receipt candidate",
    )
    .unwrap();
}
struct RefusingWriter;
impl Write for RefusingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("independent receipt archive unavailable"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn unchanged_cut_completes_once_and_lost_pair_exports_repair_without_reconciliation() {
    for lane in LANES {
        for cut in [Cut::Sent, Cut::Published, Cut::Confirmed] {
            let history = History::with_source(lane, cut);
            let saved = history.topology.as_ref().unwrap();
            let saved_bytes = archive(saved);
            let directory = history.root.store().parent().unwrap().to_owned();
            fs::remove_file(directory.join("topology.json")).unwrap();
            fs::remove_file(directory.join("inventory.json")).unwrap();
            let calls = Cell::new(0);
            let (port, receipt) = ready(&history, 71, None)
                .finish_reconciled_receipt_port_with_history(|| {
                    calls.set(calls.get() + 1);
                    ElapsedTick(5)
                })
                .unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(receipt.reconciled_from(), saved.history_anchor().revision());
            assert_eq!(
                receipt.history_anchor().revision(),
                history.anchor.revision() + 3
            );
            assert_eq!(
                receipt.snapshot().revision,
                receipt.history_anchor().revision()
            );
            assert_eq!(receipt.snapshot().control.ledger.charged, history.charge);
            assert_eq!(receipt.snapshot().executions, u64::from(cut != Cut::Sent));
            if cut == Cut::Sent {
                assert!(matches!(receipt.outcome(), Knowledge::Unknown { .. }));
                assert_eq!(
                    receipt.snapshot().control.ledger.stages[&history.attempt],
                    ActionState::Unknown
                );
            } else {
                assert!(matches!(
                    receipt.outcome(),
                    Knowledge::Known {
                        value: ActorOutcome::Executed,
                        ..
                    }
                ));
            }
            assert_eq!(observe(&port), *receipt.outcome());
            let next = successor(&history, &receipt);
            assert_eq!(next.expected(), saved.expected());
            assert_eq!(next.topology_bytes(), saved.topology_bytes());
            assert_eq!(next.inventory_bytes(), saved.inventory_bytes());
            assert_eq!(next.work(), saved.work());
            assert_eq!(next.last_operation(), saved.last_operation());
            assert!(!next.available());
            assert!(
                receipt
                    .history_anchor()
                    .write_to(&mut RefusingWriter, MAX_HISTORY_ANCHOR_BYTES)
                    .is_err()
            );
            assert!(
                next.write_to(&mut RefusingWriter, MAX_TOPOLOGY_CHECKPOINT_BYTES)
                    .is_err()
            );
            let expected_anchor = receipt.history_anchor().clone();
            let expected_source = archive(&next);
            let outcome = receipt.outcome().clone();
            let control = receipt.snapshot().control.clone();
            drop(receipt);
            drop(next);
            drop(port);
            let canonical = history.root.bytes();
            stage(&history);
            for _ in 0..2 {
                let (port, repaired) = ready(&history, 71, None)
                    .finish_reconciled_receipt_port_with_history(|| {
                        panic!("visible triplet must not read time")
                    })
                    .unwrap();
                assert_eq!(repaired.history_anchor(), &expected_anchor);
                assert_eq!(repaired.reconciled_from(), history.anchor.revision());
                assert_eq!(repaired.snapshot().control, control);
                assert_eq!(repaired.outcome(), &outcome);
                assert_eq!(observe(&port), outcome);
                assert_eq!(archive(&successor(&history, &repaired)), expected_source);
                assert_eq!(archive(saved), saved_bytes);
                assert_eq!(history.root.bytes(), canonical);
                assert!(!history.root.store().join("delivery.pending").exists());
                // Neither returned immutable value retains the exclusive owner.
                drop(begin(&history, 71, None));
            }
        }
    }
}

#[test]
fn literal_standalone_fence_time_sweep_repairs_the_source_cut_without_a_trigger_claim() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let (mut host, _) = history.ready().finish().unwrap();
        host.observe_time(host.revision(), ElapsedTick(5)).unwrap();
        host.reconcile_pending(host.revision()).unwrap();
        let current = host.history_anchor().unwrap();
        assert_eq!(current.revision(), history.anchor.revision() + 3);
        drop(host);
        let canonical = history.root.bytes();
        let receipt = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| {
                panic!("source repair does not distinguish transaction boundaries")
            })
            .unwrap();
        assert_eq!(receipt.history_anchor(), &current);
        assert!(matches!(
            receipt.outcome(),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        assert_eq!(
            successor(&history, &receipt).work(),
            history.topology.as_ref().unwrap().work()
        );
        assert_eq!(history.root.bytes(), canonical);
    }
}

#[test]
fn exact_historical_finish_and_its_confirmed_predecessor_remain_independently_selected() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::FinishPending);
        let before = history.files();
        for (request, after) in [(72, None), (72, Some(70)), (72, Some(72)), (71, Some(72))] {
            let run = ready(&history, request, after);
            assert!(run.check_request().is_err());
            assert!(
                run.finish_reconciled_receipt_with_history(|| panic!(
                    "wrong kind or predecessor must precede time"
                ))
                .is_err()
            );
            assert_eq!(history.files(), before);
        }
        let selected = ready(&history, 72, Some(71));
        selected.check_request().unwrap();
        let (port, receipt) = selected
            .finish_reconciled_receipt_port_with_history(|| ElapsedTick(5))
            .unwrap();
        let proposal = FileLearnedTextStreamActorPort::encode_release(
            72,
            LearnedTextRelease::Finish,
            ElapsedTick(100),
        )
        .unwrap();
        let ticket = port.submit(72, &proposal).unwrap();
        assert_eq!(port.poll(&ticket), *receipt.outcome());
        let wrong = FileLearnedTextStreamActorPort::encode_release(
            72,
            LearnedTextRelease::Message,
            ElapsedTick(100),
        )
        .unwrap();
        assert_eq!(
            port.submit(72, &wrong).err(),
            Some(ActorError::IdempotencyConflict)
        );
        let canonical = history.root.bytes();
        let recovered = ready(&history, 72, Some(71))
            .finish_reconciled_receipt_with_history(|| panic!("finish repair must not read time"))
            .unwrap();
        assert_eq!(recovered.outcome(), receipt.outcome());
        assert_eq!(recovered.history_anchor(), receipt.history_anchor());
        // The same source cut also supports another independently valid retained
        // message. No journal event identifies which request caused the sweep.
        let message = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| {
                panic!("selected historical message must not trigger a new sweep")
            })
            .unwrap();
        assert!(matches!(
            message.outcome(),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        assert_eq!(message.history_anchor(), receipt.history_anchor());
        assert_eq!(history.root.bytes(), canonical);
    }
}

#[test]
fn existing_triplet_repairs_at_full_ordinary_capacity_but_unchanged_cut_cannot_spend_terminal_reserve()
 {
    for lane in LANES {
        let history = History::build_configured(lane, Cut::Published, Some(3), true, false, true);
        let receipt = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| ElapsedTick(5))
            .unwrap();
        assert_eq!(
            receipt.history_anchor().revision() as usize,
            history.profile.delivery.limits.events - RecoveryReserve::terminal().events
        );
        let canonical = history.root.bytes();
        let recovered = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| {
                panic!("full ordinary capacity cannot require another event")
            })
            .unwrap();
        assert_eq!(recovered.history_anchor(), receipt.history_anchor());
        assert_eq!(history.root.bytes(), canonical);

        let full = History::build(lane, Cut::Published, true, true);
        let before = full.files();
        assert_eq!(
            ready(&full, 71, None)
                .finish_reconciled_receipt_with_history(|| ElapsedTick(5))
                .err(),
            Some(Error::Limit.into())
        );
        assert_eq!(full.files(), before);
        let unreserved = History::build_configured(lane, Cut::Sent, None, true, false, false);
        let before = unreserved.files();
        assert!(
            begin_with(
                &unreserved,
                &unreserved.expected,
                &unreserved.anchor,
                unreserved.topology.as_ref().unwrap(),
                71,
                None
            )
            .is_err()
        );
        assert_eq!(unreserved.files(), before);
    }
}

#[test]
fn independent_current_credential_epoch_and_all_higher_floors_survive_repair() {
    for lane in LANES {
        let history = History::build_configured(lane, Cut::Sent, None, true, true, true);
        assert_eq!(
            history.expected.oversight.credential_epoch,
            Some(FileCredentialEpoch {
                generation: 2,
                revoked: false
            })
        );
        let receipt = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| ElapsedTick(5))
            .unwrap();
        let saved = history.topology.as_ref().unwrap();
        let before = history.files();
        for field in 0..7 {
            let mut wrong = history.expected.clone();
            match field {
                0 => wrong.oversight.minimum.journal_revision = u64::MAX,
                1 => wrong.oversight.minimum.control_sequence = u64::MAX,
                2 => wrong.oversight.minimum.authority_epoch = u64::MAX,
                3 => {
                    wrong.oversight.credential_epoch = Some(FileCredentialEpoch {
                        generation: 1,
                        revoked: false,
                    })
                }
                4 => {
                    wrong.oversight.credential_epoch = Some(FileCredentialEpoch {
                        generation: 2,
                        revoked: true,
                    })
                }
                5 => wrong.oversight.credential_epoch = None,
                _ => wrong.oversight.guards.credential = None,
            }
            if let Ok(mut run) = begin_with(&history, &wrong, &history.anchor, saved, 71, None) {
                assert!(
                    drive(&mut run).is_err(),
                    "{lane:?}: independent requirement {field}"
                );
                assert!(
                    run.finish_reconciled_receipt_with_history(|| panic!(
                        "unmet independent requirement must not read time"
                    ))
                    .is_err()
                );
            }
            assert_eq!(history.files(), before);
        }
        // Saved minima also strengthen a caller's lower independently supplied
        // floors; they cannot be overwritten by those lower values.
        for offset in [8, 16] {
            let mut bytes = archive(saved);
            bytes[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            let high = import(&bytes);
            let mut low = history.expected.clone();
            low.oversight.minimum = FileRecoveryFloor {
                journal_revision: 0,
                control_sequence: 0,
                authority_epoch: 0,
            };
            let mut run = begin_with(&history, &low, &history.anchor, &high, 71, None).unwrap();
            assert!(drive(&mut run).is_err());
            assert!(
                run.finish_reconciled_receipt_with_history(|| panic!(
                    "saved floors must be retained"
                ))
                .is_err()
            );
            assert_eq!(history.files(), before);
        }
        let recovered = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| {
                panic!("matching current epoch needs no new reconciliation")
            })
            .unwrap();
        assert_eq!(recovered.outcome(), receipt.outcome());
        assert_eq!(recovered.snapshot().control.ledger.charged, history.charge);
        assert!(matches!(recovered.outcome(), Knowledge::Unknown { .. }));
        assert_eq!(history.files(), before);
    }
}

#[test]
fn partial_reordered_extra_and_abandonment_suffixes_refuse_without_candidate_custody() {
    for lane in LANES {
        for suffix in 0..6 {
            let history = History::with_source(lane, Cut::Sent);
            let (mut host, _) = history.ready().finish().unwrap();
            match suffix {
                0 => {} // Fence alone.
                1 => host.observe_time(host.revision(), ElapsedTick(5)).unwrap(),
                2 => {
                    // A native Fence clears clock readiness, so reordered Sweep
                    // already refuses without recording anything. Then create a
                    // valid native three-event suffix whose last event is wrong.
                    let before = host.history_anchor().unwrap();
                    assert_eq!(
                        host.reconcile_pending(host.revision()).err(),
                        Some(Error::Incomplete.into())
                    );
                    assert_eq!(host.history_anchor().unwrap(), before);
                    host.observe_time(host.revision(), ElapsedTick(5)).unwrap();
                    host.observe_time(host.revision(), ElapsedTick(6)).unwrap();
                }
                3 | 4 => {
                    host.observe_time(host.revision(), ElapsedTick(5)).unwrap();
                    host.reconcile_pending(host.revision()).unwrap();
                    if suffix == 4 {
                        host.fence(host.revision()).unwrap();
                    }
                    host.observe_time(host.revision(), ElapsedTick(6)).unwrap();
                    if suffix == 4 {
                        host.reconcile_pending(host.revision()).unwrap();
                    }
                }
                _ => {
                    let control = host.inspect().control;
                    host.abandon_unknown(
                        host.revision(),
                        FileAbandonRequest {
                            operation: 901,
                            attempt: history.attempt,
                            expected_control_sequence: control.sequence,
                            expected_authority_epoch: control.ledger.epoch,
                        },
                    )
                    .unwrap();
                }
            }
            drop(host);
            stage(&history);
            let before = history.files();
            assert!(
                begin_with(
                    &history,
                    &history.expected,
                    &history.anchor,
                    history.topology.as_ref().unwrap(),
                    71,
                    None
                )
                .is_err(),
                "{lane:?}: suffix {suffix}"
            );
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn source_profile_path_prefix_and_original_current_availability_stay_exact() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let foreign = History::with_source(lane, Cut::Published);
        let saved = history.topology.as_ref().unwrap();
        let before = history.files();
        for source in [
            history.early_topology.as_ref().unwrap(),
            foreign.topology.as_ref().unwrap(),
        ] {
            assert!(
                begin_with(
                    &history,
                    &history.expected,
                    &history.anchor,
                    source,
                    71,
                    None
                )
                .is_err()
            );
        }
        assert!(
            begin_with(
                &history,
                &history.expected,
                &foreign.anchor,
                saved,
                71,
                None
            )
            .is_err()
        );
        for field in 0..3 {
            let mut wrong = history.expected.clone();
            match field {
                0 => wrong.topology.current = graph(1, true),
                1 => wrong.topology.available = !saved.available(),
                _ => wrong.topology.initial = graph(1, true),
            }
            if let Ok(mut run) = begin_with(&history, &wrong, &history.anchor, saved, 71, None) {
                assert!(drive(&mut run).is_err());
                assert!(
                    run.finish_reconciled_receipt_with_history(|| panic!(
                        "wrong graph cannot reconcile"
                    ))
                    .is_err()
                );
            }
        }
        let mut wrong = archive(saved);
        wrong[72] ^= 1;
        assert!(
            begin_with(
                &history,
                &history.expected,
                &history.anchor,
                &import(&wrong),
                71,
                None
            )
            .is_err()
        );
        let mut changed_profile = history.profile.clone();
        changed_profile.delivery.clock_domain += 1;
        assert!(
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                history.root.store(),
                changed_profile,
                &history.expected,
                &history.config,
                &history.anchor
            )
            .is_err()
        );
        let mut original = raw(&history, &history.expected, &history.anchor).unwrap();
        original.advance().unwrap();
        assert_eq!(
            restrict(original, saved, 71, None).err(),
            Some(Error::WrongState.into())
        );
        assert_eq!(history.files(), before);
    }
}

#[test]
fn incomplete_or_malformed_selection_and_dropped_cursors_preserve_every_file() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let before = history.files();
        let run = begin(&history, 71, None);
        assert_eq!(run.check_request().err(), Some(Error::Incomplete.into()));
        assert!(
            run.finish_reconciled_receipt_port_with_history(|| panic!(
                "incomplete replay must not read time"
            ))
            .is_err()
        );
        assert_eq!(history.files(), before);
        let mut run = begin(&history, 71, None);
        run.advance(0, 1).unwrap();
        drop(run);
        assert_eq!(history.files(), before);
        drop(ready(&history, 71, None));
        assert_eq!(history.files(), before);
        for (request, after) in [(0, None), (72, None), (71, Some(70)), (71, Some(71))] {
            let run = ready(&history, request, after);
            assert!(run.check_request().is_err());
            assert!(
                run.finish_reconciled_receipt_port_with_history(|| panic!(
                    "malformed selection must not read time"
                ))
                .is_err()
            );
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn recovered_triplet_rereads_canonical_bytes_and_requires_real_confirmation_cleanup() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let settled = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| ElapsedTick(5))
            .unwrap();
        stage(&history);
        let original = history.root.bytes();
        let path = history.root.store().join("delivery.bin");
        let pending = history.root.store().join("delivery.pending");
        let before = history.files();
        drop(ready(&history, 71, None));
        assert_eq!(history.files(), before);
        let run = ready(&history, 71, None);
        let mut changed = original.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        fs::write(&path, &changed).unwrap();
        assert_eq!(
            run.finish_reconciled_receipt_port_with_history(|| panic!(
                "visible triplet must never call time"
            ))
            .err(),
            Some(Error::Binding.into())
        );
        assert_eq!(fs::read(&path).unwrap(), changed);
        assert_eq!(fs::read(&pending).unwrap(), b"unexported receipt candidate");
        fs::write(&path, &original).unwrap();
        fs::remove_file(&pending).unwrap();
        // This exercises the actual confirm_and_cleanup refusal, not a fake
        // replacement fault hook: a directory cannot be consumed as staging.
        fs::create_dir(&pending).unwrap();
        assert_eq!(
            ready(&history, 71, None)
                .finish_reconciled_receipt_with_history(|| panic!(
                    "confirmation failure cannot fall back to another sweep"
                ))
                .err(),
            Some(JournalError::InvalidFile)
        );
        assert_eq!(history.root.bytes(), original);
        assert!(pending.is_dir());
        fs::remove_dir(&pending).unwrap();
        stage(&history);
        let repaired = ready(&history, 71, None)
            .finish_reconciled_receipt_with_history(|| {
                panic!("confirmation retry remains zero event")
            })
            .unwrap();
        assert_eq!(
            repaired.history_anchor().revision(),
            history.anchor.revision() + 3
        );
        assert_eq!(repaired.history_anchor(), settled.history_anchor());
        assert_eq!(history.root.bytes(), original);
        assert!(!pending.exists());
    }
}
