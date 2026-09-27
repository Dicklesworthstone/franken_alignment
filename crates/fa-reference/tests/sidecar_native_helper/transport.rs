use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::{helper_client::{HelperClient, ClientProgress},
    helper_workers::{HelperPort, HelperRound, io::HelperConnection},
    sidecar::receiver::native::peer::{SidecarNativePeer, SidecarPeerError, SidecarPeerProgress}};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::cell::Cell;
use std::rc::Rc;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn connected(alarm: bool) -> (Setup, HelperRound, HelperConnection<UnixStream>, SidecarNativePeer<UnixStream>) {
    let mut setup = refined(); let (round, mut ports) = setup.workers(11);
    let port: HelperPort = ports.remove("reviewer").unwrap();
    let evaluator = evaluator(&setup, &port, vec![query(0, 0.5)], budget(), b"allow", alarm);
    let profile = port.request().view().actual_input().input_profile().clone();
    let (server, client) = UnixStream::pair().unwrap(); server.set_nonblocking(true).unwrap();
    let connection = HelperConnection::new(port, server).unwrap();
    let client = HelperClient::from_unix(client, profile).unwrap();
    let peer = SidecarNativePeer::new(client, evaluator, b"one-frozen-result".to_vec()).unwrap();
    (setup, round, connection, peer)
}

#[test]
fn real_socket_worker_evaluates_exact_evidence_then_sends_one_original_commit_and_reveal() {
    let (mut setup, mut round, mut server, mut worker) = connected(false);
    let mut prepared = 0;
    let mut sent = false;
    let mut completed = None;
    for _ in 0..20_000 {
        server.step().unwrap();
        match worker.step().unwrap() {
            SidecarPeerProgress::ReplyPrepared(verdict) => { assert_eq!(verdict, Verdict::Allow); prepared += 1; }
            SidecarPeerProgress::Transport(ClientProgress::ReplySent) => sent = true,
            _ => {}
        }
        round.advance(ElapsedTick(1)).unwrap();
        if sent {
            match round.finish(ElapsedTick(1)) {
                Ok(review) => { completed = Some(review); break; }
                Err(Error::Incomplete) => {}
                Err(error) => panic!("original round failed: {error:?}"),
            }
        }
    }
    assert_eq!(prepared, 1);
    let review = completed.expect("bounded actual socket/native computation must complete");
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert!(review.missing().is_empty() && review.abstained().is_empty());
    assert_eq!(worker.evaluation().native.work.sampled_draws, 2);
    assert_eq!(worker.evaluation().basis, Some(SidecarDecisionBasis::NativeModel));
    setup.owner.apply_review(review, Some(setup.packet.input()), &snapshot()).unwrap();
    let key = setup.owner.authorize(1, Some(setup.packet.input()), &snapshot()).unwrap();
    let envelope = setup.owner.dispatch(&key, &setup.action, Some(setup.packet.input()), &snapshot()).unwrap();
    setup.owner.accept_receipt(setup.endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(setup.endpoint.execution_count(), 1);
    let work = worker.evaluation();
    for _ in 0..3 { assert_eq!(worker.step(), Ok(SidecarPeerProgress::Transport(ClientProgress::ReplySent))); }
    assert_eq!(worker.evaluation(), work);
}

#[test]
fn native_failure_disconnects_without_turning_a_missing_vote_into_abstention_or_allow() {
    let (setup, mut round, mut server, mut worker) = connected(true);
    let mut failed = false;
    for _ in 0..20_000 {
        server.step().unwrap();
        if let Err(error) = worker.step() {
            assert!(matches!(error, SidecarPeerError::Evaluation(SidecarEvaluationError::Native(_))));
            failed = true; break;
        }
        round.advance(ElapsedTick(1)).unwrap();
    }
    assert!(failed);
    let before = worker.evaluation(); assert!(before.native.work.sampled_draws > 0);
    assert!(worker.step().is_err()); assert_eq!(worker.evaluation(), before);
    assert!(server.step().is_err()); // original transport sees the dropped socket
    let review = round.finish(ElapsedTick(30)).unwrap();
    assert_eq!(review.missing(), &["reviewer".to_owned()]);
    assert!(review.abstained().is_empty());
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    assert_eq!(setup.endpoint.execution_count(), 0);
}

struct InterruptedIo { reads: Rc<Cell<usize>>, drops: Rc<Cell<usize>> }
impl Read for InterruptedIo {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1); panic!("injected transport unwind")
    }
}
impl Write for InterruptedIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
impl Drop for InterruptedIo { fn drop(&mut self) { self.drops.set(self.drops.get() + 1); } }

#[test]
fn caught_transport_unwind_drops_connection_and_cannot_retry_io_or_native_work() {
    let mut setup = refined(); let (_round, ports) = setup.workers(11); let port = &ports["reviewer"];
    let evaluator = evaluator(&setup, port, vec![query(0, 0.5)], budget(), b"allow", false);
    let reads = Rc::new(Cell::new(0)); let drops = Rc::new(Cell::new(0));
    let client = HelperClient::new(InterruptedIo { reads: Rc::clone(&reads), drops: Rc::clone(&drops) },
        port.request().view().actual_input().input_profile().clone()).unwrap();
    let mut peer = SidecarNativePeer::new(client, evaluator, vec![1]).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| peer.step())).is_err());
    assert_eq!(reads.get(), 1); assert_eq!(drops.get(), 1);
    assert_eq!(peer.failure(), Some(SidecarPeerError::Contract(Error::Incomplete)));
    assert_eq!(peer.step(), Err(SidecarPeerError::Contract(Error::Incomplete)));
    assert_eq!(reads.get(), 1); assert_eq!(peer.evaluation().native.work.position, 0);
    assert!(!peer.cancel()); assert_eq!(peer.evaluation().status, SidecarEvaluationStatus::Cancelled);
}
