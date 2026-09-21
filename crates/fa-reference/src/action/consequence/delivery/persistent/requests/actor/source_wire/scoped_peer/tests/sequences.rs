//! Reuse real sockets, file evidence and the original locked journal fixture.
use super::*;

fn exchange(f: &mut Fixture, command: Command, current: u64, completed: &[u64],
    admit: bool, calls: &Cell<usize>) -> crate::action::consequence::oversight::actor_wire::WireResponse
{
    let mut bytes = encode_command(&command).unwrap(); bytes.push(b'\n');
    f.actor.write_all(&bytes).unwrap();
    let mut response = Vec::new(); let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(2));
        if admit {
            f.driver.drive_peer_sequence_from_file(&mut f.session, current, completed, &mut f.source,
                || { calls.set(calls.get() + 1); ElapsedTick(1) }, DriveBudget::default()).unwrap();
        } else {
            f.driver.drive_peer_sequence_observe(&mut f.session, current, completed, DriveBudget::default()).unwrap();
        }
        let mut buffer = [0; 4096];
        match f.actor.read(&mut buffer) {
            Ok(0) => panic!("unexpected peer EOF"),
            Ok(n) => response.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::yield_now(),
            Err(e) => panic!("{e}"),
        }
        if response.last() == Some(&b'\n') { return decode_response(&response[..response.len()-1]).unwrap(); }
    }
}
fn cancelled(f: &mut Fixture, request: u64, completed: &[u64], calls: &Cell<usize>) {
    let command = f.submit(request, b"same-owner");
    assert!(matches!(exchange(f, command, request, completed, true, calls).result, Ok(Knowledge::Pending { .. })));
    assert!(matches!(exchange(f, Command::Cancel { request }, request, completed, false, calls).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(f.driver.retire_completed_request(request), Ok(true));
}

#[test]
fn successive_keys_keep_history_rights_and_the_original_peer_session() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    cancelled(&mut f, 7, &[], &calls);
    let first = f.driver.supervisor().host().unwrap().inspect();
    cancelled(&mut f, 8, &[7], &calls);
    let second = f.driver.supervisor().host().unwrap().inspect();
    assert_eq!(second.control.ledger.stages.len(), 2);
    assert!(second.revision > first.revision);
    assert_eq!(second.control.ledger.epoch, first.control.ledger.epoch);
    assert_eq!(second.dispatcher_epoch, first.dispatcher_epoch);
    assert_eq!(second.control.ledger.available, 100);
    assert_eq!(second.control.ledger.charged, 0);
    assert_eq!(second.executions, 0);
    assert_eq!(f.session.status().connections_admitted, 1);
    assert_eq!(FileOversight::read_publication(f.root.join("store"), &profile()).unwrap(), second);
}

#[test]
fn historical_submit_retries_and_conflicts_do_not_capture_under_a_new_selection() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    cancelled(&mut f, 7, &[], &calls);
    std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    let before = f.driver.supervisor().host().unwrap().inspect(); let count = calls.get();
    let command = f.submit(7, b"same-owner");
    assert!(matches!(exchange(&mut f, command, 8, &[7], true, &calls).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let conflict = f.submit(7, b"other-bytes");
    assert_eq!(exchange(&mut f, conflict, 8, &[7], true, &calls).result, Err(WireError::IdempotencyConflict));
    assert_eq!(calls.get(), count);
    assert_eq!(f.driver.supervisor().host().unwrap().inspect(), before);
    let current = f.submit(8, b"needs-new-evidence");
    assert_eq!(exchange(&mut f, current, 8, &[7], false, &calls).result, Err(WireError::Unavailable));
    assert_eq!(calls.get(), count);
}

#[test]
fn neither_future_keys_nor_claimed_completed_pending_work_skip_the_original_gate() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    let future = f.submit(8, b"future");
    assert_eq!(exchange(&mut f, future, 7, &[], true, &calls).result, Err(WireError::Withheld));
    assert_eq!(calls.get(), 0);
    let current = f.submit(7, b"current");
    assert!(matches!(exchange(&mut f, current, 7, &[], true, &calls).result, Ok(Knowledge::Pending { .. })));
    let before = f.driver.supervisor().host().unwrap().inspect();
    assert_eq!(f.driver.retire_completed_request(7), Err(Error::WrongState.into()));
    assert!(f.driver.drive_peer_sequence_observe(&mut f.session, 8, &[7], DriveBudget::default()).is_err());
    assert_eq!(f.driver.supervisor().host().unwrap().inspect(), before);
    exchange(&mut f, Command::Cancel { request: 7 }, 7, &[], false, &calls);
    assert_eq!(f.driver.retire_completed_request(7), Ok(true));
    assert!(f.driver.drive_peer_sequence_observe(&mut f.session, 8, &[7], DriveBudget::default()).is_ok());
}

#[test]
fn invalid_sequences_foreign_owners_and_budget_errors_precede_transport_or_clock() {
    let mut f = Fixture::new(); let mut other = Fixture::new();
    f.actor.write_all(b"{}\n").unwrap();
    for (current, completed) in [(0, vec![]), (7, vec![0]), (7, vec![7]), (7, vec![8, 8]),
        (7, (100..164).collect())] {
        assert!(f.driver.drive_peer_sequence_observe(&mut f.session, current, &completed, DriveBudget::default()).is_err());
    }
    assert!(other.driver.drive_peer_sequence_observe(&mut f.session, 7, &[], DriveBudget::default()).is_err());
    let mut invalid = DriveBudget::default(); invalid.frames = usize::MAX;
    assert!(f.driver.drive_peer_sequence_observe(&mut f.session, 7, &[], invalid).is_err());
    assert_eq!(f.session.status().transport.unwrap().buffered_input_bytes, 0);
    assert_eq!(f.driver.drive_peer_sequence_observe(&mut f.session, 7, &[], DriveBudget::default()).unwrap().progress.frames, 1);
}

#[test]
fn sequence_handoff_does_not_reset_connection_budget_or_lose_old_tickets() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    cancelled(&mut f, 7, &[], &calls);
    assert!(f.session.disconnect());
    let (socket, actor) = UnixStream::pair().unwrap(); actor.set_nonblocking(true).unwrap();
    f.session.attach(socket).unwrap(); f.actor = actor;
    let command = f.submit(7, b"same-owner"); let count = calls.get();
    assert!(matches!(exchange(&mut f, command, 8, &[7], true, &calls).result, Ok(Knowledge::Known { .. })));
    assert_eq!(calls.get(), count);
    assert_eq!(f.session.status().connections_admitted, 2);
    cancelled(&mut f, 8, &[7], &calls);
    assert_eq!(f.session.status().connections_admitted, 2);
}

#[test]
fn exact_schedule_capacity_accepts_63_terminal_predecessors_and_refuses_64() {
    use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
    let mut f = Fixture::new(); let calls = Cell::new(0);
    let payload = vec![b'x'; 101]; // More than this ORIGINAL authority's 100 units.
    let mut completed = Vec::new();
    for key in 1..=63 {
        let command = f.submit(key, &payload);
        exchange(&mut f, command, key, &completed, true, &calls);
        assert!(matches!(f.driver.supervisor().host().unwrap().request_status(key).unwrap().disposition,
            FileRequestDisposition::NotAdmitted(_)));
        completed.push(key);
    }
    assert_eq!(f.driver.drive_peer_sequence_observe(&mut f.session, u64::MAX, &completed,
        DriveBudget::default()).unwrap().progress.frames, 0);
    let before = f.driver.supervisor().host().unwrap().inspect();
    completed.push(64);
    assert!(f.driver.drive_peer_sequence_observe(&mut f.session, u64::MAX, &completed,
        DriveBudget::default()).is_err());
    assert_eq!(f.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(before.control.ledger.available, 100);
    assert_eq!(before.executions, 0);
}
