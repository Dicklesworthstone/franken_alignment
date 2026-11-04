pub use fa_reference::{Error, Snapshot};
pub use fa_reference::action::{ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::{decoder::DecoderModel, experiment::KvSide},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
pub use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
    credential_broker::{BrokerCredential, BrokerRouteBinding, CredentialRevocationRequest,
        CredentialRotationRequest, ProviderCredential},
    stream::StreamProfile, persistent::{FileDeliveryProfile, JournalError, JournalLimits, Reconciliation,
        RecoveryReserve, observed::{FileHumanReviewer, FileOversight, FileOversightProfile,
            consistency::{FileConsistencyConfig, FileConsistencyParameters, learned::FileLearnedConsistencyConfig},
            credential::{FileCredentialPermit, FileCredentialPolicy, FILE_OVERSIGHT_CREDENTIAL_PROFILE},
            decoder::learned::FileLearnedConfig,
            guarded::{FileCredentialEpoch, FileCredentialRegistration, FileGuardSet, FileRecoveryFloor,
                FileRecoveryRequirements, mediated::predictive::FileMediatedPredictor},
            publication::PublicationBasis, replay::FileReviewReplayStatus,
        },
    },
};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
pub use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract,
    ReviewWindow, decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_host::sidecar::LearnedSidecarRequest, sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::full_input::InputProfileBinding;
pub use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use super::fixture;

static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Directory(PathBuf);
impl Directory {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-lcred-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub fn store(&self) -> PathBuf { self.0.join("publication") }
    pub fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = std::fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("credential learned cleanup: {error}"); }
    }
}
pub fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 4096, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: Vec::new(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
pub fn stream() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }
fn model() -> DecoderModel { fixture::model(&[u32::from(b'O'), u32::from(b'K'), fixture::END]) }
pub fn recipe() -> FileLearnedConfig {
    let model = model();
    FileLearnedConfig::new_text_stream(model.clone(), fixture::tokenizer(&model), fixture::config(&model),
        LearnedDecoderBindingLimits::default(), stream()).unwrap().with_required_sidecar().unwrap()
}
pub fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".to_owned(), route: "adapter:file-oversight".to_owned() }
}
pub fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"oversight-token","holder":"broker"}}],
      "routes":[{{"route":"adapter:file-oversight","effect":"file_write",
      "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
      "residual_nonclaims":["operator storage remains trusted"]}}]}}"#,
      FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap()
}
pub fn policy() -> FileCredentialPolicy {
    FileCredentialPolicy::from_inventory(&inventory(), &binding(), profile().delivery.scope, profile().delivery.target).unwrap()
}
pub fn selected() -> FileLearnedConfig { recipe().with_required_credential_policy(policy()).unwrap() }
pub fn changed_policies() -> Vec<FileCredentialPolicy> {
    (0..4).map(|field| { let mut changed = policy();
        match field { 0 => changed.family.push_str("-other"), 1 => changed.route.push_str("-other"),
            2 => changed.credential.push_str("-other"), _ => changed.profile_generation += 1 }
        changed
    }).collect()
}
pub fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
        source: config.required_policy_source(), identity: config.required_computed_identity_registration().cloned(),
        campaigns: None, credential: config.required_credential_policy().cloned() }
}
pub fn create(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let config = selected(); let guards = guards(&config); let inventory = inventory(); let binding = binding();
    let (host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
        &guards, Some(FileCredentialRegistration { inventory: &inventory, binding: &binding }), config,
        RecoveryReserve::terminal()).unwrap();
    (host, roles.human)
}
pub fn bind(host: &FileOversight, broker: &[u8], provider: &[u8]) -> Result<FileCredentialPermit, JournalError> {
    host.bind_credential_pair(host.revision(), &inventory(), &binding(), BrokerCredential::new(broker.to_vec()).unwrap(),
        ProviderCredential::new(provider.to_vec()).unwrap())
}
pub fn requirements(host: &FileOversight, config: &FileLearnedConfig) -> FileRecoveryRequirements {
    let status = host.credential_status().unwrap().unwrap(); let control = host.inspect().control;
    FileRecoveryRequirements { guards: guards(config), effective_policy: profile().delivery.policy,
        credential_epoch: Some(FileCredentialEpoch { generation: status.generation, revoked: status.revoked }),
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } }
}
pub fn lane_recipe(lane: u8) -> FileLearnedConfig {
    let mut config = recipe();
    if lane != 0 {
        let model = model(); let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
        let capture = if lane == 1 { model.residual_contract(1).unwrap().profile() }
            else { model.cache_profile().layers()[&1].values().profile() };
        let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0], bias: 0.0, threshold: 0.0,
            forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
                event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
            alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
        }).unwrap().with_stream_messages(stream()).unwrap();
        config = if lane == 1 {
            config.with_required_pre_output_forecast(prediction.with_hosted_residual(1).unwrap()
                .with_pre_output_forecast().unwrap()).unwrap()
        } else {
            config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(prediction,
                1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
                MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
                .with_pre_output_forecast().unwrap()).unwrap()
        };
    }
    config.with_required_credential_policy(policy()).unwrap()
}
pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub fn dispatched(host: &mut FileOversight, human: &FileHumanReviewer) -> (FrozenAction, CommitteeInput) {
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..4 {
        let n = host.learned_generation_inspection().unwrap().numerical;
        if !n.status.is_active() { break; }
        host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
    let action = host.propose_learned_text_stream_message(host.revision(), 1, ElapsedTick(100), Vec::new(), snapshot()).unwrap();
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let packet = host.begin_learned_sidecar(host.revision(), 1, actor,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let input = packet.packet.input().clone();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer",
        commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let approval = human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &approval, &action, &input, snapshot()).unwrap();
    (action, input)
}
