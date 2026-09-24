//! Actual sockets and native journals; no supplied peer credentials or fake port.
use super::*;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_wire::{Command, WireError, WireResponse, decode_response, encode_command};
use std::io::{self, Read, Write};
mod fixture;
use fixture::*;

fn no_clock() -> ElapsedTick { panic!("read-free operation sampled time") }
fn intake() -> PoolBudget {
    PoolBudget { total: DriveBudget { write_bytes: 0, frames: 2, ..DriveBudget::default() },
        per_peer: DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
}
fn send(client: &mut UnixStream, document: &[u8]) {
    let mut line = document.to_vec(); line.push(b'\n'); client.write_all(&line).unwrap();
}
fn drain(pool: &mut FileActorPool, s: &mut Setup, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    for _ in 0..64 {
        let budget = DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() };
        let report = pool.drive(&mut s.driver, &mut s.source, no_clock, PoolBudget { total: budget, per_peer: budget }).unwrap();
        assert!(!report.stopped_on_error());
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("EOF before complete reply"),
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("reply: {error}"),
        }
        if bytes.last() == Some(&b'\n') { return decode_response(&bytes[..bytes.len() - 1]).unwrap(); }
    }
    panic!("reply did not drain within bounded drives");
}

#[test]
fn pool_incomplete_peer_does_not_block_another_peers_admission() {
    let mut s = setup();
    let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a), (20, b)]).unwrap();
    ca.write_all(&document(11)).unwrap(); // deliberately no newline
    send(&mut cb, &document(21));
    let report = pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), intake()).unwrap();
    assert!(!report.stopped_on_error()); assert_eq!(report.visits.len(), 2);
    assert_eq!(report.visits[0].result.as_ref().unwrap().drive.progress.frames, 0);
    assert_eq!(report.visits[1].result.as_ref().unwrap().drive.progress.frames, 1);
    assert_eq!(s.source.status().read_attempts, 1);
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, 20);
    assert!(pool.next_request(&s.driver).unwrap().is_none());
    ca.write_all(b"\n").unwrap();
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(3), intake()).unwrap();
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, 10);
    assert_eq!(s.source.status().read_attempts, 2);
    assert_eq!(s.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn pool_shared_frame_budget_rotates_and_blocked_reply_does_not_starve_peer() {
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a), (20, b)]).unwrap();
    send(&mut ca, &document(11)); send(&mut cb, &document(21));
    let mut budget = intake(); budget.total.frames = 1;
    let first = pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), budget).unwrap();
    assert_eq!(first.visits[0].peer, 10);
    assert_eq!(first.visits.iter().filter_map(|v| v.result.as_ref().ok()).map(|r| r.drive.progress.frames).sum::<usize>(), 1);
    let second = pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), budget).unwrap();
    assert_eq!(second.visits[0].peer, 20);
    assert_eq!(s.source.status().read_attempts, 2);
    assert!(matches!(drain(&mut pool, &mut s, &mut ca).result, Ok(Knowledge::Pending { request: 11 })));
    assert!(matches!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 21 })));
    send(&mut ca, &document(12));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(3), intake()).unwrap();
    // FIFO within one peer, round-robin ACROSS peers (not numeric ID order).
    for expected in [10, 20, 10] {
        assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, expected);
    }
    assert!(pool.next_request(&s.driver).unwrap().is_none());
}

#[test]
fn pool_global_byte_and_io_caps_are_not_multiplied_by_peer_count() {
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a), (20, b)]).unwrap();
    send(&mut ca, &document(11)); send(&mut cb, &document(21));
    let one = DriveBudget { read_bytes: 1, write_bytes: 0, frames: 1, io_calls: 1 };
    for expected in [10, 20] {
        let report = pool.drive(&mut s.driver, &mut s.source, no_clock,
            PoolBudget { total: one, per_peer: one }).unwrap();
        assert_eq!(report.visits[0].peer, expected);
        let read: usize = report.visits.iter().map(|v| v.result.as_ref().unwrap().drive.progress.read_bytes).sum();
        let calls: usize = report.visits.iter().map(|v| v.result.as_ref().unwrap().drive.progress.io_calls).sum();
        assert_eq!((read, calls), (1, 1));
        assert_eq!(report.remaining.read_bytes, 0); assert_eq!(report.remaining.io_calls, 0);
    }
    assert_eq!(s.source.status().read_attempts, 0);
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), intake()).unwrap();
    assert_eq!(s.source.status().read_attempts, 2); // permitted larger-budget control
}

#[test]
fn pool_foreign_member_refuses_all_peers_before_any_io_or_hint_consumption() {
    let mut a = setup(); let b = setup();
    let (ia, mut ca) = connected(a.port.clone()); let (ib, mut cb) = connected(b.port.clone());
    let mut pool = FileActorPool::new(vec![(10, ia), (20, ib)]).unwrap();
    send(&mut ca, &document(11)); send(&mut cb, &document(21));
    let states: Vec<_> = pool.statuses().collect(); let da = disk(&a); let db = disk(&b);
    assert!(matches!(pool.drive(&mut a.driver, &mut a.source, no_clock, intake()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(pool.statuses().collect::<Vec<_>>(), states);
    assert_eq!(disk(&a), da); assert_eq!(disk(&b), db);
    assert!(matches!(pool.next_request(&a.driver), Err(JournalError::Contract(Error::Binding))));
    let mut peers = pool.into_inboxes();
    let (_, mut valid) = peers.remove(0);
    assert_eq!(valid.drive(&mut a.driver, &mut a.source, || ElapsedTick(2), intake().total).unwrap().drive.progress.frames, 1);
}

#[test]
fn pool_tickets_stay_session_local_and_exact_retry_reuses_native_admission() {
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a), (20, b)]).unwrap();
    send(&mut ca, &document(11));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), intake()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut ca).result, Ok(Knowledge::Pending { request: 11 }));
    let bytes = disk(&s); let reads = s.source.status().read_attempts;
    std::fs::remove_file(s.root.source()).unwrap();
    send(&mut cb, &encode_command(&Command::Poll { request: 11 }).unwrap());
    pool.drive(&mut s.driver, &mut s.source, no_clock, intake()).unwrap();
    assert!(matches!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Withheld { .. })));
    send(&mut cb, &document(11));
    pool.drive(&mut s.driver, &mut s.source, no_clock, intake()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 11 }));
    assert_eq!(disk(&s), bytes); assert_eq!(s.source.status().read_attempts, reads);
    send(&mut cb, &encode_command(&Command::Cancel { request: 11 }).unwrap());
    pool.drive(&mut s.driver, &mut s.source, no_clock, intake()).unwrap();
    assert!(matches!(drain(&mut pool, &mut s, &mut cb).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(pool.next_request(&s.driver).unwrap().is_none()); // BOTH stale hints disappear
}

#[test]
fn pool_revocation_keeps_ready_work_and_reconnect_preserves_the_original_session() {
    let mut s = setup(); let (a, mut client) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a)]).unwrap();
    send(&mut client, &document(11));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), intake()).unwrap();
    assert!(pool.disconnect(10).unwrap()); // drop the deliberately unsent reply
    let (server, mut replacement) = UnixStream::pair().unwrap();
    pool.attach(10, server).unwrap(); replacement.set_nonblocking(true).unwrap();
    let before = disk(&s);
    send(&mut replacement, &encode_command(&Command::Poll { request: 11 }).unwrap());
    pool.drive(&mut s.driver, &mut s.source, no_clock, intake()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut replacement).result, Ok(Knowledge::Pending { request: 11 }));
    assert_eq!(disk(&s), before); assert_eq!(pool.statuses().next().unwrap().1.connections_admitted, 2);
    pool.revoke_all();
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, 10);
    let (candidate, _) = UnixStream::pair().unwrap();
    assert_eq!(pool.attach(10, candidate), Err(PoolAttachError::Refused(PeerRefusal::Revoked)));
    assert_eq!(s.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn pool_fault_stops_later_io_and_charges_the_failed_allowance() {
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a), (20, b)]).unwrap();
    send(&mut ca, &document(11)); send(&mut cb, &document(21));
    let second = pool.statuses().nth(1).unwrap(); let before = disk(&s);
    std::fs::write(s.root.store().join("delivery.pending"), b"unacknowledged staging").unwrap();
    let budget = intake();
    let report = pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), budget).unwrap();
    assert!(report.stopped_on_error()); assert_eq!(report.visits.len(), 1);
    assert_eq!(report.remaining, subtract(budget.total, report.visits[0].allowance));
    assert_eq!(pool.statuses().nth(1).unwrap(), second);
    assert_eq!(disk(&s), before);
    assert!(matches!(pool.drive(&mut s.driver, &mut s.source, no_clock, budget),
        Err(FileActorPeerDriveError::Journal(JournalError::Unavailable))));
}

#[test]
fn pool_invalid_and_zero_budgets_leave_waiting_peers_untouched() {
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone());
    let mut pool = FileActorPool::new(vec![(10, a)]).unwrap(); send(&mut ca, &document(11));
    let states: Vec<_> = pool.statuses().collect(); let before = disk(&s);
    let mut bad = intake(); bad.per_peer.frames = crate::action::consequence::oversight::actor_transport::MAX_DRIVE_FRAMES + 1;
    assert!(matches!(pool.drive(&mut s.driver, &mut s.source, no_clock, bad), Err(FileActorPeerDriveError::Wire(WireError::Capacity))));
    let zero = DriveBudget { read_bytes: 0, write_bytes: 0, frames: 0, io_calls: 0 };
    assert!(pool.drive(&mut s.driver, &mut s.source, no_clock, PoolBudget { total: zero, per_peer: zero }).unwrap().visits.is_empty());
    assert_eq!(pool.statuses().collect::<Vec<_>>(), states); assert_eq!(disk(&s), before);
    assert_eq!(s.source.status().read_attempts, 0);
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), intake()).unwrap();
    assert_eq!(s.source.status().read_attempts, 1);
}

#[test]
fn pool_setup_limits_return_all_original_owners() {
    let s = setup(); let (a, _ca) = connected(s.port.clone()); let (b, _cb) = connected(s.port.clone());
    let failure = match FileActorPool::new(vec![(7, a), (7, b)]) { Ok(_) => panic!("duplicate admitted"), Err(e) => e };
    assert_eq!(failure.error, Error::Duplicate); assert_eq!(failure.peers.len(), 2);
    assert!(failure.peers.iter().all(|(_, inbox)| inbox.status().active.is_some()));
    let empty: Vec<(u64, FileActorInbox)> = Vec::new();
    assert!(matches!(FileActorPool::new(empty), Err(PoolSetupFailure { error: Error::Limit, .. })));
    let mut peers = Vec::new(); let mut clients = Vec::new();
    for id in 1..=MAX_POOL_PEERS + 1 {
        let (inbox, client) = connected(s.port.clone()); peers.push((id as u64, inbox)); clients.push(client);
    }
    let failure = match FileActorPool::new(peers) { Ok(_) => panic!("one-over admitted"), Err(e) => e };
    assert_eq!(failure.error, Error::Limit); assert_eq!(failure.peers.len(), MAX_POOL_PEERS + 1);
    let mut peers = failure.peers; peers.pop();
    assert_eq!(FileActorPool::new(peers).unwrap().statuses().count(), MAX_POOL_PEERS);
}
