//! Exact registered identity through original numerical, guard and effect paths.
//! Synthetic weights/anchors and scripted ballots are causal controls, not
//! signed passports, calibrated substitution coverage or serving-host attestation.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_identity_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{identity::{IdentityAnchor, ModelManifest, ModelPassport},
    tensor::kv::decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, JournalError, JournalLimits, RecoveryReserve,
    observed::{FileOversight, FileOversightProfile,
        decoder::learned::FileLearnedConfig,
        guarded::{FileGuardSet, FileIdentityRequirement, FileOversightRoles,
            FileRecoveryFloor, FileRecoveryRequirements},
        identity::{FileIdentityChallenge, FileIdentityObserver}}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    identity::{IdentityMismatch, IdentityOutcome, IdentityPolicy},
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-registered-identity-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("identity cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"owned-identity-test".to_vec(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards(passport: &ModelPassport) -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, campaigns: None, credential: None,
        identity: Some(FileIdentityRequirement { passport: passport.clone(), policy: IdentityPolicy {
            observer_id: 99, timeout_ticks: 20, validity_ticks: 80, max_checks: 8 } }) }
}
fn step(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn challenge(host: &mut FileOversight, id: u64) -> FileIdentityChallenge {
    let control = host.inspect().control;
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    host.begin_identity_check(host.revision(), id, control.sequence, actor).unwrap().unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn requirements(host: &FileOversight, passport: &ModelPassport) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards: guards(passport), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch } }
}

use fa_reference::action::consequence::delivery::persistent::observed::identity::{
    FileComputedIdentityObservation, FileLearnedIdentityInput,
};

fn recipe(required: bool, model: DecoderModel) -> FileLearnedConfig {
    let source = fixture::source(&model, false);
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap();
    if required { config.with_required_computed_identity().unwrap() } else { config }
}
fn setup(root: &Directory, model: DecoderModel)
    -> (FileOversight, FileOversightRoles, FileLearnedConfig, ModelPassport)
{
    let passport = fixture::passport(&fixture::model(1.0));
    let config = recipe(false, model).with_required_computed_identity_registration(
        guards(&passport).identity.unwrap()).unwrap();
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
        root.store(), profile(), &guards(&passport), None, config.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles, config, passport)
}
fn input(passport: &ModelPassport, sequence: u64) -> FileLearnedIdentityInput {
    FileLearnedIdentityInput { measurement_sequence: sequence,
        budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        observed_manifest: passport.manifest().clone() }
}
fn computed(host: &mut FileOversight, observer: &FileIdentityObserver,
    check: &FileIdentityChallenge, now: u64) -> FileComputedIdentityObservation
{
    let request = input(check.evidence().passport(), 700 + check.id());
    let revision = host.revision();
    observer.observe_computed_learned(host, revision, check, request, || ElapsedTick(now)).unwrap()
}
fn install(host: &mut FileOversight, check: &FileIdentityChallenge) {
    let expected = check.evidence();
    host.apply_identity_check(host.revision(), check,
        expected.control_sequence(), expected.revocation_epoch()).unwrap();
}
fn publish(host: &mut FileOversight, roles: &FileOversightRoles) {
    let action = host.propose(host.revision(), 1, spec(host), snapshot()).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let packet = host.begin_learned_sidecar(host.revision(), 1, numerical.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let current = packet.packet.input();
    assert!(host.authorize(host.revision(), 1, current, snapshot()).is_err());
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer",
        commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(current), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, current, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(current), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, current, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = roles.human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, current, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(current), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

fn registration() -> FileIdentityRequirement {
    guards(&fixture::passport(&fixture::model(1.0))).identity.unwrap()
}
fn variants(original: &FileIdentityRequirement) -> Vec<FileIdentityRequirement> {
    let mut variants = Vec::new();
    for field in 0..19 {
        let mut passport_id = original.passport.id();
        let mut generation = original.passport.generation();
        let mut manifest = original.passport.manifest().clone();
        let mut policy = original.policy;
        match field {
            0 => passport_id += 1,
            1 => generation += 1,
            2 => manifest.weights[0] ^= 1,
            3 => manifest.adapters[0] ^= 1,
            4 => manifest.tokenizer[0] ^= 1,
            5 => manifest.architecture[0] ^= 1,
            6 => manifest.numeric_profile[0] ^= 1,
            7 => manifest.host_generation += 1,
            8 => manifest.tokenizer_generation += 1,
            15 => policy.observer_id += 1,
            16 => policy.timeout_ticks += 1,
            17 => policy.validity_ticks += 1,
            18 => policy.max_checks += 1,
            _ => {}
        }
        let anchors = original.passport.anchors().values().enumerate().map(|(index, anchor)| {
            let mut id = anchor.id(); let mut profile = anchor.profile();
            let mut stream = anchor.stream(); let mut stimulus = anchor.stimulus().to_vec();
            let mut bounds: Vec<_> = anchor.bound_bits().iter().map(|[lo, hi]|
                [f32::from_bits(*lo), f32::from_bits(*hi)]).collect();
            if index == 0 {
                match field {
                    9 => id += 1,
                    10 => profile.tap += 1,
                    11 => profile.layout_generation += 1,
                    12 => stream += 1,
                    13 => stimulus[0] = (stimulus[0] + 1) % 3,
                    14 => bounds[0][1] = f32::from_bits(bounds[0][1].to_bits() + 1),
                    _ => {}
                }
            }
            IdentityAnchor::new(id, profile, stream, stimulus, &bounds).unwrap()
        }).collect();
        variants.push(FileIdentityRequirement {
            passport: ModelPassport::new(passport_id, generation, manifest, anchors).unwrap(), policy,
        });
    }
    variants
}
fn stage(root: &Directory) {
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged candidate").unwrap();
}
fn staged(root: &Directory) {
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"unacknowledged candidate");
}

use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
    identity::decoder::transaction::ComputedIdentityStatus,
};
fn joint() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 1, 0, 0, HeldOutJointBudget::default()).unwrap()
}

#[test]
fn exact_registration_binds_native_passport_and_policy_without_changing_boolean_recipes() {
    let original = recipe(false, fixture::model(1.0)); let identity = registration();
    let boolean = original.clone().with_required_computed_identity().unwrap();
    assert_eq!(boolean, recipe(true, fixture::model(1.0)));
    assert_eq!(boolean.encoded_bytes(), original.encoded_bytes() + 28);
    assert_eq!(boolean.required_computed_identity_registration(), None);
    let exact = original.clone().with_required_computed_identity_registration(identity.clone()).unwrap();
    assert!(exact.requires_computed_identity() && exact.requires_sidecar());
    assert_eq!(exact.required_computed_identity_registration(), Some(&identity));
    assert_ne!(exact, boolean);
    for changed in variants(&identity) {
        assert_ne!(exact, original.clone().with_required_computed_identity_registration(changed).unwrap());
    }
    assert_eq!(boolean.with_required_computed_identity_registration(identity.clone()).err(), Some(Error::Duplicate));
    assert_eq!(exact.clone().with_required_computed_identity().err(), Some(Error::Duplicate));
    assert_eq!(exact.clone().with_required_computed_identity_registration(identity.clone()).err(), Some(Error::Duplicate));
    for field in 0..3 {
        let mut invalid = identity.clone();
        match field { 0 => invalid.policy.observer_id = 0,
            1 => invalid.policy.timeout_ticks = 0, _ => invalid.policy.max_checks = 0 }
        assert_eq!(original.clone().with_required_computed_identity_registration(invalid).err(), Some(Error::InvalidInput));
    }
    let first = exact.with_required_held_out_joint(joint()).unwrap();
    let last = original.with_required_held_out_joint(joint()).unwrap()
        .with_required_computed_identity_registration(identity.clone()).unwrap();
    assert_ne!(first, last);
    for config in [first, last] {
        assert_eq!(config.required_computed_identity_registration(), Some(&identity));
        assert_eq!(config.required_held_out_joint(), Some(joint()));
        let root = Directory::new();
        let (host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
            root.store(), profile(), &guards(&identity.passport), None, config, RecoveryReserve::terminal()).unwrap();
        assert!(host.identity_checks_required() && roles.identity_observer.is_some());
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(joint()));
    }
}

#[test]
fn exact_identity_mismatches_and_prior_challenges_refuse_before_owner_or_write() {
    let identity = registration(); let base = recipe(false, fixture::model(1.0));
    let config = base.clone().with_required_computed_identity_registration(identity.clone()).unwrap();
    for changed in variants(&identity) {
        let root = Directory::new(); let mut wrong = guards(&identity.passport); wrong.identity = Some(changed);
        assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(
            root.store(), profile(), &wrong, None, config.clone(), RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
    let root = Directory::new(); let mut missing = guards(&identity.passport); missing.identity = None;
    assert_eq!(FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
        &missing, None, config.clone(), RecoveryReserve::terminal()).err(), Some(Error::Incomplete.into()));
    assert!(!root.store().exists());
    for prior in [false, true] {
        let root = Directory::new();
        let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &guards(&identity.passport), None).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if prior { host.begin_identity_check(host.revision(), 1, host.inspect().control.sequence, 0).unwrap().unwrap(); }
        let bytes = root.bytes(); let before = host.inspect();
        let result = host.enable_learned_generation(host.revision(), config.clone());
        if prior {
            assert_eq!(result, Err(Error::WrongState.into()));
            assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
            assert!(host.learned_generation_inspection().is_err());
        } else { result.unwrap(); assert!(host.learned_generation_inspection().is_ok()); }
    }
}

#[test]
fn computed_original_anchors_allow_publication_and_same_label_weight_substitution_is_contained() {
    let reference = fixture::model(1.0);
    for weight in [1.0, 2.0] {
        let model = fixture::model(weight); assert_eq!(model.profile(), reference.profile());
        let root = Directory::new(); let (mut host, mut roles, config, passport) = setup(&root, model);
        step(&mut host); step(&mut host);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let check = challenge(&mut host, 1); let revision = host.revision(); let before = root.bytes();
        assert_eq!(roles.identity_observer.as_ref().unwrap().observe_manifest(&mut host, revision, &check,
            passport.manifest().clone(), ElapsedTick(1)), Err(Error::Binding.into()));
        assert_eq!(root.bytes(), before);
        let mut run = roles.identity_observer.take().unwrap().begin_computed_learned(&host, host.revision(),
            &check, input(&passport, 701), ElapsedTick(1)).unwrap();
        while run.status() == ComputedIdentityStatus::Computing {
            let completed = run.work().completed_tokens;
            run.advance_with_clock(&host, run.revision(), || ElapsedTick(1)).unwrap();
            assert_eq!(run.work().completed_tokens, completed + 1);
            assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), before);
        }
        assert_eq!(run.status(), ComputedIdentityStatus::ReadyToCommit);
        let result = run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap().clone();
        roles.identity_observer = Some(run.take_observer().unwrap());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        if weight == 1.0 {
            assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
            assert_eq!(result.work.completed_tokens, 4);
            assert!(host.identity_installation(1).unwrap().is_none());
            install(&mut host, &check); publish(&mut host, &roles);
            assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config)
                .unwrap().executions, 1);
        } else {
            assert_eq!(result.observation.measurement.unwrap().outcome,
                IdentityOutcome::Mismatch(IdentityMismatch::Anchor { anchor: 10 }));
            assert!(result.observation.containment.unwrap().is_ok());
            assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.identity_report(1).unwrap().observations[&10].first_outlier().unwrap().observed_bits,
                2.0_f32.to_bits());
            assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
        }
    }
}

#[test]
fn historical_audit_and_recovery_bind_registration_before_any_identity_measurement() {
    let root = Directory::new(); let (host, old_roles, config, passport) = setup(&root, fixture::model(1.0));
    let expected = requirements(&host, &passport); let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.identity_report(1).is_err()); drop(host); stage(&root); let bytes = root.bytes();
    let base = recipe(false, fixture::model(1.0)); let mut wrong = vec![base.clone(), base.clone().with_required_computed_identity().unwrap()];
    wrong.extend(variants(&registration()).into_iter().map(|identity|
        base.clone().with_required_computed_identity_registration(identity).unwrap()));
    for config in wrong {
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).err(), Some(Error::Binding.into()));
        assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).err(), Some(Error::Binding.into()));
        assert_eq!(root.bytes(), bytes); staged(&root);
    }
    let mut wrong = expected.clone(); wrong.guards.identity.as_mut().unwrap().policy.validity_ticks += 1;
    assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &wrong, &config).err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), bytes); staged(&root);
    let (mut host, fresh) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert!(!root.store().join("delivery.pending").exists()); assert!(!host.clock_ready());
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); let check = challenge(&mut host, 1);
    let revision = host.revision();
    assert!(old_roles.identity_observer.as_ref().unwrap().observe_computed_learned(&mut host, revision,
        &check, input(&passport, 701), || panic!("old observer must fail before clock access")).is_err());
    assert_eq!(computed(&mut host, fresh.identity_observer.as_ref().unwrap(), &check, 2)
        .observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    install(&mut host, &check);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused);
}

#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod text_model;
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::delivery::{stream::StreamProfile, persistent::observed::{
    consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver,
        learned::FileLearnedConsistencyConfig},
    decoder::learned::FileLearnedRecoveryStatus,
    guarded::{learned_recovery::FileGuardedLearnedRecovery,
        predictive::{FilePredictiveRequirements, learned::FilePredictiveLearnedRecovery,
            owned::{FileOwnedPredictiveRequirements, FileOwnedPredictiveRecovery}}},
    source::FileSourcePolicy,
}};
use fa_reference::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Plain, Raw, Owned }
fn stream() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }
fn stream_profile() -> FileOversightProfile {
    let mut p = profile(); p.delivery.initial_payload.clear(); p
}
fn policy_source() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() }
}
fn text_registration(model: &DecoderModel) -> FileIdentityRequirement {
    // A permissive finite interval is sufficient for this custody-composition
    // control. The separate exact-anchor test above detects actual substitution.
    FileIdentityRequirement { passport: ModelPassport::new(61, 1, ModelManifest {
        tenant: 1, model: 2, model_generation: 3, host_generation: 1, tokenizer_generation: 4,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32],
    }, vec![IdentityAnchor::new(10, model.residual_contract(1).unwrap().profile(), 501,
        vec![text_model::PROMPT], &[[-4.0, 4.0]; 4]).unwrap()]).unwrap(), policy: registration().policy }
}
fn stream_recipe(lane: Lane) -> FileLearnedConfig {
    let model = text_model::model(&[u32::from(b'O'), u32::from(b'K'), text_model::END]);
    let identity = text_registration(&model);
    let config = FileLearnedConfig::new_text_stream(model.clone(), text_model::tokenizer(&model),
        text_model::config(&model), LearnedDecoderBindingLimits::default(), stream()).unwrap()
        .with_required_sidecar().unwrap().with_required_policy_source(policy_source()).unwrap();
    let config = if lane == Lane::Plain { config } else {
        let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
        let capture = if lane == Lane::Raw { model.residual_contract(1).unwrap().profile() }
            else { model.cache_profile().layers()[&1].values().profile() };
        let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0],
            bias: 0.0, threshold: 0.0,
            forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
                event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
            alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
        }).unwrap().with_stream_messages(stream()).unwrap();
        if lane == Lane::Raw {
            config.with_required_pre_output_forecast(prediction.with_hosted_residual(1).unwrap()
                .with_pre_output_forecast().unwrap()).unwrap()
        } else {
            config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(prediction,
                1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
                MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
                .with_pre_output_forecast().unwrap()).unwrap()
        }
    };
    config.with_required_computed_identity_registration(identity).unwrap()
        .with_required_held_out_joint(joint()).unwrap()
}
fn stream_guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
        source: config.required_policy_source(), identity: config.required_computed_identity_registration().cloned(),
        campaigns: None, credential: None }
}
fn create_stream(root: &Directory, lane: Lane, config: &FileLearnedConfig, guards: &FileGuardSet,
    profile: FileOversightProfile) -> Result<(FileOversight, FileOversightRoles, Option<FileConsistencyObserver>), JournalError>
{
    if lane == Lane::Plain {
        let (host, roles) = FileOversight::create_guarded_with_learned_generation_reserved(
            root.store(), profile, guards, None, config.clone(), RecoveryReserve::terminal())?;
        Ok((host, roles, None))
    } else {
        let (host, roles) = if lane == Lane::Raw {
            FileOversight::create_predictive_guarded_with_learned_generation_reserved(root.store(), profile,
                guards, None, config.required_pre_output_forecast().unwrap().clone(), None, config.clone(), RecoveryReserve::terminal())?
        } else {
            FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(root.store(), profile,
                guards, None, config.required_owned_pre_output_forecast().unwrap().clone(), None, config.clone(), RecoveryReserve::terminal())?
        };
        assert!(roles.evaluator.is_none());
        Ok((host, roles.oversight, Some(roles.consistency_observer)))
    }
}
enum Recovery { Plain(FileGuardedLearnedRecovery), Raw(FilePredictiveLearnedRecovery), Owned(FileOwnedPredictiveRecovery) }
impl Recovery {
    fn drive(&mut self) {
        loop {
            let progress = match self { Self::Plain(run) => run.progress(), Self::Raw(run) => run.progress(), Self::Owned(run) => run.progress() };
            if progress.status != FileLearnedRecoveryStatus::Replaying { assert_eq!(progress.status, FileLearnedRecoveryStatus::Ready); break; }
            match self { Self::Plain(run) => run.advance(progress.replayed_events, 1),
                Self::Raw(run) => run.advance(progress.replayed_events, 1), Self::Owned(run) => run.advance(progress.replayed_events, 1) }.unwrap();
        }
    }
    fn finish(self) -> (FileOversight, FileOversightRoles, Option<FileConsistencyObserver>) {
        match self {
            Self::Plain(run) => { let (host, roles) = run.finish().unwrap(); (host, roles, None) }
            Self::Raw(run) => { let (host, roles) = run.finish().unwrap(); (host, roles.oversight, Some(roles.consistency_observer)) }
            Self::Owned(run) => { let (host, roles) = run.finish().unwrap(); (host, roles.oversight, Some(roles.consistency_observer)) }
        }
    }
}
fn begin_stream(root: &Directory, lane: Lane, config: &FileLearnedConfig, expected: FileRecoveryRequirements)
    -> Result<Recovery, JournalError>
{
    Ok(match lane {
        Lane::Plain => Recovery::Plain(FileOversight::begin_open_guarded_with_learned_generation(
            root.store(), stream_profile(), &expected, config)?.require_recovery_reserve(RecoveryReserve::terminal())?),
        Lane::Raw => Recovery::Raw(FileOversight::begin_open_predictive_guarded_with_learned_generation(
            root.store(), stream_profile(), &FilePredictiveRequirements { oversight: expected,
                prediction: config.required_pre_output_forecast().unwrap().clone(), evaluation: None }, config)?
            .require_recovery_reserve(RecoveryReserve::terminal())?),
        Lane::Owned => Recovery::Owned(FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), stream_profile(), &FileOwnedPredictiveRequirements { oversight: expected,
                prediction: config.required_owned_pre_output_forecast().unwrap().clone(), evaluation: None }, config)?
            .require_recovery_reserve(RecoveryReserve::terminal())?),
    })
}

#[test]
fn reserved_stream_bootstrap_keeps_source_identity_and_all_predictor_roles_in_one_image() {
    for lane in [Lane::Plain, Lane::Raw, Lane::Owned] {
        let config = stream_recipe(lane); let g = stream_guards(&config);
        for field in 0..4 {
            let root = Directory::new(); let mut wrong = g.clone(); let mut p = stream_profile();
            match field { 0 => wrong.identity.as_mut().unwrap().policy.validity_ticks += 1,
                1 => wrong.source = None, 2 => wrong.stream = None, _ => p.delivery.limits.events = 3 }
            assert!(create_stream(&root, lane, &config, &wrong, p).is_err());
            assert!(!root.store().exists());
        }
        let root = Directory::new();
        let (mut host, roles, prediction) = create_stream(&root, lane, &config, &g, stream_profile()).unwrap();
        assert!(roles.identity_observer.is_some()); assert_eq!(prediction.is_some(), lane != Lane::Plain);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(joint()));
        assert_eq!(host.file_source_status().unwrap().policy, policy_source());
        assert!(host.file_source_status().unwrap().producer.is_none());
        assert!(host.policy_only_file_source_required() && host.learned_sidecar_required());
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        let n = host.learned_generation_inspection().unwrap().numerical;
        let check = challenge(&mut host, 1);
        let observed = computed(&mut host, roles.identity_observer.as_ref().unwrap(), &check, 1);
        assert_eq!(observed.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
        assert_eq!(observed.work.completed_tokens, 1); install(&mut host, &check);
        let control = host.inspect().control;
        let expected = FileRecoveryRequirements { guards: g, effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence, authority_epoch: control.ledger.epoch } };
        drop(host); stage(&root); let before = root.bytes();
        let mut wrong = expected.clone(); wrong.guards.identity.as_mut().unwrap().policy.max_checks += 1;
        let mut rejected = begin_stream(&root, lane, &config, wrong).unwrap();
        let result = loop {
            let progress = match &rejected { Recovery::Plain(run) => run.progress(), Recovery::Raw(run) => run.progress(), Recovery::Owned(run) => run.progress() };
            let advanced = match &mut rejected { Recovery::Plain(run) => run.advance(progress.replayed_events, 1),
                Recovery::Raw(run) => run.advance(progress.replayed_events, 1), Recovery::Owned(run) => run.advance(progress.replayed_events, 1) };
            if let Err(error) = advanced { break error; }
            assert_eq!(progress.status, FileLearnedRecoveryStatus::Replaying);
        };
        assert_eq!(result, Error::Binding.into()); drop(rejected);
        assert_eq!(root.bytes(), before); staged(&root);
        let mut run = begin_stream(&root, lane, &config, expected).unwrap(); run.drive();
        assert_eq!(root.bytes(), before); staged(&root);
        let (host, fresh, prediction) = run.finish();
        assert!(fresh.identity_observer.is_some()); assert_eq!(prediction.is_some(), lane != Lane::Plain);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready()); assert!(!root.store().join("delivery.pending").exists());
    }

    // Native exact-registration preflight succeeds, then text installation
    // rejects the missing stream. The original candidate cannot leak a learned
    // owner or the recipe's other nested requirements into this live host.
    let root = Directory::new(); let config = stream_recipe(Lane::Plain);
    let mut g = stream_guards(&config); g.stream = None; g.source = None;
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &g, None).unwrap();
    let before = host.inspect(); let bytes = root.bytes();
    assert!(host.enable_learned_generation(host.revision(), config).is_err());
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert!(host.learned_generation_inspection().is_err());
    assert_eq!(host.held_out_joint_policy().unwrap(), None);
}
