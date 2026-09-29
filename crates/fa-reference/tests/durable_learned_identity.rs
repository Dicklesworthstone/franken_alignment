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
        identity::{FileIdentityChallenge, FileIdentityObserver,
            decoder::{FileDecoderIdentityEvent, FileDecoderIdentityProbe}}}}};
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
        let root = std::env::temp_dir().join(format!("fa-owned-identity-{}-{stamp}-{}",
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
fn setup(root: &Directory, model: DecoderModel) -> (FileOversight, FileOversightRoles, FileLearnedConfig, ModelPassport) {
    let passport = fixture::passport(&fixture::model(1.0));
    let source = fixture::source(&model, false);
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap();
    let (mut host, roles) = FileOversight::create_guarded_with_learned_generation(
        root.store(), profile(), &guards(&passport), None, config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles, config, passport)
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
fn start(host: &mut FileOversight, observer: &FileIdentityObserver, id: u64) -> FileDecoderIdentityProbe {
    let challenge = challenge(host, id);
    observer.learned_decoder_probe(host, &challenge, 700 + id,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, challenge.evidence().passport().manifest().clone()).unwrap()
}
fn drive(host: &mut FileOversight, observer: &FileIdentityObserver, run: &mut FileDecoderIdentityProbe, now: u64)
    -> FileDecoderIdentityEvent
{
    let bound = run.work().planned_tokens + 2;
    let mut last = None;
    for _ in 0..bound {
        if run.is_closed() { break; }
        last = Some(run.step_with_clock(host, observer, || ElapsedTick(now)).unwrap().event);
    }
    assert!(run.is_closed(), "original fixed-stimulus probe did not finish");
    last.expect("at least one phase")
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

#[test]
fn actual_owned_identity_enables_fresh_sidecar_congress_but_not_either_publication_key() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); step(&mut host); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let mut run = start(&mut host, observer, 1);
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Pending { check: 1 }));
    assert!(matches!(drive(&mut host, observer, &mut run, 1), FileDecoderIdentityEvent::Installed(_)));
    assert_eq!(run.work().completed_tokens, 4);
    assert_eq!(host.identity_report(1).unwrap().observations.len(), 2);
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 1, .. }));
    let action = host.propose(host.revision(), 1, spec(&host), snapshot()).unwrap();
    let packet = host.begin_learned_sidecar(host.revision(), 1, numerical.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let input = packet.packet.input();
    assert!(host.authorize(host.revision(), 1, input, snapshot()).is_err());
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer",
        commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, input, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(input), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, input, snapshot()).unwrap();
    let delivered = host.publish_checked(host.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(delivered.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(disk.payload, b"visible"); assert_eq!(disk.executions, 1);
}

#[test]
fn same_label_substitute_is_measured_from_actual_owner_and_triggers_original_containment() {
    let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(2.0));
    let observer = roles.identity_observer.as_ref().unwrap(); step(&mut host);
    let mut run = start(&mut host, observer, 1);
    let event = drive(&mut host, observer, &mut run, 1);
    let FileDecoderIdentityEvent::Measured { anchor: 10, observation } = event else { panic!("actual anchor must mismatch"); };
    assert_eq!(observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(IdentityMismatch::Anchor { anchor: 10 }));
    assert!(observation.containment.unwrap().is_ok());
    let report = host.identity_report(1).unwrap();
    assert_eq!(report.observations[&10].first_outlier().unwrap().observed_bits, 2.0_f32.to_bits());
    assert_eq!(run.work().completed_tokens, 2); assert_eq!(report.observations.len(), 1);
    assert!(host.inspect().control.suspended);
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn independently_observed_manifest_mismatch_is_not_replaced_with_expected_metadata() {
    let root = Directory::new(); let (mut host, roles, _, passport) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let challenge = challenge(&mut host, 1);
    let mut actual: ModelManifest = passport.manifest().clone(); actual.weights[0] ^= 1;
    let mut run = observer.learned_decoder_probe(&host, &challenge, 701,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, actual.clone()).unwrap();
    let event = drive(&mut host, observer, &mut run, 1);
    assert!(matches!(event, FileDecoderIdentityEvent::Manifest(_)));
    assert_eq!(host.identity_report(1).unwrap().manifest, Some(actual));
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
    assert_eq!(run.work().entered_tokens, 0); assert!(host.inspect().control.suspended);
}

#[test]
fn exact_constructor_budget_and_stale_source_are_checked_without_journal_or_numerical_work() {
    let root = Directory::new(); let model = fixture::model(1.0);
    let (mut host, roles, _, passport) = setup(&root, model.clone());
    let observer = roles.identity_observer.as_ref().unwrap(); step(&mut host);
    let challenge = challenge(&mut host, 1);
    let required = passport.anchors().values().map(|anchor|
        model.estimate(0, anchor.stimulus().len()).unwrap().scalar_products().unwrap()).sum::<u64>();
    let bytes = root.bytes(); let before = host.learned_generation_inspection().unwrap();
    assert_eq!(observer.learned_decoder_probe(&host, &challenge, 701,
        DecoderBudget { scalar_products: required - 1 }, passport.manifest().clone()).err(), Some(Error::Limit.into()));
    assert_eq!(observer.learned_decoder_probe(&host, &challenge, 0,
        DecoderBudget { scalar_products: required }, passport.manifest().clone()).err(), Some(Error::InvalidInput.into()));
    assert_eq!(root.bytes(), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
    let mut run = observer.learned_decoder_probe(&host, &challenge, 701,
        DecoderBudget { scalar_products: required }, passport.manifest().clone()).unwrap();
    assert_eq!(run.work().entered_tokens, 0); assert_eq!(root.bytes(), bytes);
    assert!(matches!(drive(&mut host, observer, &mut run, 1), FileDecoderIdentityEvent::Installed(_)));
    assert_eq!(run.work().completed_scalar_products, required);
}

#[test]
fn actor_advance_or_basis_loss_retires_partial_measurement_before_further_work() {
    for change_actor in [false, true] {
        let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap(); step(&mut host);
        let mut run = start(&mut host, observer, 1);
        run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
        run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
        let before = run.work(); assert_eq!(before.completed_tokens, 1);
        if change_actor { step(&mut host); }
        else { host.identity_unavailable(host.revision(), host.identity_basis().unwrap()).unwrap(); }
        let mut calls = 0;
        assert_eq!(run.step_with_clock(&mut host, observer, || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Stale.into()));
        assert_eq!(calls, 0); assert!(run.is_closed()); assert_eq!(run.work(), before);
        assert!(host.identity_installation(1).unwrap().is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn foreign_observer_and_journal_cannot_take_over_a_current_owned_probe() {
    let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(1.0));
    let foreign_root = Directory::new(); let (mut foreign, foreign_roles, _, _) = setup(&foreign_root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap();
    let other = foreign_roles.identity_observer.as_ref().unwrap();
    let c = challenge(&mut host, 1); let manifest = c.evidence().passport().manifest().clone();
    assert_eq!(other.learned_decoder_probe(&host, &c, 701,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, manifest.clone()).err(), Some(Error::Binding.into()));
    let mut run = observer.learned_decoder_probe(&host, &c, 701,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, manifest).unwrap();
    let before = run.work(); let mut calls = 0;
    assert_eq!(run.step_with_clock(&mut foreign, observer, || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Binding.into()));
    assert_eq!(run.step_with_clock(&mut host, other, || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Binding.into()));
    assert_eq!(calls, 0); assert_eq!(run.work(), before); assert!(!run.is_closed());
    assert!(matches!(drive(&mut host, observer, &mut run, 1), FileDecoderIdentityEvent::Installed(_)));
}

#[test]
fn receipt_time_expiry_discards_partial_work_while_nearby_timely_control_installs() {
    for expired in [false, true] {
        let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap(); let mut run = start(&mut host, observer, 1);
        run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
        let deadline = run.challenge().evidence().deadline().0;
        let completion = deadline - u64::from(!expired); let mut calls = 0;
        let event = run.step_with_clock(&mut host, observer, || {
            calls += 1; ElapsedTick(if calls == 1 { 1 } else { completion })
        }).unwrap();
        assert_eq!(calls, 2); assert_eq!(event.work.completed_tokens, 1);
        if expired {
            assert!(matches!(event.event, FileDecoderIdentityEvent::Withdrawn { reason: Error::Stale, withdrawal: Ok(_) }));
            assert!(run.is_closed()); assert!(host.identity_installation(1).unwrap().is_none());
            assert!(host.identity_report(1).unwrap().observations.is_empty());
        } else {
            assert!(matches!(event.event, FileDecoderIdentityEvent::Advanced));
            assert!(matches!(drive(&mut host, observer, &mut run, completion), FileDecoderIdentityEvent::Installed(_)));
            assert_eq!(run.work().completed_tokens, 4);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn reopen_requires_fresh_owned_measurements_without_resuming_the_learned_generator() {
    let root = Directory::new(); let (mut host, roles, config, passport) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let mut old = start(&mut host, observer, 1);
    old.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
    old.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
    let expected = requirements(&host, &passport); drop(host);
    let (mut host, fresh_roles) = FileOversight::open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
    let fresh = fresh_roles.identity_observer.as_ref().unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(old.step_with_clock(&mut host, fresh, || ElapsedTick(2)).err(), Some(Error::Binding.into()));
    assert_eq!(old.work().completed_tokens, 1);
    let mut run = start(&mut host, fresh, 2);
    assert!(matches!(drive(&mut host, fresh, &mut run, 2), FileDecoderIdentityEvent::Installed(_)));
    let recovered = host.learned_generation_inspection().unwrap();
    assert!(recovered.paused); assert_eq!(recovered.numerical, numerical);
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    assert!(!host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn journal_failure_after_actual_anchor_inference_returns_no_installation_or_retry() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let mut run = start(&mut host, observer, 1);
    run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
    run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
    let before = root.bytes();
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged stage").unwrap();
    let error = run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap_err();
    assert!(matches!(error, JournalError::Io(_)));
    assert!(run.is_closed()); assert_eq!(run.work().completed_tokens, 2);
    assert_eq!(root.bytes(), before);
    assert_eq!(host.identity_installation(1).err(), Some(JournalError::Unavailable));
    assert_eq!(run.step_with_clock(&mut host, observer, || ElapsedTick(1)).err(), Some(Error::WrongState.into()));
    assert_eq!(run.work().completed_tokens, 2);
    let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
}

#[test]
fn caught_clock_unwind_retires_the_owned_probe_without_replaying_a_partial_measurement() {
    let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let mut run = start(&mut host, observer, 1);
    run.step_with_clock(&mut host, observer, || ElapsedTick(1)).unwrap();
    let before = root.bytes(); let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = run.step_with_clock(&mut host, observer, || {
            calls += 1; if calls == 2 { panic!("clock interrupted after numerical work"); }
            ElapsedTick(1)
        });
    }));
    assert!(result.is_err()); assert!(run.is_closed()); assert_eq!(run.work().completed_tokens, 1);
    assert_eq!(root.bytes(), before); assert!(host.identity_report(1).unwrap().observations.is_empty());
    assert_eq!(run.step_with_clock(&mut host, observer, || ElapsedTick(1)).err(), Some(Error::WrongState.into()));
    assert!(host.identity_installation(1).unwrap().is_none());
}

#[path = "durable_learned_identity/computed.rs"]
mod computed;
