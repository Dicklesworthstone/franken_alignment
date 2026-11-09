use super::*;
use crate::Snapshot;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanRequest, FileHumanReviewer, FileOversightProfile,
};
use crate::action::consequence::delivery::persistent::{
    FileDeliveryProfile, FilePermit, JournalLimits,
};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame,
};
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart,
};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(super) struct Root(PathBuf);

impl Root {
    pub(super) fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fa-inspection-pages-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(super) fn store(&self) -> PathBuf {
        self.0.join("store")
    }

    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.store())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("inspection page fixture cleanup: {error}");
        }
    }
}

pub(super) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget {
        adapter: 10,
        object: 11,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope {
                tenant: 1,
                principal: 2,
                run: 3,
                branch: 4,
                authority: 5,
                purpose: Purpose::Effect,
            },
            total: 100,
            max_attempts: 64,
            actor: ActorState::new(
                RestartProfile {
                    id: 1,
                    generation: 1,
                    host_generation: 1,
                    model_generation: 1,
                    tokenizer_generation: 1,
                    state_schema_generation: 1,
                    grade: RestartGrade::AuditOnly,
                },
                vec![],
                vec![1],
                vec![2],
                0,
            )
            .unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(
                1,
                vec![Predicate::ExactValue {
                    key: 7,
                    value: b"ok".to_vec(),
                }],
            )
            .unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: BTreeMap::from([(
                    "reviewer".into(),
                    MemberPolicy {
                        cohort: "reference".into(),
                        weight: 1,
                    },
                )]),
                caps: Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                continue_minimum: 1,
                continue_hold_maximum: 0,
                narrow_at: 2,
                suspend_at: 3,
                minimum_members: 1,
                minimum_cohorts: 1,
            },
            narrowed_targets: vec![target],
            target,
            initial_payload: b"initial-secret".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 64,
            clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([(
            "reviewer".into(),
            HelperContract::new(
                InputProfileBinding {
                    profile_id: 1,
                    profile_bytes: b"private-profile".to_vec(),
                    tokenizer_epoch: 1,
                    policy_epoch: 0,
                    model_epoch: 1,
                },
                7,
                b"private-question".to_vec(),
            )
            .unwrap(),
        )]))
        .unwrap(),
        human: HumanReviewPolicy {
            reviewer_id: 77,
            max_validity_ticks: 50,
            max_requests: 64,
        },
    }
}

pub(super) fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }
}

pub(super) fn create(root: &Root) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}

pub(super) fn propose(host: &mut FileOversight, id: u64, allowed: bool) -> FrozenAction {
    let mut source = snapshot();
    if !allowed {
        source.values.insert(7, b"bad".to_vec());
    }
    host.propose(
        host.revision(),
        id,
        ActionSpec {
            version: VERSION,
            scope: profile().delivery.scope,
            target: Some(host.inspect().target),
            payload: b"secret-payload".to_vec(),
            required_witnesses: vec![],
            policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100),
            units: 16,
        },
        source,
    )
    .unwrap()
}

pub(super) fn populate(host: &mut FileOversight, count: u64) {
    for id in 1..=count {
        propose(host, id, true);
    }
}

pub(super) fn prepared(
    host: &mut FileOversight,
    id: u64,
) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest) {
    let action = propose(host, id, true);
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let actual = ActualHelperInput::new(
        bytes,
        helper.profile_at(action.spec().policy_epoch),
        vec![
            SubmittedPart {
                kind: PartKind::Other,
                span: ByteSpan {
                    start: 0,
                    end: boundary,
                },
            },
            SubmittedPart {
                kind: PartKind::Question,
                span: ByteSpan {
                    start: boundary,
                    end,
                },
            },
        ],
        vec![],
    )
    .unwrap();
    let view = EvidenceViewManifest::new(
        actual,
        AuthorizationProjection {
            projection_id: 7,
            policy_epoch: action.spec().policy_epoch,
            projected_originals: vec![],
        },
        vec![],
    )
    .unwrap();
    let inputs = CommitteeInput::capture(
        &action,
        &contracts,
        BTreeMap::from([("reviewer".into(), view)]),
    )
    .unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone())
        .unwrap();
    let round = 100 + id;
    host.begin_review(
        host.revision(),
        id,
        round,
        [9; 32],
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(8),
        },
        snapshot(),
    )
    .unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest)
        .unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(
        host.revision(),
        round,
        "reviewer",
        Verdict::Allow,
        b"salt".to_vec(),
    )
    .unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot())
        .unwrap()
        .unwrap();
    let automatic = host
        .authorize(host.revision(), id, &inputs, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(host.revision(), 1000 + id, id, &inputs, ElapsedTick(10))
        .unwrap();
    (action, inputs, automatic, request)
}

pub(super) fn dispatch(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64) {
    let (action, inputs, automatic, request) = prepared(host, id);
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        snapshot(),
    )
    .unwrap();
}
