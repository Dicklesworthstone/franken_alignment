//! Real actor requests consume only their own pre-payload forecast.
#![cfg(unix)]
#[path = "support/file_consistency.rs"] mod fixture;
#[path = "support/file_oversight.rs"] mod ordinary;
use fixture::*;
use ordinary::Directory;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::*;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::{Error, Snapshot};

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn create(root: &Directory) -> (FileOversight, FilePredictiveRoles) {
    let (mut host, roles) = FileOversight::create_predictive_guarded(root.store(), ordinary::profile(),
        &guards(), None, configuration(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}
fn expected(host: &FileOversight) -> FilePredictiveRequirements {
    let control = host.inspect();
    FilePredictiveRequirements { oversight: FileRecoveryRequirements {
        guards: guards(), effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.control.sequence,
            authority_epoch: control.control.ledger.epoch } }, prediction: configuration(), evaluation: None }
}
fn keyed(host: &mut FileOversight, roles: &FilePredictiveRoles, request: u64, sequence: u64) {
    let source = frame(host, sequence, -1.0); let revision = host.revision();
    let actor = host.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.forecast_request(host, revision, request, actor, &source).unwrap().unwrap();
}

#[test]
fn actor_gateway_uses_external_key_and_original_allocator_through_two_key_publication() {
    let root = Directory::new(); let (mut host, roles) = create(&root);
    // A prior direct attempt makes the internal allocator differ from both the
    // external key and one. The old raw-forecast profile still works unchanged.
    forecast(&mut host, &roles.consistency_observer, 40, 1, -1.0);
    let mut denied = ordinary::snapshot(); denied.values.insert(7, b"not ok".to_vec());
    host.propose_consistent(host.revision(), 40, ordinary::spec(&host, b"ordinary"), denied).unwrap().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&40], ActionState::Denied);
    keyed(&mut host, &roles, 9000, 2);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(9000));
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(41));
    let spec = ordinary::spec(&host, b"ordinary");
    let before = host.inspect(); let evidence = host.action_consistency_snapshot().unwrap();
    assert!(matches!(host.submit_request(host.revision(), 9001, spec.clone(), ordinary::snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(host.propose_consistent(host.revision(), 41, spec.clone(), ordinary::snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.inspect(), before); assert_eq!(host.action_consistency_snapshot().unwrap(), evidence);
    assert!(host.storage_failure().is_none());

    let proposal = ActorProposal { target: spec.target.unwrap(), payload: spec.payload.clone(),
        expected_policy_epoch: spec.policy_epoch, deadline: spec.deadline, units: spec.units };
    let (port, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(ordinary::snapshot())).unwrap();
    let ticket = port.submit(9000, &proposal).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    {
        let mut host = supervisor.host_mut().unwrap();
        assert_eq!(host.request_status(9000).unwrap().disposition,
            FileRequestDisposition::Admitted { attempt: 41, stage: ActionState::Reviewing });
        assert_eq!(host.pending_forecast_request().unwrap(), None);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
        let action = host.request_action(9000).unwrap().clone();
        let inputs = ordinary::inputs(&action, b"original complete evidence");
        ordinary::review_existing(&mut host, 41, 141, &inputs);
        let revision = host.revision();
        let automatic = host.authorize(revision, 41, &inputs, ordinary::snapshot()).unwrap();
        let revision = host.revision();
        let request = host.request_human_approval(revision, 1041, 41, &inputs, ElapsedTick(20)).unwrap();
        let revision = host.revision();
        let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &human, &action, &inputs, ordinary::snapshot()).unwrap();
        let revision = host.revision();
        assert_eq!(host.publish_checked(revision, 41, Some(&inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap().outcome,
            EndpointOutcome::Executed { resulting_version: 2 });
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        assert_eq!(host.reconcile(revision, 41).unwrap(),
            Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    // Historical exact retry needs no new forecast or admission snapshot.
    let before = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    port.submit(9000, &proposal).unwrap();
    assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap(), before);
}

#[test]
fn consumed_refusals_remain_in_allocator_and_exact_retries_cannot_take_the_next_forecast() {
    let root = Directory::new(); let (mut host, roles) = create(&root);
    keyed(&mut host, &roles, 400, 1);
    let spec = ordinary::spec(&host, b"ordinary");
    let status = host.submit_request(host.revision(), 400, spec.clone(), Snapshot::default()).unwrap();
    assert_eq!(status.disposition, FileRequestDisposition::NotAdmitted(Error::Incomplete));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert!(!host.inspect().control.ledger.stages.contains_key(&1));
    keyed(&mut host, &roles, 500, 2);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(2));
    let before = host.action_consistency_snapshot().unwrap();
    assert_eq!(host.submit_request(0, 400, spec.clone(), ordinary::snapshot()).unwrap(), status);
    assert_eq!(host.action_consistency_snapshot().unwrap(), before);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(500));
    let mut denied = ordinary::snapshot(); denied.values.insert(7, b"not ok".to_vec());
    assert_eq!(host.submit_request(host.revision(), 500, spec.clone(), denied).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Denied });
    let before = host.action_consistency_snapshot().unwrap();
    let source = frame(&host, 3, -1.0); let revision = host.revision();
    let actor = host.actor_snapshot().unwrap().actor_revision;
    assert!(matches!(roles.consistency_observer.forecast_request(&mut host, revision, 500, actor, &source),
        Err(JournalError::Contract(Error::Duplicate))));
    let mut changed = spec; changed.payload.push(b'!');
    assert!(matches!(host.submit_request(0, 500, changed, ordinary::snapshot()), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.action_consistency_snapshot().unwrap(), before);
    assert_eq!(before.evidence.samples(), 2); assert_eq!(host.pending_forecast_request().unwrap(), None);
}

#[test]
fn expired_key_is_terminal_without_turning_its_unobserved_forecast_into_free_evidence() {
    let root = Directory::new(); let (mut host, roles) = create(&root);
    keyed(&mut host, &roles, 77, 1);
    host.observe_time(host.revision(), ElapsedTick(11)).unwrap();
    let spec = ordinary::spec(&host, b"ordinary");
    let status = host.submit_request(host.revision(), 77, spec.clone(), ordinary::snapshot()).unwrap();
    assert_eq!(status.disposition, FileRequestDisposition::NotAdmitted(Error::Stale));
    assert_eq!(host.pending_forecast_request().unwrap(), Some(77));
    let state = host.action_consistency_snapshot().unwrap();
    assert!(state.coverage_lost); assert_eq!(state.pending_attempt, Some(1)); assert_eq!(state.evidence.samples(), 0);
    let expected = expected(&host); drop(host);
    let (mut host, roles) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(77));
    assert_eq!(host.submit_request(0, 77, spec, Snapshot::default()).unwrap(), status);
    host.observe_time(host.revision(), ElapsedTick(12)).unwrap();
    let source = frame(&host, 2, -1.0); let revision = host.revision();
    let actor = host.actor_snapshot().unwrap().actor_revision;
    assert!(roles.consistency_observer.forecast_request(&mut host, revision, 88, actor, &source).is_err());
    assert_eq!(host.revision(), revision); assert!(host.storage_failure().is_none());
}

#[test]
fn clean_recovery_reprovisions_observer_but_never_reuses_old_request_ids_or_source_sequences() {
    let root = Directory::new(); let (mut host, old) = create(&root);
    keyed(&mut host, &old, 77, 1);
    host.submit_request(host.revision(), 77, ordinary::spec(&host, b"ordinary"), Snapshot::default()).unwrap();
    let expected = expected(&host); let evidence = host.action_consistency_snapshot().unwrap().evidence; drop(host);
    let (mut host, roles) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let stale = frame(&host, 1, -1.0); let revision = host.revision();
    let actor = host.actor_snapshot().unwrap().actor_revision;
    assert_eq!(roles.consistency_observer.forecast_request(&mut host, revision, 88, actor, &stale).unwrap(), Err(Error::Stale));
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    let fresh = frame(&host, 2, -1.0); let revision = host.revision();
    assert!(matches!(old.consistency_observer.forecast_request(&mut host, revision, 88, actor, &fresh),
        Err(JournalError::Contract(Error::Binding))));
    keyed(&mut host, &roles, 88, 2);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(2));
    assert_eq!(host.submit_request(host.revision(), 88, ordinary::spec(&host, b"ordinary"), ordinary::snapshot()).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Reviewing });
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
}
