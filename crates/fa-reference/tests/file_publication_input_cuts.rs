//! Producer lag must not undo an invalidation through a fresh file read.
#![cfg(unix)]
#[path = "support/publication_input_cut.rs"] mod f;
use f::{base, Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FilePublicationCapture, MAX_CAPTURE_BYTES};
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationInputCut;
use fa_reference::Error;

fn dispatch(host: &mut FileOversight, keys: &Keys) {
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn version_two_roundtrips_and_version_one_bytes_are_unchanged() {
    let root = Directory::new(); let (mut host, _) = f::host(&root, false);
    let (action, inputs) = base::reviewed(&mut host, 1, b"visible");
    let old = base::packet(1, &action, &inputs, 1, &[0, 2, 4]);
    let new = f::packet(&action, &inputs, 1, 0, &[0, 2, 4], true);
    let old_bytes = old.to_bytes().unwrap(); let bytes = new.to_bytes().unwrap();
    assert_eq!(&old_bytes[..8], b"FAPCAP01"); assert_eq!(&bytes[..8], b"FAPCAP02");
    assert_eq!(bytes.len(), old_bytes.len() + 16);
    assert_eq!(FilePublicationCapture::from_bytes(&old_bytes).unwrap(), old);
    assert_eq!(old.input_cut(), None);
    assert_eq!(FilePublicationCapture::from_bytes(&bytes).unwrap(), new);
    let mut uncut = bytes.clone(); uncut[7] = b'1'; drop(uncut.drain(32..48));
    assert_eq!(uncut, old_bytes);
    for length in 0..bytes.len() { assert!(FilePublicationCapture::from_bytes(&bytes[..length]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(FilePublicationCapture::from_bytes(&trailing).is_err());
    let mut invalid = bytes.clone(); invalid[32..40].fill(0);
    assert_eq!(FilePublicationCapture::from_bytes(&invalid), Err(Error::InvalidInput));
    invalid = bytes; invalid[7] = b'3';
    assert_eq!(FilePublicationCapture::from_bytes(&invalid), Err(Error::Binding));
    assert_eq!(FilePublicationCapture::from_bytes(&vec![0; MAX_CAPTURE_BYTES + 1]), Err(Error::Limit));
}

#[test]
fn rereads_and_larger_producer_generations_cannot_rehabilitate_an_old_cut() {
    for generation in [1, 2] {
        let root = Directory::new(); let (mut host, reviewer, keys) = f::ready(&root, true, false);
        host.record_publication_change(host.revision(), f::notice(1, 1)).unwrap();
        assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, 1);
        base::replace_source(&root, &f::packet(&keys.action, &keys.inputs, generation, 0, &[0, 2, 4], true));
        let before = host.revision();
        assert_eq!(host.refresh_publication_from_file(before, 1, &base::source(&root)), Err(JournalError::Contract(Error::Stale)));
        assert_eq!(host.revision(), before + 1); // Only durable withdrawal survived.
        assert!(host.storage_failure().is_some());
        assert_eq!(host.publication_input_cut(1), Err(JournalError::Unavailable));
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
        drop(reviewer); drop(host);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let cut = host.publication_input_cut(1).unwrap().unwrap();
        assert_eq!(cut.last, f::cut(0)); assert_eq!(cut.required_through, 1);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn a_current_cut_allows_exact_revalidation_but_does_not_override_changed_contents() {
    for inserted in [false, true] {
        let root = Directory::new(); let (mut host, _, keys) = f::ready(&root, true, false);
        base::refresh(&mut host, &root, 1); dispatch(&mut host, &keys);
        host.record_publication_change(host.revision(), f::notice(1, 1)).unwrap();
        let rows = if inserted { vec![0, 1, 2, 4] } else { vec![0, 2, 4] };
        base::replace_source(&root, &f::packet(&keys.action, &keys.inputs, 2, 1, &rows, true));
        base::refresh(&mut host, &root, 1);
        assert_eq!(host.publication_input_cut(1).unwrap().unwrap().last, f::cut(1));
        let publication = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(publication.basis, if inserted { PublicationBasis::Rejected(Error::Stale) } else { PublicationBasis::Revalidated });
        assert_eq!(publication.outcome, if inserted { sealed() } else { EndpointOutcome::Executed { resulting_version: 2 } });
        assert_eq!(host.inspect().control.ledger.charged, 16); // Publication does not refund.
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, if inserted { 0 } else { 16 });
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn unrelated_changes_preserve_structured_reuse_but_cannot_narrow_opaque_requirements() {
    for opaque in [false, true] {
        let root = Directory::new(); let (mut host, _, keys) = f::ready(&root, opaque, false);
        let report = host.record_publication_change(host.revision(), f::notice(1, 99)).unwrap();
        assert_eq!(report.affected, if opaque { vec![1] } else { vec![] });
        assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, u64::from(opaque));
        let acquired = host.refresh_publication_from_file(host.revision(), 1, &base::source(&root));
        if opaque {
            assert_eq!(acquired, Err(JournalError::Contract(Error::Stale)));
            assert_eq!(host.inspect().executions, 0);
        } else {
            acquired.unwrap().unwrap(); dispatch(&mut host, &keys);
            base::refresh(&mut host, &root, 1);
            assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap().basis,
                PublicationBasis::Revalidated);
        }
    }
}

#[test]
fn gap_repairs_retain_the_highest_required_cut_instead_of_releasing_old_captures() {
    for supplied in [2, 3] {
        let root = Directory::new(); let (mut host, _, keys) = f::ready(&root, false, false);
        host.record_publication_change(host.revision(), f::notice(3, 99)).unwrap();
        assert!(!host.publication_change_status().unwrap().complete());
        assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, 3);
        for sequence in 1..=3 { host.record_publication_change(host.revision(), f::notice(sequence, 99)).unwrap(); }
        assert!(host.publication_change_status().unwrap().complete());
        base::replace_source(&root, &f::packet(&keys.action, &keys.inputs, 2, supplied, &[0, 2, 4], false));
        let acquired = host.refresh_publication_from_file(host.revision(), 1, &base::source(&root));
        if supplied == 2 { assert_eq!(acquired, Err(JournalError::Contract(Error::Stale))); }
        else { acquired.unwrap().unwrap(); dispatch(&mut host, &keys); }
    }
}

#[test]
fn incomplete_or_foreign_initial_cut_does_not_partially_bind_the_review() {
    let root = Directory::new(); let (mut host, _) = f::host(&root, false);
    let (action, inputs) = base::reviewed(&mut host, 1, b"visible");
    host.record_publication_change(host.revision(), f::notice(1, 99)).unwrap();
    for (cut, error) in [(f::cut(0), Error::Stale), (f::cut(2), Error::Incomplete),
        (PublicationInputCut { source: 42, through: 1 }, Error::Binding)] {
        let original = FilePublicationCapture::new_at_cut(1,
            base::packet(1, &action, &inputs, 1, &[0, 2, 4]).identity(), &action,
            base::observations(&inputs, 1, &[0, 2, 4]), cut).unwrap();
        let before = host.inspect();
        assert_eq!(host.bind_publication_file_source(host.revision(), 1, original, base::requests()), Err(JournalError::Contract(error)));
        assert_eq!(host.inspect(), before); assert_eq!(host.publication_source(1).unwrap(), None);
    }
    let original = f::packet(&action, &inputs, 1, 1, &[0, 2, 4], true);
    base::replace_source(&root, &original);
    host.bind_publication_file_source(host.revision(), 1, original, base::requests()).unwrap();
    base::refresh(&mut host, &root, 1);
    assert!(host.authorize(host.revision(), 1, &inputs, snapshot()).is_ok());
}

#[test]
fn cut_omission_equivocation_and_future_coverage_are_not_permitting_fallbacks() {
    for mode in 0..4 {
        let root = Directory::new(); let (mut host, _, keys) = f::ready(&root, true, false);
        host.record_publication_change(host.revision(), f::notice(1, 99)).unwrap();
        let (capture, error) = match mode {
            0 => (base::packet(1, &keys.action, &keys.inputs, 2, &[0, 2, 4]), Error::Incomplete),
            1 => (f::packet(&keys.action, &keys.inputs, 1, 1, &[0, 2, 4], true), Error::Binding),
            2 => (f::packet(&keys.action, &keys.inputs, 2, 2, &[0, 2, 4], true), Error::Incomplete),
            _ => (FilePublicationCapture::new_at_cut(1,
                base::packet(1, &keys.action, &keys.inputs, 2, &[0, 2, 4]).identity(), &keys.action,
                base::observations(&keys.inputs, 2, &[0, 2, 4]), PublicationInputCut { source: 42, through: 1 }).unwrap(), Error::Binding),
        };
        base::replace_source(&root, &capture);
        assert_eq!(host.refresh_publication_from_file(host.revision(), 1, &base::source(&root)), Err(JournalError::Contract(error)));
        assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
}

#[test]
fn recovery_retains_cut_requirements_and_unknown_charges_but_not_old_sendable_keys() {
    let root = Directory::new(); let (mut host, reviewer, keys) = f::ready(&root, true, false);
    base::refresh(&mut host, &root, 1); dispatch(&mut host, &keys);
    host.record_publication_change(host.revision(), f::notice(1, 1)).unwrap();
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let cut = host.publication_input_cut(1).unwrap().unwrap();
    assert_eq!(cut.last, f::cut(0)); assert_eq!(cut.required_through, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect().executions, 0);
}
