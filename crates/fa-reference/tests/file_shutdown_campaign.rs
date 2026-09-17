//! Multiple actual owners, original publication and distinct stop/drain cuts.
#![cfg(unix)]
#[path = "support/file_shutdown.rs"] mod support;
use support::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{StopRequest, EndpointOutcome};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::*;
use fa_reference::Error;

#[test]
fn terminal_campaign_stops_every_owner_but_refunds_only_original_nonexecution() {
    let a = Directory::new(); let b = Directory::new();
    let (mut left, human_left) = create(&a, 1);
    let (mut right, human_right) = create(&b, 2);
    let plan = plan(vec![right.shutdown_domain(20).unwrap(), left.shutdown_domain(10).unwrap()]);
    assert_eq!(plan.domains().iter().map(FileShutdownDomain::id).collect::<Vec<_>>(), vec![10, 20]);
    let mut campaign = plan.start();
    assert_eq!(campaign.report().unobserved_stops(), vec![10, 20]);
    // Work AFTER registration may change the control sequence. Shutdown uses
    // the current native predecessor, not a stale fabricated authority command.
    let executed = ready(&mut left, &human_left, 1, 1, b"already public");
    dispatch(&mut left, &executed);
    left.publish_checked(left.revision(), 1, Some(&executed.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let unsent = ready(&mut left, &human_left, 1, 2, b"never public");
    let pending = ready(&mut right, &human_right, 2, 1, b"unknown dispatch");
    dispatch(&mut right, &pending);
    let before_payload = left.inspect().payload;
    let stopped = campaign.advance(10, &mut left, ElapsedTick(2)).unwrap();
    let progress = stopped.stop.unwrap();
    assert!(!progress.endpoint_fenced); assert_eq!(progress.charged_units, 16);
    assert_eq!(progress.reserved_units, 0); assert_eq!(progress.unresolved, vec![1]);
    assert_eq!(stopped.dispatches_since_registration, vec![1]);
    assert_eq!(left.inspect().control.ledger.stages[&2], ActionState::Cancelled);
    assert!(left.dispatch(left.revision(), &unsent.automatic, &unsent.human,
        &unsent.action, &unsent.inputs, snapshot()).is_err());
    campaign.advance(20, &mut right, ElapsedTick(2)).unwrap();
    assert!(campaign.report().all_observed_stopped());
    assert!(!campaign.report().all_observed_drained());
    assert_eq!(right.inspect().control.ledger.charged, 16);
    campaign.advance(20, &mut right, ElapsedTick(2)).unwrap();
    assert_eq!(right.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(right.inspect().control.ledger.charged, 0);
    campaign.advance(10, &mut left, ElapsedTick(2)).unwrap();
    assert_eq!(left.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(left.inspect().control.ledger.charged, 16);
    assert_eq!(left.inspect().executions, 1); assert_eq!(left.inspect().payload, before_payload);
    assert!(campaign.report().all_observed_drained());
    let report = campaign.report();
    let left_sweep = &report.domains[0].last_observation.as_ref().unwrap().last_drain.as_ref().unwrap().sweep;
    let right_sweep = &report.domains[1].last_observation.as_ref().unwrap().last_drain.as_ref().unwrap().sweep;
    assert!(matches!(left_sweep.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { .. }))));
    assert!(matches!(right_sweep.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }))));
    let revision = left.revision();
    campaign.advance(10, &mut left, ElapsedTick(0)).unwrap(); // pure inspection, no clock rewind
    assert_eq!(left.revision(), revision);
    assert!(left.propose(left.revision(), 3, spec(&left, 1, b"not reopened"), snapshot()).is_err());
}

#[test]
fn unreachable_domain_remains_in_denominator_while_other_owners_drain() {
    let a = Directory::new(); let b = Directory::new();
    let (mut left, _) = create(&a, 1); let (right, _) = create(&b, 2);
    let plan = plan(vec![left.shutdown_domain(1).unwrap(), right.shutdown_domain(2).unwrap()]);
    drop(right);
    let mut campaign = plan.start();
    campaign.unavailable(2, JournalError::Busy).unwrap();
    campaign.advance(1, &mut left, ElapsedTick(2)).unwrap();
    campaign.advance(1, &mut left, ElapsedTick(2)).unwrap();
    assert_eq!(campaign.report().domains.len(), 2);
    assert_eq!(campaign.report().unobserved_stops(), vec![2]);
    assert!(!campaign.report().all_observed_drained());
    let (mut recovered, _) = FileOversight::open(b.store(), profile(2)).unwrap();
    // A fresh recovery owner descends from the exact saved history. No current
    // evidence or clock is needed for the native stop itself.
    assert!(!recovered.clock_ready());
    campaign.advance(2, &mut recovered, ElapsedTick(2)).unwrap();
    campaign.advance(2, &mut recovered, ElapsedTick(2)).unwrap();
    let report = campaign.report();
    assert!(report.all_observed_drained()); assert_eq!(report.attempts.len(), 5);
    assert_eq!(report.attempts[0].result, FileShutdownResult::Refused(JournalError::Busy));
}

#[test]
fn failed_latest_visit_does_not_reuse_an_old_success_or_block_a_different_member() {
    let a = Directory::new(); let b = Directory::new();
    let (mut left, _) = create(&a, 1); let (mut right, _) = create(&b, 2);
    let mut campaign = plan(vec![left.shutdown_domain(1).unwrap(), right.shutdown_domain(2).unwrap()]).start();
    campaign.advance(1, &mut left, ElapsedTick(2)).unwrap();
    campaign.advance(1, &mut left, ElapsedTick(2)).unwrap();
    campaign.unavailable(1, JournalError::Unavailable).unwrap();
    assert!(campaign.report().domains[0].last_observation.as_ref().unwrap().stop.as_ref().unwrap().drained());
    assert!(!campaign.report().domains[0].latest_succeeded);
    assert_eq!(campaign.advance(2, &mut left, ElapsedTick(2)), Err(JournalError::Contract(Error::Binding)));
    assert!(right.inspect().stop.is_none());
    campaign.advance(2, &mut right, ElapsedTick(2)).unwrap();
    campaign.advance(2, &mut right, ElapsedTick(2)).unwrap();
    assert!(!campaign.report().all_observed_drained());
    campaign.advance(1, &mut left, ElapsedTick(2)).unwrap();
    assert!(campaign.report().all_observed_drained());
    assert_eq!(campaign.report().attempts.iter().filter(|row| matches!(row.result, FileShutdownResult::Refused(_))).count(), 2);
}

#[test]
fn conflicting_native_stop_is_not_relabelled_as_this_campaigns_acknowledgment() {
    let root = Directory::new(); let (mut host, _) = create(&root, 1);
    let mut campaign = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    let before = host.inspect();
    host.request_stop(host.revision(), StopRequest { operation: 901,
        expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch }).unwrap();
    let stopped = host.inspect();
    assert_eq!(campaign.advance(1, &mut host, ElapsedTick(2)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), stopped); assert_eq!(campaign.report().unobserved_stops(), vec![1]);
    // The original independent stop can still be drained, without pretending it
    // was an acknowledgment of operation 900.
    assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    assert!(!campaign.report().all_observed_stopped());
}

#[test]
fn fixed_registration_and_head_budgets_refuse_before_mutating_the_owner() {
    let root = Directory::new(); let (mut host, _) = create(&root, 1);
    let domain = host.shutdown_domain(1).unwrap();
    assert!(matches!(FileShutdownPlan::new(1, vec![], 4, 100), Err(Error::InvalidInput)));
    assert!(matches!(FileShutdownPlan::new(1, vec![domain.clone(), domain.clone()], 4,
        MAX_SHUTDOWN_HEAD_BYTES), Err(Error::Duplicate)));
    assert!(matches!(FileShutdownPlan::new(1, vec![domain.clone()], MAX_SHUTDOWN_ATTEMPTS + 1,
        MAX_SHUTDOWN_HEAD_BYTES), Err(Error::Limit)));
    let bytes = domain.retained_anchor_bytes(); let before = host.inspect();
    let mut small = FileShutdownPlan::new(900, vec![domain.clone()], 4, bytes).unwrap().start();
    assert_eq!(small.advance(1, &mut host, ElapsedTick(2)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
    let mut bounded = FileShutdownPlan::new(900, vec![domain], 2, MAX_SHUTDOWN_HEAD_BYTES).unwrap().start();
    bounded.advance(1, &mut host, ElapsedTick(2)).unwrap();
    bounded.advance(1, &mut host, ElapsedTick(2)).unwrap();
    let complete = bounded.report(); let before = host.inspect();
    assert_eq!(bounded.advance(1, &mut host, ElapsedTick(2)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(bounded.report(), complete); assert_eq!(host.inspect(), before);
}

#[test]
fn expired_endpoint_retention_never_becomes_a_drained_refunded_domain() {
    let root = Directory::new(); let (mut host, human) = create(&root, 1);
    let mut campaign = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    let keys = ready(&mut host, &human, 1, 1, b"unknown forever"); dispatch(&mut host, &keys);
    campaign.advance(1, &mut host, ElapsedTick(2)).unwrap();
    campaign.advance(1, &mut host, ElapsedTick(2000)).unwrap();
    let report = campaign.report();
    assert!(report.all_observed_stopped()); assert!(!report.all_observed_drained());
    let stop = report.domains[0].last_observation.as_ref().unwrap().stop.as_ref().unwrap();
    assert_eq!(stop.unresolved, vec![1]); assert_eq!(stop.charged_units, 16);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
}
