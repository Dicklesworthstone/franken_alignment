//! Request identity is captured from the original durable book, not inferred
//! from attempt numbers or looked up again while an older page is consumed.
use super::*;
use crate::action::consequence::delivery::persistent::requests::{
    FileRequestDisposition, FileRequestStatus,
    actor::{FileInspectionPage, FileInspectionResult},
};
use crate::action::consequence::oversight::actor::ActorError;

fn submit(host: &mut FileOversight, request: u64, stale_target: bool) -> FileRequestStatus {
    let mut target = host.inspect().target;
    if stale_target {
        target.expected_version += 1;
    }
    let spec = ActionSpec {
        version: VERSION,
        scope: profile().delivery.scope,
        target: Some(target),
        payload: b"secret-payload".to_vec(),
        required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
        units: 16,
    };
    host.submit_request(host.revision(), request, spec, snapshot())
        .unwrap()
}

fn links(rows: &[FileSituationAttempt]) -> BTreeMap<u64, Option<u64>> {
    rows.iter()
        .map(|row| (row.attempt, row.external_request))
        .collect()
}

fn inspected(result: FileInspectionResult) -> FileInspectionPage {
    match result.unwrap() {
        Knowledge::Known { value, basis } => {
            assert_eq!(basis, value.basis());
            value
        }
        other => panic!("expected original native situation: {other:?}"),
    }
}

#[test]
fn original_request_links_distinguish_refused_allocations_and_direct_attempts() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    propose(&mut host, 1, true);
    let refused = submit(&mut host, 42, true);
    assert_eq!(
        refused.disposition,
        FileRequestDisposition::NotAdmitted(Error::Stale)
    );
    // A trusted direct proposal may use the refused allocation's number. The
    // refused external key still did not admit that action and cannot label it.
    propose(&mut host, 2, true);
    let full_width = submit(&mut host, u64::MAX, false);
    assert!(matches!(
        full_width.disposition,
        FileRequestDisposition::Admitted { attempt: 3, .. }
    ));
    // This external key happens to equal the DIRECT attempt's numeric ID. It
    // belongs to its own newly allocated attempt and cannot relabel that row.
    let matching_number = submit(&mut host, 1, false);
    assert!(matches!(
        matching_number.disposition,
        FileRequestDisposition::Admitted { attempt: 4, .. }
    ));
    let before = host.inspect();
    let files = root.files();
    let report = host.situation().unwrap();
    let (observed, basis) = page(&report, None, 64);
    assert_eq!(observed.summary.attempts, 4);
    assert_eq!(observed.summary.retained_requests, 3);
    assert_eq!(
        links(&observed.attempts),
        BTreeMap::from([(1, None), (2, None), (3, Some(u64::MAX)), (4, Some(1)),])
    );
    assert_eq!(basis.scope, profile().delivery.scope);
    for row in &observed.attempts {
        assert_eq!(
            row.address.to_string(),
            format!("fa://1/attempt/{}", row.attempt)
        );
    }
    assert_eq!(host.inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn native_granted_pages_keep_captured_request_links_when_the_owner_advances() {
    let root = Root::new();
    let mut configuration = profile();
    configuration.delivery.max_attempts = 32;
    let (mut host, _reviewer) = FileOversight::create(root.store(), configuration).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for request in 9001..=9017 {
        submit(&mut host, request, false);
    }
    propose(&mut host, 18, true);
    let (_actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let (foreign, _foreign_grant) = supervisor.inspection_port(2).unwrap();
    let first = inspected(port.situation());
    let cursor = first.continuation().unwrap().clone();
    assert_eq!(
        links(first.attempts()),
        (1_u64..=16)
            .map(|attempt| (attempt, Some(9000 + attempt)))
            .collect::<BTreeMap<_, _>>()
    );
    let other = inspected(foreign.situation());
    assert_eq!(
        port.situation_page(other.continuation()),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 3);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.cancel_request(revision, 9001).unwrap();
        submit(&mut host, 40001, false);
    }
    let advanced = supervisor.host().unwrap().inspect();
    let files = root.files();
    let tail = inspected(port.clone().situation_page(Some(&cursor)));
    assert_eq!(tail.basis(), first.basis());
    assert_eq!(tail.summary(), first.summary());
    assert_eq!(tail.summary().attempts, 18);
    assert_eq!(tail.summary().terminal_attempts, 0);
    assert_eq!(
        links(tail.attempts()),
        BTreeMap::from([(17, Some(9017)), (18, None)])
    );
    assert!(tail.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(
        port.situation_page(Some(&cursor)),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 2);

    let fresh = inspected(port.situation());
    assert_ne!(fresh.basis(), first.basis());
    assert_eq!(fresh.summary().attempts, 19);
    assert_eq!(fresh.summary().terminal_attempts, 1);
    let fresh_tail = inspected(port.situation_page(fresh.continuation()));
    assert_eq!(
        links(fresh_tail.attempts()),
        BTreeMap::from([(18, None), (19, Some(40001)), (1, Some(9001)),])
    );
    assert_eq!(
        fresh_tail.attempts().last().unwrap().stage,
        ActionState::Cancelled
    );
    assert!(fresh_tail.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), advanced);
    assert_eq!(root.files(), files);
}

#[test]
fn canonical_replay_and_reopened_owner_preserve_original_request_identity() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    propose(&mut host, 1, true);
    submit(&mut host, 42, true);
    submit(&mut host, 7001, false);
    submit(&mut host, u64::MAX, false);
    let captured = host.situation().unwrap();
    let (original, original_basis) = page(&captured, None, 64);
    let expected = links(&original.attempts);
    let anchor = host.history_anchor().unwrap();
    let files = root.files();
    let replayed = FileOversight::read_situation(root.store(), &profile(), Some(&anchor)).unwrap();
    let (canonical, canonical_basis) = page(&replayed, None, 64);
    assert_eq!(canonical.attempts, original.attempts);
    assert_eq!(canonical_basis.origin, FileSituationOrigin::CanonicalImage);
    assert_eq!(
        canonical_basis.anchored_through,
        Some(original_basis.journal_revision)
    );
    assert_eq!(root.files(), files);
    drop(host);
    drop(reviewer);

    let (reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let before = reopened.inspect();
    let files = root.files();
    let (current, basis) = page(&reopened.situation().unwrap(), None, 64);
    assert_eq!(links(&current.attempts), expected);
    assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
    assert!(basis.journal_revision > original_basis.journal_revision);
    assert!(
        current
            .attempts
            .iter()
            .all(|row| row.stage == ActionState::Cancelled)
    );
    assert_eq!(
        reopened.request_status(42).unwrap().disposition,
        FileRequestDisposition::NotAdmitted(Error::Stale)
    );
    assert_eq!(links(&page(&captured, None, 64).0.attempts), expected);
    assert_eq!(reopened.inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn equal_counter_owners_cannot_rebind_request_links_through_a_foreign_page() {
    let left = Root::new();
    let right = Root::new();
    let (mut a, _reviewer_a) = create(&left);
    let (mut b, _reviewer_b) = create(&right);
    for request in [7001, 7002] {
        submit(&mut a, request, false);
    }
    for request in [8001, 8002] {
        submit(&mut b, request, false);
    }
    let a = a.situation().unwrap();
    let b = b.situation().unwrap();
    let (first_a, basis_a) = page(&a, None, 1);
    let (first_b, basis_b) = page(&b, None, 1);
    assert_eq!(
        basis_a, basis_b,
        "equal numeric cuts are not capture identity"
    );
    assert_eq!(first_a.attempts[0].external_request, Some(7001));
    assert_eq!(first_b.attempts[0].external_request, Some(8001));
    let token_a = first_a.next.unwrap();
    assert!(matches!(
        b.page(Some(&token_a), 1),
        Err(JournalError::Contract(Error::Binding))
    ));
    let (last_a, _) = page(&a, Some(&token_a), 1);
    let (last_b, _) = page(&b, first_b.next.as_ref(), 1);
    assert_eq!(last_a.attempts[0].external_request, Some(7002));
    assert_eq!(last_b.attempts[0].external_request, Some(8002));
}
