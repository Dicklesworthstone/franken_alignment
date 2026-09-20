//! Real canonical files and the original complete-input/two-key authority path.
//! Fixed helper verdicts are fixtures, not native inference qualification.
#![cfg(unix)]
#[path = "support/publication_input_cut.rs"] mod support;
use support::*;
use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::deferred::FileCaptureObservation;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationCaptureOutcome;

fn refresh(host: &mut FileOversight, root: &Directory) -> Result<FileCaptureObservation, JournalError> {
    host.refresh_publication_from_file_or_defer(host.revision(), 1, &base::source(root))?
        .map_err(|error| JournalError::from(error.contract_error()))
}
fn lag(host: &mut FileOversight, root: &Directory, keys: &Keys, generation: u64, opaque: bool) {
    host.record_publication_change(host.revision(), notice(1, 0)).unwrap();
    base::replace_source(root, &packet(&keys.action, &keys.inputs, generation, 0, &[0, 2, 4], opaque));
}

#[test]
fn valid_lag_preserves_both_original_keys_until_caught_up_exact_publication() {
    for opaque in [false, true] {
        let root = Directory::new(); let (mut host, _, keys) = ready(&root, opaque, false);
        lag(&mut host, &root, &keys, 2, opaque);
        let before = host.inspect(); let human = host.human_status(1001).unwrap();
        let observed = refresh(&mut host, &root).unwrap();
        assert_eq!(observed.identity.generation, 2);
        assert_eq!(observed.outcome, PublicationCaptureOutcome::Deferred {
            revision: host.publication_input_revision(1).unwrap(), observed: cut(0), required_through: 1,
        });
        assert_eq!(host.inspect().control, before.control);
        assert_eq!(host.human_status(1001).unwrap(), human);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Authorized);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().executions, 0); assert!(host.storage_failure().is_none());
        let state = host.publication_source(1).unwrap().unwrap();
        assert!(!state.fresh); assert!(!state.capture_pending);
        assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        assert_eq!(host.inspect().control, before.control);
        base::replace_source(&root, &packet(&keys.action, &keys.inputs, 3, 1, &[0, 2, 4], opaque));
        assert!(!refresh(&mut host, &root).unwrap().outcome.deferred());
        base::dispatch(&mut host, &keys);
        // Dispatch consumes the first read. Publication still needs ANOTHER read.
        base::refresh(&mut host, &root, 1);
        let published = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(published.basis, PublicationBasis::Revalidated);
        assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(published.outcome));
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn reopened_deferral_keeps_high_water_marks_but_never_resurrects_old_keys() {
    let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
    lag(&mut host, &root, &keys, 7, false); refresh(&mut host, &root).unwrap();
    let cut_before = host.publication_input_cut(1).unwrap(); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_source(1).unwrap().unwrap().generation, 7);
    assert_eq!(host.publication_input_cut(1).unwrap(), cut_before);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(Error::Binding.into()));
    let before = host.inspect();
    assert_eq!(refresh(&mut host, &root), Err(Error::WrongState.into()));
    assert_eq!(host.inspect(), before);
}

#[test]
fn strict_and_deferrable_routes_both_preserve_deferred_generation_and_snapshot_floors() {
    for strict in [false, true] {
        for defect in 0..4 {
            let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
            lag(&mut host, &root, &keys, 7, false); refresh(&mut host, &root).unwrap();
            let (generation, through, snapshot_revision, members, expected) = match defect {
                0 => (6, 0, 6, vec![0, 2, 4], Error::Stale),
                1 => (7, 1, 7, vec![0, 2, 4], Error::Binding),
                2 => (8, 1, 6, vec![0, 2, 4], Error::Stale),
                _ => (7, 0, 7, vec![0, 1, 2, 4], Error::Binding),
            };
            let observation = base::observations(&keys.inputs, snapshot_revision, &members);
            let inputs = FilePublicationInputs::new(observation.structured().cloned(), None);
            let changed = FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: base::SOURCE, generation },
                &keys.action, inputs, cut(through)).unwrap();
            base::replace_source(&root, &changed);
            let error = if strict {
                host.refresh_publication_from_file(host.revision(), 1, &base::source(&root)).unwrap_err()
            } else { refresh(&mut host, &root).unwrap_err() };
            assert_eq!(error, JournalError::from(expected));
            assert!(host.storage_failure().is_some());
            assert_eq!(host.inspect().control.ledger.reserved, 16);
            assert_eq!(host.inspect().executions, 0);
            drop(host);
            let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
            assert_eq!(host.publication_source(1).unwrap().unwrap().generation, 7);
            assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, 1);
        }
    }
}

#[test]
fn catching_up_cannot_rebase_the_original_negative_key_or_range_witness() {
    for members in [vec![0, 1, 2, 4], vec![0, 2, 3, 4], vec![0, 2, 4, 7]] {
        let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
        lag(&mut host, &root, &keys, 2, false); refresh(&mut host, &root).unwrap();
        base::replace_source(&root, &packet(&keys.action, &keys.inputs, 3, 1, &members, false));
        assert!(!refresh(&mut host, &root).unwrap().outcome.deferred());
        let before = host.inspect();
        assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        assert_eq!(host.inspect(), before); assert_eq!(host.inspect().executions, 0);
        // A new actual image matching the ORIGINAL requirement is a permitting neighbor.
        base::replace_source(&root, &packet(&keys.action, &keys.inputs, 4, 1, &[0, 2, 4], false));
        refresh(&mut host, &root).unwrap(); base::dispatch(&mut host, &keys);
    }
}

#[test]
fn a_missing_change_tail_requires_repair_before_producer_lag_can_defer() {
    for repair in [false, true] {
        let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
        host.record_publication_change(host.revision(), notice(2, 0)).unwrap();
        if repair {
            host.record_publication_change(host.revision(), notice(1, 0)).unwrap();
            host.record_publication_change(host.revision(), notice(2, 0)).unwrap();
        }
        base::replace_source(&root, &packet(&keys.action, &keys.inputs, 2, 0, &[0, 2, 4], false));
        let result = refresh(&mut host, &root);
        if repair {
            assert!(matches!(result.unwrap().outcome,
                PublicationCaptureOutcome::Deferred { required_through: 2, .. }));
            assert!(host.storage_failure().is_none());
        } else { assert_eq!(result, Err(Error::Incomplete.into())); assert!(host.storage_failure().is_some()); }
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
}

#[test]
fn unreadable_packets_withdraw_without_fabricating_a_successful_deferral() {
    let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
    lag(&mut host, &root, &keys, 2, false);
    std::fs::write(root.0.join("witness-input.bin"), b"broken").unwrap();
    assert!(host.refresh_publication_from_file_or_defer(host.revision(), 1, &base::source(&root)).unwrap().is_err());
    assert!(host.storage_failure().is_none()); assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    base::replace_source(&root, &packet(&keys.action, &keys.inputs, 2, 1, &[0, 2, 4], false));
    assert!(!refresh(&mut host, &root).unwrap().outcome.deferred()); base::dispatch(&mut host, &keys);
}

#[test]
fn stale_revisions_legacy_bindings_and_dispatched_work_refuse_before_withdrawal() {
    let root = Directory::new(); let (mut host, _, keys) = ready(&root, false, false);
    let before = host.inspect();
    assert_eq!(host.refresh_publication_from_file_or_defer(before.revision - 1, 1, &base::source(&root)),
        Err(Error::Stale.into()));
    assert_eq!(host.inspect(), before);
    base::refresh(&mut host, &root, 1); base::dispatch(&mut host, &keys);
    let before = host.inspect();
    assert_eq!(refresh(&mut host, &root), Err(Error::WrongState.into())); assert_eq!(host.inspect(), before);
    let legacy = Directory::new(); let (mut host, reviewer) = base::source_host(&legacy);
    let _keys = base::source_keys(&mut host, &reviewer, &legacy, 1);
    let before = host.inspect();
    assert_eq!(refresh(&mut host, &legacy), Err(Error::Binding.into())); assert_eq!(host.inspect(), before);
    assert!(host.storage_failure().is_none());
}
