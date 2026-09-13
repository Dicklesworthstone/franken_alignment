#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanRequest};
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{ReviewerConnection, ReviewerError, ReviewerProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewerClient, ReviewerExpectation, ReviewClientPhase, ReviewClientProgress, ReviewClientInterest,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, MAX_REVIEW_BYTES};
use fa_reference::action::consequence::oversight::{human::HumanDisposition, supervised::DriverEvidence};
use fa_reference::action::ElapsedTick;
use fa_reference::Error;
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

fn expectation() -> ReviewerExpectation {
    let profile = profile();
    ReviewerExpectation { reviewer: profile.human.reviewer_id, scope: profile.delivery.scope,
        clock_domain: profile.delivery.clock_domain }
}
fn request(rig: &mut Rig) -> FileHumanRequest {
    let _ = rig.submit(1);
    rig.reviewed(1);
    let input = rig.inputs.as_ref().unwrap();
    rig.driver.request_human_approval(1001, input, ElapsedTick(20), ElapsedTick(1)).unwrap()
}
fn channels(rig: &mut Rig) -> (ReviewerConnection<UnixStream>, ReviewerClient<UnixStream>) {
    let request = request(rig);
    let (a, b) = UnixStream::pair().unwrap();
    let host = rig.driver.supervisor().host().unwrap();
    (ReviewerConnection::from_unix(&host, &rig.reviewer, request, a, [19; 32]).unwrap(),
        ReviewerClient::from_unix(b, expectation()).unwrap())
}
fn offer<S: Read + Write, C: Read + Write>(rig: &mut Rig, server: &mut ReviewerConnection<S>, client: &mut ReviewerClient<C>) {
    for _ in 0..10_000 {
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let event = server.step(&mut host, &rig.reviewer, || panic!("transmission is not a decision")).unwrap();
            assert!(!matches!(event, ReviewerProgress::Applied(_)));
        }
        client.step().unwrap();
        if client.phase() == ReviewClientPhase::NeedsDecision { return; }
    }
    panic!("offer exceeded bounded fixture iterations");
}
fn finish<S: Read + Write, C: Read + Write>(rig: &mut Rig, server: &mut ReviewerConnection<S>, client: &mut ReviewerClient<C>) -> FileHumanPermit {
    let mut approval = None;
    for _ in 0..10_000 {
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            if let ReviewerProgress::Applied(applied) = server.step(&mut host, &rig.reviewer, || ElapsedTick(2)).unwrap() {
                assert!(approval.is_none(), "approval must be delivered exactly once");
                approval = applied.approval;
            }
        }
        client.step().unwrap();
        if client.phase() == ReviewClientPhase::Complete { return approval.expect("original host approval key"); }
    }
    panic!("exchange exceeded bounded fixture iterations");
}

#[test]
fn actor_helpers_and_independent_reviewer_client_complete_the_original_driver_pipeline() {
    let mut rig = Rig::new();
    let (mut server, mut client) = channels(&mut rig);
    assert_eq!(client.respond(ReviewDecision::Approve), Err(Error::WrongState));
    offer(&mut rig, &mut server, &mut client);
    assert_eq!(client.packet().unwrap().views(), rig.inputs.as_ref().unwrap().views());
    assert_eq!(client.interest(), ReviewClientInterest::HumanDecision);
    for _ in 0..8 { assert_eq!(client.step().unwrap(), ReviewClientProgress::NeedsDecision); }
    assert!(!client.outcome_unknown());
    assert!(matches!(rig.step(None), FileDriverEvent::AwaitingHuman { request: 1 }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    client.respond(ReviewDecision::Approve).unwrap();
    assert_eq!(client.respond(ReviewDecision::Reject), Err(Error::WrongState));
    let human = finish(&mut rig, &mut server, &mut client);
    assert_eq!(client.receipt(), server.committed());
    assert!(!client.outcome_unknown());
    assert_eq!(client.receipt().unwrap().decision, ReviewDecision::Approve);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { request: 1, .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().payload, b"publication");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(client.respond(ReviewDecision::Approve), Err(Error::WrongState));
}

#[test]
fn acknowledged_human_decision_cannot_override_a_later_changed_provider_cut() {
    let mut rig = Rig::new();
    let (mut server, mut client) = channels(&mut rig);
    offer(&mut rig, &mut server, &mut client);
    client.respond(ReviewDecision::Approve).unwrap();
    let human = finish(&mut rig, &mut server, &mut client);
    let action = rig.inputs.as_ref().unwrap().action().clone();
    let changed = helper::inputs(&action, b"provider changed after human review");
    let result = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| {
        Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(changed.clone()) })
    }, Some(&human));
    assert_eq!(result.unwrap_err(), JournalError::Contract(Error::Stale));
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    // The receipt stays historical Approved, not a current permission claim.
    assert_eq!(client.receipt().unwrap().decision, ReviewDecision::Approve);
}

#[test]
fn client_pins_reviewer_scope_and_clock_independently_of_the_received_packet() {
    for field in 0..3 {
        let mut rig = Rig::new();
        let request = request(&mut rig);
        let (a, b) = UnixStream::pair().unwrap();
        let mut expected = expectation();
        match field { 0 => expected.reviewer += 1, 1 => expected.scope.branch += 1, _ => expected.clock_domain += 1 }
        let mut server = {
            let host = rig.driver.supervisor().host().unwrap();
            ReviewerConnection::from_unix(&host, &rig.reviewer, request, a, [20; 32]).unwrap()
        };
        let mut client = ReviewerClient::from_unix(b, expected).unwrap();
        let mut refused = false;
        for _ in 0..512 {
            {
                let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
                server.step(&mut host, &rig.reviewer, || panic!("no human decision")).unwrap();
            }
            if let Err(error) = client.step() {
                assert_eq!(error, ReviewerError::Protocol(Error::Binding));
                refused = true; break;
            }
        }
        assert!(refused);
        assert!(client.packet().is_none());
        assert!(!client.outcome_unknown());
        assert_eq!(client.respond(ReviewDecision::Approve), Err(Error::WrongState));
        assert_eq!(rig.driver.supervisor().host().unwrap().human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    }
}

#[test]
fn client_reports_unknown_when_approval_committed_but_its_receipt_is_lost() {
    let mut rig = Rig::new();
    let (mut server, mut client) = channels(&mut rig);
    offer(&mut rig, &mut server, &mut client);
    client.respond(ReviewDecision::Approve).unwrap();
    client.step().unwrap();
    assert!(client.outcome_unknown());
    let applied = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let ReviewerProgress::Applied(applied) = server.step(&mut host, &rig.reviewer, || ElapsedTick(2)).unwrap()
            else { panic!("expected complete framed decision"); };
        applied
    };
    assert!(applied.approval.is_some());
    drop(server);
    assert_eq!(client.step().unwrap_err(), ReviewerError::Io(io::ErrorKind::UnexpectedEof));
    assert!(client.outcome_unknown());
    assert!(client.receipt().is_none());
    assert_eq!(client.respond(ReviewDecision::Approve), Err(Error::WrongState));
    let revision = rig.driver.supervisor().host().unwrap().revision();
    assert!(client.step().is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(rig.driver.supervisor().host().unwrap().human_status(1001).unwrap().disposition, HumanDisposition::Approved);
}

#[test]
fn client_rejects_oversized_headers_without_a_packet_or_implicit_choice() {
    let mut bytes = b"FAHRVW\0\x01".to_vec();
    bytes.extend_from_slice(&(MAX_REVIEW_BYTES as u64 + 1).to_be_bytes());
    let mut client = ReviewerClient::new(io::Cursor::new(bytes), expectation()).unwrap();
    assert_eq!(client.step().unwrap_err(), ReviewerError::Protocol(Error::Limit));
    assert_eq!(client.phase(), ReviewClientPhase::Failed);
    assert_eq!(client.interest(), ReviewClientInterest::Finished);
    assert!(client.packet().is_none());
    assert!(client.decision().is_none());
    assert!(!client.outcome_unknown());
}

#[derive(Default)]
struct Counts { reads: Cell<usize>, writes: Cell<usize>, flushes: Cell<usize> }
struct Choppy { stream: UnixStream, counts: Rc<Counts> }
impl Choppy {
    fn new(stream: UnixStream) -> (Self, Rc<Counts>) {
        stream.set_nonblocking(true).unwrap();
        let counts = Rc::new(Counts::default());
        (Self { stream, counts: Rc::clone(&counts) }, counts)
    }
}
impl Read for Choppy {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.counts.reads.get() + 1; self.counts.reads.set(count);
        if count % 3 == 0 { return Err(io::ErrorKind::Interrupted.into()); }
        let end = bytes.len().min(7); self.stream.read(&mut bytes[..end])
    }
}
impl Write for Choppy {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.counts.writes.get() + 1; self.counts.writes.set(count);
        if count % 4 == 0 { return Err(io::ErrorKind::WouldBlock.into()); }
        self.stream.write(&bytes[..bytes.len().min(11)])
    }
    fn flush(&mut self) -> io::Result<()> {
        let count = self.counts.flushes.get() + 1; self.counts.flushes.set(count);
        if count % 2 == 1 { return Err(io::ErrorKind::Interrupted.into()); }
        self.stream.flush()
    }
}
fn counts(counts: &Counts) -> (usize, usize) { (counts.reads.get() + counts.writes.get(), counts.flushes.get()) }
fn bounded(before: (usize, usize), after: (usize, usize)) {
    assert!(after.0 - before.0 <= 1, "more than one read/write in a step");
    assert!(after.1 - before.1 <= 1, "more than one flush in a step");
}

#[test]
fn short_reads_writes_and_interrupted_flushes_preserve_one_exact_decision() {
    let mut rig = Rig::new();
    let request = request(&mut rig);
    let (a, b) = UnixStream::pair().unwrap();
    let (a, server_counts) = Choppy::new(a); let (b, client_counts) = Choppy::new(b);
    let mut server = {
        let host = rig.driver.supervisor().host().unwrap();
        ReviewerConnection::new(&host, &rig.reviewer, request, a, [21; 32]).unwrap()
    };
    let mut client = ReviewerClient::new(b, expectation()).unwrap();
    let mut applications = 0;
    let mut chosen = false;
    for _ in 0..10_000 {
        let before = counts(&server_counts);
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            if let ReviewerProgress::Applied(applied) = server.step(&mut host, &rig.reviewer, || ElapsedTick(2)).unwrap() {
                applications += 1;
                assert!(applied.approval.is_some());
            }
        }
        bounded(before, counts(&server_counts));
        let before = counts(&client_counts);
        client.step().unwrap();
        bounded(before, counts(&client_counts));
        if client.phase() == ReviewClientPhase::NeedsDecision {
            assert!(!chosen);
            assert_eq!(client.packet().unwrap().views(), rig.inputs.as_ref().unwrap().views());
            client.respond(ReviewDecision::Approve).unwrap(); chosen = true;
        }
        if client.phase() == ReviewClientPhase::Complete { break; }
    }
    assert_eq!(client.phase(), ReviewClientPhase::Complete);
    assert_eq!(applications, 1);
    assert_eq!(client.receipt(), server.committed());
    assert!(server_counts.flushes.get() >= 4);
    assert!(client_counts.flushes.get() >= 2);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().human_status(1001).unwrap().disposition, HumanDisposition::Approved);
}
