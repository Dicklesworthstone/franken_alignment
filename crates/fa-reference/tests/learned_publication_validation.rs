//! Original learned numerical owner, final witnesses, review, two keys and sink.
//! Tiny weights and scripted ballots establish causal enforcement, not quality
//! of a detector, helper, external source or deployment perimeter.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "support/learned_text_model.rs"] mod fixture;
#[path = "support/file_mediation.rs"] mod topology;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    identity::{IdentityAnchor, ModelManifest, ModelPassport},
    monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::{decoder::DecoderModel, experiment::KvSide},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
    stream::StreamProfile, persistent::{FileDeliveryProfile, JournalError, JournalLimits, RecoveryReserve,
        observed::{FileHumanReviewer, FileOversight, FileOversightProfile,
            consistency::{FileConsistencyConfig, FileConsistencyParameters, learned::FileLearnedConsistencyConfig},
            credibility::held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
            decoder::learned::{FileLearnedConfig, FileLearnedPublicationProfile},
            guarded::{FileGuardSet, FileIdentityRequirement, mediated::predictive::FileMediatedPredictor},
            publication::{PublicationBasis, witnesses::{FilePublicationEvidence, FilePublicationInputs, FileWitnessInput}},
            stream::generated::checked::GeneratedPublicationFeed,
        },
    },
    publication_gate::{PublicationLimits, MAX_PUBLICATION_BINDINGS, changes::{PublicationChangePolicy,
        freshness::PublicationFreshnessPolicy}},
};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    identity::IdentityPolicy, learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::full_input::{ActualHelperInput, InputProfileBinding};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole,
    SnapshotEntry, WitnessRequest, refinement::{RefinementBudget, index::routing::RoutingBudget}};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-checked-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = std::fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("checked learned cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
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
fn stream() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }
fn model() -> DecoderModel { fixture::model(&[u32::from(b'O'), u32::from(b'K'), fixture::END]) }
fn recipe() -> FileLearnedConfig {
    let model = model();
    FileLearnedConfig::new_text_stream(model.clone(), fixture::tokenizer(&model), fixture::config(&model),
        LearnedDecoderBindingLimits::default(), stream()).unwrap().with_required_sidecar().unwrap()
}
fn selected(feed: bool) -> FileLearnedPublicationProfile {
    FileLearnedPublicationProfile {
        limits: PublicationLimits { bindings: 8,
            validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 } },
        feed: feed.then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy { source: 41, after: 0,
                lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 5 },
            snapshot_fallback: true,
        }),
    }
}
fn checked(selection: FileLearnedPublicationProfile) -> FileLearnedConfig {
    recipe().with_required_publication_validation(selection).unwrap()
}
fn changed_selections(original: FileLearnedPublicationProfile) -> Vec<FileLearnedPublicationProfile> {
    (0..11).map(|field| {
        let mut p = original;
        match field {
            0 => p.limits.bindings -= 1,
            1 => p.limits.validation.steps += 1,
            2 => p.limits.validation.value_bytes += 1,
            3 => p.feed = None,
            4 => p.feed.as_mut().unwrap().changes.source += 1,
            5 => p.feed.as_mut().unwrap().changes.after += 1,
            6 => p.feed.as_mut().unwrap().changes.lookup.steps += 1,
            7 => p.feed.as_mut().unwrap().changes.lookup.bytes += 1,
            8 => p.feed.as_mut().unwrap().freshness.clock_domain += 1,
            9 => p.feed.as_mut().unwrap().freshness.max_age_ticks += 1,
            _ => p.feed.as_mut().unwrap().snapshot_fallback = false,
        }
        p
    }).collect()
}
fn joint() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 1, 0, 0, HeldOutJointBudget::default()).unwrap()
}
fn identity() -> FileIdentityRequirement {
    FileIdentityRequirement { passport: ModelPassport::new(61, 1, ModelManifest {
        tenant: 1, model: 2, model_generation: 3, host_generation: 1, tokenizer_generation: 4,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32],
    }, vec![IdentityAnchor::new(10, model().residual_contract(1).unwrap().profile(), 501,
        vec![fixture::PROMPT], &[[-4.0, 4.0]; 4]).unwrap()]).unwrap(),
        policy: IdentityPolicy { observer_id: 99, timeout_ticks: 20, validity_ticks: 80, max_checks: 8 } }
}
fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
        source: config.required_policy_source(), identity: config.required_computed_identity_registration().cloned(),
        campaigns: None, credential: None }
}
fn lane_recipe(lane: u8) -> FileLearnedConfig {
    let mut config = recipe();
    if lane != 0 {
        let model = model(); let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
        let capture = if lane == 1 { model.residual_contract(1).unwrap().profile() }
            else { model.cache_profile().layers()[&1].values().profile() };
        let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0],
            bias: 0.0, threshold: 0.0,
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
    config.with_required_computed_identity_registration(identity()).unwrap()
        .with_required_held_out_joint(joint()).unwrap()
        .with_required_publication_validation(selected(true)).unwrap()
}

#[test]
fn selection_pins_every_original_field_and_composes_with_existing_recipe_wrappers() {
    let old = recipe(); assert_eq!(old, recipe());
    assert_eq!(old.required_publication_validation(), None);
    let selected = selected(true); let pinned = checked(selected);
    assert_ne!(pinned, old); assert!(pinned.encoded_bytes() > old.encoded_bytes());
    assert_eq!(pinned.required_publication_validation(), Some(selected));
    assert_eq!(pinned.clone().with_required_publication_validation(selected).err(), Some(Error::Duplicate));
    for variant in changed_selections(selected) { assert_ne!(pinned, checked(variant)); }
    let first = pinned.with_required_computed_identity_registration(identity()).unwrap()
        .with_required_held_out_joint(joint()).unwrap();
    let last = old.with_required_held_out_joint(joint()).unwrap()
        .with_required_computed_identity_registration(identity()).unwrap()
        .with_required_publication_validation(selected).unwrap();
    assert_ne!(first, last, "wrapper order is independently retained recipe identity");
    for config in [first, last] {
        assert_eq!(config.required_publication_validation(), Some(selected));
        assert!(config.requires_sidecar() && config.requires_computed_identity());
        assert_eq!(config.required_computed_identity_registration(), Some(&identity()));
        assert_eq!(config.required_held_out_joint(), Some(joint()));
        assert_eq!(config.text_stream_profile(), Some(stream()));
    }
}

#[test]
fn original_guards_install_atomically_in_plain_raw_owned_and_topology_lanes() {
    for mediated in [false, true] { for lane in 0..3 {
        let root = Directory::new(); let config = lane_recipe(lane); let guards = guards(&config);
        let (host, roles) = if mediated {
            let graph = topology::graph(profile().delivery.scope, profile().delivery.target, 1, false);
            let (host, roles) = if lane == 0 {
                FileOversight::create_mediated_guarded_with_learned_generation_reserved(root.store(),
                    profile(), &guards, None, graph, None, config, RecoveryReserve::terminal()).unwrap()
            } else {
                let prediction = if lane == 1 {
                    FileMediatedPredictor::Raw(config.required_pre_output_forecast().unwrap().clone())
                } else { FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone()) };
                FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(root.store(),
                    profile(), &guards, None, graph, prediction, None, config, RecoveryReserve::terminal()).unwrap()
            };
            assert_eq!(roles.consistency_observer.is_some(), lane != 0);
            assert!(roles.evaluator.is_none());
            (host, roles.oversight)
        } else if lane == 0 {
            FileOversight::create_guarded_with_learned_generation_reserved(root.store(), profile(),
                &guards, None, config, RecoveryReserve::terminal()).unwrap()
        } else {
            let (host, roles) = if lane == 1 {
                FileOversight::create_predictive_guarded_with_learned_generation_reserved(root.store(), profile(),
                    &guards, None, config.required_pre_output_forecast().unwrap().clone(), None,
                    config, RecoveryReserve::terminal()).unwrap()
            } else {
                FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(root.store(), profile(),
                    &guards, None, config.required_owned_pre_output_forecast().unwrap().clone(), None,
                    config, RecoveryReserve::terminal()).unwrap()
            };
            assert!(roles.evaluator.is_none()); (host, roles.oversight)
        };
        assert!(roles.identity_observer.is_some());
        assert!(host.identity_checks_required() && host.publication_guard_required());
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(joint()));
        assert_eq!(host.publication_validation_profile().unwrap(), Some(selected(true).limits));
        let feed = selected(true).feed.unwrap();
        let change = host.publication_change_status().unwrap();
        assert_eq!(change.source, feed.changes.source); assert_eq!(change.through, feed.changes.after);
        let fresh = host.publication_change_freshness().unwrap();
        assert_eq!(fresh.policy, feed.freshness); assert_eq!(fresh.heartbeat, None);
        assert!(fresh.eligibility.is_err(), "registration is not a fresh producer observation");
        assert!(host.publication_snapshot_fallback_enabled().unwrap());
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0); assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.work.admitted_tokens, 0); assert_eq!(host.inspect().executions, 0);
    } }
}

#[test]
fn invalid_limits_source_and_producer_clock_refuse_before_store_creation() {
    for field in 0..6 {
        let root = Directory::new(); let mut selection = selected(true);
        match field {
            0 => selection.limits.bindings = 0,
            1 => selection.limits.bindings = MAX_PUBLICATION_BINDINGS + 1,
            2 => selection.feed.as_mut().unwrap().changes.source = 0,
            3 => selection.feed.as_mut().unwrap().freshness.clock_domain = 0,
            4 => selection.feed.as_mut().unwrap().freshness.max_age_ticks = 0,
            _ => selection.feed.as_mut().unwrap().freshness.clock_domain += 1,
        }
        let result = FileOversight::create_with_learned_text_stream(root.store(), profile(), checked(selection));
        assert!(result.is_err(), "invalid field {field} escaped bootstrap");
        if field == 5 { assert_eq!(result.err(), Some(JournalError::Contract(Error::Binding))); }
        assert!(!root.store().exists(), "invalid field {field} left a visible partial owner");
    }
}

#[test]
fn independent_selection_mismatches_precede_replay_staging_cleanup_and_fence() {
    let root = Directory::new(); let config = checked(selected(true));
    let (mut host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control; drop(host);
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged candidate").unwrap();
    let before = root.files();
    let mut variants = vec![recipe()]; variants.extend(changed_selections(selected(true)).into_iter().map(checked));
    for wrong in variants {
        assert_eq!(FileOversight::read_stream_publication_with_learned_generation(
            root.store(), &profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(FileOversight::open_with_learned_text_stream(root.store(), profile(), &wrong).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(root.files(), before, "mismatch may not clean staging or install a fence");
    }
    let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.inspect().control.ledger.epoch > control.ledger.epoch);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(selected(true).limits));
    assert!(host.publication_change_freshness().unwrap().eligibility.is_err());
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn step(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn generate(host: &mut FileOversight) {
    for _ in 0..4 {
        if !host.learned_generation_inspection().unwrap().numerical.status.is_active() { break; }
        step(host);
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
}
fn observations(input: &CommitteeInput, revision: u64, keys: &[u64], closed: bool) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 };
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    if closed { frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(marker).unwrap(); }
    let structured = FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(marker)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationInputs::new(Some(structured), Some(input.views()["reviewer"].actual_input().clone()))
}
fn requests() -> Vec<WitnessRequest> {
    vec![WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject }, WitnessRequest::AbsentKey { key: 1 },
        WitnessRequest::EmptyRange { start: 6, end: 9 }, WitnessRequest::RangeMembers { start: 2, end: 6 }]
}
fn proposal(host: &mut FileOversight, attempt: u64) -> (FrozenAction, CommitteeInput, FilePublicationEvidence) {
    let action = host.propose_learned_text_stream_message(host.revision(), attempt, ElapsedTick(100), Vec::new(), snapshot()).unwrap();
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let packet = host.begin_learned_sidecar(host.revision(), attempt, actor,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + attempt, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let input = packet.packet.input().clone();
    let original = FilePublicationEvidence::new(observations(&input, 1, &[0, 2, 4], true), requests()).unwrap();
    host.bind_publication_evidence(host.revision(), attempt, original.clone()).unwrap();
    (action, input, original)
}
fn review(host: &mut FileOversight, attempt: u64, input: &CommitteeInput) {
    let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    host.commit_review(host.revision(), round, "reviewer",
        commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(input), snapshot()).unwrap().unwrap();
}
fn current(host: &mut FileOversight, attempt: u64, input: Option<FilePublicationInputs>) {
    let previous = host.publication_input_revision(attempt).unwrap();
    host.record_publication_inputs(host.revision(), attempt, previous, input).unwrap();
}
fn dispatch(host: &mut FileOversight, human: &FileHumanReviewer, action: &FrozenAction, input: &CommitteeInput) {
    let automatic = host.authorize(host.revision(), 1, input, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(input), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let approval = human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &approval, action, input, snapshot()).unwrap();
}

#[test]
fn real_generated_text_passes_both_keys_but_late_phantom_or_changed_whole_input_seals_once() {
    // Identical learned output, sidecar review and approvals in every branch.
    // Only current final-cut evidence differs after original dispatch admission.
    for change in 0..5 {
        let root = Directory::new(); let config = checked(selected(false));
        let (mut host, human) = FileOversight::create_with_learned_text_stream(root.store(), profile(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); generate(&mut host);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let (action, input, original) = proposal(&mut host, 1); review(&mut host, 1, &input);
        assert_eq!(host.authorize(host.revision(), 1, &input, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        current(&mut host, 1, Some(original.original().clone())); dispatch(&mut host, &human, &action, &input);
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
        let keys: &[u64] = if change == 1 { &[0, 2, 4, 7] } else { &[0, 2, 4, 99] };
        let mut latest = observations(&input, 2, keys, change != 3);
        if change == 2 {
            let old = latest.opaque().unwrap(); let mut changed = old.input_profile().clone(); changed.tokenizer_epoch += 1;
            let opaque = ActualHelperInput::new(old.submitted_bytes().to_vec(), changed,
                old.ordered_parts().to_vec(), old.omissions().to_vec()).unwrap();
            latest = FilePublicationInputs::new(latest.structured().cloned(), Some(opaque));
        }
        current(&mut host, 1, if change == 4 { None } else { Some(latest) });
        let receipt = host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
        let positive = change == 0;
        assert_eq!(receipt.basis, match change { 0 => PublicationBasis::Revalidated,
            1 | 2 => PublicationBasis::Rejected(Error::Stale), _ => PublicationBasis::Rejected(Error::Incomplete) });
        assert_eq!(receipt.outcome, if positive { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } });
        assert_eq!(host.retained_publication_evidence(1).unwrap(), &original);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units, "sealing alone is not settlement");
        let retry = host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved); assert_eq!(retry.outcome, receipt.outcome);
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, if positive { action.spec().units } else { 0 });
        let cut = host.stream_snapshot().unwrap();
        assert_eq!(cut.confirmed.visible(), if positive { &b"OK"[..] } else { &b""[..] });
        assert_eq!(cut.publication.executions, u64::from(positive));
        let before = root.files();
        assert_eq!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &config).unwrap(), cut);
        assert_eq!(root.files(), before); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_text_stream(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, cut.confirmed);
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let old = host.publish_checked(host.revision(), 1, None, snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(old.basis, PublicationBasis::PreviouslyResolved); assert_eq!(old.outcome, receipt.outcome);
        assert_eq!(host.inspect().executions, u64::from(positive));
    }
}

#[test]
fn original_validation_work_ceiling_is_enforced_without_spending_effect_rights() {
    let root = Directory::new(); let mut selection = selected(false); selection.limits.validation.value_bytes = 0;
    let (mut host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile(), checked(selection)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (_, input, original) = proposal(&mut host, 1); review(&mut host, 1, &input);
    current(&mut host, 1, Some(original.original().clone()));
    let revision = host.revision();
    assert_eq!(host.authorize(host.revision(), 1, &input, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    // A failed durable transition does not acknowledge its speculative report.
    assert_eq!(host.revision(), revision); assert_eq!(host.publication_validation(1).unwrap(), None);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(selection.limits));
    assert_eq!(host.inspect().control.ledger.reserved, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
}
