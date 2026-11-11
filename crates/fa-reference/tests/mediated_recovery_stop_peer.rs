//! Public ordinary mediated recovery through the original authenticated Stop
//! transport. Synthetic helper ballots exercise authority, not detector quality.
#![cfg(target_os = "linux")]
#![forbid(unsafe_code)]

#[path = "support/file_mediated.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation, RecoveryReserve,
    observed::{
        FileOversight, FileOversightProfile,
        guarded::{
            FileCredentialEpoch, anchored::FileHistoryAnchor,
            mediated::{FileMediatedBootstrap, FileMediatedRecovery, FileMediatedRequirements},
            recovery::{FileGuardedRecoveryStatus, FilePublicationRecoveryRequirements},
        },
        reviewer::{
            ReviewerError,
            peer::{
                PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
                control::{
                    FileRecoveryStop, StopClient, StopClientProgress, StopControlProgress,
                    StopRecoveryConnection, StopStatus,
                },
            },
        },
    },
};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::witness::refinement::RefinementBudget;
use std::{
    cell::Cell,
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    rc::Rc,
};
use support::{Directory, certify, graph, guards, ordinary, profile, requirements};

const OPERATION: u64 = 901;

fn publication(validation: Option<PublicationLimits>) -> FilePublicationRecoveryRequirements {
    FilePublicationRecoveryRequirements { validation, feed: None, joint: None }
}

fn seed(root: &Directory, p: FileOversightProfile, liabilities: bool) -> FileOversight {
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(
        root.store(), p, &guards(), None,
        FileMediatedBootstrap { topology: graph(1, false), reserve: RecoveryReserve::terminal(), publication: None },
        None, None,
    ).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    certify(&mut host, &roles.topology_observer);
    if liabilities {
        let sent = ordinary::ready(&mut host, &roles.oversight.human, 1, b"unknown");
        ordinary::dispatch(&mut host, &sent);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        let _reserved = ordinary::ready(&mut host, &roles.oversight.human, 2, b"reserved");
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Authorized);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
    host
}

fn begin(root: &Directory, p: FileOversightProfile, expected: &FileMediatedRequirements,
    anchor: &FileHistoryAnchor) -> Result<FileMediatedRecovery, JournalError>
{
    FileOversight::begin_open_mediated_guarded_anchored(root.store(), p, expected, anchor)?
        .require_recovery_reserve(RecoveryReserve::terminal())?
        .require_publication_profile(publication(None))
}

fn advance(run: &mut FileMediatedRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    Ok(())
}

fn ready(root: &Directory, p: FileOversightProfile, expected: &FileMediatedRequirements,
    anchor: &FileHistoryAnchor) -> FileMediatedRecovery
{
    let mut run = begin(root, p, expected, anchor).unwrap();
    advance(&mut run).unwrap();
    run
}

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(root.store()).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
}

fn bytes(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

fn sockets() -> (UnixStream, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap(); client.set_nonblocking(true).unwrap();
    (server, client)
}

fn own_policy(stream: &UnixStream) -> PeerPolicy {
    let peer = PeerCredentials::observe(stream).unwrap();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
}

fn connect(target: FileRecoveryStop) -> (StopRecoveryConnection<UnixStream>, StopClient<UnixStream>) {
    let expected = target.expectation(); let operation = target.request().operation;
    let (server, client) = sockets();
    let server_policy = own_policy(&server); let client_policy = own_policy(&client);
    (
        VerifiedReviewerSocket::verify(server, server_policy).unwrap()
            .into_recovery_stop_connection(target, [37; 32]).unwrap(),
        VerifiedReviewerSocket::verify(client, client_policy).unwrap()
            .into_stop_client(expected, operation).unwrap(),
    )
}

fn offer<S: Read + Write, C: Read + Write>(server: &mut StopRecoveryConnection<S>, client: &mut StopClient<C>) {
    for _ in 0..1024 {
        server.step().unwrap();
        if client.step().unwrap() == StopClientProgress::NeedsDecision { return; }
    }
    panic!("original Stop challenge did not complete");
}

fn finish<S: Read + Write, C: Read + Write>(server: &mut StopRecoveryConnection<S>, client: &mut StopClient<C>) {
    let mut applied = 0;
    for _ in 0..1024 {
        if server.step().unwrap() == StopControlProgress::Applied { applied += 1; }
        if client.step().unwrap() == StopClientProgress::Complete {
            assert_eq!(applied, 1);
            return;
        }
    }
    panic!("original Stop acknowledgment did not complete");
}

#[test]
fn authenticated_stop_keeps_unknown_charged_and_refunds_only_undispatched_reservations() {
    let root = Directory::new(); let host = seed(&root, profile(), true);
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap();
    let before = host.inspect(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"retain until explicit request").unwrap();
    let unchanged = files(&root);
    let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    assert_eq!(target.request().expected_control_sequence, before.control.sequence);
    assert_eq!(target.request().expected_authority_epoch, before.control.ledger.epoch);
    assert_eq!(target.expectation().scope, profile().delivery.scope);
    assert_eq!(target.expectation().reviewer, profile().human.reviewer_id);
    assert_eq!(target.expectation().clock_domain, profile().delivery.clock_domain);
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client);
    for _ in 0..4 {
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
    }
    assert!(server.application().is_none()); assert_eq!(files(&root), unchanged);
    assert!(matches!(begin(&root, profile(), &expected, &anchor), Err(JournalError::Busy)));
    client.request_stop().unwrap(); finish(&mut server, &mut client);
    let application = server.into_application().unwrap();
    assert_eq!(client.receipt(), Some(application.receipt));
    assert_eq!(application.receipt.status, StopStatus::StoppedPending);
    assert!(!client.outcome_unknown());
    let stopped = application.result.unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, before.revision + 2);
    assert_eq!(stopped.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(stopped.snapshot().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(stopped.snapshot().control.ledger.stages[&2], ActionState::Cancelled);
    assert_eq!(stopped.progress().charged_units, 16);
    assert_eq!(stopped.progress().reserved_units, 0);
    assert!(!stopped.progress().drained());
    assert_eq!(stopped.snapshot().payload, before.payload);
    assert_eq!(stopped.snapshot().executions, 0);
    assert!(stopped.snapshot().control.suspended);
    assert!(!root.store().join("delivery.pending").exists());
    let mut terminal = expected;
    terminal.topology.available = false;
    let restored = FileOversight::read_mediation(root.store(), &profile(), &terminal).unwrap();
    assert_eq!(restored.journal, *stopped.snapshot());
    assert!(!restored.topology.available);
}

#[test]
fn incomplete_or_mismatched_full_recovery_never_hands_off_authority_or_cleans_staging() {
    let root = Directory::new(); let host = seed(&root, profile(), false);
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap();
    let count = host.revision() as usize; drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
    let unchanged = files(&root);
    for cut in 0..count {
        let mut run = begin(&root, profile(), &expected, &anchor).unwrap();
        for predecessor in 0..cut { run.advance(predecessor, 1).unwrap(); }
        assert_eq!(run.into_recovery_stop(OPERATION).err(), Some(Error::Incomplete.into()));
        assert_eq!(files(&root), unchanged);
    }
    for mismatch in 0..9 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => wrong.topology.initial = graph(1, true),
            1 => wrong.topology.current = graph(2, false),
            2 => wrong.topology.available = false,
            3 => wrong.oversight.minimum.journal_revision += 1,
            4 => wrong.oversight.minimum.control_sequence += 1,
            5 => wrong.oversight.minimum.authority_epoch += 1,
            6 => wrong.oversight.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap(),
            7 => wrong.oversight.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }),
            _ => wrong.evaluation = Some(support::protocol()),
        }
        let target = begin(&root, profile(), &wrong, &anchor).and_then(|mut run| {
            advance(&mut run)?;
            run.into_recovery_stop(OPERATION)
        });
        assert!(target.is_err(), "full independent requirement {mismatch}");
        assert_eq!(files(&root), unchanged);
    }
    assert_eq!(ready(&root, profile(), &expected, &anchor).into_recovery_stop(0).err(),
        Some(Error::InvalidInput.into()));
    let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    drop(target);
    assert_eq!(files(&root), unchanged);
    // Dropping the sealed target releases its original lock without a Fence.
    drop(begin(&root, profile(), &expected, &anchor).unwrap());
    assert_eq!(files(&root), unchanged);
}

#[test]
fn retained_reserve_and_explicit_publication_contract_survive_the_terminal_handoff() {
    let root = Directory::new();
    let (host, _) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &guards(), None, graph(1, false), None, None).unwrap();
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap(); drop(host);
    let unchanged = files(&root);
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
    advance(&mut run).unwrap();
    assert_eq!(run.into_recovery_stop(OPERATION).err(), Some(Error::Incomplete.into()));
    assert_eq!(files(&root), unchanged);

    let root = Directory::new();
    let validation = PublicationLimits { bindings: 4, validation: RefinementBudget { steps: 32, value_bytes: 128 } };
    let (host, _) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(), &guards(), None,
        FileMediatedBootstrap { topology: graph(1, false), reserve: RecoveryReserve::terminal(), publication: Some(validation) },
        None, None).unwrap();
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap(); drop(host);
    let unchanged = files(&root);
    for validation in [None, Some(PublicationLimits { bindings: 3, ..validation })] {
        let run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        assert!(run.require_publication_profile(publication(validation)).is_err());
        assert_eq!(files(&root), unchanged);
    }
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
        .require_publication_profile(publication(Some(validation))).unwrap();
    advance(&mut run).unwrap();
    let (mut server, mut client) = connect(run.into_recovery_stop(OPERATION).unwrap());
    offer(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    let stopped = server.into_application().unwrap().result.unwrap();
    assert_eq!(stopped.appended_events(), 2);
    let mut terminal = expected; terminal.topology.available = false;
    let mut retry = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &terminal,
        stopped.history_anchor()).unwrap().require_publication_profile(publication(Some(validation))).unwrap();
    advance(&mut retry).unwrap();
    assert!(retry.into_recovery_stop(OPERATION).is_ok());
}

struct Fragmented {
    stream: UnixStream,
    changed_byte: Option<usize>,
    written: usize,
    fail_write: Rc<Cell<bool>>,
}
impl Read for Fragmented {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let size = bytes.len().min(3); self.stream.read(&mut bytes[..size])
    }
}
impl Write for Fragmented {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write.get() { return Err(io::ErrorKind::BrokenPipe.into()); }
        let size = bytes.len().min(3);
        let mut chunk = bytes[..size].to_vec();
        if let Some(index) = self.changed_byte
            && index >= self.written && index < self.written + size {
            chunk[index - self.written] ^= 1;
        }
        let count = self.stream.write(&chunk)?;
        self.written += count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { self.stream.flush() }
}

#[test]
fn silent_wrong_binding_and_truncated_requests_never_become_an_ordinary_recovery() {
    let root = Directory::new(); let host = seed(&root, profile(), false);
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
    let unchanged = files(&root);
    for changed_byte in [None, Some(0), Some(64), Some(72), Some(103)] {
        let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
        let audience = target.expectation();
        let (server_stream, client_stream) = sockets();
        let server_policy = own_policy(&server_stream);
        // The same connected client socket is checked before the fault adapter.
        own_policy(&client_stream).verify(&client_stream).unwrap();
        let mut server = VerifiedReviewerSocket::verify(server_stream, server_policy).unwrap()
            .into_recovery_stop_connection(target, [37; 32]).unwrap();
        let mut client = StopClient::new(Fragmented { stream: client_stream, changed_byte,
            written: 0, fail_write: Rc::new(Cell::new(false)) }, audience, OPERATION).unwrap();
        offer(&mut server, &mut client);
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        assert_eq!(files(&root), unchanged);
        if changed_byte.is_some() {
            client.request_stop().unwrap();
            let mut refused = false;
            for _ in 0..1024 {
                client.step().unwrap();
                if let Err(error) = server.step() {
                    assert_eq!(error, ReviewerError::Protocol(Error::Binding));
                    assert_eq!(server.step(), Err(error));
                    refused = true; break;
                }
            }
            assert!(refused, "altered original request was not refused");
        }
        assert!(server.application().is_none());
        drop(client); drop(server);
        assert_eq!(files(&root), unchanged);
    }
    let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    let audience = target.expectation(); let (server_stream, client_stream) = sockets();
    let mut server = StopRecoveryConnection::new(target, server_stream, [37; 32]).unwrap();
    let mut client = StopClient::new(Fragmented { stream: client_stream, changed_byte: None,
        written: 0, fail_write: Rc::new(Cell::new(false)) }, audience, OPERATION).unwrap();
    offer(&mut server, &mut client); client.request_stop().unwrap();
    client.step().unwrap(); // Only the first three original request bytes.
    assert_eq!(server.step().unwrap(), StopControlProgress::Progress);
    drop(client);
    assert_eq!(server.step(), Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)));
    assert!(server.into_application().is_none()); assert_eq!(files(&root), unchanged);
    // Near-identical complete fragmented transport reaches the actual native Stop.
    let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    let audience = target.expectation(); let (server_stream, client_stream) = sockets();
    let mut server = StopRecoveryConnection::new(target, server_stream, [37; 32]).unwrap();
    let mut client = StopClient::new(Fragmented { stream: client_stream, changed_byte: None,
        written: 0, fail_write: Rc::new(Cell::new(false)) }, audience, OPERATION).unwrap();
    offer(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    assert_eq!(server.into_application().unwrap().result.unwrap().appended_events(), 2);
}

#[test]
fn substituted_canonical_image_after_ready_returns_unconfirmed_and_preserves_staging() {
    let root = Directory::new(); let host = seed(&root, profile(), true);
    let expected = requirements(&host, None); let anchor = host.history_anchor().unwrap(); drop(host);
    let target = ready(&root, profile(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    let mut changed = bytes(&root); changed.push(0);
    fs::write(root.store().join("delivery.bin"), &changed).unwrap();
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
    let unchanged = files(&root);
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    let application = server.into_application().unwrap();
    assert_eq!(application.result, Err(Error::Binding.into()));
    assert_eq!(application.receipt.status, StopStatus::Unconfirmed);
    assert!(client.outcome_unknown()); assert_eq!(files(&root), unchanged);
}

#[test]
fn lost_reply_at_ordinary_capacity_retains_exact_stop_without_a_second_event_or_clock() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 80;
    let mut host = seed(&root, p.clone(), true);
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(host.revision() + 2)).unwrap();
    }
    let before = host.inspect(); let expected = requirements(&host, None);
    let anchor = host.history_anchor().unwrap(); drop(host);
    let target = ready(&root, p.clone(), &expected, &anchor).into_recovery_stop(OPERATION).unwrap();
    let audience = target.expectation(); let (server_stream, client_stream) = sockets();
    let fail = Rc::new(Cell::new(false));
    let mut server = StopRecoveryConnection::new(target, Fragmented { stream: server_stream,
        changed_byte: None, written: 0, fail_write: Rc::clone(&fail) }, [37; 32]).unwrap();
    let mut client = StopClient::new(client_stream, audience, OPERATION).unwrap();
    offer(&mut server, &mut client); client.request_stop().unwrap();
    for _ in 0..1024 {
        if server.step().unwrap() == StopControlProgress::Applied { break; }
        client.step().unwrap();
    }
    let application = server.application().expect("native acknowledgment before reply").clone();
    assert!(application.receipt.acknowledged());
    let stopped = application.result.as_ref().unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, before.revision + 2);
    assert_eq!(stopped.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(stopped.progress().charged_units, 16);
    let canonical = bytes(&root); fail.set(true);
    assert_eq!(server.step(), Err(ReviewerError::Io(io::ErrorKind::BrokenPipe)));
    assert_eq!(server.application(), Some(&application));
    assert!(client.step().is_err()); assert!(client.outcome_unknown());
    assert_eq!(server.into_application(), Some(application.clone()));
    let mut terminal = expected; terminal.topology.available = false;
    assert_eq!(ready(&root, p.clone(), &terminal, &anchor).into_recovery_stop(OPERATION + 1).err(),
        Some(Error::Duplicate.into()));
    let target = ready(&root, p, &terminal, stopped.history_anchor()).into_recovery_stop(OPERATION).unwrap();
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    let retried = server.into_application().unwrap().result.unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), stopped.snapshot());
    assert_eq!(retried.progress(), stopped.progress());
    assert_eq!(retried.history_anchor(), stopped.history_anchor());
    assert_eq!(bytes(&root), canonical);
}
