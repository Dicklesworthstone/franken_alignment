use super::*;
use super::super::{PeerCredentials, PeerPolicy, VerifiedReviewerSocket};
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-stop-control-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn owner(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.0.join("store"), profile()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn expected() -> ReviewerExpectation {
    ReviewerExpectation { reviewer: 77, scope: profile().delivery.scope, clock_domain: 99 }
}
fn sockets() -> (UnixStream, UnixStream) {
    let (a, b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap(); b.set_nonblocking(true).unwrap(); (a, b)
}
fn own_policy(stream: &UnixStream) -> PeerPolicy {
    let observed = PeerCredentials::observe(stream).unwrap();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}
fn decide<S: Read + Write, T: Read + Write>(server: &mut StopControlConnection<S>, client: &mut StopClient<T>, driver: &mut FileSupervisedDriver) {
    for _ in 0..1024 {
        server.step(driver, || ElapsedTick(2)).unwrap();
        if client.step().unwrap() == StopClientProgress::NeedsDecision { return; }
    }
    panic!("stop offer did not finish");
}
fn finish<S: Read + Write, T: Read + Write>(server: &mut StopControlConnection<S>, client: &mut StopClient<T>, driver: &mut FileSupervisedDriver, tick: u64) {
    let mut applied = 0;
    for _ in 0..1024 {
        if server.step(driver, || ElapsedTick(tick)).unwrap() == StopControlProgress::Applied { applied += 1; }
        if client.step().unwrap() == StopClientProgress::Complete {
            assert_eq!(applied, 1); return;
        }
    }
    panic!("stop exchange did not finish");
}

#[test]
fn verified_stop_is_independent_of_human_offer_and_requires_explicit_choice() {
    let root = Directory::new(); let (host, reviewer) = owner(&root);
    let (a, b) = sockets(); let ap = own_policy(&a); let bp = own_policy(&b);
    let mut server = VerifiedReviewerSocket::verify(a, ap).unwrap()
        .into_stop_connection(&host, &reviewer, 9, [3; 32]).unwrap();
    let mut client = VerifiedReviewerSocket::verify(b, bp).unwrap().into_stop_client(expected(), 9).unwrap();
    let before = host.revision(); let (_, mut driver) = host.into_supervised_driver();
    decide(&mut server, &mut client, &mut driver);
    for _ in 0..8 {
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
        assert_eq!(server.step(&mut driver, || panic!("clock before stop")).unwrap(), StopControlProgress::Blocked);
    }
    assert_eq!(driver.supervisor().host().unwrap().revision(), before);
    client.request_stop().unwrap(); finish(&mut server, &mut client, &mut driver, 2);
    assert!(client.receipt().unwrap().drained()); assert!(!client.outcome_unknown());
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().stop.unwrap().request().operation, 9);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
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
fn fragmented_exchange_retains_native_stop_after_receipt_loss_without_reapplying() {
    let root = Directory::new(); let (host, reviewer) = owner(&root); let (a, b) = sockets();
    let fail = Rc::new(Cell::new(false));
    let mut server = StopControlConnection::new(&host, &reviewer, 9,
        Fragmented { stream: a, fail_write: Rc::clone(&fail) }, [3; 32]).unwrap();
    let mut client = StopClient::new(b, expected(), 9).unwrap();
    let (_, mut driver) = host.into_supervised_driver(); decide(&mut server, &mut client, &mut driver);
    client.request_stop().unwrap(); client.step().unwrap();
    for _ in 0..1024 {
        if server.step(&mut driver, || ElapsedTick(2)).unwrap() == StopControlProgress::Applied { break; }
    }
    assert!(server.application().unwrap().receipt.drained());
    let state = driver.supervisor().host().unwrap().inspect();
    fail.set(true);
    assert!(server.step(&mut driver, || panic!("reapplied after receipt loss")).is_err());
    assert!(server.application().unwrap().result.is_ok());
    assert!(server.step(&mut driver, || panic!("reapplied after failure")).is_err());
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
    assert!(client.outcome_unknown());
    let (a, b) = sockets();
    let mut retry = {
        let host = driver.supervisor().host().unwrap();
        StopControlConnection::new(&host, &reviewer, 9, a, [4; 32]).unwrap()
    };
    let mut client = StopClient::new(b, expected(), 9).unwrap();
    decide(&mut retry, &mut client, &mut driver); client.request_stop().unwrap();
    finish(&mut retry, &mut client, &mut driver, 2);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
}

#[test]
fn stale_drain_clock_preserves_acknowledged_stop_in_both_reports() {
    let root = Directory::new(); let (host, reviewer) = owner(&root); let (a, b) = sockets();
    let mut server = StopControlConnection::new(&host, &reviewer, 9, a, [3; 32]).unwrap();
    let mut client = StopClient::new(b, expected(), 9).unwrap();
    let (_, mut driver) = host.into_supervised_driver(); decide(&mut server, &mut client, &mut driver);
    client.request_stop().unwrap(); finish(&mut server, &mut client, &mut driver, 0);
    assert_eq!(client.receipt().unwrap().status, StopStatus::StoppedDrainRefused);
    assert!(server.application().unwrap().result.as_ref().unwrap().drain.is_err());
    assert!(driver.supervisor().host().unwrap().inspect().stop.is_some());
}

#[test]
fn caught_clock_unwind_latches_application_and_cannot_repeat_native_work() {
    let root = Directory::new(); let (host, reviewer) = owner(&root); let (a, b) = sockets();
    let mut server = StopControlConnection::new(&host, &reviewer, 9, a, [3; 32]).unwrap();
    let mut client = StopClient::new(b, expected(), 9).unwrap();
    let (_, mut driver) = host.into_supervised_driver(); decide(&mut server, &mut client, &mut driver);
    client.request_stop().unwrap(); client.step().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        server.step(&mut driver, || panic!("clock failed after stop"))
    }));
    assert!(result.is_err());
    let state = driver.supervisor().host().unwrap().inspect(); assert!(state.stop.is_some());
    assert_eq!(server.phase(), StopControlPhase::Applying);
    assert!(server.step(&mut driver, || panic!("retried clock")).is_err());
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
}

#[test]
fn audience_nonce_and_message_kind_mutations_never_stop_the_original_owner() {
    for index in [0, 8, 24, 40, 48, 56, 64, 72, 103] {
        let root = Directory::new(); let (host, reviewer) = owner(&root); let (a, mut b) = sockets();
        let mut server = StopControlConnection::new(&host, &reviewer, 9, a, [3; 32]).unwrap();
        let (_, mut driver) = host.into_supervised_driver(); let before = driver.supervisor().host().unwrap().inspect();
        server.step(&mut driver, || panic!("clock during offer")).unwrap();
        let mut offer = [0; OFFER_BYTES]; b.read_exact(&mut offer).unwrap();
        let binding = StopBinding::decode_offer(&offer, expected(), 9).unwrap();
        let mut request = binding.request().unwrap(); request[index] ^= 1; b.write_all(&request).unwrap();
        assert!(server.step(&mut driver, || panic!("clock on bad request")).is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    }
}

#[test]
fn wrong_peer_and_wrong_native_owner_are_rejected_before_protocol_or_mutation() {
    let (a, mut b) = sockets(); let observed = PeerCredentials::observe(&a).unwrap();
    let wrong = PeerPolicy::new(if observed.uid() == 0 { 1 } else { 0 }, observed.gid(), None).unwrap();
    assert!(VerifiedReviewerSocket::verify(a, wrong).is_err());
    let mut byte = [0]; assert_eq!(b.read(&mut byte).unwrap(), 0);
    let root = Directory::new(); let other = Directory::new();
    let (host, reviewer) = owner(&root); let (other, foreign_reviewer) = owner(&other); let (a, b) = sockets();
    assert!(StopControlConnection::new(&host, &foreign_reviewer, 9, a, [3; 32]).is_err());
    let (a, _) = sockets(); let mut server = StopControlConnection::new(&host, &reviewer, 9, a, [3; 32]).unwrap();
    let (_, mut wrong_driver) = other.into_supervised_driver();
    assert!(server.step(&mut wrong_driver, || panic!("wrong owner clock")).is_err());
    assert_eq!(server.phase(), StopControlPhase::Offering); drop(b);
}

#[test]
fn wire_rejects_zero_session_wrong_operation_and_forged_unconfirmed_counters() {
    let binding = StopBinding { expected: expected(), operation: 9, session: [3; 32] };
    let offer = binding.offer().unwrap();
    assert!(StopBinding::decode_offer(&offer, expected(), 10).is_err());
    assert!(StopBinding { session: [0; 32], ..binding }.offer().is_err());
    let result = Err(JournalError::Contract(Error::Incomplete));
    let receipt = StopControlReceipt::project(binding, &result); let mut bytes = receipt.encode().unwrap();
    assert_eq!(StopControlReceipt::decode(&bytes, binding).unwrap(), receipt);
    bytes[112] = 1; assert!(StopControlReceipt::decode(&bytes, binding).is_err());
}
