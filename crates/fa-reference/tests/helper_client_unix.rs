#![cfg(unix)]

#[path = "support/helper_workers.rs"]
mod support;

use support::{Fixture, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_workers::{HelperFailure, HelperLimits, HelperPort, HelperRound};
use fa_reference::action::consequence::oversight::helper_workers::io::{HelperConnection, WorkerIoError};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::os::unix::net::UnixStream;

struct Pair { server: HelperConnection<UnixStream>, client: HelperClient<UnixStream> }
fn pair(port: HelperPort) -> Pair {
    let expected = port.request().view().actual_input().input_profile().clone();
    let (server, client) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    Pair { server: HelperConnection::new(port, server).unwrap(),
        client: HelperClient::from_unix(client, expected).unwrap() }
}
fn begin(f: &mut Fixture, id: u64) -> (HelperRound, Pair, Pair) {
    let (round, mut ports) = HelperRound::new(f.start(id), HelperLimits::default()).unwrap();
    (round, pair(ports.remove("alpha").unwrap()), pair(ports.remove("beta").unwrap()))
}
fn tick(pair: &mut Pair) {
    pair.server.step().unwrap();
    let report = pair.client.drive(8).unwrap();
    report.progress.unwrap();
    assert!(report.steps <= 8);
}
fn load(a: &mut Pair, b: &mut Pair) {
    for _ in 0..10_000 {
        tick(a); tick(b);
        if a.client.phase() == ClientPhase::NeedsInference && b.client.phase() == ClientPhase::NeedsInference { return; }
    }
    panic!("real socket inputs did not complete within the step budget");
}
fn complete(round: &mut HelperRound, a: &mut Pair, b: &mut Pair, now: u64) {
    for _ in 0..10_000 {
        tick(a); tick(b); round.advance(ElapsedTick(now)).unwrap();
        if round.statuses().values().all(|s| s.revealed) { return; }
    }
    panic!("real socket replies did not complete within the step budget");
}

#[test]
fn native_worker_clients_receive_exact_inputs_and_reach_publication() {
    let mut f = Fixture::new();
    let (mut round, mut a, mut b) = begin(&mut f, 7);
    load(&mut a, &mut b);
    assert_eq!(a.client.input().unwrap().actual_input(), f.inputs.views()["alpha"].actual_input());
    assert_eq!(b.client.input().unwrap().actual_input(), f.inputs.views()["beta"].actual_input());
    assert_ne!(a.client.input(), b.client.input());
    assert_eq!(f.broker.inspect().ledger.available, 100);
    a.client.respond(Verdict::Allow, b"alpha-salt").unwrap();
    b.client.respond(Verdict::Allow, b"beta-salt").unwrap();
    complete(&mut round, &mut a, &mut b, 2);
    let reviewed = round.finish(ElapsedTick(3)).unwrap();
    assert_eq!(reviewed.decision().consequence, Consequence::Continue);
    f.broker.observe_time(ElapsedTick(3)).unwrap();
    f.broker.apply_review(reviewed, Some(&f.inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(1, Some(&f.inputs), &snapshot()).unwrap();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).unwrap();
    f.endpoint.observe_time(ElapsedTick(3)).unwrap();
    f.broker.accept_receipt(f.endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(f.endpoint.payload(), b"publish");
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert!(f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).is_err());
}

#[test]
fn one_fast_helper_cannot_reveal_before_the_other_commits() {
    let mut f = Fixture::new();
    let (mut round, mut a, mut b) = begin(&mut f, 7);
    load(&mut a, &mut b);
    a.client.respond(Verdict::Allow, b"a").unwrap();
    for _ in 0..100 { tick(&mut a); tick(&mut b); round.advance(ElapsedTick(2)).unwrap(); }
    assert!(round.statuses()["alpha"].committed);
    assert!(!round.statuses()["alpha"].revealed);
    assert!(!round.statuses()["beta"].committed);
    assert_eq!(a.client.phase(), ClientPhase::AwaitingReveal);
    assert_eq!(b.client.phase(), ClientPhase::NeedsInference);
    assert_eq!(a.client.respond(Verdict::Deny, b"reroll"), Err(Error::WrongState));
    assert_eq!(round.finish(ElapsedTick(2)).unwrap_err(), Error::Incomplete);
    b.client.respond(Verdict::Hold, b"b").unwrap();
    complete(&mut round, &mut a, &mut b, 3);
    let review = round.finish(ElapsedTick(3)).unwrap();
    assert!(review.missing().is_empty());
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    f.broker.observe_time(ElapsedTick(3)).unwrap();
    f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
    assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
    assert_eq!(f.broker.inspect().ledger.available, 100);
}

#[test]
fn fully_sent_reveal_is_not_a_claim_of_timely_collector_acceptance() {
    let mut f = Fixture::new();
    let (mut round, mut a, mut b) = begin(&mut f, 7);
    load(&mut a, &mut b);
    a.client.respond(Verdict::Allow, b"a").unwrap();
    b.client.respond(Verdict::Allow, b"b").unwrap();
    for _ in 0..100 {
        tick(&mut a); tick(&mut b); round.advance(ElapsedTick(2)).unwrap();
        if round.statuses().values().all(|s| s.committed) { break; }
    }
    assert!(round.statuses().values().all(|s| s.committed && !s.revealed));
    // Transfer every reveal but deliberately do not run the collector yet.
    for _ in 0..100 { tick(&mut a); tick(&mut b); }
    assert_eq!(a.client.phase(), ClientPhase::ReplySent);
    assert_eq!(b.client.phase(), ClientPhase::ReplySent);
    assert!(round.statuses().values().all(|s| !s.revealed));
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned(), "beta".to_owned()]);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    assert!(round.statuses().values().all(|s| s.failure == Some(HelperFailure::RevealDeadline)));
}

#[test]
fn dropping_worker_after_its_full_reply_does_not_erase_queued_evidence() {
    let mut f = Fixture::new();
    let (mut round, mut a, mut b) = begin(&mut f, 7);
    load(&mut a, &mut b);
    a.client.respond(Verdict::Allow, b"a").unwrap();
    b.client.respond(Verdict::Allow, b"b").unwrap();
    for _ in 0..100 {
        tick(&mut a); tick(&mut b); round.advance(ElapsedTick(2)).unwrap();
        if round.statuses().values().all(|s| s.committed) { break; }
    }
    for _ in 0..100 { tick(&mut a); tick(&mut b); }
    assert_eq!(a.client.phase(), ClientPhase::ReplySent);
    assert_eq!(b.client.phase(), ClientPhase::ReplySent);
    let Pair { server: _a_server, client: a_client } = a;
    let Pair { server: _b_server, client: b_client } = b;
    // Servers retain the HelperPorts until the collector accepts the replies.
    drop(a_client); drop(b_client);
    let review = round.finish(ElapsedTick(3)).unwrap();
    assert!(review.missing().is_empty());
    assert_eq!(review.decision().consequence, Consequence::Continue);
}

#[test]
fn wrong_declared_model_profile_is_not_used_for_inference_or_voting() {
    let mut f = Fixture::new();
    let (mut round, mut ports) = HelperRound::new(f.start(7), HelperLimits::default()).unwrap();
    let alpha = ports.remove("alpha").unwrap();
    let mut wrong = alpha.request().view().actual_input().input_profile().clone();
    wrong.model_epoch += 1;
    let (server_socket, client_socket) = UnixStream::pair().unwrap();
    server_socket.set_nonblocking(true).unwrap();
    let mut server = HelperConnection::new(alpha, server_socket).unwrap();
    let mut client = HelperClient::from_unix(client_socket, wrong).unwrap();
    for _ in 0..100 {
        server.step().unwrap();
        if client.drive(8).unwrap().progress.is_err() { break; }
    }
    assert_eq!(client.failure(), Some(WorkerIoError::Protocol(Error::Binding)));
    assert!(client.input().is_none());
    assert!(!round.statuses()["alpha"].committed);
    drop(client);
    assert!(server.step().is_err());
    let mut beta = pair(ports.remove("beta").unwrap());
    for _ in 0..100 { tick(&mut beta); if beta.client.phase() == ClientPhase::NeedsInference { break; } }
    beta.client.respond(Verdict::Allow, b"b").unwrap();
    for _ in 0..100 { tick(&mut beta); round.advance(ElapsedTick(2)).unwrap(); }
    assert!(round.statuses()["beta"].committed);
    round.advance(ElapsedTick(5)).unwrap();
    for _ in 0..100 { tick(&mut beta); round.advance(ElapsedTick(6)).unwrap(); }
    assert!(round.statuses()["beta"].revealed);
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert!(review.missing().contains(&"alpha".to_owned()));
    assert_ne!(review.decision().consequence, Consequence::Continue);
}

#[test]
fn budgets_and_idle_nonblocking_reads_do_not_create_hidden_work() {
    let mut f = Fixture::new();
    let (_round, mut ports) = HelperRound::new(f.start(7), HelperLimits::default()).unwrap();
    let alpha = ports.remove("alpha").unwrap();
    let expected = alpha.request().view().actual_input().input_profile().clone();
    let (server, client) = UnixStream::pair().unwrap();
    let mut c = HelperClient::from_unix(client, expected).unwrap();
    assert_eq!(c.drive(0), Err(Error::InvalidInput));
    assert_eq!(c.drive(257), Err(Error::Limit));
    assert_eq!(c.phase(), ClientPhase::ReadingRequest);
    let idle = c.drive(256).unwrap();
    assert_eq!(idle.steps, 1);
    assert_eq!(idle.progress, Ok(ClientProgress::Blocked));
    drop(server);
    let closed = c.drive(16).unwrap();
    assert_eq!(closed.steps, 1);
    assert_eq!(closed.progress, Err(WorkerIoError::Io(std::io::ErrorKind::UnexpectedEof)));
    assert_eq!(c.drive(16).unwrap().steps, 0);
}

#[test]
fn worker_loss_does_not_obstruct_reconciliation_of_a_dispatched_effect() {
    let mut f = Fixture::new();
    let (mut round, mut a, mut b) = begin(&mut f, 7);
    load(&mut a, &mut b);
    a.client.respond(Verdict::Allow, b"a").unwrap();
    b.client.respond(Verdict::Allow, b"b").unwrap();
    complete(&mut round, &mut a, &mut b, 2);
    let review = round.finish(ElapsedTick(3)).unwrap();
    f.broker.observe_time(ElapsedTick(3)).unwrap();
    f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(1, Some(&f.inputs), &snapshot()).unwrap();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).unwrap();
    f.endpoint.observe_time(ElapsedTick(3)).unwrap();
    let _lost_ack = f.endpoint.deliver(&envelope).unwrap();
    f.broker.acknowledgment_lost(1).unwrap();
    drop(a); drop(b); drop(round);
    let revision = f.broker.input_revision(1).unwrap();
    f.broker.inputs_unavailable(1, revision).unwrap();
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    f.broker.reconcile_pending(&mut f.endpoint).unwrap();
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(f.endpoint.execution_count(), 1);
}
