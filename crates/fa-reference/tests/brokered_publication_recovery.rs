//! Credential and endpoint identity survive file-endpoint process interruption.
#![cfg(unix)]

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, FileEndpointRecovery,
    FilePublicationLimits, NonExecutionReason, PublicationEndpoint,
    credential_broker::{BrokerCredential, BrokerReconnectError, BrokerRouteBinding, ProviderCredential,
        RecoverableCredentialBroker, DISPOSABLE_FILE_PROFILE}};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-broker-recovery-{label}-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
}
impl Drop for Temp {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("recovery cleanup: {error}"); } }
}
fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn target() -> ResolvedTarget { ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 } }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"publish-token","holder":"broker"}}],
      "routes":[{{"route":"adapter:disposable-file","effect":"file_write",
      "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
      "residual_nonclaims":["reference"]}}]}}"#, DISPOSABLE_FILE_PROFILE).as_bytes()).unwrap()
}
fn binding() -> BrokerRouteBinding { BrokerRouteBinding { family: "publication".into(), route: "adapter:disposable-file".into() } }
fn broker_secret() -> BrokerCredential { BrokerCredential::new(b"opaque-secret".to_vec()).unwrap() }
fn provider_secret() -> ProviderCredential { ProviderCredential::new(b"opaque-secret".to_vec()).unwrap() }

struct Fixture {
    broker: OversightBroker,
    endpoint: Option<PublicationEndpoint>,
    recovery: Option<FileEndpointRecovery>,
    contracts: CommitteeContract,
}
impl Fixture {
    fn new(root: &Temp) -> Self {
        let contracts = CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"profile-v1".to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 9, b"Review".to_vec()).unwrap())])).unwrap();
        let config = ControllerConfig { scope: scope(), total: 100, max_attempts: 16,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
                vec![1], vec![2], vec![3], 1).unwrap(), suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".into(),
                MemberPolicy { cohort: "a".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap() };
        let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(&root.0, target(), b"old".to_vec(),
            200, 16, FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
        let ack = endpoint.install_fence(broker.fence_request()).unwrap(); broker.confirm_fence(ack).unwrap();
        Self { broker, endpoint: Some(endpoint), recovery: Some(recovery), contracts }
    }
    fn dispatch(&mut self, id: u64) -> fa_reference::action::consequence::delivery::DispatchEnvelope {
        let action = self.broker.propose(id, ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
            payload: b"visible".to_vec(), required_witnesses: vec![], policy_epoch: self.broker.inspect().ledger.epoch,
            deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action;
        let helper = &self.contracts.members()["helper"];
        let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question());
        let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
            SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
        ], vec![]).unwrap();
        let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 9,
            policy_epoch: action.spec().policy_epoch, projected_originals: vec![] }, vec![]).unwrap();
        let inputs = CommitteeInput::capture(&action, &self.contracts, BTreeMap::from([("helper".into(), manifest)])).unwrap();
        self.broker.record_inputs(id, 0, inputs.clone()).unwrap();
        let now = self.broker.inspect().ledger.elapsed.unwrap();
        let mut review = self.broker.begin_review(id, id + 100, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, &snapshot()).unwrap();
        let digest = review.commitment("helper", Verdict::Allow, b"salt").unwrap();
        review.commit("helper", digest, now).unwrap(); review.open_reveals(now).unwrap();
        review.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
        self.broker.apply_review(review.finish(now).unwrap(), Some(&inputs), &snapshot()).unwrap();
        let permit = self.broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
        self.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap()
    }
    fn take_recoverable(&mut self) -> RecoverableCredentialBroker {
        RecoverableCredentialBroker::new(inventory(), binding(), scope(), broker_secret(), provider_secret(),
            self.endpoint.take().unwrap(), self.recovery.take().unwrap()).unwrap()
    }
}

#[test]
fn executed_lost_ack_is_recovered_without_representing_the_credential() {
    let root = Temp::new("executed"); let mut fixture = Fixture::new(&root); let message = fixture.dispatch(1);
    let mut guarded = fixture.take_recoverable();
    let first = guarded.broker_mut().deliver(&message).unwrap();
    assert_eq!(first.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    fixture.broker.acknowledgment_lost(1).unwrap();
    let offline = guarded.into_offline();
    let mut guarded = offline.reopen().unwrap();
    guarded.broker_mut().observe_time(ElapsedTick(2)).unwrap(); fixture.broker.observe_time(ElapsedTick(2)).unwrap();
    let query = fixture.broker.status_query(1).unwrap();
    let EndpointStatus::Resolved(receipt) = guarded.broker().status(&query).unwrap() else { panic!("execution receipt lost"); };
    assert_eq!(receipt, first);
    fixture.broker.accept_receipt(receipt).unwrap();
    assert_eq!(guarded.broker().inspect().credential_exercises, 1);
    assert_eq!(guarded.broker().inspect().endpoint_executions, 1);
    assert_eq!(fixture.broker.inspect().ledger.charged, 16);
}

#[test]
fn missing_effect_is_sealed_after_reopen_without_effect_credential_use() {
    let root = Temp::new("missing"); let mut fixture = Fixture::new(&root); let _message = fixture.dispatch(1);
    fixture.broker.acknowledgment_lost(1).unwrap();
    let mut guarded = fixture.take_recoverable().into_offline().reopen().unwrap();
    guarded.broker_mut().observe_time(ElapsedTick(2)).unwrap(); fixture.broker.observe_time(ElapsedTick(2)).unwrap();
    let query = fixture.broker.status_query(1).unwrap();
    assert_eq!(guarded.broker().status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    let receipt = guarded.broker_mut().seal_unexecuted(&query).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    fixture.broker.accept_receipt(receipt).unwrap();
    assert_eq!(guarded.broker().inspect().credential_exercises, 0);
    assert_eq!(fixture.broker.inspect().ledger.available, 100);
}

#[test]
fn exact_envelope_remains_idempotent_after_broker_reopen() {
    let root = Temp::new("retry"); let mut fixture = Fixture::new(&root); let message = fixture.dispatch(1);
    let mut guarded = fixture.take_recoverable(); let first = guarded.broker_mut().deliver(&message).unwrap();
    let mut guarded = guarded.into_offline().reopen().unwrap(); guarded.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    let second = guarded.broker_mut().deliver(&message).unwrap();
    assert_eq!(first, second); assert_eq!(guarded.broker().inspect().endpoint_executions, 1);
    assert_eq!(guarded.broker().inspect().credential_exercises, 2);
}

#[test]
fn mismatched_endpoint_and_recovery_key_cannot_rebind_the_credential() {
    let left = Temp::new("left"); let right = Temp::new("right");
    let mut left_fixture = Fixture::new(&left); let mut right_fixture = Fixture::new(&right);
    let endpoint = left_fixture.endpoint.take().unwrap();
    let foreign_key = right_fixture.recovery.take().unwrap();
    // Release the foreign lock so its key can reopen; the retained process-local
    // endpoint binding must still reject it after reopen.
    drop(right_fixture.endpoint.take());
    let guarded = RecoverableCredentialBroker::new(inventory(), binding(), scope(),
        broker_secret(), provider_secret(), endpoint, foreign_key).unwrap();
    let failure = guarded.into_offline().reopen().unwrap_err();
    assert!(matches!(failure.error, BrokerReconnectError::Contract(Error::Binding)));
    assert_eq!(format!("{:?}", failure.offline).contains("opaque-secret"), false);
}
