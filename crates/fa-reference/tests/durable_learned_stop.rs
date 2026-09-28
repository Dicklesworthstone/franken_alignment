//! Original numerical trips and real journal cuts, not simulated stop decisions.
//! Fixture weights/ballots are controls, not detector effectiveness evidence.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "support/restart_model.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "support/learned_text_model.rs"]
mod text_fixture;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose,
    ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationEvent,
        GenerationSpec, GenerationStatus, GenerationTelemetryBudget}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalIo, JournalLimits, Reconciliation,
    observed::{FileOversight as Host, FileOversightProfile, FileHumanPermit, FileHumanReviewer,
        decoder::learned::FileLearnedConfig, guarded::FileGuardSet}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_host::HostedStopPolicy,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_host::{LearnedHostStopCause, sidecar::LearnedSidecarRequest},
    learned_source::{LearnedEvidenceLimits, LearnedSourceConfig},
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-durable-learned-stop-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("stop cleanup: {error}"); } }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn profile() -> FileOversightProfile {
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target()], target: target(), initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"stop-test-v1".to_vec(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn stop_policy() -> HostedStopPolicy { HostedStopPolicy::new(10, 2, 600).unwrap() }
fn recipe(mode: u8, exhaust: bool) -> FileLearnedConfig {
    let model = fixture::model();
    let mut source = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    if exhaust {
        let mut original = model.observed_learned_generation(source.clone()).unwrap();
        original.advance(0).unwrap();
        source.telemetry.source_check_values = original.telemetry_work().source_check_values;
    }
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
}
fn create(root: &Directory, config: &FileLearnedConfig) -> (Host, FileHumanReviewer) {
    let (host, roles) = Host::create_guarded_with_learned_generation(root.store(), profile(),
        &guards(), None, config.clone()).unwrap();
    (host, roles.human)
}
fn start(root: &Directory, config: &FileLearnedConfig) -> (Host, FileHumanReviewer) {
    let (mut host, human) = create(root, config);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human)
}
fn step(host: &mut Host) -> Result<Rc<GenerationEvent>, Error> {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap()
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn prepare(host: &mut Host, human: &FileHumanReviewer)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanPermit)
{
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let input = if host.learned_sidecar_required() {
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_sidecar(host.revision(), 1, n.actor_revision, LearnedSidecarRequest {
            identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default(),
        }).unwrap().packet.input().clone()
    } else {
        let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
        let mut bytes = action_frame(&action); let split = bytes.len(); bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: split } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: split, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
            policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
        let input = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
        host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap(); input
    };
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer",
        commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let key = human.approve(host, revision, &request).unwrap();
    (action, input, automatic, key)
}

#[test]
fn first_canonical_image_has_the_fixed_policy_without_clock_or_inference() {
    let plain = recipe(0, false);
    assert_eq!(plain.automatic_stop_policy(), None);
    let stopped = plain.clone().with_automatic_stop(stop_policy()).unwrap();
    assert_ne!(plain, stopped);
    assert_eq!(stopped.clone().with_automatic_stop(stop_policy()).err(), Some(Error::Duplicate));
    for sidecar_first in [false, true] {
        let config = if sidecar_first {
            plain.clone().with_required_sidecar().unwrap().with_automatic_stop(stop_policy()).unwrap()
        } else { stopped.clone().with_required_sidecar().unwrap() };
        let root = Directory::new(); let (host, _) = create(&root, &config);
        assert_eq!(host.revision(), 2); assert!(!host.clock_ready());
        assert!(host.learned_sidecar_required() && host.publication_guard_required());
        assert_eq!(host.learned_host_stop_policy(), Some(stop_policy()));
        assert_eq!(host.learned_host_stop_incident().unwrap(), None);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(Host::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().revision, 2);
    }
}

#[test]
fn quiet_original_generation_and_two_key_publication_are_unchanged() {
    let base = recipe(0, false).with_required_sidecar().unwrap();
    let config = base.clone().with_automatic_stop(stop_policy()).unwrap();
    let left = Directory::new(); let (mut control, _) = start(&left, &base);
    let right = Directory::new(); let (mut host, human) = start(&right, &config);
    while control.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let a = step(&mut control).unwrap(); let b = step(&mut host).unwrap();
        assert_eq!(a.status(), b.status()); assert_eq!(a.sample(), b.sample());
        assert_eq!(control.learned_generation_inspection().unwrap().numerical,
            host.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.learned_host_stop_incident().unwrap(), None);
    }
    let (action, input, automatic, key) = prepare(&mut host, &human);
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(!host.inspect().control.suspended); assert!(host.inspect().stop.is_none());
}

#[test]
fn real_alarm_stops_and_withdraws_existing_keys_in_the_same_acknowledged_step() {
    let config = recipe(2, false).with_automatic_stop(stop_policy()).unwrap();
    let root = Directory::new(); let (mut host, human) = start(&root, &config);
    step(&mut host).unwrap(); let (action, input, automatic, key) = prepare(&mut host, &human);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    let before = host.learned_generation_inspection().unwrap();
    let event = step(&mut host).unwrap();
    assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(event.accepted().is_none() && event.sample().is_none());
    let after = host.learned_generation_inspection().unwrap();
    assert_eq!(after.journal_revision, before.journal_revision + 2);
    assert!(after.pending.is_none() && after.paused);
    assert_eq!(after.numerical.position, before.numerical.position);
    assert_eq!(after.numerical.sampled_draws, before.numerical.sampled_draws);
    assert!(after.numerical.work.admitted_tokens > before.numerical.work.admitted_tokens);
    let incident = host.learned_host_stop_incident().unwrap().unwrap();
    assert_eq!(incident.policy(), stop_policy());
    assert_eq!(incident.cause(), LearnedHostStopCause::Monitoring(MonitorOutcome::Alarm));
    let receipt = host.inspect().stop.unwrap();
    assert_eq!(incident.stop_receipt(), Some(&receipt));
    assert_eq!(receipt.request().operation, 600); assert_eq!(receipt.refunded_units(), 16);
    assert!(host.inspect().control.suspended);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).is_err());
    host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.learned_host_stop_incident().unwrap(), Some(incident.clone()));
    drop(host);
    let (mut recovered, _) = Host::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovered.learned_host_stop_incident().unwrap(), Some(incident));
    assert_eq!(recovered.inspect().executions, 0);
    recovered.observe_time(recovered.revision(), ElapsedTick(3)).unwrap();
    let n = recovered.learned_generation_inspection().unwrap().numerical;
    assert!(recovered.resume_learned_generation(recovered.revision(), n.actor_revision, n.position).is_err());
}

#[test]
fn numerical_exhaustion_is_a_durable_operational_trip_not_a_fabricated_alarm() {
    for automatic in [false, true] {
        let base = recipe(0, true);
        let config = if automatic { base.with_automatic_stop(stop_policy()).unwrap() } else { base };
        let root = Directory::new(); let (mut host, _) = start(&root, &config);
        step(&mut host).unwrap();
        assert_eq!(step(&mut host).err(), Some(Error::Limit));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.status, GenerationStatus::Failed(Error::Limit));
        assert_eq!(host.inspect().control.suspended, automatic);
        if automatic {
            let incident = host.learned_host_stop_incident().unwrap().unwrap();
            assert_eq!(incident.cause(), LearnedHostStopCause::Failure(Error::Limit));
            assert!(incident.stop_receipt().is_some());
            drop(host);
            let (recovered, _) = Host::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            assert_eq!(recovered.learned_host_stop_incident().unwrap(), Some(incident));
        } else {
            assert_eq!(host.learned_host_stop_incident().unwrap(), None);
            assert!(host.inspect().stop.is_none());
        }
    }
}

#[test]
fn changed_or_missing_stop_recipe_refuses_before_cleanup_even_before_a_trip() {
    let base = recipe(0, false);
    let config = base.clone().with_automatic_stop(stop_policy()).unwrap();
    let root = Directory::new(); let (mut host, _) = start(&root, &config);
    step(&mut host).unwrap(); let bytes = root.bytes(); drop(host);
    let staged = root.store().join("delivery.pending"); std::fs::write(&staged, b"retained").unwrap();
    let alternatives = [base.clone(),
        base.clone().with_automatic_stop(HostedStopPolicy::new(11, 2, 600).unwrap()).unwrap(),
        base.clone().with_automatic_stop(HostedStopPolicy::new(10, 3, 600).unwrap()).unwrap(),
        base.with_automatic_stop(HostedStopPolicy::new(10, 2, 601).unwrap()).unwrap()];
    for wrong in alternatives {
        assert_eq!(Host::open_with_learned_generation(root.store(), profile(), &wrong).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert!(Host::read_publication_with_learned_generation(root.store(), &profile(), &wrong).is_err());
        assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&staged).unwrap(), b"retained");
    }
    let (mut recovered, _) = Host::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(!staged.exists()); assert_eq!(recovered.learned_host_stop_policy(), Some(stop_policy()));
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert_eq!(recovered.learned_host_stop_incident().unwrap(), None);
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    let n = recovered.learned_generation_inspection().unwrap().numerical;
    recovered.resume_learned_generation(recovered.revision(), n.actor_revision, n.position).unwrap();
    step(&mut recovered).unwrap(); assert_eq!(recovered.learned_host_stop_incident().unwrap(), None);
}

#[test]
fn failed_completion_hides_candidate_stop_and_recovery_must_finish_the_same_intent() {
    let config = recipe(2, false).with_automatic_stop(stop_policy()).unwrap();
    let root = Directory::new(); let (mut host, _) = start(&root, &config);
    step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let bytes = root.bytes();
    std::fs::write(root.store().join("delivery.pending"), b"occupied").unwrap();
    let error = host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap_err();
    let JournalError::Io(failure) = error else { panic!("expected original staging failure"); };
    assert_eq!(failure.operation, JournalIo::Stage);
    assert_eq!(host.learned_host_stop_incident(), Err(JournalError::Unavailable));
    assert_eq!(root.bytes(), bytes); assert!(!host.clock_ready());
    let historical = Host::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert!(historical.stop.is_none()); assert_eq!(historical.executions, 0);
    drop(host);
    let (mut host, _) = Host::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().pending.is_some());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(host.learned_generation_inspection().unwrap().pending.is_some());
    let event = host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    assert_eq!(host.learned_host_stop_incident().unwrap().unwrap().cause(),
        LearnedHostStopCause::Monitoring(MonitorOutcome::Alarm));
    assert!(host.inspect().control.suspended);
}

#[test]
fn stopping_and_restart_do_not_refund_dispatched_or_executed_effects() {
    for executed in [false, true] {
        let config = recipe(2, false).with_automatic_stop(stop_policy()).unwrap();
        let root = Directory::new(); let (mut host, human) = start(&root, &config);
        step(&mut host).unwrap(); let (action, input, automatic, key) = prepare(&mut host, &human);
        host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).unwrap();
        if executed { host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).unwrap(); }
        assert_eq!(step(&mut host).unwrap().status(), GenerationStatus::Held(MonitorOutcome::Alarm));
        assert_eq!(host.inspect().stop.as_ref().unwrap().refunded_units(), 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, u64::from(executed));
        let incident = host.learned_host_stop_incident().unwrap();
        drop(host);
        let (mut host, _) = Host::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_host_stop_incident().unwrap(), incident);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert!(host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).is_err());
        // Only the original endpoint can settle absence. Stop/recovery themselves
        // left the liability intact; sealing an existing execution cannot undo it.
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let resolved = host.seal_unexecuted(host.revision(), 1).unwrap();
        if executed {
            assert_eq!(resolved, Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.charged, 16);
        } else {
            assert_eq!(resolved, Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
        assert_eq!(host.inspect().executions, u64::from(executed));
    }
}

#[test]
fn stale_preflight_is_not_an_admitted_numerical_failure_or_stop_trigger() {
    let config = recipe(0, false).with_automatic_stop(stop_policy()).unwrap();
    let root = Directory::new(); let (mut host, _) = start(&root, &config);
    let n = host.learned_generation_inspection().unwrap(); let bytes = root.bytes();
    assert!(host.advance_learned_generation(host.revision(), n.numerical.actor_revision + 1, 0).is_err());
    assert!(host.advance_learned_generation(host.revision() - 1, n.numerical.actor_revision, 0).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap(), n); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.learned_host_stop_incident().unwrap(), None);
    step(&mut host).unwrap(); assert!(!host.inspect().control.suspended);
}

#[test]
fn native_text_keeps_its_completion_and_sidecar_rules_in_both_quiet_and_alarm_cases() {
    for alarm in [false, true] {
        let model = text_fixture::model(&[b'O' as u32, b'K' as u32, text_fixture::END]);
        let mut source = text_fixture::config(&model);
        source.policy = text_fixture::policy(&model, alarm);
        let config = FileLearnedConfig::new_text(model.clone(), text_fixture::tokenizer(&model), source,
            LearnedDecoderBindingLimits::default()).unwrap().with_automatic_stop(stop_policy()).unwrap()
            .with_required_sidecar().unwrap();
        let root = Directory::new();
        let (mut host, _) = Host::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
        assert!(host.learned_text_required() && host.learned_sidecar_required());
        assert_eq!(host.learned_host_stop_policy(), Some(stop_policy()));
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        while host.learned_generation_inspection().unwrap().numerical.status.is_active() { step(&mut host).unwrap(); }
        if alarm {
            assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
            assert_eq!(host.learned_host_stop_incident().unwrap().unwrap().cause(),
                LearnedHostStopCause::Monitoring(MonitorOutcome::Alarm));
            assert!(host.inspect().control.suspended);
        } else {
            assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
            assert_eq!(host.learned_host_stop_incident().unwrap(), None);
        }
        assert_eq!(host.inspect().payload, b"initial"); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn unresolved_monitor_stops_without_relabelling_uncertainty_as_an_alarm() {
    for retain in [false, true] {
        let model = fixture::model();
        // Start off the codec's trained axis. The original first-coordinate
        // probe is actually quiet; only a retained exact residual can settle it.
        let source = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
            spec: GenerationSpec::new(vec![2], 3, BTreeSet::new(), SamplingStart {
                policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
            }).unwrap(), policy: fixture::policy(&model, 1, u8::from(retain)),
            budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
        let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
            .with_automatic_stop(stop_policy()).unwrap();
        let root = Directory::new(); let (mut host, _) = start(&root, &config);
        let event = step(&mut host).unwrap();
        if retain {
            assert!(event.accepted().is_some());
            assert_eq!(event.audit().outcome(), MonitorOutcome::NoAlarm);
            assert!(event.audit().work().refinements > 0);
            assert_eq!(host.learned_host_stop_incident().unwrap(), None);
            assert!(!host.inspect().control.suspended);
        } else {
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Unresolved));
            assert!(event.accepted().is_none() && event.sample().is_none());
            assert_eq!(host.learned_host_stop_incident().unwrap().unwrap().cause(),
                LearnedHostStopCause::Monitoring(MonitorOutcome::Unresolved));
            assert!(host.inspect().control.suspended);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}
