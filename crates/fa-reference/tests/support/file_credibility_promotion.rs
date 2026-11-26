#![allow(dead_code)]
//! Original durable reviews and separately held labels for promotion-port tests.
//! Reused from the native joint-credibility fixture; synthetic verdicts exercise
//! enforcement and do not claim trained-helper quality.
use fa_reference::Snapshot;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliveryProfile, FilePermit, JournalLimits, RecoveryReserve,
    observed::{FileHumanReviewer, FileOversight, FileOversightProfile,
        credibility::FileIndependentEvaluator},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::oversight::credibility::{
    Assessment, EvaluationProtocol, Fraction, GroundTruth,
};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::oversight::joint_credibility::{
    JointPromotionPolicy, JointReplayBudget,
};
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ReviewWindow,
};
use fa_reference::action::{
    ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "fa-live-credibility-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    pub fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.0.join("delivery.bin")).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists()
            && let Err(error) = std::fs::remove_dir_all(&self.0)
        {
            eprintln!("joint journal fixture cleanup: {error}");
        }
    }
}
fn fraction(numerator: u64, denominator: u64) -> Fraction {
    Fraction {
        numerator,
        denominator,
    }
}
pub fn protocol() -> EvaluationProtocol {
    EvaluationProtocol {
        domain: 1,
        stratum: 2,
        period: 3,
        minimum_violation_origins: 2,
        minimum_benign_origins: 1,
        precision_floor: fraction(1, 2),
        recall_floor: fraction(1, 2),
        false_positive_ceiling: fraction(1, 2),
        false_stop_budget: 100,
    }
}
pub fn guard(budget: JointReplayBudget) -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, fraction(0, 1), fraction(0, 1), budget).unwrap()
}
pub fn profile(hold_maximum: u64) -> FileOversightProfile {
    let target = ResolvedTarget {
        adapter: 10,
        object: 11,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    };
    let scope = Scope {
        tenant: 1,
        principal: 2,
        run: 3,
        branch: 4,
        authority: 5,
        purpose: Purpose::Effect,
    };
    let actor = ActorState::new(
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 1,
            tokenizer_generation: 1,
            state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart,
        },
        vec![1],
        vec![2],
        vec![3],
        1,
    )
    .unwrap();
    let committee = CommitteeContract::new(
        ["a", "b"]
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    HelperContract::new(
                        InputProfileBinding {
                            profile_id: 1,
                            profile_bytes: vec![],
                            model_epoch: 1,
                            tokenizer_epoch: 1,
                            policy_epoch: 0,
                        },
                        1,
                        b"approve?".to_vec(),
                    )
                    .unwrap(),
                )
            })
            .collect(),
    )
    .unwrap();
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope,
            total: 100,
            max_attempts: 16,
            actor,
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: ["a", "b"]
                    .into_iter()
                    .map(|name| {
                        (
                            name.to_owned(),
                            MemberPolicy {
                                cohort: name.to_owned(),
                                weight: 10,
                            },
                        )
                    })
                    .collect(),
                caps: Caps {
                    per_member: 10,
                    per_cohort: 10,
                },
                continue_minimum: 5,
                continue_hold_maximum: hold_maximum,
                narrow_at: 20,
                suspend_at: 30,
                minimum_members: 2,
                minimum_cohorts: 2,
            },
            narrowed_targets: vec![target],
            target,
            initial_payload: b"initial".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 16,
            clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee,
        human: HumanReviewPolicy {
            reviewer_id: 77,
            max_validity_ticks: 50,
            max_requests: 16,
        },
    }
}
pub fn ready(
    hold: u64,
    budget: JointReplayBudget,
) -> (
    Directory,
    FileOversight,
    FileHumanReviewer,
    FileIndependentEvaluator,
) {
    let root = Directory::new();
    let (mut host, human) = FileOversight::create(&root.0, profile(hold)).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let evaluator = host
        .enable_joint_credibility(host.revision(), protocol(), guard(budget))
        .unwrap();
    (root, host, human, evaluator)
}
pub fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::new(),
    }
}
pub fn begin(host: &mut FileOversight, id: u64) -> (FrozenAction, CommitteeInput) {
    let now = ElapsedTick(id * 4);
    host.observe_time(host.revision(), now).unwrap();
    let p = profile(0);
    let action = host
        .propose(
            host.revision(),
            id,
            ActionSpec {
                version: VERSION,
                scope: p.delivery.scope,
                target: Some(host.inspect().target),
                payload: b"visible".to_vec(),
                required_witnesses: vec![],
                policy_epoch: host.inspect().control.ledger.epoch,
                deadline: ElapsedTick(1000),
                units: 16,
            },
            snapshot(),
        )
        .unwrap();
    let source = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 1,
            generation: id,
            scope: p.delivery.scope,
        },
        snapshot(),
        ["a", "b"]
            .into_iter()
            .map(|name| (name.to_owned(), b"context".to_vec()))
            .collect(),
    )
    .unwrap();
    let inputs = source.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone())
        .unwrap();
    host.begin_review(
        host.revision(),
        id,
        id + 100,
        [id as u8; 32],
        ReviewWindow {
            commit_by: ElapsedTick(now.0 + 1),
            reveal_by: ElapsedTick(now.0 + 2),
        },
        snapshot(),
    )
    .unwrap();
    (action, inputs)
}
pub fn review(
    host: &mut FileOversight,
    id: u64,
    a: Verdict,
    b: Verdict,
) -> (FrozenAction, CommitteeInput) {
    let result = begin(host, id);
    finish(host, id, &result.1, a, b);
    result
}

/// Complete the SAME original review after an eligibility preview; no replay
/// fixture or replacement round supplies its missing member outcomes.
pub fn finish(
    host: &mut FileOversight,
    id: u64,
    inputs: &CommitteeInput,
    a: Verdict,
    b: Verdict,
) {
    for (name, verdict) in [("a", a), ("b", b)] {
        let digest = commitment(id + 100, name, &[id as u8; 32], verdict, b"salt").unwrap();
        host.commit_review(host.revision(), id + 100, name, digest)
            .unwrap();
    }
    host.open_reveals(host.revision(), id + 100).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        host.reveal_review(host.revision(), id + 100, name, verdict, b"salt".to_vec())
            .unwrap();
    }
    host.finish_review(host.revision(), id + 100, Some(inputs), snapshot())
        .unwrap()
        .unwrap();
}

pub fn assess(
    host: &mut FileOversight,
    evaluator: &FileIndependentEvaluator,
    id: u64,
    origin: u64,
    truth: GroundTruth,
) {
    let ticket = host.evaluation_ticket(id + 100).unwrap();
    let revision = host.revision();
    evaluator
        .assess(
            host,
            revision,
            &ticket,
            Assessment {
                origin,
                evidence_id: [2; 32],
                truth,
            },
        )
        .unwrap();
}
pub fn calibrate(host: &mut FileOversight, evaluator: &FileIndependentEvaluator) {
    for (id, a, b, truth) in [
        (1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation),
        (2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation),
        (3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign),
    ] {
        review(host, id, a, b);
        assess(host, evaluator, id, id, truth);
    }
    assert!(host.credibility_report().unwrap().qualified());
}
pub fn dispatch(
    host: &mut FileOversight,
    human: &FileHumanReviewer,
    id: u64,
    action: &FrozenAction,
    inputs: &CommitteeInput,
) -> FilePermit {
    let key = host
        .authorize(host.revision(), id, inputs, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(
            host.revision(),
            1000 + id,
            id,
            inputs,
            ElapsedTick(id * 4 + 30),
        )
        .unwrap();
    let revision = host.revision();
    let second = human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &key, &second, action, inputs, snapshot())
        .unwrap();
    key
}
