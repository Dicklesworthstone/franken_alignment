//! Retained receipt history is captured from the acknowledged original replay,
//! even when the selected result remains Unknown or reporting later fails.
use super::*;
use crate::workflow::actor_service::learned::anchors;

fn retaining(mut args: Vec<String>, output: &Path) -> Vec<String> {
    args.extend([
        "--retain-history-anchor".into(),
        output.display().to_string(),
    ]);
    args
}

#[test]
fn retained_reconciliation_exports_message_and_finish_history_for_every_forecast_mode() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let root = Root::new();
            let case = sent(&root, mode, published);
            let args = command_args(&root, &case, None);
            let input = fs::read(root.0.join("independent-settlement.anchor")).unwrap();
            let output = root.0.join("retained-message.anchor");
            remove_live_assets(&root);
            let calls = Cell::new(0);
            let mut bytes = Vec::new();
            let result = query::command_reconciled_with_elapsed(
                &retaining(args, &output),
                None,
                &mut bytes,
                || {
                    calls.set(calls.get() + 1);
                    ElapsedTick(1001)
                },
                &mut || Duration::ZERO,
            );
            assert_eq!(result.is_ok(), published);
            assert_eq!(calls.get(), 1);
            let actual = state(&case);
            assert_eq!(actual.revision, case.revision + 3);
            assert_eq!(actual.executions, u64::from(published));
            assert_eq!(actual.control.ledger.charged, case.charged);
            let saved = anchors::load(&output).unwrap();
            assert_eq!(saved.revision(), actual.revision);
            let readonly =
                query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
            assert_eq!(bytes, encoded(&readonly));
            assert_eq!(
                fs::read(root.0.join("independent-settlement.anchor")).unwrap(),
                input
            );
            if !published {
                assert!(matches!(
                    readonly.result,
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::OutcomeUnknown
                    })
                ));
            }
        }
        for receipt_case in ReceiptCase::ALL {
            let root = Root::new();
            let case = fixture::finish_sent(&root, mode, receipt_case);
            let args = command_args(&root, &case, Some(1));
            let output = root.0.join("retained-finish.anchor");
            remove_live_assets(&root);
            let mut bytes = Vec::new();
            let result = query::command_reconciled_with_elapsed(
                &retaining(args, &output),
                None,
                &mut bytes,
                || ElapsedTick(1001),
                &mut || Duration::ZERO,
            );
            assert_eq!(result.is_ok(), receipt_case.permitted());
            let actual = state(&case);
            assert_eq!(actual.revision, case.revision + 3);
            assert_eq!(actual.executions, 1 + u64::from(receipt_case.permitted()));
            assert_eq!(actual.control.ledger.charged, case.charged);
            assert_eq!(anchors::load(&output).unwrap().revision(), actual.revision);
            let readonly =
                query::read(&case.config, &case.loaded, Some(1), &mut || Duration::ZERO).unwrap();
            assert_eq!(bytes, encoded(&readonly));
            assert!(matches!(readonly.result, Ok(Knowledge::Known { value, .. })
                if value == receipt_case.outcome()));
        }
    }
}

#[test]
fn retained_reconciliation_keeps_acknowledged_history_after_deadline_and_report_failure() {
    for mode in [None, Some(false), Some(true)] {
        for deadline_failure in [false, true] {
            let root = Root::new();
            let case = sent(&root, mode, true);
            let args = command_args(&root, &case, None);
            let output = root.0.join("retained-after-error.anchor");
            remove_live_assets(&root);
            let clock_ran = Cell::new(false);
            let mut report = BrokenOutput { remaining: 0 };
            let error = query::command_reconciled_with_elapsed(
                &retaining(args, &output),
                None,
                &mut report,
                || {
                    clock_ran.set(true);
                    ElapsedTick(1001)
                },
                &mut || {
                    if deadline_failure && clock_ran.get() {
                        Duration::from_secs(3600)
                    } else {
                        Duration::ZERO
                    }
                },
            )
            .unwrap_err();
            assert!(clock_ran.get());
            assert!(error.contains("reconciliation acknowledged"));
            assert!(
                error.contains(if deadline_failure {
                    "deadline expired"
                } else {
                    "original response output failed"
                }),
                "{error}"
            );
            assert_eq!(
                anchors::load(&output).unwrap().revision(),
                case.revision + 3
            );
            let actual = state(&case);
            assert_eq!(actual.revision, case.revision + 3);
            assert_eq!(actual.executions, 1);
            assert_eq!(actual.control.ledger.charged, case.charged);
            assert!(matches!(
                query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO)
                    .unwrap()
                    .result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
        }
    }
}
