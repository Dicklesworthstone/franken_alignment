//! Shared public-API fixtures for the bounded streaming reference profile.
//! The endpoint is in-memory and helper outcomes are explicit test inputs.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{DispatchEnvelope, PublicationEndpoint};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReceipt, OversightBroker, ReviewWindow, action_frame,
};
use fa_reference::action::{ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;

pub const TOTAL: u64 = 100_000;

pub fn target(version: u64) -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: version, generation: 4 }
}

pub fn policy(generation: u64) -> Policy {
    Policy::new(generation, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap()
}

pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

pub fn fixture() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    fixture_with(StreamProfile::new(9, 2, 8, 64, 512).unwrap())
}

pub fn fixture_with(profile: StreamProfile) -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"complete-cumulative-stream-v1".to_vec(),
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review the full prior messages and this complete release; never approve from only its suffix".to_vec(),
    ).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: TOTAL, max_attempts: 32,
        actor: ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: policy(1),
        congress: CongressPolicy {
            generation: 1,
            members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[target(1), target(2), target(3), target(4), target(5)]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new_stream(target(1), profile, 200, 32).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    (broker, endpoint, contracts)
}

pub fn input(action: &FrozenAction, contracts: &CommitteeContract) -> CommitteeInput {
    let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), manifest)])).unwrap()
}

pub fn prepare(
    broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, message: Option<&str>,
) -> (FrozenAction, CommitteeInput) {
    let spec = match message {
        Some(message) => broker.stream_message_spec(message, ElapsedTick(100)).unwrap(),
        None => broker.stream_finish_spec(ElapsedTick(100)).unwrap(),
    };
    let proposal = broker.propose(id, spec, &snapshot()).unwrap();
    let inputs = input(&proposal.action, contracts);
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    (proposal.action, inputs)
}

pub fn review(broker: &mut OversightBroker, id: u64, round: u64, inputs: &CommitteeInput, verdict: Verdict) -> ObservedReceipt {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let commitment = session.commitment("helper", verdict, b"salt").unwrap();
    session.commit("helper", commitment, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("helper", verdict, b"salt", now).unwrap();
    let completed = session.finish(now).unwrap();
    broker.apply_review(completed, Some(inputs), &snapshot()).unwrap()
}

pub fn ready(
    broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64, message: Option<&str>,
) -> (FrozenAction, CommitteeInput, Permit) {
    let (action, inputs) = prepare(broker, contracts, id, message);
    review(broker, id, id, &inputs, Verdict::Allow);
    let permit = broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
    (action, inputs, permit)
}

pub fn publish(
    broker: &mut OversightBroker, endpoint: &mut PublicationEndpoint, contracts: &CommitteeContract,
    id: u64, message: Option<&str>,
) -> DispatchEnvelope {
    let (action, inputs, permit) = ready(broker, contracts, id, message);
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap();
    broker.accept_receipt(receipt).unwrap();
    envelope
}

pub fn conserved(broker: &OversightBroker) {
    let ledger = broker.inspect().ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, TOTAL);
}
