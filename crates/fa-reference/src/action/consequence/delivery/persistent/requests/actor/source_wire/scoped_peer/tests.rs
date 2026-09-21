//! Actual Unix sockets, native file admission and durable request outcomes.
use super::*;
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile, source::FileSourcePolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use crate::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, WireError, decode_response, encode_command};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, driver: FileSupervisedDriver, session: PeerSession<Port>,
    actor: UnixStream, source: FileEvidenceSource, target: ResolvedTarget }
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-scoped-peer-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        let p = profile(); let target = p.delivery.target; let scope = p.delivery.scope;
        let (mut host, _) = FileOversight::create(root.join("store"), p).unwrap();
        host.enable_file_source(host.revision(), FileSourcePolicy {
            source: StateSource { scope, source: 51, generation: 1 }, limits: StateLimits::default(),
            freshness: StateFreshness::new(100).unwrap(),
        }).unwrap();
        let evidence = EvidenceSnapshot::new(EvidenceIdentity { scope, source: 51, generation: 1 },
            Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
            BTreeMap::from([("helper".into(), b"context".to_vec())])).unwrap();
        std::fs::write(root.join("evidence.json"), evidence.encode()).unwrap();
        let source = FileEvidenceSource::new(root.join("evidence.json"), 51, scope, 1048576).unwrap();
        let (port, driver) = host.into_supervised_driver();
        let (socket, actor) = UnixStream::pair().unwrap(); actor.set_nonblocking(true).unwrap();
        let peer = PeerCredentials::observe(&socket).unwrap();
        let mut session = PeerSession::new(PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap(),
            ActorWire::new(port), ChannelLimits::default(), 4).unwrap();
        session.attach(socket).unwrap();
        Self { root, driver, session, actor, source, target }
    }
    fn submit(&self, request: u64, bytes: &[u8]) -> Command {
        Command::Submit { request, proposal: ActorProposal { target: self.target, payload: bytes.to_vec(),
            units: bytes.len() as u64, deadline: ElapsedTick(100), expected_policy_epoch: 0 } }
    }
    fn exchange(&mut self, command: Command, new: bool, calls: &Cell<usize>) -> crate::action::consequence::oversight::actor_wire::WireResponse {
        let mut bytes = encode_command(&command).unwrap(); bytes.push(b'\n');
        self.actor.write_all(&bytes).unwrap();
        let mut response = Vec::new(); let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(2));
            if new {
                self.driver.drive_peer_request_from_file(&mut self.session, 7, &mut self.source,
                    || { calls.set(calls.get() + 1); ElapsedTick(1) }, DriveBudget::default()).unwrap();
            } else { self.driver.drive_peer_request_observe(&mut self.session, 7, DriveBudget::default()).unwrap(); }
            let mut buffer = [0; 4096];
            match self.actor.read(&mut buffer) {
                Ok(0) => panic!("unexpected peer EOF"),
                Ok(n) => response.extend_from_slice(&buffer[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("{error}"),
            }
            if response.last() == Some(&b'\n') { return decode_response(&response[..response.len()-1]).unwrap(); }
        }
    }
}
impl Drop for Fixture { fn drop(&mut self) { if let Err(e) = std::fs::remove_dir_all(&self.root) { eprintln!("fixture cleanup: {e}"); } } }
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::AuditOnly }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".into(), MemberPolicy { cohort: "a".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default() },
        committee: CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"scoped-peer".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 },
            1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}

#[test]
fn selected_intake_then_read_free_poll_retry_conflict_and_cancel() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    let submitted = f.exchange(f.submit(7, b"payload"), true, &calls);
    assert!(matches!(submitted.result, Ok(Knowledge::Pending { request: 7 })));
    assert!(calls.get() > 0); let reads = calls.get();
    std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    assert!(matches!(f.exchange(Command::Poll { request: 7 }, false, &calls).result, Ok(Knowledge::Pending { .. })));
    assert!(matches!(f.exchange(f.submit(7, b"payload"), false, &calls).result, Ok(Knowledge::Pending { .. })));
    assert_eq!(f.exchange(f.submit(7, b"changed"), false, &calls).result, Err(WireError::IdempotencyConflict));
    assert!(matches!(f.exchange(Command::Cancel { request: 7 }, false, &calls).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(calls.get(), reads);
    assert_eq!(f.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn foreign_key_and_observe_only_mode_cannot_admit_new_work() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    let before = f.driver.supervisor().host().unwrap().inspect();
    assert_eq!(f.exchange(f.submit(8, b"wrong"), true, &calls).result, Err(WireError::Withheld));
    assert_eq!(f.exchange(f.submit(7, b"correct"), false, &calls).result, Err(WireError::Unavailable));
    assert_eq!(calls.get(), 0); assert_eq!(f.driver.supervisor().host().unwrap().inspect(), before);
    assert!(matches!(f.exchange(f.submit(7, b"correct"), true, &calls).result, Ok(Knowledge::Pending { .. })));
}

#[test]
fn foreign_gateway_invalid_selection_and_invalid_budget_do_no_io() {
    let mut f = Fixture::new(); let mut other = Fixture::new();
    f.actor.write_all(b"{}\n").unwrap();
    assert!(other.driver.drive_peer_request_observe(&mut f.session, 7, DriveBudget::default()).is_err());
    assert!(f.driver.drive_peer_request_observe(&mut f.session, 0, DriveBudget::default()).is_err());
    let mut invalid = DriveBudget::default(); invalid.frames = usize::MAX;
    assert!(f.driver.drive_peer_request_observe(&mut f.session, 7, invalid).is_err());
    assert_eq!(f.session.status().transport.unwrap().buffered_input_bytes, 0);
    let report = f.driver.drive_peer_request_observe(&mut f.session, 7, DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1);
}

#[test]
fn incomplete_frame_does_not_read_evidence_or_observe_time() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    let bytes = encode_command(&f.submit(7, b"fragment")).unwrap();
    f.actor.write_all(&bytes[..bytes.len()-1]).unwrap();
    let before = f.driver.supervisor().host().unwrap().inspect();
    for _ in 0..3 { f.driver.drive_peer_request_from_file(&mut f.session, 7, &mut f.source,
        || { calls.set(calls.get() + 1); ElapsedTick(1) }, DriveBudget::default()).unwrap(); }
    assert_eq!(calls.get(), 0); assert_eq!(f.driver.supervisor().host().unwrap().inspect(), before);
    f.actor.write_all(&bytes[bytes.len()-1..]).unwrap(); f.actor.write_all(b"\n").unwrap();
    f.driver.drive_peer_request_from_file(&mut f.session, 7, &mut f.source,
        || { calls.set(calls.get() + 1); ElapsedTick(1) }, DriveBudget::default()).unwrap();
    assert!(calls.get() > 0); assert!(f.driver.supervisor().host().unwrap().request_status(7).is_ok());
}

#[test]
fn reconnect_retains_original_ticket_without_new_intake() {
    let mut f = Fixture::new(); let calls = Cell::new(0);
    f.exchange(f.submit(7, b"once"), true, &calls); let revision = f.driver.supervisor().host().unwrap().revision();
    assert!(f.session.disconnect());
    let (socket, actor) = UnixStream::pair().unwrap(); actor.set_nonblocking(true).unwrap();
    f.session.attach(socket).unwrap(); f.actor = actor;
    std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    assert!(matches!(f.exchange(Command::Poll { request: 7 }, false, &calls).result, Ok(Knowledge::Pending { .. })));
    assert_eq!(f.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(f.session.status().connections_admitted, 2);
}
