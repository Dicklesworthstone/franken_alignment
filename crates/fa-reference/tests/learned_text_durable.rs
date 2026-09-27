//! Original learned numerics, original journal/storage, and reference ballots.
//! Tiny model weights and helper votes do not qualify a detector or deployment.
#![cfg(unix)]
#[path = "support/learned_text_model.rs"]
mod fixture;
use fixture::*;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits, Reconciliation,
    observed::{FileOversight, FileOversightProfile, FileHumanReviewer, FileHumanRequest,
        decoder::learned::{FileLearnedConfig, LearnedStepIntent}},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy, learned_host::text::LearnedTextTarget,
    learned_source::{LearnedSourceConfig, LearnedEvidenceLimits,
        text::{LearnedTextConfig, LearnedTextCompletion}},
};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, TokenBytes};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{GenerationSpec, GenerationStatus};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-learned-text-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("text test cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec(),
    ).unwrap())])).unwrap(), human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn recipe(model: &DecoderModel, source: LearnedTextConfig) -> FileLearnedConfig {
    FileLearnedConfig::new_text(model.clone(), tokenizer(model), source, LearnedDecoderBindingLimits::default()).unwrap()
}
fn numeric_recipe(model: &DecoderModel, source: LearnedTextConfig) -> FileLearnedConfig {
    let encoded = tokenizer(model).encode(source.prompt.as_bytes(), source.tokenization).unwrap();
    let spec = GenerationSpec::new(encoded.tokens().to_vec(), source.max_new_tokens,
        source.stop_tokens, source.sampling).unwrap();
    FileLearnedConfig::new(model.clone(), LearnedSourceConfig { stream: source.stream,
        evaluation_origin: source.evaluation_origin, monitor_generation: source.monitor_generation,
        spec, policy: source.policy, budget: source.budget, telemetry: source.telemetry },
        LearnedDecoderBindingLimits::default()).unwrap()
}
fn owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = if config.is_text() {
        FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap()
    } else {
        let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
        host.enable_learned_generation(host.revision(), config.clone()).unwrap();
        (host, reviewer)
    };
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn step(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}
fn finish(host: &mut FileOversight) {
    for _ in 0..4 {
        if !host.learned_generation_inspection().unwrap().numerical.status.is_active() { break; }
        step(host);
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
}
fn reopen(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    FileOversight::open_with_learned_generation(root.store(), profile(), config).unwrap()
}
fn resume(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}
fn destination(host: &FileOversight) -> LearnedTextTarget {
    LearnedTextTarget { target: host.inspect().target, required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
fn action_spec(host: &FileOversight, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: payload.to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
fn prepared(host: &mut FileOversight) -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest) {
    let target = destination(host);
    let action = host.propose_learned_text(host.revision(), 1, target, snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
    (action, inputs, automatic, request)
}

#[test]
fn every_recovery_cut_retains_original_text_rng_and_spent_budgets() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let source = config(&model); let recipe = recipe(&model, source.clone());
    let mut original = model.observed_learned_text_generation(tokenizer(&model), source).unwrap();
    original.run_to_stop().unwrap();
    let expected = original.text_message(LearnedEvidenceLimits::default()).unwrap();
    for cut in 0..=4 {
        let root = Directory::new(); let (mut host, _) = owner(&root, &recipe);
        assert!(host.learned_text_required());
        for _ in 0..cut { step(&mut host); }
        let before = host.learned_generation_inspection().unwrap().numerical;
        drop(host);
        assert!(FileOversight::open(root.store(), profile()).is_err());
        let (mut host, _) = reopen(&root, &recipe);
        assert!(host.learned_text_required());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).err(), Some(JournalError::Contract(Error::Incomplete)));
        resume(&mut host); finish(&mut host);
        let actual = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
        assert_eq!(actual.bytes(), expected.bytes());
        assert_eq!(actual.stop(), expected.stop());
        assert_eq!(actual.work(), expected.work());
        assert_eq!(actual.telemetry_work(), expected.telemetry_work());
        assert_eq!(actual.evidence().tokens(), expected.evidence().tokens());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 3);
        assert_eq!(host.inspect().executions, 0);
        // The generic durable API still reaches the original text constraint.
        let before = host.revision(); let changed = action_spec(&host, b"substituted");
        assert_eq!(host.propose(before, 1, changed, snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(host.revision(), before);
        let target = destination(&host);
        assert_eq!(host.propose_learned_text(before, 1, target, snapshot()).unwrap().spec().payload, b"OK");
    }
}

#[test]
fn exact_text_recipe_prevents_tokenizer_budget_and_completion_downgrades() {
    let model = model(&[b'O' as u32, b'K' as u32, END]); let good = recipe(&model, config(&model));
    let root = Directory::new(); let (host, _) = owner(&root, &good); drop(host);
    let rebuilt = ByteBpe::from_bytes(model.profile(), &tokenizer(&model).to_bytes().unwrap()).unwrap();
    assert_eq!(FileLearnedConfig::new_text(model.clone(), rebuilt, config(&model),
        LearnedDecoderBindingLimits::default()).unwrap(), good);
    let mut alternatives = vec![numeric_recipe(&model, config(&model))];
    for case in 0..4 {
        let mut changed = config(&model);
        match case {
            0 => changed.prompt = "P".into(),
            1 => changed.tokenization.input_bytes = 4,
            2 => changed.output.max_bytes -= 1,
            _ => changed.output.completion = LearnedTextCompletion::StopOrTokenLimit,
        }
        alternatives.push(recipe(&model, changed));
    }
    // Same profile and prompt IDs, different token spellings: numerical replay
    // alone cannot establish that resulting text still has the original bytes.
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.swap(b'O' as usize, b'K' as usize);
    vocabulary.extend([TokenBytes::Control, TokenBytes::Control, TokenBytes::Content(b"PP".to_vec())]);
    let changed = ByteBpe::new(model.profile().clone(), vocabulary,
        vec![Merge { left: PROMPT, right: PROMPT, result: MERGED_PROMPT }]).unwrap();
    let mut control = model.observed_learned_text_generation(changed.clone(), config(&model)).unwrap();
    control.run_to_stop().unwrap();
    assert_eq!(control.text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"KO");
    alternatives.push(FileLearnedConfig::new_text(model.clone(), changed, config(&model), LearnedDecoderBindingLimits::default()).unwrap());
    for wrong in alternatives {
        assert_ne!(wrong, good);
        assert_eq!(FileOversight::open_with_learned_generation(root.store(), profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
    }
    let (mut host, _) = reopen(&root, &good); resume(&mut host); finish(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}

#[test]
fn an_unacknowledged_final_stop_cannot_be_presented_as_completed_text() {
    let model = model(&[b'O' as u32, b'K' as u32, END]); let recipe = recipe(&model, config(&model));
    let root = Directory::new(); let (mut host, _) = owner(&root, &recipe);
    for _ in 0..3 { step(&mut host); }
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    drop(host); let (mut host, _) = reopen(&root, &recipe); resume(&mut host);
    assert_eq!(host.learned_generation_inspection().unwrap().pending,
        Some(LearnedStepIntent { actor_revision: n.actor_revision, position: n.position }));
    let before = host.revision(); let target = destination(&host);
    assert_eq!(host.propose_learned_text(before, 1, target, snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before);
    host.complete_learned_step(before, n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(host.revision(), before + 1);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}

#[test]
fn completed_text_requires_original_two_key_review_and_checked_publication() {
    let model = model(&[b'O' as u32, b'K' as u32, END]); let recipe = recipe(&model, config(&model));
    let root = Directory::new(); let (mut host, reviewer) = owner(&root, &recipe); finish(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let (action, inputs, automatic, request) = prepared(&mut host);
    assert_eq!(action.spec().payload, b"OK"); assert_eq!(host.inspect().executions, 0);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    let result = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"OK"); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    drop(host); let (mut host, _) = reopen(&root, &recipe);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
    let history = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &recipe).unwrap();
    assert_eq!(history.payload, b"OK"); assert_eq!(history.executions, 1);
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
}

#[test]
fn unknown_dispatch_reconciles_while_text_is_paused_without_reissuing_permission() {
    let model = model(&[b'O' as u32, b'K' as u32, END]); let recipe = recipe(&model, config(&model));
    for executed in [false, true] {
        let root = Directory::new(); let (mut host, reviewer) = owner(&root, &recipe); finish(&mut host);
        let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if executed { host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap(); }
        drop(host); let (mut host, _) = reopen(&root, &recipe);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let result = host.reconcile(host.revision(), 1).unwrap();
        if !executed {
            assert_eq!(result, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), 1).unwrap();
        }
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.inspect().control.ledger.charged, if executed { 16 } else { 0 });
        assert!(host.learned_generation_inspection().unwrap().paused);
    }
}

#[test]
fn original_holds_and_exhausted_telemetry_survive_text_recovery() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    for alarm in [false, true] {
        let mut source = config(&model);
        if alarm { source.policy = policy(&model, true); }
        else { source.telemetry.source_check_values = model.cache_profile().values_per_token() as u64; }
        let recipe = recipe(&model, source); let root = Directory::new(); let (mut host, _) = owner(&root, &recipe);
        step(&mut host); let n = host.learned_generation_inspection().unwrap().numerical;
        let result = host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
        if alarm { assert!(result.unwrap().accepted().is_none()); } else { assert_eq!(result.err(), Some(Error::Limit)); }
        let failed = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(failed.sampled_draws, 0);
        assert_eq!(failed.work.sampling_attempts, 1);
        assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
        drop(host); let (mut host, _) = reopen(&root, &recipe);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, failed);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert!(host.resume_learned_generation(host.revision(), failed.actor_revision, failed.position).is_err());
        let substitute = action_spec(&host, b"OK");
        assert!(host.propose(host.revision(), 1, substitute, snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn finite_horizon_and_undecodable_results_keep_their_original_completion_policy() {
    for case in 0..4 {
        let chain = if case == 3 { vec![OTHER_CONTROL, END] } else { vec![b'O' as u32, b'K' as u32, END] };
        let model = model(&chain); let mut source = config(&model);
        if case < 2 { source.max_new_tokens = 2; }
        if case == 1 { source.output.completion = LearnedTextCompletion::StopOrTokenLimit; }
        if case == 2 { source.output.max_bytes = 1; }
        let recipe = recipe(&model, source); let root = Directory::new(); let (mut host, _) = owner(&root, &recipe);
        finish(&mut host);
        let before = host.learned_text_message(LearnedEvidenceLimits::default()).map(|m| (m.bytes().to_vec(), m.stop()));
        assert_eq!(before.is_ok(), case == 1);
        drop(host); let (mut host, _) = reopen(&root, &recipe); resume(&mut host);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).map(|m| (m.bytes().to_vec(), m.stop())), before);
    }
}

#[test]
fn legacy_numeric_recipes_remain_generic_and_cannot_be_reinterpreted_as_text() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let numeric = numeric_recipe(&model, config(&model)); let text = recipe(&model, config(&model));
    assert!(!numeric.is_text()); assert!(text.is_text()); assert_ne!(numeric, text);
    let root = Directory::new(); let (mut host, _) = owner(&root, &numeric); step(&mut host);
    assert!(!host.learned_text_required());
    let action = action_spec(&host, b"generic effect");
    host.propose(host.revision(), 1, action, snapshot()).unwrap();
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    drop(host);
    assert_eq!(FileOversight::open_with_learned_generation(root.store(), profile(), &text).err(), Some(JournalError::Contract(Error::Binding)));
    let (host, _) = reopen(&root, &numeric);
    assert!(!host.learned_text_required());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.status, GenerationStatus::Generating);
}

#[test]
fn first_canonical_image_already_requires_text_without_fabricating_a_clock_or_token() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let recipe = recipe(&model, config(&model));
    let root = Directory::new();
    let numeric = numeric_recipe(&model, config(&model));
    assert_eq!(FileOversight::create_with_learned_text(root.store(), profile(), numeric).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert!(!root.store().exists());
    let (host, _) = FileOversight::create_with_learned_text(root.store(), profile(), recipe.clone()).unwrap();
    assert_eq!(host.revision(), 1);
    assert!(host.learned_text_required() && host.learned_generation_required() && host.publication_guard_required());
    assert!(!host.clock_ready());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.position, 0); assert_eq!(n.sampled_draws, 0);
    assert_eq!(n.work.admitted_tokens, 0); assert_eq!(host.inspect().executions, 0);
    drop(host);
    assert!(FileOversight::open(root.store(), profile()).is_err());
    let (mut host, _) = reopen(&root, &recipe);
    assert!(host.learned_text_required());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    resume(&mut host); finish(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}
