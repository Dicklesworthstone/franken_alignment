//! Fixed endpoint/request binding on actual original Unix inboxes.
use super::*;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_wire::{Command, WireError, WireResponse, decode_response, encode_command};
use std::io::{self, Read, Write};
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn no_clock() -> ElapsedTick { panic!("refused or recorded request sampled time") }
fn input() -> PoolBudget {
    PoolBudget { total: DriveBudget { write_bytes: 0, ..DriveBudget::default() },
        per_peer: DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
}
fn send(client: &mut UnixStream, bytes: &[u8]) {
    client.write_all(bytes).unwrap(); client.write_all(b"\n").unwrap();
}
fn drain(pool: &mut FileActorPool, s: &mut Setup, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    for _ in 0..64 {
        let quantum = DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() };
        let report = pool.observe(&mut s.driver, PoolBudget { total: quantum, per_peer: quantum }).unwrap();
        assert!(!report.stopped_on_error());
        let mut chunk = [0; 513];
        match client.read(&mut chunk) {
            Ok(0) => panic!("response EOF"),
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("response: {error}"),
        }
        if bytes.last() == Some(&b'\n') {
            return decode_response(&bytes[..bytes.len() - 1]).unwrap();
        }
    }
    panic!("bounded reply drain exhausted");
}

#[test]
fn scoped_pool_wrong_keys_are_read_free_but_both_matching_endpoints_admit() {
    let mut s = setup();
    let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::for_requests(vec![(1, a), (2, b)]).unwrap();
    let before = disk(&s); let reads = s.source.status().read_attempts;
    send(&mut ca, &document(2)); send(&mut cb, &document(1));
    let report = pool.drive(&mut s.driver, &mut s.source, no_clock, input()).unwrap();
    assert_eq!(report.visits.len(), 2);
    assert!(report.visits.iter().all(|v| v.result.as_ref().unwrap().intakes.is_empty()));
    assert_eq!(drain(&mut pool, &mut s, &mut ca).result, Err(WireError::Withheld));
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Err(WireError::Withheld));
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
    assert!(pool.next_request(&s.driver).unwrap().is_none());
    send(&mut ca, &document(1)); send(&mut cb, &document(2));
    let report = pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.visits.iter().map(|v| v.result.as_ref().unwrap().intakes.len()).sum::<usize>(), 2);
    assert_eq!(drain(&mut pool, &mut s, &mut ca).result, Ok(Knowledge::Pending { request: 1 }));
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 2 }));
    let a = pool.next_request(&s.driver).unwrap().unwrap();
    let b = pool.next_request(&s.driver).unwrap().unwrap();
    assert_eq!((a.peer, a.status.request, b.peer, b.status.request), (1, 1, 2, 2));
    assert_eq!(s.source.status().read_attempts, reads + 2);
    assert_eq!(s.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn scoped_pool_cannot_reacquire_another_peers_recorded_ticket_or_schedule_it() {
    let mut s = setup();
    let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::for_requests(vec![(1, a), (2, b)]).unwrap();
    send(&mut cb, &document(2));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 2 }));
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, 2);
    let before = disk(&s); std::fs::remove_file(s.root.source()).unwrap();
    for command in [document(2), encode_command(&Command::Poll { request: 2 }).unwrap(),
        encode_command(&Command::Cancel { request: 2 }).unwrap()] {
        send(&mut ca, &command);
        pool.observe(&mut s.driver, input()).unwrap();
        let reply = drain(&mut pool, &mut s, &mut ca);
        assert!(matches!(reply.result, Err(WireError::Withheld) | Ok(Knowledge::Withheld { .. })));
        assert!(pool.next_request(&s.driver).unwrap().is_none());
    }
    assert_eq!(disk(&s), before);
    assert!(matches!(s.driver.supervisor().host().unwrap().request_status(2).unwrap().disposition,
        FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }));
    send(&mut cb, &document(2));
    pool.observe(&mut s.driver, input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 2 }));
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().peer, 2);
    send(&mut cb, &encode_command(&Command::Cancel { request: 2 }).unwrap());
    pool.observe(&mut s.driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut s, &mut cb).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
}

#[test]
fn scoped_pool_reconnect_preserves_binding_and_its_original_ticket() {
    let mut s = setup(); let (inbox, mut client) = connected(s.port.clone());
    let mut pool = FileActorPool::for_requests(vec![(u64::MAX, inbox)]).unwrap();
    send(&mut client, &document(u64::MAX));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    let before = disk(&s); let reads = s.source.status().read_attempts;
    assert!(pool.disconnect(u64::MAX).unwrap()); drop(client);
    let (server, mut client) = UnixStream::pair().unwrap();
    pool.attach(u64::MAX, server).unwrap(); client.set_nonblocking(true).unwrap();
    send(&mut client, &document(1));
    pool.drive(&mut s.driver, &mut s.source, no_clock, input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut client).result, Err(WireError::Withheld));
    send(&mut client, &encode_command(&Command::Poll { request: u64::MAX }).unwrap());
    pool.observe(&mut s.driver, input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut client).result, Ok(Knowledge::Pending { request: u64::MAX }));
    assert_eq!(disk(&s), before); assert_eq!(s.source.status().read_attempts, reads);
    assert_eq!(pool.statuses().next().unwrap().1.connections_admitted, 2);
}

#[test]
fn scoped_pool_setup_preserves_incompatible_pending_work_and_legacy_behavior() {
    let mut s = setup(); let (inbox, mut client) = connected(s.port.clone());
    let mut ordinary = FileActorPool::new(vec![(1, inbox)]).unwrap();
    send(&mut client, &document(7));
    ordinary.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    let peers = ordinary.into_inboxes();
    let failed = match FileActorPool::for_requests(peers) {
        Ok(_) => panic!("unrelated pending hint must not be silently discarded"),
        Err(error) => error,
    };
    assert_eq!(failed.error, Error::Binding); assert_eq!(failed.peers.len(), 1);
    assert_eq!(failed.peers[0].1.queued(), 1);
    let mut restored = FileActorPool::new(failed.peers).unwrap();
    assert_eq!(drain(&mut restored, &mut s, &mut client).result, Ok(Knowledge::Pending { request: 7 }));
    assert_eq!(restored.next_request(&s.driver).unwrap().unwrap().status.request, 7);
}

#[test]
fn scoped_pool_defers_unrecorded_peers_without_refusal_or_resend() {
    let mut s = setup(); let source = std::fs::read(s.root.source()).unwrap();
    let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = FileActorPool::for_requests(vec![(1, a), (2, b)]).unwrap();
    send(&mut ca, &document(1));
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut ca).result, Ok(Knowledge::Pending { request: 1 }));
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().status.request, 1);
    send(&mut cb, &document(2)); // keep this exact frame queued; no resend
    send(&mut ca, &encode_command(&Command::Cancel { request: 1 }).unwrap());
    let reads = s.source.status().read_attempts; std::fs::remove_file(s.root.source()).unwrap();
    let report = pool.observe_registered(&mut s.driver, input()).unwrap();
    assert!(report.visits.iter().all(|visit| visit.peer == 1));
    assert!(matches!(drain(&mut pool, &mut s, &mut ca).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(cb.read(&mut [0; 1]).unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert_eq!(s.source.status().read_attempts, reads);
    assert!(matches!(s.driver.supervisor().host().unwrap().request_status(2), Err(JournalError::Contract(Error::Missing))));
    std::fs::write(s.root.source(), source).unwrap();
    pool.drive(&mut s.driver, &mut s.source, || ElapsedTick(3), input()).unwrap();
    assert_eq!(drain(&mut pool, &mut s, &mut cb).result, Ok(Knowledge::Pending { request: 2 }));
    assert_eq!(pool.next_request(&s.driver).unwrap().unwrap().status.request, 2);
    assert_eq!(s.source.status().read_attempts, reads + 1);
    let (inbox, mut client) = connected(s.port.clone());
    let mut unrestricted = FileActorPool::new(vec![(9, inbox)]).unwrap();
    send(&mut client, &document(3));
    let before = disk(&s);
    assert!(matches!(unrestricted.observe_registered(&mut s.driver, input()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(disk(&s), before);
    unrestricted.drive(&mut s.driver, &mut s.source, || ElapsedTick(4), input()).unwrap();
    assert_eq!(drain(&mut unrestricted, &mut s, &mut client).result, Ok(Knowledge::Pending { request: 3 }));
}
