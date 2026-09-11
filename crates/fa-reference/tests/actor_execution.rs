//! Full actor request -> congress -> original permit -> endpoint -> actor outcome.
#[path = "support/actor_gateway.rs"]
mod support;
use support::{fixture, proposal, review, snapshot};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

fn outcome(status: Knowledge<ActorOutcome>) -> ActorOutcome {
    match status { Knowledge::Known { value, .. } => value, other => panic!("not terminal: {other:?}") }
}

#[test]
fn actor_request_publishes_once_and_retry_never_creates_a_second_effect() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(77, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    assert_eq!(supervisor.authorize_request(77, None, &snapshot()).unwrap_err(), Error::Incomplete);
    let inputs = review(&mut supervisor, 77, 11);
    let permit = supervisor.authorize_request(77, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 77 });
    let receipt = supervisor.deliver_request(77, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap();
    let terminal = port.poll(&ticket);
    assert_eq!(outcome(terminal.clone()), ActorOutcome::Executed);
    assert_eq!(endpoint.payload(), b"publish");
    assert_eq!(endpoint.execution_count(), 1);
    let duplicate = port.clone().submit(77, &proposal()).unwrap();
    assert_eq!(port.poll(&duplicate), terminal);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(supervisor.deliver_request(77, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap_err(), Error::WrongState);
    assert!(!supervisor.accept_receipt(receipt).unwrap());
    assert_eq!(port.poll(&ticket), terminal);
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn identical_proposals_cannot_exchange_their_automatic_permits() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let first = port.submit(1, &proposal()).unwrap();
    let second = port.submit(2, &proposal()).unwrap();
    for _ in 0..2 { let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap(); }
    let first_input = review(&mut supervisor, 1, 11);
    let second_input = review(&mut supervisor, 2, 12);
    let first_key = supervisor.authorize_request(1, Some(&first_input), &snapshot()).unwrap();
    let second_key = supervisor.authorize_request(2, Some(&second_input), &snapshot()).unwrap();
    assert_eq!(supervisor.action(1).unwrap(), supervisor.action(2).unwrap());
    let before = supervisor.broker().inspect();
    assert_eq!(supervisor.dispatch_request(1, DispatchKeys::single(&second_key), Some(&first_input), &snapshot()).unwrap_err(), Error::Binding);
    assert_eq!(supervisor.broker().inspect(), before);
    supervisor.deliver_request(1, DispatchKeys::single(&first_key), Some(&first_input), &snapshot(), &mut endpoint).unwrap();
    assert_eq!(outcome(port.poll(&first)), ActorOutcome::Executed);
    assert_eq!(port.poll(&second), Knowledge::Pending { request: 2 });
    port.cancel(&second).unwrap(); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.available, 84);
}

#[test]
fn two_key_profile_has_no_gateway_fallback_and_expired_delivery_reconciles() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let reviewer = supervisor.broker_mut().enable_human_review(HumanReviewPolicy {
        reviewer_id: 90, max_validity_ticks: 20, max_requests: 16,
    }).unwrap();
    let ticket = port.submit(7, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 7, 11);
    let permit = supervisor.authorize_request(7, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(supervisor.dispatch_request(7, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    let attempt = supervisor.attempt(7).unwrap();
    let request = supervisor.broker_mut().request_human_approval(101, attempt, Some(&inputs), ElapsedTick(5)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = supervisor.dispatch_request(7, DispatchKeys::two(&permit, &human), Some(&inputs), &snapshot()).unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    drop(reviewer);
    supervisor.broker_mut().inputs_unavailable(attempt, 1).unwrap();
    port.cancel(&ticket).unwrap(); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    endpoint.observe_time(ElapsedTick(5)).unwrap();
    supervisor.reconcile_pending(&mut endpoint).unwrap();
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::ConfirmedNotExecuted);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    assert_eq!(supervisor.broker().human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    endpoint.deliver(&message).unwrap();
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn cancellation_at_dispatch_boundary_refunds_only_an_undispatched_reservation() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    port.cancel(&ticket).unwrap();
    assert_eq!(supervisor.deliver_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap_err(), Error::WrongState);
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::CancelledBeforeDispatch);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn endpoint_error_is_unknown_and_can_be_resolved_without_helpers_or_a_retry() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let (_foreign_port, _foreign_owner, mut wrong_endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(supervisor.deliver_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut wrong_endpoint).unwrap_err(), Error::Binding);
    let attempt = supervisor.attempt(1).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.stages[&attempt], ActionState::Unknown);
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    assert_eq!(wrong_endpoint.execution_count(), 0);
    assert_eq!(supervisor.deliver_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap_err(), Error::WrongState);
    supervisor.broker_mut().inputs_unavailable(attempt, 1).unwrap();
    endpoint.observe_time(ElapsedTick(100)).unwrap();
    supervisor.reconcile_pending(&mut endpoint).unwrap();
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::ConfirmedNotExecuted);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn changed_evidence_refusal_retains_original_key_for_a_valid_dispatch() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 11);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let mut changed = snapshot(); changed.values.insert(7, vec![8]);
    let before = supervisor.broker().inspect();
    assert_eq!(supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &changed).unwrap_err(), Error::Binding);
    assert_eq!(supervisor.broker().inspect(), before);
    assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 1 });
    supervisor.deliver_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot(), &mut endpoint).unwrap();
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::Executed);
}

#[cfg(unix)]
#[test]
fn real_file_publication_recovers_lost_ack_without_replaying_the_actor_request() {
    use fa_reference::action::consequence::delivery::{PublicationEndpoint, filesystem::FilePublicationLimits};
    use std::fs;
    use std::os::unix::fs::DirBuilderExt;
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("cleanup: {error}"); } }
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Temp(std::env::temp_dir().join(format!("fa-actor-file-{}-{stamp}", std::process::id())));
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(
        root.0.join("publication"), proposal().target, b"old".to_vec(), 200, 128,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (port, mut supervisor) = support::attach(&mut endpoint, IntakeLimits::default());
    let ticket = port.submit(55, &proposal()).unwrap();
    let _ = supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 55, 11);
    let permit = supervisor.authorize_request(55, Some(&inputs), &snapshot()).unwrap();
    let message = supervisor.dispatch_request(55, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    supervisor.acknowledgment_lost(55).unwrap();
    port.cancel(&ticket).unwrap(); supervisor.synchronize().unwrap();
    assert_eq!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    let visible = PublicationEndpoint::read_file_publication(recovery.directory()).unwrap();
    assert_eq!(visible.payload, b"publish"); assert_eq!(visible.execution_count, 1);
    let attempt = supervisor.attempt(55).unwrap();
    supervisor.broker_mut().inputs_unavailable(attempt, 1).unwrap();
    drop(endpoint);
    let mut reopened = recovery.reopen().unwrap();
    reopened.observe_time(ElapsedTick(2)).unwrap();
    let fence = supervisor.broker_mut().restart_dispatcher().unwrap();
    let ack = reopened.install_fence(fence).unwrap();
    supervisor.broker_mut().confirm_fence(ack).unwrap();
    supervisor.reconcile_pending(&mut reopened).unwrap();
    assert_eq!(outcome(port.poll(&ticket)), ActorOutcome::Executed);
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    assert!(!supervisor.accept_receipt(receipt).unwrap());
    port.submit(55, &proposal()).unwrap();
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(reopened.execution_count(), 1);
}
