//! Real source-backed owners for the separate framing, socket and client paths.
//! Fixtures are reference control histories, not provider or production evidence.

mod channel;
#[cfg(unix)]
mod transport;
#[cfg(target_os = "linux")]
mod peer;

use super::{FilePrecheckWire, PrecheckRequest, encode_precheck_request};
use super::super::{FilePrecheckDisclosure, FilePrecheckOutcome};
use crate::{Snapshot, action::{ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope}};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, source::FileSourcePolicy};
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorPort,
    FileActorSupervisor};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract,
    actor::{ActorProposal, Knowledge}, human::HumanReviewPolicy,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource}};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-precheck-transport-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn source(&self) -> PathBuf { self.0.join("source.json") }
    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("precheck transport cleanup: {error}");
        }
    }
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}

fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::AuditOnly }, vec![], vec![1], vec![2], 0).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
                Predicate::PayloadAtMost(4), Predicate::All(vec![0, 1])]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("r".into(),
                MemberPolicy { cohort: "test".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("r".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"precheck-transport".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}

pub(super) struct Fixture {
    pub(super) actor: FileActorPort<FileOversight>,
    pub(super) supervisor: FileActorSupervisor<FileOversight>,
    pub(super) disclosure: Option<FilePrecheckDisclosure>,
    pub(super) source: FileEvidenceSource,
    // Drop the real owner before removing its files.
    pub(super) root: Root,
}

impl Fixture {
    pub(super) fn new(exchanges: usize, queries: usize) -> (Self, FilePrecheckWire) {
        let root = Root::new();
        let p = profile();
        let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
        host.enable_file_source(host.revision(), FileSourcePolicy {
            source: StateSource { scope: p.delivery.scope, source: 51, generation: 1 },
            limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap(),
        }).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
            scope: p.delivery.scope }, snapshot(),
            BTreeMap::from([("r".into(), b"private review input".to_vec())])).unwrap();
        fs::write(root.source(), data.encode()).unwrap();
        let mut source = FileEvidenceSource::new(root.source(), 51, p.delivery.scope, 1048576).unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        let (actor, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let (port, disclosure) = supervisor.disclose_exact_precheck(revision, 81, queries).unwrap();
        let wire = FilePrecheckWire::new(port, exchanges).unwrap();
        (Self { actor, supervisor, disclosure: Some(disclosure), source, root }, wire)
    }

    pub(super) fn remaining_queries(&self) -> usize {
        self.disclosure.as_ref().unwrap().remaining_queries()
    }
}

pub(super) fn request(query: u64, payload: &[u8]) -> PrecheckRequest {
    PrecheckRequest { query, proposal: ActorProposal { target: profile().delivery.target,
        payload: payload.to_vec(), units: 8, deadline: ElapsedTick(100), expected_policy_epoch: 0 } }
}

fn frame(request: &PrecheckRequest) -> Vec<u8> {
    let mut bytes = encode_precheck_request(request).unwrap();
    bytes.push(b'\n'); bytes
}

fn known(response: super::PrecheckWireResponse, expected: FilePrecheckOutcome, query: u64) {
    assert_eq!(response.query, Some(query));
    let Knowledge::Known { value, basis } = response.result.unwrap() else {
        panic!("expected known exact-precheck observation");
    };
    assert_eq!(value, expected);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch, basis.observed_at),
        (81, query, 0, ElapsedTick(1)));
}
