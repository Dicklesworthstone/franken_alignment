//! Full-width source prefixes through original witness, journal and effect paths.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationEvidence, FilePublicationInputs, FileWitnessInput, MAX_REPLAY_PREFIX,
};
use fa_reference::full_input::ActualHelperInput;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::{Error, Snapshot};

fn key() -> ProjectionKey { ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 } }
fn marker(end: u64) -> TrustedClosingMarker { TrustedClosingMarker { key: key(), final_sequence: end, marker_generation: 7 } }
fn input(end: u64, revision: u64, keys: &[u64], admitted: bool, opaque: Option<ActualHelperInput>) -> FilePublicationInputs {
    let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
    if admitted {
        if end != 0 { frontiers.accept_contiguous(key(), FrontierStage::Authenticated, 1, end).unwrap(); }
        frontiers.record_close(marker(end)).unwrap();
    }
    let witness = FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key()), DomainClosure::Closed(marker(end))),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationInputs::new(Some(witness), opaque)
}
fn prepared(root: &Directory, end: u64) -> (FileOversight, FileHumanReviewer, Keys) {
    let (mut host, reviewer) = fixture::source_host(root);
    let (action, inputs) = fixture::reviewed(&mut host, 1, b"visible");
    let current = input(end, 10, &[0, 2, 4], true, Some(inputs.views()["alpha"].actual_input().clone()));
    let evidence = FilePublicationEvidence::new(current.clone(), fixture::requests()).unwrap();
    host.bind_publication_evidence(host.revision(), 1, evidence).unwrap();
    host.record_publication_inputs(host.revision(), 1, 0, Some(current.clone())).unwrap();
    // The existing action-scoped interchange also preserves its inner packet.
    let capture = FilePublicationCapture::new(1, FileCaptureIdentity { source: 91, generation: 1 }, &action, current).unwrap();
    assert_eq!(FilePublicationCapture::from_bytes(&capture.to_bytes().unwrap()).unwrap(), capture);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    (host, reviewer, Keys { action, inputs, automatic, human, request })
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn packet_versions_preserve_legacy_neighbors_and_bound_full_width_prefixes() {
    let short = input(MAX_REPLAY_PREFIX, 10, &[0, 2, 4], true, None).to_bytes().unwrap();
    assert_eq!(&short[..8], b"FAPWIN01");
    for end in [MAX_REPLAY_PREFIX + 1, 1_u64 << 40, u64::MAX] {
        let original = input(end, 10, &[0, 2, 4], true, None);
        let bytes = original.to_bytes().unwrap();
        assert_eq!(&bytes[..8], b"FAPWIN02"); assert_eq!(bytes.len(), short.len());
        let decoded = FilePublicationInputs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, original); assert_eq!(decoded.to_bytes().unwrap(), bytes);
        let reconstructed = decoded.materialize().unwrap();
        let (_, frontiers) = reconstructed.structured.as_ref().unwrap();
        assert_eq!(frontiers.frontier(key(), FrontierStage::Authenticated), Ok(end));
        assert_eq!(frontiers.frontier(key(), FrontierStage::Judged), Ok(0));
        assert_eq!(frontiers.closing_marker(key()), Some(marker(end)));
        let evidence = FilePublicationEvidence::new(original, fixture::requests()).unwrap();
        let bytes = evidence.to_bytes().unwrap(); assert_eq!(&bytes[..8], b"FAPWEV02");
        assert_eq!(FilePublicationEvidence::from_bytes(&bytes).unwrap(), evidence);
    }
}

#[test]
fn wrong_versions_truncation_and_trailing_bytes_cannot_bypass_legacy_admission() {
    for end in [MAX_REPLAY_PREFIX, MAX_REPLAY_PREFIX + 1] {
        let original = input(end, 10, &[0, 2, 4], true, None);
        let evidence = FilePublicationEvidence::new(original.clone(), fixture::requests()).unwrap();
        let mut bytes = original.to_bytes().unwrap(); bytes[7] = if end > MAX_REPLAY_PREFIX { b'1' } else { b'2' };
        assert_eq!(FilePublicationInputs::from_bytes(&bytes), Err(if end > MAX_REPLAY_PREFIX { Error::Limit } else { Error::Binding }));
        let mut bytes = evidence.to_bytes().unwrap(); bytes[7] = if end > MAX_REPLAY_PREFIX { b'1' } else { b'2' };
        assert_eq!(FilePublicationEvidence::from_bytes(&bytes), Err(if end > MAX_REPLAY_PREFIX { Error::Limit } else { Error::Binding }));
    }
    let original = input(u64::MAX, 10, &[0, 2, 4], true, None);
    let bytes = original.to_bytes().unwrap();
    for length in 0..bytes.len() { assert!(FilePublicationInputs::from_bytes(&bytes[..length]).is_err()); }
    let mut trailing = bytes; trailing.push(0);
    assert!(FilePublicationInputs::from_bytes(&trailing).is_err());
}

#[test]
fn a_large_asserted_close_without_independent_admission_cannot_establish_absence() {
    let missing = input(u64::MAX, 10, &[0, 2, 4], false, None);
    let bytes = missing.to_bytes().unwrap();
    // Version selection depends on admitted evidence, not the snapshot assertion.
    assert_eq!(&bytes[..8], b"FAPWIN01");
    let missing = FilePublicationInputs::from_bytes(&bytes).unwrap();
    assert_eq!(missing.structured().unwrap().admitted_close(), None);
    assert_eq!(FilePublicationEvidence::new(missing, fixture::requests()), Err(Error::Incomplete));
    let empty = input(0, 10, &[], true, None);
    assert_eq!(&empty.to_bytes().unwrap()[..8], b"FAPWIN01");
    assert!(FilePublicationEvidence::new(empty, vec![fa_reference::witness::WitnessRequest::AbsentKey { key: 1 }]).is_ok());
}

#[test]
fn large_closed_domains_publish_once_and_recover_the_original_settled_result() {
    for end in [MAX_REPLAY_PREFIX + 1, 1_u64 << 40, u64::MAX] {
        let root = Directory::new(); let (mut host, reviewer, keys) = prepared(&root, end);
        host.record_publication_inputs(host.revision(), 1, 1, Some(input(end, 11, &[0, 2, 4, 99], true,
            Some(keys.inputs.views()["alpha"].actual_input().clone())))).unwrap();
        fixture::dispatch(&mut host, &keys);
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, PublicationBasis::Revalidated);
        assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
        drop(reviewer); drop(host);
        let (mut recovered, _) = FileOversight::open_with_publication_validation(root.store(), profile(), fixture::limits()).unwrap();
        assert_eq!(recovered.retained_publication_evidence(1).unwrap().original().structured().unwrap().admitted_close(), Some(marker(end)));
        let historical = recovered.publish_checked(recovered.revision(), 1, None, Snapshot::default(), ElapsedTick(3)).unwrap();
        assert_eq!(historical.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(historical.outcome, result.outcome);
        assert_eq!(recovered.inspect().executions, 1); assert_eq!(recovered.inspect().control.ledger.charged, 16);
    }
}

#[test]
fn late_value_absence_range_and_closure_changes_still_seal_without_early_refund() {
    for (keys_now, admitted) in [(vec![2, 4], true), (vec![0, 1, 2, 4], true),
        (vec![0, 2, 3, 4], true), (vec![0, 2, 4, 7], true), (vec![0, 2, 4], false)] {
        let root = Directory::new(); let (mut host, _, keys) = prepared(&root, u64::MAX);
        fixture::dispatch(&mut host, &keys);
        host.record_publication_inputs(host.revision(), 1, 1, Some(input(u64::MAX, 11, &keys_now, admitted,
            Some(keys.inputs.views()["alpha"].actual_input().clone())))).unwrap();
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, PublicationBasis::Rejected(if admitted { Error::Stale } else { Error::Incomplete }));
        assert_eq!(result.outcome, sealed()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn compact_prefixes_do_not_narrow_the_whole_opaque_helper_input() {
    let root = Directory::new(); let (mut host, _, keys) = prepared(&root, u64::MAX);
    fixture::dispatch(&mut host, &keys);
    let original = keys.inputs.views()["alpha"].actual_input();
    let mut changed_profile = original.input_profile().clone(); changed_profile.model_epoch += 1;
    let changed = ActualHelperInput::new(original.submitted_bytes().to_vec(), changed_profile,
        original.ordered_parts().to_vec(), original.omissions().to_vec()).unwrap();
    host.record_publication_inputs(host.revision(), 1, 1, Some(input(u64::MAX, 11, &[0, 2, 4], true, Some(changed)))).unwrap();
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn compact_reconstruction_does_not_remove_the_exact_validation_work_budget() {
    let root = Directory::new(); let mut limits = fixture::limits(); limits.validation.steps = 0;
    let (mut host, _) = FileOversight::create_with_publication_validation(root.store(), profile(), limits).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (_, inputs) = fixture::reviewed(&mut host, 1, b"visible");
    let original = input(u64::MAX, 10, &[0, 2, 4], true, None);
    host.bind_publication_evidence(host.revision(), 1,
        FilePublicationEvidence::new(original.clone(), fixture::requests()).unwrap()).unwrap();
    host.record_publication_inputs(host.revision(), 1, 0, Some(original)).unwrap();
    assert!(matches!(host.authorize(host.revision(), 1, &inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.inspect().control.ledger.reserved, 0); assert_eq!(host.inspect().executions, 0);
}
