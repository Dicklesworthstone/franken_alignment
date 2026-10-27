//! Original journal barriers must never expose an unacknowledged receipt pair.
use super::*;
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedRecovery, FileLearnedRecoveryStatus,
};
use std::cell::Cell;

fn ready(root: &Directory, config: &FileLearnedConfig) -> FileLearnedRecovery {
    let mut run = FileOversight::begin_open_with_learned_generation(
        root.store(),
        stream_delivery_profile(),
        config,
    )
    .unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}

#[test]
fn every_reconciliation_storage_barrier_returns_no_receipt_or_anchor_and_replays_only_visible_history()
 {
    for port_output in [false, true] {
        for barrier in [
            JournalIo::Stage,
            JournalIo::Write,
            JournalIo::FileSync,
            JournalIo::Rename,
            JournalIo::DirectorySync,
        ] {
            let root = Directory::new();
            let config = stream_config();
            let (host, _) = stream_owner(&root, &config);
            let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
            observe(&mut supervisor, snapshot());
            let ticket = port
                .submit(71, LearnedTextRelease::Message, ElapsedTick(100))
                .unwrap();
            assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 71 });
            let host = supervisor.host().unwrap();
            let before = host.inspect();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let original = host.history_anchor().unwrap();
            drop(host);
            drop(supervisor);
            drop(port);
            let run = ready(&root, &config);
            run.fail_once(barrier);
            let calls = Cell::new(0);
            let clock = || {
                calls.set(calls.get() + 1);
                ElapsedTick(2)
            };
            let failed = if port_output {
                run.finish_reconciled_receipt_port_with_history(71, None, clock)
                    .err()
            } else {
                run.finish_reconciled_receipt_with_history(71, None, clock)
                    .err()
            };
            let JournalError::Io(failure) =
                failed.expect("failed native replacement returns no custody")
            else {
                panic!("expected original journal barrier failure");
            };
            assert_eq!(calls.get(), 1);
            assert_eq!(failure.operation, barrier);
            assert_eq!(
                failure.replacement_may_be_visible,
                matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync)
            );

            // The original fault seam exposes the full transaction only after
            // rename. Neither branch returns a candidate acknowledgment.
            let visible_growth = if barrier == JournalIo::DirectorySync {
                3
            } else {
                0
            };
            let run = ready(&root, &config);
            assert_eq!(
                run.progress().total_events as u64,
                before.revision + visible_growth
            );
            let observation = run.finish_receipt(71, None).unwrap();
            if visible_growth == 0 {
                assert_eq!(observation, Knowledge::Pending { request: 71 });
            } else {
                assert!(matches!(
                    observation,
                    Knowledge::Known {
                        value: ActorOutcome::CancelledBeforeDispatch,
                        ..
                    }
                ));
            }

            // A new successful original transaction is the permitted control.
            // It captures its own cut, never the failed attempt's private result.
            let receipt = ready(&root, &config)
                .finish_reconciled_receipt_with_history(71, None, || ElapsedTick(3))
                .unwrap();
            assert_eq!(
                receipt.reconciled_from(),
                original.revision() + visible_growth
            );
            assert_eq!(
                receipt.history_anchor().revision(),
                before.revision + visible_growth + 3
            );
            assert_eq!(receipt.snapshot().executions, 0);
            assert!(matches!(
                receipt.outcome(),
                Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                }
            ));
            let (host, _) = FileOversight::open_with_learned_generation(
                root.store(),
                stream_delivery_profile(),
                &config,
            )
            .unwrap();
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
            assert_eq!(host.inspect().executions, 0);
        }
    }
}
