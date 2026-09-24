//! Real native generation, source files, independent sockets and original keys.
use super::*;
use super::super::super::*;
use crate::Error;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::requests::actor::FileActorInbox;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use crate::action::consequence::oversight::actor_transport::DriveBudget;
use crate::action::consequence::oversight::actor_wire::{
    ChannelLimits, Command, WireError, WireResponse, decode_response, encode_command,
};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
#[path = "../../tests/fixture.rs"]
mod fixture;
use fixture::*;

type Pool = FileActorPool<FileGeneratedTextActorPort>;
fn no_clock() -> ElapsedTick { panic!("read-free operation sampled time") }
fn input() -> PoolBudget {
    PoolBudget { total: DriveBudget { write_bytes: 0, frames: 2, ..DriveBudget::default() },
        per_peer: DriveBudget { write_bytes: 0, frames: 1, ..DriveBudget::default() } }
}
fn connected(port: FileGeneratedTextActorPort) -> (FileActorInbox<FileGeneratedTextActorPort>, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    let c = PeerCredentials::observe(&server).unwrap();
    let policy = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    let mut inbox = FileActorInbox::new(policy, ActorWire::new(port), ChannelLimits::default(), 4).unwrap();
    inbox.attach(server).unwrap(); client.set_nonblocking(true).unwrap(); (inbox, client)
}
fn send(client: &mut UnixStream, doc: &[u8]) {
    let mut bytes = doc.to_vec(); bytes.push(b'\n'); client.write_all(&bytes).unwrap();
}
fn drain(pool: &mut Pool, driver: &mut FileSupervisedDriver, client: &mut UnixStream) -> WireResponse {
    let mut bytes = Vec::new();
    let budget = DriveBudget { read_bytes: 0, frames: 0, ..DriveBudget::default() };
    for _ in 0..64 {
        let report = pool.observe(driver, PoolBudget { total: budget, per_peer: budget }).unwrap();
        assert!(!report.stopped_on_error());
        assert!(report.visits.iter().all(|v| v.result.as_ref().unwrap().intakes.is_empty()));
        let mut buffer = [0; 513];
        match client.read(&mut buffer) {
            Ok(0) => panic!("EOF before complete reply"),
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("reply: {error}"),
        }
        if bytes.last() == Some(&b'\n') { return decode_response(&bytes[..bytes.len() - 1]).unwrap(); }
    }
    panic!("response did not drain in bounded turns");
}

#[test]
fn generated_pool_malformed_peer_does_not_starve_native_admission_or_change_numerical_state() {
    use crate::action::consequence::delivery::stream::ReleaseFrame;
    let mut s = setup(); let reads = s.source.status().read_attempts;
    let n = s.supervisor.host().unwrap().decoder_inspection().unwrap().numerical;
    let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = Pool::new(vec![(10, a), (20, b)]).unwrap();
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    let mut bad = FileGeneratedTextActorPort::encode_message(&s.input).unwrap(); bad.payload[0] ^= 1;
    send(&mut ca, &encode_command(&Command::Submit { request: 91, proposal: bad }).unwrap());
    send(&mut cb, &document(&s.input));
    let report = pool.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    assert_eq!(report.visits.len(), 2); assert!(!report.stopped_on_error());
    assert!(report.visits[0].result.as_ref().unwrap().intakes.is_empty());
    assert_eq!(report.visits[1].result.as_ref().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut pool, &mut driver, &mut ca).result, Err(WireError::MalformedRequest));
    assert_eq!(drain(&mut pool, &mut driver, &mut cb).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(s.source.status().read_attempts, reads + 1);
    assert_eq!(pool.next_request(&driver).unwrap().unwrap().peer, 20);
    let host = driver.supervisor().host().unwrap();
    let action = host.request_action(91).unwrap();
    assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
    assert_eq!(action.spec().units, action.spec().payload.len() as u64);
    assert!(action.spec().units > FileGeneratedTextActorPort::INTENT_BYTES as u64);
    assert_eq!(host.decoder_inspection().unwrap().numerical, n);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn generated_pool_observation_services_active_review_without_new_admission_or_source_reads() {
    use crate::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverLaunch, FileDriverPhase};
    use crate::action::consequence::oversight::{ReviewWindow, helper_workers::HelperLimits};
    use std::collections::BTreeMap;
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = Pool::new(vec![(10, a), (20, b)]).unwrap();
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut ca, &document(&s.input));
    pool.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    drain(&mut pool, &mut driver, &mut ca);
    let ready = pool.next_request(&driver).unwrap().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = ready.status.disposition else { panic!("admission"); };
    let host = driver.supervisor().host().unwrap();
    let action = host.request_action(91).unwrap().clone();
    let revision = host.input_revision(attempt).unwrap(); drop(host);
    let evidence = capture(1, true, b"allow");
    let inputs = evidence.inputs_for(&action, &profile().committee).unwrap();
    let (worker, _helper) = UnixStream::pair().unwrap();
    driver.start_review(FileDriverLaunch { request: 91, round: 101, evidence_root: [9; 32],
        window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
        expected_input_revision: revision, inputs,
        workers: BTreeMap::from([("reviewer".into(), worker)]), limits: HelperLimits::default(),
    }, evidence.snapshot().clone(), || ElapsedTick(3)).unwrap();
    assert_eq!(driver.phase(), FileDriverPhase::Reviewing { request: 91 });
    let reads = s.source.status().read_attempts;
    let before = std::fs::read(s.root.store().join("delivery.bin")).unwrap();
    std::fs::remove_file(s.root.source()).unwrap();
    send(&mut ca, &document(&s.input)); // read-free retry requeues its original hint
    let mut next = s.input.clone(); next.request = 92; send(&mut cb, &document(&next));
    let report = pool.observe(&mut driver, input()).unwrap();
    assert!(report.visits.iter().all(|v| v.result.as_ref().unwrap().intakes.is_empty()));
    assert_eq!(drain(&mut pool, &mut driver, &mut ca).result, Ok(Knowledge::Pending { request: 91 }));
    assert_eq!(drain(&mut pool, &mut driver, &mut cb).result, Err(WireError::Withheld));
    assert_eq!(std::fs::read(s.root.store().join("delivery.bin")).unwrap(), before);
    let queued: usize = pool.statuses().map(|(_, _, n)| n).sum(); assert_eq!(queued, 1);
    assert!(matches!(pool.next_request(&driver), Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(pool.statuses().map(|(_, _, n)| n).sum::<usize>(), queued);
    send(&mut ca, &encode_command(&Command::Cancel { request: 91 }).unwrap());
    pool.observe(&mut driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut driver, &mut ca).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let event = driver.step_with_evidence(no_clock, |_, _| panic!("cancelled review acquired evidence"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Stopped { request: 91, stage: ActionState::Cancelled }));
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert!(pool.next_request(&driver).unwrap().is_none());
    assert_eq!(s.source.status().read_attempts, reads);
    // The withheld ID did not become a recorded policy refusal. Repair the real
    // source and explicitly re-enable intake through the original generation.
    s.root.replace(&capture(2, true, b"allow")); send(&mut cb, &document(&next));
    pool.drive(&mut driver, &mut s.source, || ElapsedTick(4), input()).unwrap();
    assert_eq!(drain(&mut pool, &mut driver, &mut cb).result, Ok(Knowledge::Pending { request: 92 }));
    assert_eq!(pool.next_request(&driver).unwrap().unwrap().status.request, 92);
}

#[test]
fn generated_pool_preserves_two_key_publication_and_unknown_until_native_reconciliation() {
    use crate::action::consequence::delivery::EndpointOutcome;
    use crate::action::consequence::oversight::ReviewWindow;
    use crate::round::{Verdict, commitment};
    let mut s = setup(); let (a, mut ca) = connected(s.port.clone()); let (b, mut cb) = connected(s.port.clone());
    let mut pool = Pool::new(vec![(10, a), (20, b)]).unwrap();
    let mut driver = FileSupervisedDriver::new(s.supervisor);
    send(&mut ca, &document(&s.input));
    pool.drive(&mut driver, &mut s.source, || ElapsedTick(2), input()).unwrap();
    drain(&mut pool, &mut driver, &mut ca);
    let ready = pool.next_request(&driver).unwrap().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = ready.status.disposition else { panic!("admission"); };
    let mut borrowed = driver.supervisor_mut().host_mut().unwrap(); let host = &mut *borrowed;
    let action = host.request_action(91).unwrap().clone();
    let evidence = capture(1, true, b"allow"); let snapshot = evidence.snapshot().clone();
    let inputs = evidence.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), attempt, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot.clone()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer", commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot.clone()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot.clone()).unwrap();
    assert!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot.clone(), ElapsedTick(2)).is_err());
    assert_eq!(host.inspect().executions, 0);
    let request = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = s.reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    drop(borrowed);
    let reads = s.source.status().read_attempts;
    // Another peer has no ticket merely because it uses the same owner.
    send(&mut cb, &encode_command(&Command::Poll { request: 91 }).unwrap());
    pool.observe(&mut driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut driver, &mut cb).result, Ok(Knowledge::Withheld { .. })));
    send(&mut cb, &document(&s.input));
    pool.observe(&mut driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut driver, &mut cb).result, Ok(Knowledge::Unknown { .. })));
    send(&mut cb, &encode_command(&Command::Cancel { request: 91 }).unwrap());
    pool.observe(&mut driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut driver, &mut cb).result, Ok(Knowledge::Unknown { .. })));
    let mut borrowed = driver.supervisor_mut().host_mut().unwrap(); let host = &mut *borrowed;
    host.reconcile(host.revision(), attempt).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"A");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    drop(borrowed);
    send(&mut cb, &encode_command(&Command::Poll { request: 91 }).unwrap());
    pool.observe(&mut driver, input()).unwrap();
    assert!(matches!(drain(&mut pool, &mut driver, &mut cb).result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(s.source.status().read_attempts, reads);
    assert!(pool.next_request(&driver).unwrap().is_none());
}

#[test]
fn generated_pool_foreign_owner_preflight_precedes_all_native_source_acquisition() {
    let mut a = setup(); let b = setup();
    let before_a = disk(&a); let before_b = disk(&b);
    let (ia, mut ca) = connected(a.port.clone()); let (ib, mut cb) = connected(b.port.clone());
    let mut pool = Pool::new(vec![(10, ia), (20, ib)]).unwrap();
    send(&mut ca, &document(&a.input)); send(&mut cb, &document(&b.input));
    let states: Vec<_> = pool.statuses().collect(); let mut driver = FileSupervisedDriver::new(a.supervisor);
    assert!(matches!(pool.drive(&mut driver, &mut a.source, no_clock, input()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert!(matches!(pool.observe(&mut driver, input()),
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(Error::Binding)))));
    assert_eq!(pool.statuses().collect::<Vec<_>>(), states);
    assert_eq!(std::fs::read(a.root.store().join("delivery.bin")).unwrap(), before_a);
    assert_eq!(disk(&b), before_b);
    let mut inboxes = pool.into_inboxes(); let (_, mut own) = inboxes.remove(0);
    assert_eq!(own.drive(&mut driver, &mut a.source, || ElapsedTick(2), input().total).unwrap().drive.progress.frames, 1);
}
