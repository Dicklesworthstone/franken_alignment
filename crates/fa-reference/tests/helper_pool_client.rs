//! Compose the actual worker client and supervising Unix pool, not mock votes.
//! The classifier is a deterministic test fixture, not a trained helper model.
#![cfg(unix)]
#[path = "support/helper_workers.rs"]
mod support;

use support::{Fixture, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_workers::{HelperLimits, HelperPhase};
use fa_reference::action::consequence::oversight::helper_workers::io::{HelperPool, WorkerIoError};
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

#[test]
fn profile_checked_clients_complete_one_shared_round_without_controller_supplied_votes() {
    let mut f = Fixture::new();
    let mut streams = BTreeMap::new();
    let mut clients = BTreeMap::new();
    let mut invocations = BTreeMap::new();
    for member in ["alpha", "beta"] {
        let (server, worker) = UnixStream::pair().unwrap();
        streams.insert(member.to_owned(), server);
        let expected = f.inputs.views()[member].actual_input().input_profile().clone();
        clients.insert(member.to_owned(), HelperClient::from_unix(worker, expected).unwrap());
        invocations.insert(member.to_owned(), 0);
    }
    let mut pool = HelperPool::new(f.start(11), streams, HelperLimits::default()).unwrap();
    for _ in 0..256 {
        pool.pump(ElapsedTick(1)).unwrap();
        for (member, client) in &mut clients {
            client.step().unwrap();
            if client.phase() == ClientPhase::NeedsInference {
                let input = client.input().unwrap();
                assert_eq!(input.actual_input(), f.inputs.views()[member].actual_input());
                let verdict = if input.actual_input().submitted_bytes().windows(7).any(|part| part == b"publish") {
                    Verdict::Allow
                } else { Verdict::Hold };
                *invocations.get_mut(member).unwrap() += 1;
                client.respond(verdict, member.as_bytes()).unwrap();
                assert_eq!(client.respond(Verdict::Deny, b"replacement"), Err(Error::WrongState));
            }
        }
        if pool.ready_to_finish() { break; }
    }
    assert!(pool.ready_to_finish());
    assert!(clients.values().all(|client| client.phase() == ClientPhase::ReplySent));
    assert!(invocations.values().all(|count| *count == 1));
    let review = pool.finish(ElapsedTick(1)).unwrap();
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert!(review.missing().is_empty());
    f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(1, Some(&f.inputs), &snapshot()).unwrap();
    let message = f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).unwrap();
    f.broker.accept_receipt(f.endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(f.endpoint.payload(), b"publish");
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
}

#[test]
fn a_client_profile_mismatch_remains_missing_while_its_healthy_peer_finishes() {
    let mut f = Fixture::new();
    let mut streams = BTreeMap::new();
    let mut clients = BTreeMap::new();
    for member in ["alpha", "beta"] {
        let (server, worker) = UnixStream::pair().unwrap();
        streams.insert(member.to_owned(), server);
        let mut expected = f.inputs.views()[member].actual_input().input_profile().clone();
        if member == "alpha" { expected.model_epoch += 1; }
        clients.insert(member.to_owned(), HelperClient::from_unix(worker, expected).unwrap());
    }
    let mut pool = HelperPool::new(f.start(11), streams, HelperLimits::default()).unwrap();
    let mut failure = None;
    let mut inferred = Vec::new();
    let mut now = 1;
    for _ in 0..256 {
        let report = pool.pump(ElapsedTick(now)).unwrap();
        if report.workers.values().any(|worker| worker.phase == HelperPhase::Failed)
            && report.workers.values().all(|worker| worker.committed || worker.phase == HelperPhase::Failed)
        { now = 5; }
        let mut closed = Vec::new();
        for (member, client) in &mut clients {
            match client.step() {
                Err(error) => {
                    assert_eq!(member, "alpha");
                    assert!(client.input().is_none());
                    failure = Some(error); closed.push(member.clone());
                }
                Ok(_) if client.phase() == ClientPhase::NeedsInference => {
                    inferred.push(member.clone());
                    let bytes = client.input().unwrap().actual_input().submitted_bytes();
                    let verdict = if bytes.windows(7).any(|part| part == b"publish") { Verdict::Allow } else { Verdict::Hold };
                    client.respond(verdict, member.as_bytes()).unwrap();
                }
                Ok(_) => {}
            }
        }
        // Actual closure of the rejected client's stream, not a fabricated vote.
        for member in closed { clients.remove(&member); }
        if pool.statuses().values().all(|worker| matches!(worker.phase, HelperPhase::Complete | HelperPhase::Failed)) { break; }
    }
    assert_eq!(failure, Some(WorkerIoError::Protocol(Error::Binding)));
    assert_eq!(inferred, vec!["beta".to_owned()]);
    assert!(pool.statuses()["beta"].revealed);
    assert!(!pool.statuses()["alpha"].revealed);
    assert!(!pool.ready_to_finish());
    let review = pool.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned()]);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    f.broker.observe_time(ElapsedTick(10)).unwrap();
    f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
    assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
    assert_eq!(f.broker.inspect().ledger.available, 100);
    assert_eq!(f.endpoint.payload(), b"old");
    assert_eq!(f.endpoint.execution_count(), 0);
}
