//! Concrete brokered file publication over the existing authority and endpoint.
#![cfg(unix)]

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, FilePublicationLimits,
    NonExecutionReason, PublicationEndpoint, credential_broker::{BrokerCredential, BrokerRouteBinding,
    CredentialBroker, ProviderCredential, DISPOSABLE_FILE_PROFILE}};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::perimeter_inventory::{InventoryLoadError, LoadedPerimeterInventory};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-credential-broker-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("broker test cleanup: {error}"); } }
}

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn inventory(profile: &str, effect: &str, mediation: &str, purpose: u64) -> Result<LoadedPerimeterInventory, InventoryLoadError> {
    let bytes = format!(r#"{{
      "version":1,
      "families":[{{
        "scope":{{"tenant":1,"principal":2,"purpose":{purpose}}},
        "family":"publication",
        "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
        "credentials":[{{"credential":"publish-token","holder":"broker"}}],
        "routes":[{{
          "route":"adapter:disposable-file",
          "effect":"{effect}",
          "profile":{{"id":"{profile}","generation":1}},
          "trust_path":["actor","enforcement"],
          "threat":"direct_credential_or_egress",
          "actor_credential":{{"kind":"broker_mediated"}},
          "mediation":"{mediation}",
          "bypass":"blocked",
          "residual_nonclaims":["operator storage and credential authenticity remain assumptions"]
        }}],
        "residual_nonclaims":["no complete deployment perimeter claim"]
      }}]
    }}"#);
    LoadedPerimeterInventory::from_json_bytes(bytes.as_bytes())
}
fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".to_owned(), route: "adapter:disposable-file".to_owned() }
}
fn broker_secret(bytes: &[u8]) -> BrokerCredential { BrokerCredential::new(bytes.to_vec()).unwrap() }
fn provider_secret(bytes: &[u8]) -> ProviderCredential { ProviderCredential::new(bytes.to_vec()).unwrap() }

struct Fixture {
    broker: OversightBroker,
    endpoint: Option<PublicationEndpoint>,
    recovery: fa_reference::action::consequence::delivery::FileEndpointRecovery,
    contracts: CommitteeContract,
}
impl Fixture {
    fn new(root: &Temp) -> Self {
        let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"profile-v1".to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 9, b"Review".to_vec(),
        ).unwrap())])).unwrap();
        let config = ControllerConfig {
            scope: scope(), total: 100, max_attempts: 16,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".to_owned(),
                MemberPolicy { cohort: "a".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
        };
        let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(&root.0, target(), b"old".to_vec(),
            200, 16, FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        let ack = endpoint.install_fence(broker.fence_request()).unwrap();
        broker.confirm_fence(ack).unwrap();
        Self { broker, endpoint: Some(endpoint), recovery, contracts }
    }

    fn dispatch(&mut self, id: u64) -> fa_reference::action::consequence::delivery::DispatchEnvelope {
        let spec = ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
            required_witnesses: vec![], policy_epoch: self.broker.inspect().ledger.epoch,
            deadline: ElapsedTick(100), units: 16 };
        let action = self.broker.propose(id, spec, &snapshot()).unwrap().action;
        let helper = &self.contracts.members()["helper"];
        let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question());
        let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
            SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
        ], vec![]).unwrap();
        let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 9,
            policy_epoch: action.spec().policy_epoch, projected_originals: vec![] }, vec![]).unwrap();
        let inputs = CommitteeInput::capture(&action, &self.contracts,
            BTreeMap::from([("helper".to_owned(), manifest)])).unwrap();
        self.broker.record_inputs(id, 0, inputs.clone()).unwrap();
        let now = self.broker.inspect().ledger.elapsed.unwrap();
        let mut review = self.broker.begin_review(id, id + 100, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, &snapshot()).unwrap();
        let commitment = review.commitment("helper", Verdict::Allow, b"salt").unwrap();
        review.commit("helper", commitment, now).unwrap(); review.open_reveals(now).unwrap();
        review.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
        self.broker.apply_review(review.finish(now).unwrap(), Some(&inputs), &snapshot()).unwrap();
        let permit = self.broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
        self.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap()
    }

    fn perimeter(&mut self) -> CredentialBroker {
        CredentialBroker::new(inventory(DISPOSABLE_FILE_PROFILE, "file_write", "brokered_effects", 1).unwrap(),
            binding(), scope(), broker_secret(b"opaque-secret"), provider_secret(b"opaque-secret"),
            self.endpoint.take().unwrap()).unwrap()
    }
}

#[test]
fn brokered_file_route_executes_only_the_original_authorized_envelope() {
    let root = Temp::new(); let mut fixture = Fixture::new(&root); let message = fixture.dispatch(1);
    let mut perimeter = fixture.perimeter();
    let receipt = perimeter.deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    fixture.broker.accept_receipt(receipt).unwrap();
    assert_eq!(perimeter.payload(), b"visible");
    let state = PublicationEndpoint::read_file_publication(fixture.recovery.directory()).unwrap();
    assert_eq!(state.payload, b"visible"); assert_eq!(state.execution_count, 1);
    assert_eq!(perimeter.inspect().credential_exercises, 1);
    assert_eq!(fixture.broker.inspect().ledger.charged, 16);
}

#[test]
fn independently_supplied_provider_secret_must_match_before_attachment() {
    let root = Temp::new(); let mut fixture = Fixture::new(&root);
    let endpoint = fixture.endpoint.take().unwrap();
    let result = CredentialBroker::new(inventory(DISPOSABLE_FILE_PROFILE, "file_write", "brokered_effects", 1).unwrap(),
        binding(), scope(), broker_secret(b"broker-secret"), provider_secret(b"provider-secret"), endpoint);
    assert_eq!(result.unwrap_err(), Error::Binding);
    let state = PublicationEndpoint::read_file_publication(fixture.recovery.directory()).unwrap();
    assert_eq!(state.payload, b"old"); assert_eq!(state.execution_count, 0);
}

#[test]
fn lost_acknowledgment_reuses_the_same_effect_without_a_second_execution() {
    let root = Temp::new(); let mut fixture = Fixture::new(&root); let message = fixture.dispatch(1);
    let mut perimeter = fixture.perimeter();
    let first = perimeter.deliver(&message).unwrap();
    let second = perimeter.deliver(&message).unwrap();
    assert_eq!(first, second); assert_eq!(perimeter.inspect().endpoint_executions, 1);
    assert_eq!(perimeter.inspect().credential_exercises, 2);
    fixture.broker.acknowledgment_lost(1).unwrap();
    fixture.broker.accept_receipt(second).unwrap();
    assert_eq!(fixture.broker.inspect().ledger.charged, 16);
}

#[test]
fn missing_effect_can_be_sealed_and_refunded_without_presenting_the_effect_credential() {
    let root = Temp::new(); let mut fixture = Fixture::new(&root); let _message = fixture.dispatch(1);
    let mut perimeter = fixture.perimeter();
    fixture.broker.acknowledgment_lost(1).unwrap();
    let query = fixture.broker.status_query(1).unwrap();
    assert_eq!(perimeter.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    let receipt = perimeter.seal_unexecuted(&query).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    fixture.broker.accept_receipt(receipt).unwrap();
    assert_eq!(perimeter.inspect().credential_exercises, 0);
    assert_eq!(fixture.broker.inspect().ledger.available, 100);
}

#[test]
fn foreign_dispatch_binding_cannot_use_a_matching_declared_route() {
    let left = Temp::new(); let right = Temp::new();
    let mut owner = Fixture::new(&left); let mut foreign = Fixture::new(&right);
    let _owner_message = owner.dispatch(1); let foreign_message = foreign.dispatch(1);
    let mut perimeter = owner.perimeter();
    assert_eq!(perimeter.deliver(&foreign_message), Err(Error::Binding));
    assert_eq!(perimeter.inspect().credential_exercises, 1);
    assert_eq!(perimeter.inspect().endpoint_executions, 0);
}

#[test]
fn nonbrokered_wrong_effect_wrong_profile_and_wrong_scope_all_refuse_attachment() {
    for (profile_id, effect, mediation, purpose) in [
        (DISPOSABLE_FILE_PROFILE, "file_write", "cooperative_gate", 1),
        (DISPOSABLE_FILE_PROFILE, "network_request", "brokered_effects", 1),
        ("other-profile", "file_write", "brokered_effects", 1),
        (DISPOSABLE_FILE_PROFILE, "file_write", "brokered_effects", 2),
    ] {
        let root = Temp::new(); let mut fixture = Fixture::new(&root);
        let endpoint = fixture.endpoint.take().unwrap();
        let result = CredentialBroker::new(inventory(profile_id, effect, mediation, purpose).unwrap(), binding(), scope(),
            broker_secret(b"secret"), provider_secret(b"secret"), endpoint);
        assert!(result.is_err());
    }
}

#[test]
fn memory_only_endpoint_and_experiment_scope_are_not_the_disposable_file_profile() {
    let endpoint = PublicationEndpoint::new(target(), b"old".to_vec(), 100, 4).unwrap();
    assert!(CredentialBroker::new(inventory(DISPOSABLE_FILE_PROFILE, "file_write", "brokered_effects", 1).unwrap(),
        binding(), scope(), broker_secret(b"secret"), provider_secret(b"secret"), endpoint).is_err());

    let root = Temp::new(); let mut fixture = Fixture::new(&root); let endpoint = fixture.endpoint.take().unwrap();
    let experiment = Scope { purpose: Purpose::Experiment, ..scope() };
    assert_eq!(CredentialBroker::new(inventory(DISPOSABLE_FILE_PROFILE, "file_write", "brokered_effects", 1).unwrap(),
        binding(), experiment, broker_secret(b"secret"), provider_secret(b"secret"), endpoint).unwrap_err(), Error::Binding);
}

#[test]
fn broker_and_provider_credential_bytes_are_bounded_and_redacted() {
    assert_eq!(BrokerCredential::new(Vec::new()).unwrap_err(), Error::InvalidInput);
    assert_eq!(ProviderCredential::new(Vec::new()).unwrap_err(), Error::InvalidInput);
    let too_large = vec![0; fa_reference::action::consequence::delivery::credential_broker::MAX_BROKER_CREDENTIAL_BYTES + 1];
    assert_eq!(BrokerCredential::new(too_large.clone()).unwrap_err(), Error::Limit);
    assert_eq!(ProviderCredential::new(too_large).unwrap_err(), Error::Limit);
    let broker = BrokerCredential::new(b"extremely-sensitive".to_vec()).unwrap();
    let provider = ProviderCredential::new(b"equally-sensitive".to_vec()).unwrap();
    let broker_text = format!("{broker:?}"); let provider_text = format!("{provider:?}");
    assert!(!broker_text.contains("sensitive")); assert!(broker_text.contains("redacted"));
    assert!(!provider_text.contains("sensitive")); assert!(provider_text.contains("redacted"));
}
