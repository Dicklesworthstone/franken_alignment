use super::*;
use crate::action::{Purpose, Scope, ResolvedTarget};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile, source::FileSourcePolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use crate::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, encode_command};
use crate::action::consequence::oversight::evidence_source::{EvidenceSnapshot, FileEvidenceSource};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-peer-pool-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn source(&self) -> PathBuf { self.0.join("evidence.json") }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("peer pool cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 4096, max_attempts: 16,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(4096),
            Predicate::ExactValue { key: 7, value: b"allow".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: Vec::new(), retention_ticks: 1000,
        max_deliveries: 16, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"multi-peer".to_vec(),
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 } }
}
pub(super) struct Setup {
    pub root: Root,
    pub port: Port,
    pub driver: FileSupervisedDriver,
    pub source: FileEvidenceSource,
}
pub(super) fn setup() -> Setup {
    let root = Root::new();
    let observed = EvidenceSnapshot::new(EvidenceIdentity { scope: profile().delivery.scope, source: 7, generation: 1 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"allow".to_vec())]) },
        BTreeMap::from([("reviewer".into(), b"private evidence".to_vec())])).unwrap();
    std::fs::write(root.source(), observed.encode()).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 7, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(10).unwrap(),
    }).unwrap();
    let source = FileEvidenceSource::new(root.source(), 7, profile().delivery.scope, 4096).unwrap();
    let (port, driver) = host.into_supervised_driver();
    Setup { root, port, driver, source }
}
pub(super) fn connected(port: Port) -> (FileActorInbox<Port>, UnixStream) {
    let (server, client) = UnixStream::pair().unwrap();
    let observed = PeerCredentials::observe(&server).unwrap();
    let policy = PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap();
    let mut inbox = FileActorInbox::new(policy, ActorWire::new(port), ChannelLimits::default(), 4).unwrap();
    inbox.attach(server).unwrap(); client.set_nonblocking(true).unwrap();
    (inbox, client)
}
pub(super) fn document(request: u64) -> Vec<u8> {
    encode_command(&Command::Submit { request, proposal: ActorProposal {
        target: profile().delivery.target, payload: b"package".to_vec(), units: 7,
        deadline: ElapsedTick(100), expected_policy_epoch: 0,
    }}).unwrap()
}
pub(super) fn disk(s: &Setup) -> Vec<u8> { std::fs::read(s.root.store().join("delivery.bin")).unwrap() }
