#![allow(dead_code)]
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-durable-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    pub fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("durable test cleanup: {error}"); }
    }
}
pub fn profile() -> FileDeliveryProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
            Predicate::PayloadAtMost(128), Predicate::All(vec![0, 1])]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([
            ("alpha".into(), MemberPolicy { cohort: "one".into(), weight: 5 }),
            ("beta".into(), MemberPolicy { cohort: "two".into(), weight: 5 }),
        ]), caps: Caps { per_member: 5, per_cohort: 5 }, continue_minimum: 8,
            continue_hold_maximum: 0, narrow_at: 8, suspend_at: 10, minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }
}
pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub fn spec(host: &FileDelivery, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().scope, target: Some(host.inspect().target),
        payload: payload.to_vec(), required_witnesses: vec![], policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
pub fn review(attempt: u64, round: u64, verdict: Verdict) -> ReferenceReview {
    ReferenceReview { attempt, round, evidence_root: [9; 32], snapshot: snapshot(),
        ballots: ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
            ReferenceBallot { verdict, salt: format!("reference-{member}").into_bytes() })).collect() }
}
pub fn create(root: &Directory) -> FileDelivery {
    let mut host = FileDelivery::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
pub fn approved(host: &mut FileDelivery, id: u64, payload: &[u8]) -> (FrozenAction, FilePermit) {
    let action = host.propose(host.revision(), id, spec(host, payload), snapshot()).unwrap();
    host.review(host.revision(), review(id, id + 100, Verdict::Allow)).unwrap();
    let permit = host.authorize(host.revision(), id, snapshot()).unwrap();
    (action, permit)
}
pub fn dispatched(host: &mut FileDelivery, id: u64, payload: &[u8]) -> FrozenAction {
    let (action, key) = approved(host, id, payload);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap(); action
}
