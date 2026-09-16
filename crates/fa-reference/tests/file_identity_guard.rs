//! Original full-input review, human keys and endpoint accounting under identity liveness.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
#[path = "support/file_identity.rs"] mod identity;
use fixture::*;
use fa_reference::action::{ElapsedTick, ActionState};
use fa_reference::action::consequence::activation::SourceFrame;
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelPassport};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityStatus, IdentityMismatch};
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};

#[test]
fn a_matched_identity_requires_fresh_congress_and_both_effect_keys() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let observer = identity::enable(&mut host);
    assert!(host.publication_guard_required());
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let action = host.propose(host.revision(), 1, spec(&host, b"held"), snapshot()).unwrap();
    let input = inputs(&action, b"real review input");
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
    votes(&mut host, 101, Verdict::Allow);
    assert_eq!(host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap(), Err(Error::Incomplete));
    identity::matched(&mut host, &observer, 1, 1);
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    let keys = ready(&mut host, &reviewer, 2, b"allowed"); dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 2), Err(JournalError::Contract(Error::Incomplete)));
    let result = host.publish_checked(host.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn identity_expiry_is_rechecked_after_dispatch_before_first_publication() {
    for expired in [false, true] {
        let root = Directory::new(); let (mut host, reviewer) = create(&root);
        let observer = identity::enable(&mut host); let check = identity::matched(&mut host, &observer, 1, 1);
        let keys = ready(&mut host, &reviewer, 1, b"liveness"); dispatch(&mut host, &keys);
        let at = if expired { check.evidence().valid_until() } else { ElapsedTick(2) };
        assert!(at < keys.request.evidence().expires_at());
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), at).unwrap();
        let expected = if expired { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
            else { EndpointOutcome::Executed { resulting_version: 2 } };
        assert_eq!(result.outcome, expected);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(expected));
        assert_eq!(host.inspect().control.ledger.charged, if expired { 0 } else { 16 });
    }
}

#[test]
fn manifest_mismatch_fences_immediately_without_refunding_an_admitted_effect() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let observer = identity::enable(&mut host); identity::matched(&mut host, &observer, 1, 1);
    let keys = ready(&mut host, &reviewer, 1, b"never publish"); dispatch(&mut host, &keys);
    let check = identity::begin(&mut host, 2);
    let mut manifest = identity::manifest(); manifest.weights[0] ^= 1;
    let revision = host.revision();
    let result = observer.observe_manifest(&mut host, revision, &check, manifest, ElapsedTick(1)).unwrap();
    assert_eq!(result.measurement.unwrap().outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
    let containment = result.containment.unwrap().unwrap();
    assert_eq!(containment.refunded_units, 0);
    assert!(host.inspect().control.suspended);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.charged, 0);
    let basis = host.identity_basis().unwrap(); host.identity_unavailable(host.revision(), basis).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
}

#[test]
fn equal_shape_wrong_tap_and_outside_coordinate_are_real_identity_incidents() {
    for kind in 0..3 {
        let root = Directory::new(); let (mut host, _) = create(&root);
        let observer = identity::enable(&mut host); let check = identity::begin(&mut host, 1);
        let revision = host.revision();
        observer.observe_manifest(&mut host, revision, &check, identity::manifest(), ElapsedTick(1)).unwrap().measurement.unwrap();
        let mut frame = identity::frame(1, &[0.0, 1.0]);
        if kind == 1 {
            let mut id = frame.identity(); id.profile.tap += 1;
            frame = SourceFrame::capture(id, &[0.0, 1.0]).unwrap();
        } else if kind == 2 { frame = identity::frame(1, &[0.0, 3.0]); }
        let revision = host.revision();
        let result = observer.observe_anchor(&mut host, revision, &check, 10, &frame, ElapsedTick(1)).unwrap();
        if kind == 0 {
            assert_eq!(result.measurement.unwrap().outcome, IdentityOutcome::Matched);
            assert!(result.containment.is_none()); identity::install(&mut host, &check);
            assert!(!host.inspect().control.suspended);
        } else {
            assert!(matches!(result.measurement.unwrap().outcome, IdentityOutcome::Mismatch(_)));
            result.containment.unwrap().unwrap(); assert!(host.inspect().control.suspended);
        }
    }
}

#[test]
fn native_capacity_and_timeout_refusals_are_committed_not_rolled_back() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let mut policy = identity::policy(); policy.max_checks = 1;
    let observer = host.enable_identity_checks(host.revision(), identity::passport(), policy).unwrap();
    identity::matched(&mut host, &observer, 1, 1);
    let keys = ready(&mut host, &reviewer, 1, b"reserved");
    let revision = host.revision(); let control = host.inspect().control;
    let actor = host.actor_snapshot().unwrap().actor_revision;
    assert!(matches!(host.begin_identity_check(revision, 2, control.sequence, actor).unwrap(), Err(Error::Limit)));
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.reserved, 16);

    let root = Directory::new(); let (mut host, _) = create(&root); let observer = identity::enable(&mut host);
    let check = identity::begin(&mut host, 1); let revision = host.revision();
    let result = observer.observe_manifest(&mut host, revision, &check, identity::manifest(), check.evidence().deadline()).unwrap();
    assert_eq!(result.measurement, Err(Error::Stale)); assert!(result.containment.is_none());
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Expired);
    identity::matched(&mut host, &observer, 2, 1);
}

#[test]
fn recovery_retains_floors_but_requires_new_observer_challenge_and_measurement() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let observer = identity::enable(&mut host); let old = identity::matched(&mut host, &observer, 1, 1);
    let keys = ready(&mut host, &reviewer, 1, b"recoverable"); dispatch(&mut host, &keys);
    drop(host);
    let (mut host, _, fresh) = FileOversight::open_with_identity_observer(root.store(), profile(), &identity::passport(), identity::policy()).unwrap();
    assert_eq!(host.identity_status(), Err(JournalError::Contract(Error::Incomplete)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
    let revision = host.revision();
    assert!(matches!(observer.observe_manifest(&mut host, revision, &old, identity::manifest(), ElapsedTick(2)), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(fresh.observe_manifest(&mut host, revision, &old, identity::manifest(), ElapsedTick(2)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.revision(), revision);
    let check = identity::begin(&mut host, 2); let revision = host.revision();
    fresh.observe_manifest(&mut host, revision, &check, identity::manifest(), ElapsedTick(2)).unwrap().measurement.unwrap();
    let revision = host.revision();
    assert_eq!(fresh.observe_anchor(&mut host, revision, &check, 10, &identity::frame(1, &[0.0, 1.0]), ElapsedTick(2)).unwrap().measurement, Err(Error::Stale));
    let revision = host.revision();
    fresh.observe_anchor(&mut host, revision, &check, 10, &identity::frame(2, &[0.0, 1.0]), ElapsedTick(2)).unwrap().measurement.unwrap();
    identity::install(&mut host, &check);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn pinned_reopen_checks_passport_and_policy_before_mutating_canonical_history() {
    let root = Directory::new(); let (mut host, _) = create(&root); identity::enable(&mut host); drop(host);
    let path = root.store().join("delivery.bin"); let before = std::fs::read(&path).unwrap();
    let wrong = ModelPassport::new(52, 1, identity::manifest(), vec![IdentityAnchor::new(10,
        identity::capture_profile(), 5, vec![7], &[[-1.0, 1.0], [0.0, 2.0]]).unwrap()]).unwrap();
    assert!(matches!(FileOversight::open_with_identity_observer(root.store(), profile(), &wrong, identity::policy()), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut policy = identity::policy(); policy.validity_ticks += 1;
    assert!(FileOversight::open_with_identity_observer(root.store(), profile(), &identity::passport(), policy).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (_, _, _) = FileOversight::open_with_identity_observer(root.store(), profile(), &identity::passport(), identity::policy()).unwrap();
}

#[test]
fn stop_and_historical_execution_do_not_depend_on_current_identity_availability() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); let observer = identity::enable(&mut host);
    identity::matched(&mut host, &observer, 1, 1); let keys = ready(&mut host, &reviewer, 1, b"already visible");
    dispatch(&mut host, &keys);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let basis = host.identity_basis().unwrap(); host.identity_unavailable(host.revision(), basis).unwrap();
    let historical = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(2)).unwrap();
    assert_eq!(historical.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(historical.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 3, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(3)).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().payload, b"already visible");
}
