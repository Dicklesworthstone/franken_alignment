//! Original predictions, actual durable admissions and the original two-key gate.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_consistency.rs"] mod prediction;
use ordinary::*;
use prediction::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::activation::consistency::LikelihoodFactor;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyObserver};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn owner(root: &Directory) -> (FileOversight, fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer, FileConsistencyObserver) {
    let (mut host, human) = create(root);
    let observer = host.enable_action_consistency(host.revision(), configuration()).unwrap();
    (host, human, observer)
}

#[test]
fn measured_forecast_then_congress_and_human_key_reach_guarded_publication() {
    let root = Directory::new(); let (mut host, human, observer) = owner(&root);
    let p = forecast(&mut host, &observer, 1, 1, 1.0);
    assert_eq!(p.forecast().null_numerator(), 49152);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(1));
    let keys = ready(&mut host, &human, 1, b"risk is an operational category, not a label");
    let observation = host.action_consistency_observation(1).unwrap();
    assert!(observation.event()); assert!(!observation.crossed());
    assert_eq!(observation.factor(), LikelihoodFactor { numerator: 16384, denominator: 49152 });
    assert_eq!(host.inspect().executions, 0);
    dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
}

#[test]
fn downstream_refusals_consume_forecasts_and_can_cross_the_lifetime_process() {
    let root = Directory::new(); let (mut host, _, observer) = owner(&root);
    forecast(&mut host, &observer, 1, 1, -1.0);
    let mut incomplete = snapshot(); incomplete.complete = false;
    let before = host.revision();
    assert_eq!(host.propose_consistent(host.revision(), 1, spec(&host, b"risk one"), incomplete), Ok(Err(Error::Incomplete)));
    assert_eq!(host.revision(), before + 1);
    assert!(!host.inspect().control.ledger.stages.contains_key(&1));
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, None);
    assert_eq!(host.action_consistency_observation(1).unwrap().sample(), 1);
    // Neither reusing the action ID nor the ordinary convenience API can undo it.
    assert_eq!(host.propose(host.revision(), 1, spec(&host, b"safe substitute"), snapshot()), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    forecast(&mut host, &observer, 2, 2, -1.0);
    let mut denied = snapshot(); denied.values.insert(7, b"wrong".to_vec());
    host.propose(host.revision(), 2, spec(&host, b"risk two"), denied).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Denied);
    let state = host.action_consistency_snapshot().unwrap();
    assert_eq!(state.evidence.samples(), 2); assert_eq!(state.evidence.first_crossing(), Some(2));
    let source = frame(&host, 3, 1.0); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert_eq!(observer.forecast_action(&mut host, revision, 3, actor, &source), Ok(Err(Error::Incomplete)));
    assert_eq!(host.inspect().executions, 0);
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, state.evidence);
    assert!(host.action_consistency_required());
}

#[test]
fn crossing_does_not_let_unanimous_helpers_authorize_the_next_effect() {
    let root = Directory::new(); let (mut host, _, observer) = owner(&root);
    for id in 1..=2 {
        forecast(&mut host, &observer, id, id, -1.0);
        let action = host.propose(host.revision(), id, spec(&host, b"risk"), snapshot()).unwrap();
        if id == 1 { host.cancel(host.revision(), id).unwrap(); continue; }
        let current = inputs(&action, b"complete independent view");
        host.record_inputs(host.revision(), id, 0, current.clone()).unwrap();
        host.begin_review(host.revision(), id, 102, ROOT, window(&host), snapshot()).unwrap();
        votes(&mut host, 102, Verdict::Allow);
        assert_eq!(host.finish_review(host.revision(), 102, Some(&current), snapshot()), Ok(Err(Error::Incomplete)));
        assert!(host.authorize(host.revision(), id, &current, snapshot()).is_err());
    }
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn pending_prediction_blocks_prior_dispatch_publication_but_not_reconciliation() {
    let root = Directory::new(); let (mut host, human, observer) = owner(&root);
    forecast(&mut host, &observer, 1, 1, 0.0);
    let keys = ready(&mut host, &human, 1, b"safe"); dispatch(&mut host, &keys);
    forecast(&mut host, &observer, 2, 2, 0.0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert!(matches!(outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(2));
}

#[test]
fn recovery_censors_only_unresolved_forecasts_and_never_refreshes_the_error_budget() {
    for pending in [false, true] {
        let root = Directory::new(); let (mut host, _, old) = owner(&root);
        forecast(&mut host, &old, 1, 1, 0.0);
        if !pending { host.propose(host.revision(), 1, spec(&host, b"safe"), snapshot()).unwrap(); }
        let before = host.action_consistency_snapshot().unwrap(); drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let after = host.action_consistency_snapshot().unwrap();
        assert_eq!(after.evidence, before.evidence);
        assert_eq!(after.pending_attempt, before.pending_attempt);
        assert_eq!(after.coverage_lost, pending); assert!(!host.clock_ready());
        let source = frame(&host, 2, 0.0); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
        assert_eq!(old.forecast_action(&mut host, revision, 2, actor, &source), Err(JournalError::Contract(Error::Binding)));
        assert!(host.enable_action_consistency(host.revision(), configuration()).is_err());
    }
}

#[test]
fn expired_pending_forecast_and_prediction_capacity_refusal_are_not_recapture_loopholes() {
    let root = Directory::new(); let (mut host, _, observer) = owner(&root);
    forecast(&mut host, &observer, 1, 1, -1.0);
    host.observe_time(host.revision(), ElapsedTick(11)).unwrap();
    assert_eq!(host.propose_consistent(host.revision(), 1, spec(&host, b"risk"), snapshot()), Ok(Err(Error::Stale)));
    let revision = host.revision(); observer.unavailable(&mut host, revision).unwrap();
    let state = host.action_consistency_snapshot().unwrap();
    assert!(state.coverage_lost); assert_eq!(state.pending_attempt, Some(1)); assert_eq!(state.evidence.samples(), 0);

    let root = Directory::new(); let (mut host, _) = create(&root); let mut p = parameters(); p.max_predictions = 1;
    let observer = host.enable_action_consistency(host.revision(), FileConsistencyConfig::new(p).unwrap()).unwrap();
    forecast(&mut host, &observer, 1, 1, 0.0);
    host.propose(host.revision(), 1, spec(&host, b"safe"), snapshot()).unwrap();
    let source = frame(&host, 2, 0.0); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert_eq!(observer.forecast_action(&mut host, revision, 2, actor, &source), Ok(Err(Error::Limit)));
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
}

#[test]
fn source_binding_refusal_has_a_valid_control_and_foreign_observers_cannot_predict() {
    let a = Directory::new(); let b = Directory::new();
    let (mut host, _, observer) = owner(&a); let (_, _, other) = owner(&b);
    let source = frame(&host, 1, 0.0); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert_eq!(other.forecast_action(&mut host, revision, 1, actor, &source), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision);
    assert_eq!(observer.forecast_action(&mut host, revision, 1, actor + 1, &source), Ok(Err(Error::Stale)));
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    forecast(&mut host, &observer, 1, 1, 0.0);
    host.propose_consistent(host.revision(), 1, spec(&host, b"safe"), snapshot()).unwrap().unwrap();
}

#[test]
fn unavailable_clock_and_invalid_request_preflights_leave_a_usable_owner() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_action_consistency(host.revision(), configuration()).unwrap();
    let source = frame(&host, 1, 0.0); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert_eq!(observer.forecast_action(&mut host, revision, 1, actor, &source), Err(JournalError::Contract(Error::Incomplete)));
    assert!(host.storage_failure().is_none());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(host.submit_request(host.revision(), 0, spec(&host, b"safe"), snapshot()).is_err());
    assert!(host.storage_failure().is_none());
    forecast(&mut host, &observer, 1, 1, 0.0);
    host.submit_request(host.revision(), 19, spec(&host, b"safe"), snapshot()).unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
}
