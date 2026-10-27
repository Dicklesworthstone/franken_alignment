//! Acknowledged native receipt custody and exact source expenditure succession.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::MAX_HISTORY_ANCHOR_BYTES;
use std::io::{self, Write};

fn checkpoint_bytes(saved: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    saved
        .write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES)
        .unwrap();
    bytes
}
fn import(bytes: &[u8]) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut &bytes[..], MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}
fn successor_requirements(
    history: &History,
    saved: &TopologyCheckpoint,
) -> FileMediatedRequirements {
    let mut expected = history.expected.clone();
    expected.topology.available = saved.available();
    expected.oversight.minimum = saved.minimum();
    expected
}

#[test]
fn acknowledged_receipt_retains_exact_source_pair_and_only_confirms_real_publication() {
    for lane in LANES {
        for cut in [Cut::Sent, Cut::Published, Cut::Confirmed] {
            let history = History::with_source(lane, cut);
            let saved = history.topology.as_ref().unwrap();
            let original_source = checkpoint_bytes(saved);
            let identity = std::fs::canonicalize(history.root.store()).unwrap();
            // Recovery and succession consume independently retained documents.
            // Neither may read the current source files or reserve another check.
            let directory = history.root.store().parent().unwrap().to_owned();
            std::fs::remove_file(directory.join("topology.json")).unwrap();
            std::fs::remove_file(directory.join("inventory.json")).unwrap();
            let calls = Cell::new(0);
            let (port, receipt) = history
                .ready()
                .port_history(saved, || {
                    calls.set(calls.get() + 1);
                    ElapsedTick(5)
                })
                .unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(receipt.reconciled_from(), history.anchor.revision());
            assert_eq!(
                receipt.history_anchor().revision(),
                history.anchor.revision() + 3
            );
            assert_eq!(
                receipt.snapshot().revision,
                receipt.history_anchor().revision()
            );
            assert_eq!(receipt.outcome(), &observe(&port));
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
            let next = saved
                .after_reconciled_receipt(&history.profile, &identity, &receipt)
                .unwrap();
            assert_eq!(next.history_anchor(), receipt.history_anchor());
            assert_eq!(next.expected(), saved.expected());
            assert_eq!(next.topology_bytes(), saved.topology_bytes());
            assert_eq!(next.inventory_bytes(), saved.inventory_bytes());
            assert_eq!(next.work(), saved.work());
            assert_eq!(next.last_operation(), saved.last_operation());
            assert!(!next.available());
            assert_eq!(
                next.minimum().control_sequence,
                receipt.snapshot().control.sequence
            );
            assert_eq!(
                next.minimum().authority_epoch,
                receipt.snapshot().control.ledger.epoch
            );
            assert_eq!(import(&checkpoint_bytes(&next)), next);
            assert_eq!(checkpoint_bytes(saved), original_source);

            let committed = history.files();
            let expected = successor_requirements(&history, &next);
            // Neither the receipt nor the actor port retains the exclusive lock.
            let mut run = history
                .begin_with(&expected, next.history_anchor())
                .unwrap();
            run.drive().unwrap();
            let second_port = run.port(71, None).unwrap();
            assert_eq!(observe(&second_port), *receipt.outcome());
            assert_eq!(history.files(), committed);
            assert!(
                saved
                    .after_reconciled_receipt(
                        &history.profile,
                        &identity.join("different-store"),
                        &receipt
                    )
                    .is_err()
            );
            let mut wrong_profile = history.profile.clone();
            wrong_profile.delivery.clock_domain += 1;
            assert!(
                saved
                    .after_reconciled_receipt(&wrong_profile, &identity, &receipt)
                    .is_err()
            );

            // Only the matching immediately preceding source cut can advance
            // again; old custody never skips an intervening reconciliation.
            let mut run = history
                .begin_with(&expected, next.history_anchor())
                .unwrap();
            run.drive().unwrap();
            let later = run
                .receipt_history(71, None, Some(&next), || ElapsedTick(6))
                .unwrap();
            assert!(
                saved
                    .after_reconciled_receipt(&history.profile, &identity, &later)
                    .is_err()
            );
            let newest = next
                .after_reconciled_receipt(&history.profile, &identity, &later)
                .unwrap();
            assert_eq!(newest.work(), saved.work());
            assert_eq!(
                newest.history_anchor().revision(),
                history.anchor.revision() + 6
            );
            assert_eq!(observe(&port), *receipt.outcome());
        }
    }
}

#[test]
fn selected_source_and_request_mismatches_refuse_before_clock_cleanup_or_fence() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let foreign = History::with_source(lane, Cut::Published);
        let saved = history.topology.as_ref().unwrap();
        let before = history.files();
        let mut wrong = vec![
            history.early_topology.as_ref().unwrap().clone(),
            foreign.topology.as_ref().unwrap().clone(),
        ];
        // Valid transport controls with incorrect availability or impossible
        // independently required floors must fail the original current cursor.
        let mut availability = checkpoint_bytes(saved);
        availability[72] ^= 1;
        wrong.push(import(&availability));
        for offset in [8, 16] {
            let mut floors = checkpoint_bytes(saved);
            floors[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            wrong.push(import(&floors));
        }
        for source in &wrong {
            assert!(
                history
                    .ready()
                    .receipt_history(71, None, Some(source), || panic!(
                        "bad source custody must not read time"
                    ))
                    .is_err()
            );
            assert_eq!(history.files(), before);
        }
        assert!(
            history
                .begin()
                .receipt_history(71, None, Some(saved), || panic!(
                    "incomplete replay must not read time"
                ))
                .is_err()
        );
        for (request, after) in [(72, None), (71, Some(70))] {
            assert!(
                history
                    .ready()
                    .receipt_history(request, after, Some(saved), || panic!(
                        "wrong selected request must not read time"
                    ))
                    .is_err()
            );
            assert_eq!(history.files(), before);
        }
        for field in 0..3 {
            let mut expected = history.expected.clone();
            match field {
                0 => expected.oversight.credential_epoch = Some(fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCredentialEpoch { generation: 1, revoked: false }),
                1 => expected.oversight.minimum.authority_epoch = u64::MAX,
                _ => expected.topology.current = graph(1, true),
            }
            if let Ok(mut run) = history.begin_with(&expected, &history.anchor) {
                assert!(run.drive().is_err());
                assert!(
                    run.receipt_history(71, None, Some(saved), || panic!(
                        "bad independent current guard must not read time"
                    ))
                    .is_err()
                );
            }
            assert_eq!(history.files(), before);
        }
        let receipt = history
            .ready()
            .receipt_history(71, None, Some(saved), || ElapsedTick(5))
            .unwrap();
        assert!(matches!(
            receipt.outcome(),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
    }
}

struct RefusingWriter;
impl Write for RefusingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "independent archive destination failed",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn changed_canonical_images_and_failed_exports_never_erase_or_reissue_acknowledged_custody() {
    for lane in LANES {
        let history = History::with_source(lane, Cut::Published);
        let saved = history.topology.as_ref().unwrap();
        let identity = std::fs::canonicalize(history.root.store()).unwrap();
        let original_source = checkpoint_bytes(saved);
        let path = history.root.store().join("delivery.bin");
        let original = history.root.bytes();
        let mut changed = original.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        let run = history.ready();
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(
            run.receipt_history(71, None, Some(saved), || panic!(
                "changed canonical cut must not read time"
            ))
            .err(),
            Some(Error::Binding.into())
        );
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        assert_eq!(
            std::fs::read(history.root.store().join("delivery.pending")).unwrap(),
            b"preserve receipt staging"
        );
        std::fs::write(&path, &original).unwrap();
        let run = history.ready();
        assert_eq!(
            run.receipt_history(71, None, Some(saved), || {
                std::fs::write(&path, &changed).unwrap();
                ElapsedTick(5)
            })
            .err(),
            Some(Error::Binding.into())
        );
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        assert_eq!(
            std::fs::read(history.root.store().join("delivery.pending")).unwrap(),
            b"preserve receipt staging"
        );
        std::fs::write(&path, original).unwrap();

        let receipt = history
            .ready()
            .receipt_history(71, None, Some(saved), || ElapsedTick(5))
            .unwrap();
        let next = saved
            .after_reconciled_receipt(&history.profile, &identity, &receipt)
            .unwrap();
        let acknowledged = history.files();
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
        assert_eq!(history.files(), acknowledged);
        assert_eq!(checkpoint_bytes(saved), original_source);
        assert_eq!(import(&checkpoint_bytes(&next)), next);
        let mut archive = Vec::new();
        receipt
            .history_anchor()
            .write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        assert_eq!(
            FileHistoryAnchor::read_trusted(&mut archive.as_slice(), MAX_HISTORY_ANCHOR_BYTES)
                .unwrap(),
            *receipt.history_anchor()
        );
        let expected = successor_requirements(&history, &next);
        let mut run = history
            .begin_with(&expected, next.history_anchor())
            .unwrap();
        run.drive().unwrap();
        assert_eq!(observe(&run.port(71, None).unwrap()), *receipt.outcome());
        assert_eq!(history.files(), acknowledged);
    }
}

#[test]
fn history_only_custody_preserves_ordinary_capacity_and_existing_passive_receipts() {
    for lane in LANES {
        let full = History::new(lane, Cut::Published, true);
        let before = full.files();
        assert_eq!(
            full.ready()
                .receipt_history(71, None, None, || ElapsedTick(5))
                .err(),
            Some(Error::Limit.into())
        );
        assert_eq!(full.files(), before);
        assert_eq!(
            observe(&full.ready().port(71, None).unwrap()),
            full.original
        );
        assert_eq!(full.files(), before);

        let history = History::new(lane, Cut::Sent, false);
        let receipt = history
            .ready()
            .receipt_history(71, None, None, || ElapsedTick(5))
            .unwrap();
        assert!(matches!(receipt.outcome(), Knowledge::Unknown { .. }));
        assert_eq!(receipt.snapshot().control.ledger.charged, history.charge);
        assert_eq!(receipt.snapshot().executions, 0);
        assert_eq!(
            receipt.history_anchor().revision(),
            history.anchor.revision() + 3
        );
        let (outcome, anchor) = receipt.into_parts();
        let mut expected = history.expected.clone();
        expected.topology.available = false;
        let mut run = history.begin_with(&expected, &anchor).unwrap();
        run.drive().unwrap();
        assert_eq!(observe(&run.port(71, None).unwrap()), outcome);
    }
}
