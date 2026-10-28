use super::*;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event,
    guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        anchored::FileHistoryAnchor, learned_recovery::FileGuardedLearnedRecovery},
    reviewer::peer::{PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::{StopClient, StopClientProgress, StopStatus}},
};
use std::{cell::Cell, collections::BTreeMap, fs, os::unix::net::UnixStream, path::PathBuf};

#[allow(dead_code)]
#[path = "../../../../../decoder/learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

mod profiled;

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements { guards: guards(), effective_policy: host.profile.delivery.policy.clone(),
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch } }
}
fn build(root: &Directory, config: &FileLearnedConfig, unknown: bool) -> FileOversight {
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
        root.store(), profile(), &guards(), None, config.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap(); step(&mut host).unwrap();
    if unknown {
        let (action, inputs, automatic, offer) = prepared(&mut host);
        let revision = host.revision();
        let human = roles.human.approve(&mut host, revision, &offer).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    }
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    host
}
fn begin(root: &Directory, config: &FileLearnedConfig, expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor) -> FileGuardedLearnedRecovery
{
    FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile(), expected, config, anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
}
fn ready(run: &mut FileGuardedLearnedRecovery) {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let old = run.progress();
        let new = run.advance(old.replayed_events, 1).unwrap();
        assert_eq!(new.replayed_events, old.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
}
fn verified(root: &Directory, config: &FileLearnedConfig, expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor) -> FileGuardedLearnedRecovery
{
    let mut run = begin(root, config, expected, anchor); ready(&mut run); run
}
fn bytes(root: &Directory) -> Vec<u8> { fs::read(root.store().join("delivery.bin")).unwrap() }
fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(root.store()).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file()).map(|path| {
            let bytes = fs::read(&path).unwrap(); (path, bytes)
        }).collect()
}
fn sockets() -> (UnixStream, UnixStream) {
    let (a, b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap(); b.set_nonblocking(true).unwrap(); (a, b)
}
fn own_policy(stream: &UnixStream) -> PeerPolicy {
    let observed = PeerCredentials::observe(stream).unwrap();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}
fn connect(target: FileRecoveryStop) -> (StopRecoveryConnection<UnixStream>, StopClient<UnixStream>) {
    let expected = target.expectation(); let operation = target.request().operation;
    let (a, b) = sockets(); let ap = own_policy(&a); let bp = own_policy(&b);
    (VerifiedReviewerSocket::verify(a, ap).unwrap().into_recovery_stop_connection(target, [37; 32]).unwrap(),
        VerifiedReviewerSocket::verify(b, bp).unwrap().into_stop_client(expected, operation).unwrap())
}
fn decide<S: Read + Write, T: Read + Write>(server: &mut StopRecoveryConnection<S>, client: &mut StopClient<T>) {
    for _ in 0..1024 {
        server.step().unwrap();
        if client.step().unwrap() == StopClientProgress::NeedsDecision { return; }
    }
    panic!("native recovery stop offer did not complete");
}
fn finish<S: Read + Write, T: Read + Write>(server: &mut StopRecoveryConnection<S>, client: &mut StopClient<T>) {
    let mut applied = 0;
    for _ in 0..1024 {
        if server.step().unwrap() == StopControlProgress::Applied { applied += 1; }
        if client.step().unwrap() == StopClientProgress::Complete {
            assert_eq!(applied, 1); return;
        }
    }
    panic!("native recovery stop receipt did not complete");
}

#[test]
fn explicit_verified_request_matches_original_stop_fence_and_preserves_unknown_numerics() {
    let root = Directory::new(); let config = config(false, 1);
    let host = build(&root, &config, true);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
    let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap();
    let actor = host.machine.broker.retained_actor_state();
    let retained = (actor.tokens().to_vec(), actor.cache().to_vec(), actor.sampler().to_vec());
    let request = StopRequest { operation: 701, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch };
    let mut events = host.events.clone(); let mut oracle = Machine::replay(&profile(), &events).unwrap();
    for event in [BaseEvent::Stop(request), BaseEvent::Fence] {
        let event = Event::Core(event); oracle.apply(&event).unwrap(); events.push(event);
    }
    let terminal = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    let terminal_state = oracle.snapshot(events.len());
    let terminal_anchor = FileHistoryAnchor::from_acknowledged(events.len(), terminal.clone());
    drop(host);
    fs::write(root.store().join("delivery.pending"), b"retain diagnostic staging").unwrap();
    let unchanged = files(&root);
    let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
    assert_eq!(target.request(), request);
    assert_eq!(target.expectation(), ReviewerExpectation { reviewer: 77,
        scope: profile().delivery.scope, clock_domain: 99 });
    let (mut server, mut client) = connect(target);
    decide(&mut server, &mut client);
    for _ in 0..8 {
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
    }
    assert!(server.application().is_none()); assert_eq!(files(&root), unchanged);
    assert!(matches!(FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config),
        Err(JournalError::Busy)));
    client.request_stop().unwrap(); finish(&mut server, &mut client);
    let application = server.into_application().unwrap(); let history = application.result.unwrap();
    assert_eq!(history.snapshot(), &terminal_state); assert_eq!(history.history_anchor(), &terminal_anchor);
    assert_eq!(history.appended_events(), 2); assert_eq!(bytes(&root), terminal);
    assert!(!root.store().join("delivery.pending").exists());
    assert_eq!(history.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(history.snapshot().payload, before.payload); assert_eq!(history.snapshot().executions, 0);
    assert_eq!(history.progress().charged_units, 16); assert!(!history.progress().drained());
    assert_eq!(history.snapshot().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(application.receipt.status, StopStatus::StoppedPending);
    assert_eq!(client.receipt(), Some(application.receipt)); assert!(!client.outcome_unknown());
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let machine = run.verified_guarded_machine().unwrap();
    assert_eq!(machine.broker.hosted_learned_generation().unwrap(), numerical.numerical);
    assert_eq!(machine.pending_learned_step(), numerical.pending);
    let actor = machine.broker.retained_actor_state();
    assert_eq!(actor.tokens(), retained.0); assert_eq!(actor.cache(), retained.1); assert_eq!(actor.sampler(), retained.2);
}

#[test]
fn every_partial_replay_and_wrong_full_guard_refuses_before_cleanup_or_handoff() {
    let root = Directory::new(); let config = config(false, 1);
    let host = build(&root, &config, false);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
    let count = host.events.len(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let unchanged = files(&root);
    for cut in 0..count {
        let mut run = begin(&root, &config, &expected, &anchor);
        for index in 0..cut { run.advance(index, 1).unwrap(); }
        assert_eq!(run.into_recovery_stop(701).err(), Some(Error::Incomplete.into()));
        assert_eq!(files(&root), unchanged);
    }
    for mismatch in 0..5 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => wrong.minimum.journal_revision += 1,
            1 => wrong.minimum.control_sequence += 1,
            2 => wrong.minimum.authority_epoch += 1,
            3 => wrong.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap(),
            _ => wrong.credential_epoch = Some(crate::action::consequence::delivery::persistent::observed::guarded::FileCredentialEpoch {
                generation: 1, revoked: false }),
        }
        let mut run = begin(&root, &config, &wrong, &anchor);
        assert!(run.advance(0, count).is_err());
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert!(run.into_recovery_stop(701).is_err()); assert_eq!(files(&root), unchanged);
    }
    assert_eq!(verified(&root, &config, &expected, &anchor).into_recovery_stop(0).err(), Some(Error::InvalidInput.into()));
    let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
    let (a, _) = sockets();
    assert_eq!(StopRecoveryConnection::new(target, a, [0; 32]).err(), Some(Error::InvalidInput.into()));
    assert_eq!(files(&root), unchanged);
    let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
    drop(target); assert_eq!(files(&root), unchanged);
}

#[test]
fn missing_retained_reserve_refuses_without_installing_or_refilling_one() {
    let root = Directory::new(); let config = config(false, 1);
    let (host, _) = owner(&root, &config); drop(host); let unchanged = files(&root);
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.into_recovery_stop(701).err(), Some(Error::Incomplete.into()));
    assert_eq!(files(&root), unchanged);
}

#[test]
fn malformed_silent_and_truncated_peers_never_apply_or_clean_the_original_cursor() {
    let root = Directory::new(); let config = config(false, 1);
    let host = build(&root, &config, true);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let unchanged = files(&root);
    for field in [0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 103] {
        let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
        let (a, mut b) = sockets();
        let mut server = StopRecoveryConnection::new(target, a, [37; 32]).unwrap();
        assert_eq!(server.step().unwrap(), StopControlProgress::Progress);
        let mut offer = [0; OFFER_BYTES]; b.read_exact(&mut offer).unwrap();
        let binding = StopBinding::decode_offer(&offer, server.binding.expected, 701).unwrap();
        let mut request = binding.request().unwrap(); request[field] ^= 1; b.write_all(&request).unwrap();
        assert_eq!(server.step(), Err(Error::Binding.into()));
        assert_eq!(server.phase(), StopControlPhase::Failed); assert!(server.application().is_none());
        assert_eq!(server.step(), Err(Error::Binding.into())); assert_eq!(files(&root), unchanged);
    }
    for prefix in [0, 1, OFFER_BYTES - 1] {
        let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
        let (a, mut b) = sockets(); let mut server = StopRecoveryConnection::new(target, a, [37; 32]).unwrap();
        server.step().unwrap(); let mut offer = [0; OFFER_BYTES]; b.read_exact(&mut offer).unwrap();
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        if prefix > 0 {
            b.write_all(&server.binding.request().unwrap()[..prefix]).unwrap();
            assert_eq!(server.step().unwrap(), StopControlProgress::Progress);
        }
        drop(b);
        assert_eq!(server.step(), Err(ReviewerError::Io(io::ErrorKind::UnexpectedEof)));
        assert!(server.into_application().is_none()); assert_eq!(files(&root), unchanged);
    }
}

struct Fragmented { stream: UnixStream, fail_write: Rc<Cell<bool>> }
impl Read for Fragmented {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let len = bytes.len().min(3); self.stream.read(&mut bytes[..len])
    }
}
impl Write for Fragmented {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write.get() { return Err(io::ErrorKind::BrokenPipe.into()); }
        self.stream.write(&bytes[..bytes.len().min(3)])
    }
    fn flush(&mut self) -> io::Result<()> { self.stream.flush() }
}
#[test]
fn fragmented_request_and_lost_reply_retain_native_acknowledgement_and_exact_retry() {
    let root = Directory::new(); let config = config(false, 1);
    let host = build(&root, &config, true);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
    let expectation = target.expectation(); let (a, b) = sockets(); let fail = Rc::new(Cell::new(false));
    let mut server = StopRecoveryConnection::new(target, Fragmented { stream: a, fail_write: Rc::clone(&fail) }, [37; 32]).unwrap();
    let mut client = StopClient::new(b, expectation, 701).unwrap();
    decide(&mut server, &mut client); client.request_stop().unwrap();
    for _ in 0..1024 {
        if server.step().unwrap() == StopControlProgress::Applied { break; }
        client.step().unwrap();
    }
    let acknowledged = server.application().expect("full fragmented request applied once").clone();
    assert!(acknowledged.receipt.acknowledged()); let canonical = bytes(&root);
    fail.set(true);
    assert_eq!(server.step(), Err(ReviewerError::Io(io::ErrorKind::BrokenPipe)));
    assert_eq!(server.step(), Err(ReviewerError::Io(io::ErrorKind::BrokenPipe)));
    assert_eq!(server.application(), Some(&acknowledged)); assert_eq!(bytes(&root), canonical);
    assert!(client.step().is_err()); assert!(client.outcome_unknown());
    assert_eq!(server.into_application(), Some(acknowledged.clone()));
    assert_eq!(verified(&root, &config, &expected, &anchor).into_recovery_stop(702).err(), Some(Error::Duplicate.into()));
    let run = verified(&root, &config, &expected, &anchor); run.fail_once(JournalIo::Stage);
    let (mut server, mut client) = connect(run.into_recovery_stop(701).unwrap());
    decide(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    let retried = server.into_application().unwrap(); let history = retried.result.unwrap();
    assert_eq!(history.appended_events(), 0); assert_eq!(bytes(&root), canonical);
    let original = acknowledged.result.unwrap();
    assert_eq!(history.history_anchor(), original.history_anchor()); assert_eq!(history.snapshot(), original.snapshot());
    assert_eq!(history.progress(), original.progress()); assert_eq!(client.receipt(), Some(retried.receipt));
}

#[test]
fn every_storage_barrier_is_unconfirmed_and_retry_keeps_original_stop_identity() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let host = build(&root, &config, true);
        let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
        let before = host.inspect(); drop(host);
        let run = verified(&root, &config, &expected, &anchor); run.fail_once(barrier);
        let (mut server, mut client) = connect(run.into_recovery_stop(701).unwrap());
        decide(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
        let application = server.into_application().unwrap();
        let JournalError::Io(failure) = application.result.unwrap_err() else { panic!("original Store fault"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(application.receipt.status, StopStatus::Unconfirmed);
        assert_eq!(client.receipt(), Some(application.receipt)); assert!(client.outcome_unknown());
        let run = verified(&root, &config, &expected, &anchor);
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(run.progress().total_events as u64, before.revision + if visible { 2 } else { 0 });
        let (mut server, mut client) = connect(run.into_recovery_stop(701).unwrap());
        decide(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
        let history = server.into_application().unwrap().result.unwrap();
        assert_eq!(history.appended_events(), if visible { 0 } else { 2 });
        assert_eq!(history.snapshot().revision, before.revision + 2);
        assert_eq!(history.progress().charged_units, 16); assert!(!history.progress().drained());
        assert_eq!(history.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(history.snapshot().stop.as_ref().unwrap().request().operation, 701);
        assert!(client.receipt().unwrap().acknowledged());
    }
}

#[test]
fn canonical_substitution_after_target_selection_refuses_before_staging_cleanup() {
    let root = Directory::new(); let config = config(false, 1);
    let host = build(&root, &config, false);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    let target = verified(&root, &config, &expected, &anchor).into_recovery_stop(701).unwrap();
    let mut substituted = bytes(&root); substituted.push(0);
    fs::write(root.store().join("delivery.bin"), &substituted).unwrap();
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap(); let unchanged = files(&root);
    let (mut server, mut client) = connect(target); decide(&mut server, &mut client);
    client.request_stop().unwrap(); finish(&mut server, &mut client);
    let application = server.into_application().unwrap();
    assert_eq!(application.result, Err(Error::Binding.into()));
    assert_eq!(application.receipt.status, StopStatus::Unconfirmed); assert_eq!(files(&root), unchanged);
}
