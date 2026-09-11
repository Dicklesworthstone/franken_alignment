//! A receipt-time cutoff crossed inside I/O is not an on-time commitment.
#![cfg(unix)]
#[path = "support/helper_workers.rs"]
mod support;

use support::Fixture;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_workers::{HelperFailure, HelperLimits};
use fa_reference::action::consequence::oversight::helper_workers::io::{HelperPool, IoProgress};
use fa_reference::action::consequence::oversight::helper_workers::wire::{
    REQUEST_HEADER_BYTES, WorkerInput, decode_request, request_frame_len,
};
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn prepared() -> (HelperPool, Vec<(UnixStream, WorkerInput)>) {
    let mut f = Fixture::new();
    let mut streams = BTreeMap::new();
    let mut peers = Vec::new();
    for member in ["alpha", "beta"] {
        let (server, peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        streams.insert(member.to_owned(), server); peers.push(peer);
    }
    let mut pool = HelperPool::new(f.start(11), streams, HelperLimits::default()).unwrap();
    pool.pump(ElapsedTick(1)).unwrap();
    let peers = peers.into_iter().map(|mut peer| {
        let mut header = [0; REQUEST_HEADER_BYTES]; peer.read_exact(&mut header).unwrap();
        let mut bytes = vec![0; request_frame_len(&header).unwrap()];
        bytes[..REQUEST_HEADER_BYTES].copy_from_slice(&header);
        peer.read_exact(&mut bytes[REQUEST_HEADER_BYTES..]).unwrap();
        let packet = decode_request(&bytes).unwrap();
        peer.write_all(&packet.commitment_frame(Verdict::Allow, b"salt").unwrap()).unwrap();
        (peer, packet)
    }).collect();
    (pool, peers)
}

#[test]
fn fresh_post_io_clock_refuses_a_cutoff_crossing_next_to_on_time_success() {
    for crosses_cutoff in [false, true] {
        let (mut pool, mut peers) = prepared();
        let mut observations = 0;
        let report = pool.pump_with_clock(|| {
            observations += 1;
            ElapsedTick(if crosses_cutoff && observations >= 3 { 5 } else { 4 })
        }).unwrap();
        // The bytes were actually read and queued before the post-I/O clock.
        assert_eq!(report.io["alpha"], Ok(IoProgress::AwaitCoordinator));
        if crosses_cutoff {
            assert_eq!(report.workers["alpha"].failure, Some(HelperFailure::CommitDeadline));
            assert!(!report.workers["alpha"].committed);
            assert!(!report.workers["beta"].committed);
            assert_eq!(pool.finish(ElapsedTick(10)).unwrap().decision().consequence, Consequence::HoldEffect);
        } else {
            assert!(report.workers.values().all(|worker| worker.committed));
            pool.pump(ElapsedTick(4)).unwrap();
            for (peer, packet) in &mut peers {
                let mut signal = [0]; peer.read_exact(&mut signal).unwrap(); assert_eq!(&signal, b"R");
                peer.write_all(&packet.reveal_frame(Verdict::Allow, b"salt").unwrap()).unwrap();
            }
            pool.pump(ElapsedTick(6)).unwrap();
            pool.pump(ElapsedTick(6)).unwrap();
            assert_eq!(pool.finish(ElapsedTick(6)).unwrap().decision().consequence, Consequence::Continue);
        }
    }
}

#[test]
fn clock_rollback_after_receipt_preserves_work_but_cannot_resurrect_it_after_expiry() {
    let (mut pool, _peers) = prepared();
    let mut observations = 0;
    assert_eq!(pool.pump_with_clock(|| {
        observations += 1;
        ElapsedTick(if observations >= 3 { 3 } else { 4 })
    }), Err(Error::Stale));
    assert!(!pool.statuses()["alpha"].committed);
    pool.pump(ElapsedTick(5)).unwrap();
    assert_eq!(pool.statuses()["alpha"].failure, Some(HelperFailure::CommitDeadline));
    let review = pool.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing().len(), 2);
    assert_ne!(review.decision().consequence, Consequence::Continue);
}
