//! Two independent feed/witness reads on the original atomic effect path.
#![cfg(unix)]
#[path = "support/publication_input_cut.rs"] mod f;
use f::{base, Directory, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::completion::CapturedCompletionKeys;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;

#[test]
fn staged_invalidation_requires_a_caught_up_image_and_then_still_checks_actual_contents() {
    // 0: producer lags the second feed; 1: caught up but violates absence;
    // 2: caught up and unchanged dependencies, so genuine execution is allowed.
    for mode in 0..3 {
        let root = Directory::new(); let (mut host, _, keys) = f::ready(&root, true, true);
        f::write_feed(&root, 1);
        base::replace_source(&root, &f::packet(&keys.action, &keys.inputs, 2, 1, &[0, 2, 4], true));
        let before = host.revision(); let mut clocks = 0;
        let report = host.complete_publication_from_feed(before, CapturedCompletionKeys {
            automatic: &keys.automatic, human: &keys.human, credential: None,
        }, &base::source(&root), &f::feed(&root), || {
            clocks += 1;
            if clocks == 2 {
                // First witness bytes have already been acquired, not restamped.
                f::write_feed(&root, 2);
                let rows = if mode == 1 { vec![0, 1, 2, 4] } else { vec![0, 2, 4] };
                base::replace_source(&root, &f::packet(&keys.action, &keys.inputs, 3,
                    if mode == 0 { 1 } else { 2 }, &rows, true));
            }
            ElapsedTick(2)
        }, |_, _| {
            let canonical = FileOversight::read_publication(root.store(), &profile()).unwrap();
            assert_eq!(canonical.executions, 0);
            assert_eq!(canonical.control.ledger.stages[&1], ActionState::Authorized);
            Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(keys.inputs.clone()) })
        });
        assert_eq!(report.reads.len(), 2); assert_eq!(report.completion.reads.len(), 2);
        if mode == 0 {
            assert_eq!(report.completion.result, Err(JournalError::Contract(Error::Stale)));
            assert!(report.committed.is_empty()); assert!(host.storage_failure().is_some());
            assert_eq!(host.revision(), before + 2); // No speculative suffix/dispatch escaped.
            assert_eq!(host.inspect().control.ledger.reserved, 16);
        } else {
            assert_eq!(report.committed.len(), 2);
            let result = report.completion.result.unwrap();
            assert_eq!(result.basis, if mode == 1 { PublicationBasis::Rejected(Error::Stale) } else { PublicationBasis::Revalidated });
            assert_eq!(result.outcome, if mode == 1 { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
                else { EndpointOutcome::Executed { resulting_version: 2 } });
            assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, 2);
            assert_eq!(host.publication_input_cut(1).unwrap().unwrap().last, f::cut(2));
            assert_eq!(host.inspect().control.ledger.charged, if mode == 1 { 0 } else { 16 });
        }
        assert_eq!(host.inspect().executions, u64::from(mode == 2));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}
