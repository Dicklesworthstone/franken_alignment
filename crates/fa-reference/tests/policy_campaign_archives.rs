//! Detached archive consumer and regression for the public controller::replay path.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{ControllerConfig, PolicyAuthority};
use fa_reference::action::consequence::gate::containment::session::policy::controller::replay::{DecisionArchive, ReviewAnchor};
use fa_reference::action::consequence::policy_campaign::{PolicyDelta, PolicyReplayReport, ReplayLimits};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn limits() -> ReplayLimits { ReplayLimits { cases: 8, input_bytes: 1_048_576 } }
fn candidate(predicate: Predicate) -> Policy { Policy::new(2, vec![predicate]).unwrap() }

fn archived() -> (DecisionArchive, ReviewAnchor) {
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    let mut controller = PolicyAuthority::new(ControllerConfig {
        scope, total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    }).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    let snapshot = Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, vec![9]), (99, b"not-retained".to_vec())]) };
    controller.propose(1, ActionSpec { version: VERSION, scope, target: Some(target),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16 }, &snapshot).unwrap();
    let mut session = controller.begin_review(1, 11, [1; 32], &snapshot).unwrap();
    let anchor = session.replay_anchor();
    let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", commitment).unwrap();
    session.open_reveals().unwrap();
    session.reveal("helper", Verdict::Allow, b"salt").unwrap();
    let archive = controller.apply_review_archived(session.finish().unwrap(), &snapshot).unwrap().archive;
    (archive, anchor)
}

#[test]
fn actual_archives_support_detached_comparison_but_not_unobserved_reads() {
    let (archive, anchor) = archived();
    let before = archive.clone();
    let report = PolicyReplayReport::from_archives(&anchor.policy,
        candidate(Predicate::PayloadAtMost(2)), &[(&archive, &anchor)], limits()).unwrap();
    assert_eq!(report.cases()[0].delta(), PolicyDelta::NewlyBlocked);
    let report = PolicyReplayReport::from_archives(&anchor.policy,
        candidate(Predicate::Absent { key: 99 }), &[(&archive, &anchor)], limits()).unwrap();
    assert!(report.requires_shadow());
    assert_eq!(report.cases()[0].missing_nodes(), &[0]);
    assert_eq!(archive, before);
}

#[test]
fn corrupt_duplicate_and_wrong_policy_archives_never_yield_a_partial_report() {
    let (archive, anchor) = archived();
    let mut corrupted = archive.clone();
    corrupted.tally.permit_weight += 1;
    assert_eq!(PolicyReplayReport::from_archives(&anchor.policy,
        candidate(Predicate::PayloadAtMost(100)), &[(&corrupted, &anchor)], limits()).unwrap_err(), Error::Binding);
    assert_eq!(PolicyReplayReport::from_archives(&anchor.policy,
        candidate(Predicate::PayloadAtMost(100)), &[(&archive, &anchor), (&archive, &anchor)], limits()).unwrap_err(), Error::Duplicate);
    let wrong = Policy::new(1, vec![Predicate::PayloadAtMost(100)]).unwrap();
    assert_eq!(PolicyReplayReport::from_archives(&wrong,
        candidate(Predicate::PayloadAtMost(100)), &[(&archive, &anchor)], limits()).unwrap_err(), Error::Binding);
}
