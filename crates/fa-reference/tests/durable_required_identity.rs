//! Original owned inference, identity gate and real local publication journals.
//! Scripted congress ballots isolate identity gating, not helper effectiveness.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_identity_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{identity::{ModelManifest, ModelPassport},
    tensor::kv::decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, JournalError, JournalLimits,
    observed::{FileOversight, FileOversightProfile,
        decoder::learned::FileLearnedConfig,
        guarded::{FileGuardSet, FileIdentityRequirement, FileOversightRoles,
            FileRecoveryFloor, FileRecoveryRequirements},
        identity::{FileIdentityChallenge, FileIdentityObserver}}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    identity::{IdentityMismatch, IdentityOutcome, IdentityPolicy, IdentityStatus},
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
        let root = std::env::temp_dir().join(format!("fa-required-identity-{}-{stamp}-{}",
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

use fa_reference::action::consequence::activation::identity::decoder::{
    DecoderIdentityProbe, IdentityProbeProgress,
};
use fa_reference::action::consequence::delivery::persistent::observed::identity::{
    FileComputedIdentityObservation, FileLearnedIdentityInput,
};

fn recipe(required: bool, model: DecoderModel) -> FileLearnedConfig {
    let source = fixture::source(&model, false);
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap();
    if required { config.with_required_computed_identity().unwrap() } else { config }
}
fn setup(root: &Directory, required: bool, model: DecoderModel)
    -> (FileOversight, FileOversightRoles, FileLearnedConfig, ModelPassport)
{
    let passport = fixture::passport(&fixture::model(1.0));
    let config = recipe(required, model);
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation(
        root.store(), profile(), &guards(&passport), None, config.clone()).unwrap();
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

#[test]
fn strict_first_image_computes_identity_then_requires_original_congress_and_both_keys() {
    let root = Directory::new();
    let (mut host, roles, config, _) = setup(&root, true, fixture::model(1.0));
    assert!(config.requires_computed_identity() && config.requires_sidecar());
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Missing));
    step(&mut host); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap();
    let check = challenge(&mut host, 1);
    let observed = computed(&mut host, roles.identity_observer.as_ref().unwrap(), &check, 1);
    assert_eq!(observed.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    assert_eq!(observed.work.entered_tokens, 4);
    assert_eq!(observed.work.completed_tokens, 4);
    assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
    assert!(host.identity_installation(1).unwrap().is_none());
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Missing));
    install(&mut host, &check);
    publish(&mut host, &roles);
    let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(disk.payload, b"visible"); assert_eq!(disk.executions, 1);
}

#[test]
fn manual_actual_frames_remain_legacy_only_and_refusal_does_not_poison_computed_path() {
    for required in [false, true] {
        let root = Directory::new();
        let (mut host, roles, config, passport) = setup(&root, required, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap();
        step(&mut host); let check = challenge(&mut host, 1);
        let before = root.bytes(); let revision = host.revision();
        let result = observer.observe_manifest(&mut host, revision, &check,
            passport.manifest().clone(), ElapsedTick(1));
        if required {
            assert_eq!(result, Err(Error::Binding.into()));
            assert_eq!(root.bytes(), before); assert_eq!(host.revision(), revision);
        } else { assert_eq!(result.unwrap().measurement.unwrap().outcome, IdentityOutcome::Collecting); }
        let mut original = DecoderIdentityProbe::new(fixture::model(1.0), &passport, 701,
            DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        for _ in 0..original.work().planned_tokens {
            if let IdentityProbeProgress::Measured(frame) = original.advance().unwrap() {
                let current_revision = revision_if_current(&host, revision, required);
                let result = observer.observe_anchor(&mut host, current_revision,
                    &check, frame.anchor(), frame.source(), ElapsedTick(1));
                if required { assert_eq!(result, Err(Error::Binding.into())); }
                else { assert!(result.unwrap().measurement.is_ok()); }
            }
        }
        if required {
            assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), before);
            assert!(host.identity_report(1).unwrap().manifest.is_none());
            assert!(host.identity_report(1).unwrap().observations.is_empty());
            assert_eq!(computed(&mut host, observer, &check, 1).observation.measurement.unwrap().outcome,
                IdentityOutcome::Matched);
        }
        install(&mut host, &check);
        assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { .. }));
        FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    }
}
fn revision_if_current(host: &FileOversight, original: u64, required: bool) -> u64 {
    if required { original } else { host.revision() }
}

#[test]
fn missing_identity_guard_refuses_before_directory_creation_and_wrapper_order_is_exact() {
    let model = fixture::model(1.0);
    let passport = fixture::passport(&model);
    let source = fixture::source(&model, false);
    let plain = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    assert!(!plain.requires_computed_identity());
    let a = plain.clone().with_required_sidecar().unwrap().with_required_computed_identity().unwrap();
    let b = plain.with_required_computed_identity().unwrap().with_required_sidecar().unwrap();
    assert!(a.requires_sidecar() && b.requires_sidecar());
    assert!(a.requires_computed_identity() && b.requires_computed_identity());
    assert_ne!(a, b);
    assert_eq!(a.clone().with_required_computed_identity().err(), Some(Error::Duplicate));
    for config in [a, b] {
        let root = Directory::new(); let mut missing = guards(&passport); missing.identity = None;
        assert_eq!(FileOversight::create_guarded_with_learned_generation(root.store(), profile(),
            &missing, None, config.clone()).err(), Some(Error::Incomplete.into()));
        assert!(!root.store().exists());
        let (mut host, roles) = FileOversight::create_guarded_with_learned_generation(root.store(), profile(),
            &guards(&passport), None, config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let check = challenge(&mut host, 1);
        assert_eq!(computed(&mut host, roles.identity_observer.as_ref().unwrap(), &check, 1)
            .observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
        install(&mut host, &check);
    }
}

#[test]
fn a_prior_challenge_cannot_be_grandfathered_into_the_strict_bootstrap() {
    for prior in [false, true] {
        let root = Directory::new(); let model = fixture::model(1.0);
        let passport = fixture::passport(&model);
        let config = recipe(true, model);
        let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &guards(&passport), None).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if prior {
            host.begin_identity_check(host.revision(), 1, host.inspect().control.sequence, 0).unwrap().unwrap();
        }
        let bytes = root.bytes(); let revision = host.revision();
        let result = host.enable_learned_generation(revision, config);
        if prior {
            assert_eq!(result, Err(Error::WrongState.into()));
            assert_eq!(root.bytes(), bytes); assert_eq!(host.revision(), revision);
            assert!(host.learned_generation_inspection().is_err());
        } else {
            result.unwrap(); assert!(host.learned_generation_inspection().is_ok());
        }
    }
}

#[test]
fn matching_frames_from_a_surrogate_cannot_hide_a_same_label_model_change() {
    let root = Directory::new();
    let (mut host, roles, config, passport) = setup(&root, true, fixture::model(2.0));
    let observer = roles.identity_observer.as_ref().unwrap();
    let check = challenge(&mut host, 1); let revision = host.revision();
    assert_eq!(observer.observe_manifest(&mut host, revision, &check, passport.manifest().clone(), ElapsedTick(1)),
        Err(Error::Binding.into()));
    let result = computed(&mut host, observer, &check, 1);
    assert_eq!(result.observation.measurement.unwrap().outcome,
        IdentityOutcome::Mismatch(IdentityMismatch::Anchor { anchor: 10 }));
    assert!(result.observation.containment.unwrap().is_ok());
    assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
    let report = host.identity_report(1).unwrap();
    assert_eq!(report.observations[&10].first_outlier().unwrap().observed_bits, 2.0_f32.to_bits());
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config)
        .unwrap().executions, 0);
}

#[test]
fn strict_recipe_cannot_be_downgraded_on_read_or_reopen_and_recovery_needs_fresh_computation() {
    let root = Directory::new();
    let (mut host, roles, config, passport) = setup(&root, true, fixture::model(1.0));
    step(&mut host); let check = challenge(&mut host, 1);
    computed(&mut host, roles.identity_observer.as_ref().unwrap(), &check, 1);
    install(&mut host, &check);
    let expected = requirements(&host, &passport);
    let before = root.bytes(); let numerical = host.learned_generation_inspection().unwrap().numerical;
    let weak = recipe(false, fixture::model(1.0));
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &weak).err(),
        Some(Error::Binding.into()));
    drop(host);
    assert_eq!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &weak).err(),
        Some(Error::Binding.into()));
    assert_eq!(root.bytes(), before);
    let (mut host, fresh) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let new_check = challenge(&mut host, 2); let revision = host.revision();
    assert!(roles.identity_observer.as_ref().unwrap().observe_computed_learned(&mut host, revision,
        &new_check, input(&passport, 702), || panic!("foreign role must not observe time")).is_err());
    let observer = fresh.identity_observer.as_ref().unwrap();
    assert_eq!(observer.observe_manifest(&mut host, revision, &new_check,
        passport.manifest().clone(), ElapsedTick(1)), Err(Error::Binding.into()));
    computed(&mut host, observer, &new_check, 1); install(&mut host, &new_check);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.apply_identity_check(host.revision(), &check,
        check.evidence().control_sequence(), check.evidence().revocation_epoch()).is_err());
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    assert!(!host.learned_generation_inspection().unwrap().paused);
}

#[test]
fn strict_mode_preserves_exact_work_admission_and_native_deadline_withdrawal() {
    for expired in [false, true] {
        let root = Directory::new(); let model = fixture::model(1.0);
        let (mut host, roles, _, passport) = setup(&root, true, model.clone());
        let observer = roles.identity_observer.as_ref().unwrap();
        let check = challenge(&mut host, 1); let revision = host.revision(); let before = root.bytes();
        let products = passport.anchors().values().map(|anchor|
            model.estimate(0, anchor.stimulus().len()).unwrap().scalar_products().unwrap()).sum();
        let mut request = input(&passport, 701); request.budget.scalar_products = products - 1;
        assert_eq!(observer.observe_computed_learned(&mut host, revision, &check, request.clone(),
            || panic!("insufficient budget must refuse before clock or inference")).err(), Some(Error::Limit.into()));
        assert_eq!(root.bytes(), before);
        request.budget.scalar_products = products;
        let mut times = [ElapsedTick(1), ElapsedTick(if expired { 21 } else { 20 })].into_iter();
        let result = observer.observe_computed_learned(&mut host, revision, &check, request,
            || times.next().expect("exact original pre/post clock observations")).unwrap();
        assert_eq!(result.work.completed_scalar_products, products);
        if expired {
            assert_eq!(result.observation.measurement, Err(Error::Stale));
            assert!(host.identity_installation(1).unwrap().is_none());
            assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Missing));
            let revision = host.revision();
            assert_eq!(observer.observe_manifest(&mut host, revision, &check, passport.manifest().clone(), ElapsedTick(21)),
                Err(Error::Binding.into()));
            let next = challenge(&mut host, 2);
            computed(&mut host, observer, &next, 21); install(&mut host, &next);
        } else { install(&mut host, &check); }
        assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { .. }));
    }
}

#[test]
fn separately_observed_manifest_mismatch_keeps_original_containment_without_anchor_work() {
    let root = Directory::new(); let (mut host, roles, _, passport) = setup(&root, true, fixture::model(1.0));
    let check = challenge(&mut host, 1); let mut request = input(&passport, 701);
    let mut observed: ModelManifest = passport.manifest().clone(); observed.weights[0] ^= 1;
    request.observed_manifest = observed.clone();
    let revision = host.revision();
    let result = roles.identity_observer.as_ref().unwrap().observe_computed_learned(&mut host, revision,
        &check, request, || ElapsedTick(1)).unwrap();
    assert_eq!(result.work.entered_tokens, 0);
    assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
    assert!(result.observation.containment.unwrap().is_ok());
    assert_eq!(host.identity_report(1).unwrap().manifest, Some(observed));
    assert!(host.inspect().control.suspended);
}
