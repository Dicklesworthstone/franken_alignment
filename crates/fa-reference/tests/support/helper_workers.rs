//! Public fixture: two distinct helper profiles, one actual reference broker.
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedSession, OversightBroker, ReviewWindow, action_frame,
};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::Snapshot;
use std::collections::BTreeMap;

pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

pub struct Fixture {
    pub broker: OversightBroker,
    pub endpoint: PublicationEndpoint,
    pub action: FrozenAction,
    pub inputs: CommitteeInput,
}

impl Fixture {
    pub fn new() -> Self {
        let target = ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 };
        Self::with_endpoint(PublicationEndpoint::new(target, b"old".to_vec(), 200, 16).unwrap())
    }

    pub fn with_endpoint(mut endpoint: PublicationEndpoint) -> Self {
        let target = endpoint.target();
        let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
        let mut members = BTreeMap::new();
        let mut contracts = BTreeMap::new();
        for (i, name) in ["alpha", "beta"].into_iter().enumerate() {
            members.insert(name.to_owned(), MemberPolicy { cohort: name.to_owned(), weight: 1 });
            contracts.insert(name.to_owned(), HelperContract::new(InputProfileBinding {
                profile_id: i as u64 + 1, profile_bytes: format!("{name}-profile").into_bytes(),
                model_epoch: i as u64 + 1, tokenizer_epoch: 1, policy_epoch: 0,
            }, 9, format!("{name}-private-question").into_bytes()).unwrap());
        }
        let contracts = CommitteeContract::new(contracts).unwrap();
        let config = ControllerConfig {
            scope, total: 100, max_attempts: 16,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
            congress: CongressPolicy { generation: 1, members,
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2, continue_hold_maximum: 0,
                narrow_at: 3, suspend_at: 4, minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
        };
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
        let action = broker.propose(1, ActionSpec { version: VERSION, scope, target: Some(target),
            payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
            deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action;
        let mut views = BTreeMap::new();
        for (name, helper) in contracts.members() {
            let mut bytes = action_frame(&action);
            let end = bytes.len();
            bytes.extend_from_slice(helper.question());
            let input = ActualHelperInput::new(bytes.clone(), helper.profile_at(0), vec![
                SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
                SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
            ], vec![]).unwrap();
            views.insert(name.clone(), EvidenceViewManifest::new(input, AuthorizationProjection {
                projection_id: 9, policy_epoch: 0, projected_originals: vec![],
            }, vec![]).unwrap());
        }
        let inputs = CommitteeInput::capture(&action, &contracts, views).unwrap();
        broker.record_inputs(1, 0, inputs.clone()).unwrap();
        Self { broker, endpoint, action, inputs }
    }

    pub fn start(&mut self, round: u64) -> ObservedSession {
        self.broker.begin_review(1, round, [8; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
        }, &snapshot()).unwrap()
    }
}
