use super::*;
use crate::Snapshot;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::observed::{
    FileOversightProfile, source::FileSourcePolicy,
};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReceipt,
    actor::ActorProposal,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    human::HumanReviewPolicy,
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use crate::action::{ActionSpec, Purpose, VERSION};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::commitment;
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
            "fa-review-explanation-{}-{stamp}-{}",
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
            eprintln!("review explanation cleanup: {error}");
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
                    profile_bytes: b"review-explanation".to_vec(),
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

pub(super) fn known(result: FileReviewExplanationResult) -> FileReviewExplanation {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original completed review explanation: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

pub(super) fn unknown(result: FileReviewExplanationResult) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

pub(super) const ROOT: [u8; 32] = [9; 32];
pub(super) const SALT: &[u8] = b"private-reveal-salt";

pub(super) fn inputs(action: &crate::action::FrozenAction, evidence: &[u8]) -> CommitteeInput {
    EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: profile().delivery.scope,
        },
        snapshot(),
        BTreeMap::from([("r".into(), evidence.to_vec())]),
    )
    .unwrap()
    .inputs_for(action, &profile().committee)
    .unwrap()
}

pub(super) fn admit(host: &mut FileOversight, request: u64, payload: &[u8]) -> u64 {
    let status = host
        .submit_request(host.revision(), request, spec(payload), snapshot())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("fixture request did not reach admission");
    };
    attempt
}

pub(super) fn begin(host: &mut FileOversight, request: u64, round: u64) -> CommitteeInput {
    let action = host.request_action(request).unwrap().clone();
    let input = inputs(&action, b"private helper input");
    let FileRequestDisposition::Admitted { attempt, .. } =
        host.request_status(request).unwrap().disposition
    else {
        panic!("fixture request was not admitted");
    };
    let previous = host.input_revision(attempt).unwrap();
    host.record_inputs(host.revision(), attempt, previous, input.clone())
        .unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap().0;
    host.begin_review(
        host.revision(),
        attempt,
        round,
        ROOT,
        ReviewWindow {
            commit_by: ElapsedTick(now + 5),
            reveal_by: ElapsedTick(now + 10),
        },
        snapshot(),
    )
    .unwrap();
    input
}

pub(super) fn votes(host: &mut FileOversight, round: u64, verdict: Verdict) {
    let digest = commitment(round, "r", &ROOT, verdict, SALT).unwrap();
    host.commit_review(host.revision(), round, "r", digest)
        .unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "r", verdict, SALT.to_vec())
        .unwrap();
}

pub(super) fn completed(
    host: &mut FileOversight,
    request: u64,
    round: u64,
) -> (CommitteeInput, ObservedReceipt) {
    admit(host, request, b"ok");
    let input = begin(host, request, round);
    votes(host, round, Verdict::Allow);
    let receipt = host
        .finish_review(host.revision(), round, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    (input, receipt)
}
