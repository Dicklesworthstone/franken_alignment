use super::*;
use crate::Snapshot;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::observed::{
    FileOversightProfile, source::FileSourcePolicy,
};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::oversight::{
    CommitteeContract, HelperContract,
    actor::ActorProposal,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    human::HumanReviewPolicy,
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, VERSION};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::delivery::persistent::observed::{FileHumanRequest, FileHumanReviewer};
use crate::action::consequence::delivery::persistent::FilePermit;
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow, action_frame};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use crate::round::{Verdict, commitment};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Root(PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "fa-hold-diagnosis-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    pub(super) fn store(&self) -> PathBuf {
        self.0.join("store")
    }
    pub(super) fn source(&self) -> PathBuf {
        self.0.join("source.json")
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
            eprintln!("hold diagnosis cleanup: {error}");
        }
    }
}

pub(super) fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 17,
        complete: true,
        values: BTreeMap::from([
            (7, b"ok".to_vec()),
            (99, b"unselected private source".to_vec()),
        ]),
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
            max_attempts: 8,
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
                vec![
                    Predicate::ExactValue {
                        key: 7,
                        value: b"ok".to_vec(),
                    },
                    Predicate::PayloadAtMost(4),
                    Predicate::All(vec![0, 1]),
                ],
            )
            .unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: BTreeMap::from([(
                    "r".into(),
                    MemberPolicy {
                        cohort: "test".into(),
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
            initial_payload: b"initial".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 8,
            clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([(
            "r".into(),
            HelperContract::new(
                InputProfileBinding {
                    profile_id: 1,
                    profile_bytes: b"admission-explanation".to_vec(),
                    tokenizer_epoch: 1,
                    policy_epoch: 0,
                    model_epoch: 1,
                },
                7,
                b"approve?".to_vec(),
            )
            .unwrap(),
        )]))
        .unwrap(),
        human: HumanReviewPolicy {
            reviewer_id: 77,
            max_validity_ticks: 50,
            max_requests: 8,
        },
    }
}

pub(super) fn owner(root: &Root) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}

pub(super) fn source_owner(root: &Root) -> (FileOversight, FileEvidenceSource) {
    let p = profile();
    let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.enable_file_source(
        host.revision(),
        FileSourcePolicy {
            source: StateSource {
                scope: p.delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(5).unwrap(),
        },
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: p.delivery.scope,
        },
        snapshot(),
        BTreeMap::from([("r".into(), b"private helper input".to_vec())]),
    )
    .unwrap();
    fs::write(root.source(), evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(root.source(), 51, p.delivery.scope, 1048576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    (host, source)
}

pub(super) fn proposal(payload: &[u8]) -> ActorProposal {
    ActorProposal {
        target: profile().delivery.target,
        payload: payload.to_vec(),
        units: 8,
        deadline: ElapsedTick(100),
        expected_policy_epoch: 0,
    }
}

pub(super) fn spec(payload: &[u8]) -> ActionSpec {
    let candidate = proposal(payload);
    ActionSpec {
        version: VERSION,
        scope: profile().delivery.scope,
        target: Some(candidate.target),
        payload: candidate.payload,
        required_witnesses: Vec::new(),
        policy_epoch: candidate.expected_policy_epoch,
        deadline: candidate.deadline,
        units: candidate.units,
    }
}

pub(super) fn create(root: &Root) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}

/// Run the actual original full-input congress on an existing durable request.
/// This fixture never substitutes a direct attempt for an actor request.
pub(super) fn prepare_existing(
    host: &mut FileOversight,
    request: u64,
) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest) {
    let FileRequestDisposition::Admitted { attempt, .. } =
        host.request_status(request).unwrap().disposition
    else {
        panic!("expected admitted original request");
    };
    let action = host.request_action(request).unwrap().clone();
    let contracts = profile().committee;
    let helper = &contracts.members()["r"];
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
        BTreeMap::from([("r".into(), view)]),
    )
    .unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone())
        .unwrap();
    let round = 100 + attempt;
    host.begin_review(
        host.revision(),
        attempt,
        round,
        [9; 32],
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(8),
        },
        snapshot(),
    )
    .unwrap();
    let digest = commitment(round, "r", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "r", digest)
        .unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(
        host.revision(),
        round,
        "r",
        Verdict::Allow,
        b"salt".to_vec(),
    )
    .unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot())
        .unwrap()
        .unwrap();
    let automatic = host
        .authorize(host.revision(), attempt, &inputs, snapshot())
        .unwrap();
    let human = host
        .request_human_approval(
            host.revision(),
            1000 + attempt,
            attempt,
            &inputs,
            ElapsedTick(10),
        )
        .unwrap();
    (action, inputs, automatic, human)
}

pub(super) fn dispatch_existing(
    host: &mut FileOversight,
    reviewer: &FileHumanReviewer,
    request: u64,
) {
    let (action, inputs, automatic, request) = prepare_existing(host, request);
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
