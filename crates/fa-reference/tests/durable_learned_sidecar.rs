//! Real original numerics and Unix journals; helper ballots are test controls.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits,
    observed::{FileOversightProfile, FileOversight as Host,
        FileHumanPermit, FileHumanReviewer,
        decoder::learned::{FileLearnedConfig, sidecar::FileSidecarSnapshot},
        guarded::FileGuardSet},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy, learned_source::LearnedSourceConfig,
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart,
    monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("fa-durable-sidecar-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&p).unwrap(); Self(p)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = std::fs::remove_dir_all(&self.0) { eprintln!("sidecar cleanup: {e}"); } }
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
            InputProfileBinding { profile_id: 1, profile_bytes: b"sidecar-test-v1".to_vec(),
                tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn config(required: bool) -> FileLearnedConfig {
    let model = fixture::model();
    let source = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, 0, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    let c = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    if required { c.with_required_sidecar().unwrap() } else { c }
}
fn owner(root: &Directory, c: &FileLearnedConfig) -> (Host, FileHumanReviewer) {
    let (mut host, roles) = Host::create_guarded_with_learned_generation(root.store(), profile(), &guards(), None, c.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles.human)
}
fn step(h: &mut Host) {
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.advance_learned_generation(h.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn propose(h: &mut Host, id: u64) -> FrozenAction {
    h.propose(h.revision(), id, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(h.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap()
}
fn request(id: u64) -> LearnedSidecarRequest {
    LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + id, generation: 1, transform_id: 7 },
        priority: Vec::new(), budget: SidecarCongressBudget::default() }
}
fn prepare(h: &mut Host, id: u64) -> FileSidecarSnapshot {
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.begin_learned_sidecar(h.revision(), id, n.actor_revision, request(id)).unwrap()
}
fn window() -> ReviewWindow { ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } }
fn ballot(h: &mut Host, id: u64, round: u64, verdict: Verdict) {
    h.begin_review(h.revision(), id, round, [9; 32], window(), snapshot()).unwrap();
    h.commit_review(h.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32], verdict, b"salt").unwrap()).unwrap();
    h.open_reveals(h.revision(), round).unwrap();
    h.reveal_review(h.revision(), round, "reviewer", verdict, b"salt".to_vec()).unwrap();
}
fn keys(h: &mut Host, reviewer: &FileHumanReviewer, id: u64, round: u64, input: &CommitteeInput)
    -> (FilePermit, FileHumanPermit)
{
    ballot(h, id, round, Verdict::Allow);
    h.finish_review(h.revision(), round, Some(input), snapshot()).unwrap().unwrap();
    authorize(h, reviewer, id, input)
}
fn authorize(h: &mut Host, reviewer: &FileHumanReviewer, id: u64, input: &CommitteeInput)
    -> (FilePermit, FileHumanPermit)
{
    let automatic = h.authorize(h.revision(), id, input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1000 + id, id, input, ElapsedTick(40)).unwrap();
    let revision = h.revision(); let human = reviewer.approve(h, revision, &request).unwrap();
    (automatic, human)
}
fn text_input(action: &FrozenAction) -> CommitteeInput {
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(action); let split = bytes.len(); bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: split } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: split, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    CommitteeInput::capture(action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap()
}

#[test]
fn required_recipe_is_atomic_and_the_original_packet_enters_two_key_publication() {
    let root = Directory::new(); let c = config(true);
    assert!(c.requires_sidecar()); assert_ne!(c, config(false));
    assert_eq!(c.clone().with_required_sidecar().err(), Some(Error::Duplicate));
    let (mut h, reviewer) = owner(&root, &c);
    assert!(h.learned_sidecar_required() && h.publication_guard_required());
    step(&mut h); let action = propose(&mut h, 1);
    let before = h.learned_generation_inspection().unwrap().numerical;
    let revision = h.revision(); let packet = prepare(&mut h, 1);
    assert_eq!(h.revision(), revision + 1);
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(&packet.packet.payload()[..8], b"FASIDE\0\x01");
    assert_eq!(packet.input_revision, h.input_revision(1).unwrap());
    assert_eq!(packet.packet.work().rounds, 1);
    let input = packet.packet.input();
    assert_eq!(input.views()["reviewer"].actual_input().part_bytes(1).unwrap(), packet.packet.payload());
    assert!(h.authorize(h.revision(), 1, input, snapshot()).is_err());
    let (automatic, human) = keys(&mut h, &reviewer, 1, 101, input);
    h.dispatch(h.revision(), &automatic, &human, &action, input, snapshot()).unwrap();
    assert!(h.publish(h.revision(), 1).is_err());
    assert_eq!(h.publish_checked(h.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    h.reconcile(h.revision(), 1).unwrap();
    assert_eq!(h.inspect().payload, b"visible"); assert_eq!(h.inspect().executions, 1);
    assert_eq!(h.inspect().control.ledger.charged, 16);
}

#[test]
fn text_only_and_byte_identical_manual_packets_cannot_mint_the_required_marker() {
    let root = Directory::new(); let (mut h, reviewer) = owner(&root, &config(true)); step(&mut h);
    let first = propose(&mut h, 1); let original = prepare(&mut h, 1);
    for (id, input) in [(2, text_input(&first)), (3, original.packet.input().clone())] {
        assert_eq!(propose(&mut h, id), first);
        h.record_inputs(h.revision(), id, 0, input.clone()).unwrap();
        assert_eq!(h.begin_review(h.revision(), id, 100 + id, [9; 32], window(), snapshot()).err(),
            Some(JournalError::Contract(Error::Incomplete)));
        let n = h.learned_generation_inspection().unwrap().numerical;
        assert!(h.begin_learned_sidecar(h.revision(), id, n.actor_revision, request(id)).is_err());
        assert_eq!(h.retained_learned_sidecar(id).err(), Some(JournalError::Contract(Error::Missing)));
    }
    // Identical source/action and genuine registration retain the positive path.
    let (automatic, human) = keys(&mut h, &reviewer, 1, 101, original.packet.input());
    h.dispatch(h.revision(), &automatic, &human, &first, original.packet.input(), snapshot()).unwrap();
}

#[test]
fn source_advancement_invalidates_old_review_but_allows_a_fresh_original_sidecar() {
    let root = Directory::new(); let (mut h, reviewer) = owner(&root, &config(true)); step(&mut h);
    propose(&mut h, 1); let old = prepare(&mut h, 1);
    ballot(&mut h, 1, 101, Verdict::Allow); step(&mut h);
    assert!(h.finish_review(h.revision(), 101, Some(old.packet.input()), snapshot()).unwrap().is_err());
    assert_eq!(h.retained_learned_sidecar(1).unwrap().packet, old.packet);
    assert!(h.authorize(h.revision(), 1, old.packet.input(), snapshot()).is_err());
    propose(&mut h, 2); let fresh = prepare(&mut h, 2);
    assert!(fresh.actor_revision > old.actor_revision);
    keys(&mut h, &reviewer, 2, 102, fresh.packet.input());
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn recovery_pins_requirement_and_keeps_packets_historical_without_reviving_keys() {
    let root = Directory::new(); let c = config(true); let (mut h, reviewer) = owner(&root, &c); step(&mut h);
    let action = propose(&mut h, 1); let saved = prepare(&mut h, 1);
    let (automatic, human) = keys(&mut h, &reviewer, 1, 101, saved.packet.input());
    let n = h.learned_generation_inspection().unwrap().numerical;
    let original = root.bytes(); drop(h);
    assert_eq!(Host::open_with_learned_generation(root.store(), profile(), &config(false)).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(root.bytes(), original);
    let (mut h, reviewer) = Host::open_with_learned_generation(root.store(), profile(), &c).unwrap();
    assert!(h.learned_sidecar_required()); assert!(!h.clock_ready());
    assert_eq!(h.retained_learned_sidecar(1).unwrap().packet, saved.packet);
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, n);
    assert_eq!(h.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert!(h.dispatch(h.revision(), &automatic, &human, &action, saved.packet.input(), snapshot()).is_err());
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    h.resume_learned_generation(h.revision(), n.actor_revision, n.position).unwrap();
    propose(&mut h, 2); let fresh = prepare(&mut h, 2);
    keys(&mut h, &reviewer, 2, 102, fresh.packet.input());
}

#[test]
fn withdrawn_inputs_do_not_reopen_the_original_planner_or_refund_its_packet() {
    let root = Directory::new(); let (mut h, _) = owner(&root, &config(true)); step(&mut h);
    propose(&mut h, 1); let saved = prepare(&mut h, 1);
    h.inputs_unavailable(h.revision(), 1, saved.input_revision).unwrap();
    let before = root.bytes(); let revision = h.revision();
    assert!(h.begin_learned_sidecar(revision, 1, saved.actor_revision, request(1)).is_err());
    h.record_inputs(revision, 1, h.input_revision(1).unwrap(), saved.packet.input().clone()).unwrap();
    assert!(h.begin_review(h.revision(), 1, 101, [9; 32], window(), snapshot()).is_err());
    assert_eq!(h.retained_learned_sidecar(1).unwrap().packet, saved.packet);
    assert_ne!(root.bytes(), before); // Only the explicit input replacement wrote.
    propose(&mut h, 2); prepare(&mut h, 2);
}

#[test]
fn invalid_or_underfunded_preparation_changes_neither_journal_nor_original_input() {
    let root = Directory::new(); let (mut h, _) = owner(&root, &config(true)); step(&mut h); propose(&mut h, 1);
    let n = h.learned_generation_inspection().unwrap().numerical; let bytes = root.bytes(); let revision = h.revision();
    let mut tiny = request(1); tiny.budget.committee_bytes = 1;
    assert_eq!(h.begin_learned_sidecar(revision, 1, n.actor_revision, tiny).err(), Some(JournalError::Contract(Error::Limit)));
    let mut invalid = request(1); invalid.identity.object_id = 0;
    assert_eq!(h.begin_learned_sidecar(revision, 1, n.actor_revision, invalid).err(), Some(JournalError::Contract(Error::InvalidInput)));
    assert!(h.begin_learned_sidecar(revision - 1, 1, n.actor_revision, request(1)).is_err());
    assert!(h.begin_learned_sidecar(revision, 1, n.actor_revision + 1, request(1)).is_err());
    assert_eq!(h.revision(), revision); assert_eq!(h.input_revision(1).unwrap(), 0); assert_eq!(root.bytes(), bytes);
    prepare(&mut h, 1); assert_eq!(h.input_revision(1).unwrap(), 1);
}

#[test]
fn pending_numerical_intent_blocks_sidecar_creation_until_original_completion() {
    let root = Directory::new(); let (mut h, _) = owner(&root, &config(true)); step(&mut h); propose(&mut h, 1);
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.begin_learned_step(h.revision(), n.actor_revision, n.position).unwrap();
    let bytes = root.bytes();
    assert_eq!(h.begin_learned_sidecar(h.revision(), 1, n.actor_revision, request(1)).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(root.bytes(), bytes);
    h.complete_learned_step(h.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert!(h.begin_learned_sidecar(h.revision(), 1, n.actor_revision, request(1)).is_err());
    propose(&mut h, 2); prepare(&mut h, 2);
}

#[test]
fn legacy_optional_recipe_keeps_its_existing_explicit_text_review_behavior() {
    let root = Directory::new(); let (mut h, reviewer) = owner(&root, &config(false)); step(&mut h);
    assert!(!h.learned_sidecar_required()); let action = propose(&mut h, 1);
    let n = h.learned_generation_inspection().unwrap().numerical;
    assert_eq!(h.begin_learned_sidecar(h.revision(), 1, n.actor_revision, request(1)).err(), Some(JournalError::Contract(Error::Binding)));
    let input = text_input(&action); h.record_inputs(h.revision(), 1, 0, input.clone()).unwrap();
    keys(&mut h, &reviewer, 1, 101, &input);
}

#[path = "durable_learned_sidecar/refinement.rs"]
mod refinement;
