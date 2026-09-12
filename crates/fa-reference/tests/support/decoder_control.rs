//! Existing congress/endpoint fixture plus an actual, identity-matched decoder.
#[path = "two_key_delivery.rs"]
#[allow(dead_code)]
pub mod delivery_fixture;
#[path = "monitored_source.rs"]
#[allow(dead_code)]
pub mod numerical;
pub use delivery_fixture::Fixture;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredDecoder;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::oversight::{CommitteeInput, ObservedReview, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;

pub fn run() -> MonitoredDecoder {
    let p = numerical::fixture::profile(16);
    let p = DecoderProfile::new(DecoderIdentity { model_generation: 1, tokenizer_generation: 1,
        ..p.identity() }, p.shape(), p.epsilon(), p.theta()).unwrap();
    numerical::monitored(numerical::fixture::model(p), 1_000_000.0, numerical::allowance())
}
pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
pub fn spec(f: &Fixture) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(f.endpoint.target()), payload: b"publish".to_vec(), required_witnesses: vec![],
        policy_epoch: f.broker.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
pub fn actor(tokens: &[u32]) -> ActorState {
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, tokens.to_vec(), vec![2], vec![3], tokens.len() as u64).unwrap()
}
pub fn admit(f: &mut Fixture, id: u64) -> (FrozenAction, CommitteeInput) {
    let spec = spec(f);
    let action = f.broker.propose(id, spec, &snapshot()).unwrap().action;
    let contracts = f.broker.contracts(); let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(&action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    let inputs = CommitteeInput::capture(&action, contracts, BTreeMap::from([("helper".into(), manifest)])).unwrap();
    f.broker.record_inputs(id, 0, inputs.clone()).unwrap();
    (action, inputs)
}
pub fn ballot(broker: &mut OversightBroker, id: u64, round: u64, verdict: Verdict) -> ObservedReview {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let commit = session.commitment("helper", verdict, b"salt").unwrap();
    session.commit("helper", commit, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", verdict, b"salt", now).unwrap(); session.finish(now).unwrap()
}
pub fn approved(f: &mut Fixture, id: u64) -> (FrozenAction, CommitteeInput, Permit) {
    let (action, inputs) = admit(f, id);
    let review = ballot(&mut f.broker, id, id + 10, Verdict::Allow);
    f.broker.apply_review(review, Some(&inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
    (action, inputs, permit)
}

pub fn surviving_owners(f: Fixture) -> (OversightBroker, fa_reference::action::consequence::delivery::PublicationEndpoint) {
    (f.broker, f.endpoint)
}
