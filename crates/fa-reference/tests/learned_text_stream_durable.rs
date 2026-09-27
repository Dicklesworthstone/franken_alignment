//! Original numerical, journal, endpoint and two-key paths; synthetic ballots.
//! Disk observations distinguish published output from receipt-confirmed history.
#![cfg(unix)]
#[path = "support/learned_text_model.rs"]
mod fixture;
use fixture::{config, model, policy, tokenizer, END, OTHER_CONTROL};
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, stream::StreamProfile, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits, Reconciliation,
    observed::{FileOversight, FileOversightProfile, FileHumanReviewer, FileHumanRequest,
        decoder::learned::FileLearnedConfig},
}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy, learned_source::LearnedEvidenceLimits,
};
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
        let path = std::env::temp_dir().join(format!("fa-learned-text-stream-{}-{time}-{}",
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
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("stream test cleanup: {error}"); }
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
fn recipe() -> FileLearnedConfig {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default(), stream()).unwrap()
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
        root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, reviewer)
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
fn reopen(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    FileOversight::open_with_learned_text_stream(root.store(), profile(), config).unwrap()
}
fn resume(host: &mut FileOversight, tick: u64) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}
fn message(host: &mut FileOversight, attempt: u64) -> FrozenAction {
    host.propose_learned_text_stream_message(host.revision(), attempt, ElapsedTick(100), Vec::new(), snapshot()).unwrap()
}
fn closing(host: &mut FileOversight, attempt: u64) -> FrozenAction {
    host.propose_learned_text_stream_finish(host.revision(), attempt, ElapsedTick(100), Vec::new(), snapshot()).unwrap()
}
fn prepared(host: &mut FileOversight, attempt: u64, action: &FrozenAction)
    -> (CommitteeInput, FilePermit, FileHumanRequest)
{
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + attempt,
        attempt, &inputs, ElapsedTick(40)).unwrap();
    (inputs, automatic, request)
}
fn dispatched(host: &mut FileOversight, reviewer: &FileHumanReviewer,
    attempt: u64, action: &FrozenAction) -> CommitteeInput
{
    let (inputs, permit, request) = prepared(host, attempt, action);
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &permit, &human, action, &inputs, snapshot()).unwrap(); inputs
}
fn publish(host: &mut FileOversight, attempt: u64, inputs: &CommitteeInput, tick: u64) {
    assert!(host.publish(host.revision(), attempt).is_err());
    let receipt = host.publish_checked(host.revision(), attempt, Some(inputs), snapshot(), ElapsedTick(tick)).unwrap();
    assert!(matches!(receipt.outcome, EndpointOutcome::Executed { .. }));
}

#[test]
fn first_canonical_image_requires_stream_and_exact_recipe_without_inference() {
    let root = Directory::new(); let model = model(&[b'O' as u32, b'K' as u32, END]);
    let raw = FileLearnedConfig::new_text(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(FileOversight::create_with_learned_text_stream(root.store(), profile(), raw.clone()).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert!(!root.store().exists());
    let pinned = recipe();
    assert_ne!(pinned, raw); assert_eq!(pinned.text_stream_profile(), Some(stream()));
    assert!(FileOversight::create_with_learned_text(root.store(), profile(), pinned.clone()).is_err());
    assert!(!root.store().exists());
    let mut incompatible = profile(); incompatible.delivery.initial_payload = b"unreviewed".to_vec();
    assert!(FileOversight::create_with_learned_text_stream(root.store(), incompatible, pinned.clone()).is_err());
    assert!(!root.store().exists());
    let (host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile(), pinned.clone()).unwrap();
    assert_eq!(host.revision(), 2);
    assert!(host.learned_text_stream_required() && host.learned_text_required() && host.publication_guard_required());
    assert!(!host.clock_ready());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.position, 0); assert_eq!(n.work.admitted_tokens, 0); assert_eq!(n.sampled_draws, 0);
    let disk = root.files();
    let observed = FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &pinned).unwrap();
    assert_eq!(observed.confirmed.profile(), stream());
    assert!(observed.confirmed.visible().is_empty() && observed.published.visible().is_empty());
    assert_eq!(observed.pending, None); assert_eq!(root.files(), disk);
    drop(host);
    assert!(FileOversight::open_stream(root.store(), profile(), stream()).is_err());
    let (host, _) = reopen(&root, &pinned);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
}

#[test]
fn every_prompt_sample_and_stop_cut_recovers_exact_original_output_before_release() {
    let pinned = recipe();
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let mut control = model.observed_learned_text_generation(tokenizer(&model), config(&model)).unwrap();
    control.run_to_stop().unwrap();
    let expected = control.text_message(LearnedEvidenceLimits::default()).unwrap();
    for cut in 0..=4 {
        let root = Directory::new(); let (mut host, _) = owner(&root, &pinned);
        for _ in 0..cut { step(&mut host); }
        let before = host.learned_generation_inspection().unwrap().numerical; drop(host);
        let (mut host, _) = reopen(&root, &pinned);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        let revision = host.revision();
        assert_eq!(host.propose_learned_text_stream_message(revision, 1, ElapsedTick(100), Vec::new(), snapshot()).err(),
            Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(host.revision(), revision);
        resume(&mut host, 2); generate(&mut host);
        let actual = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
        assert_eq!(actual.bytes(), expected.bytes()); assert_eq!(actual.stop(), expected.stop());
        assert_eq!(actual.work(), expected.work()); assert_eq!(actual.telemetry_work(), expected.telemetry_work());
        assert_eq!(actual.evidence().tokens(), expected.evidence().tokens());
        let frame = host.stream_message_spec("OK", ElapsedTick(100)).unwrap();
        assert_eq!(message(&mut host, 1).spec(), &frame);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn published_append_and_finish_recover_separately_without_new_draws_or_duplicate_text() {
    let pinned = recipe(); let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned);
    generate(&mut host); let numerical = host.learned_generation_inspection().unwrap().numerical;
    let action = message(&mut host, 1); let input = dispatched(&mut host, &reviewer, 1, &action);
    publish(&mut host, 1, &input, 2);
    let cut = host.stream_snapshot().unwrap();
    assert_eq!(cut.pending, Some(1)); assert!(cut.confirmed.visible().is_empty());
    assert_eq!(cut.published.visible(), b"OK"); assert_eq!(cut.publication.executions, 1);
    let files = root.files();
    assert_eq!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &pinned).unwrap(), cut);
    assert_eq!(files, root.files()); drop(host);
    let (mut host, reviewer) = reopen(&root, &pinned);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["OK"]);
    assert!(host.propose_learned_text_stream_finish(host.revision(), 2, ElapsedTick(100), Vec::new(), snapshot()).is_err());
    resume(&mut host, 3);
    let close = closing(&mut host, 2); let input = dispatched(&mut host, &reviewer, 2, &close);
    publish(&mut host, 2, &input, 4);
    let cut = host.stream_snapshot().unwrap();
    assert!(!cut.confirmed.finished()); assert!(cut.published.finished()); assert_eq!(cut.pending, Some(2));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical); drop(host);
    let (mut host, _) = reopen(&root, &pinned);
    host.observe_time(host.revision(), ElapsedTick(5)).unwrap(); host.reconcile(host.revision(), 2).unwrap();
    let complete = host.stream_snapshot().unwrap();
    assert!(complete.confirmed.finished() && complete.published.finished()); assert_eq!(complete.pending, None);
    assert_eq!(complete.published.messages().collect::<Vec<_>>(), vec!["OK"]);
    assert_eq!(complete.publication.executions, 2);
    assert_eq!(complete.publication.control.ledger.charged, action.spec().units + close.spec().units);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    resume(&mut host, 5);
    assert!(host.propose_learned_text_stream_message(host.revision(), 3, ElapsedTick(100), Vec::new(), snapshot()).is_err());
    assert!(host.propose_learned_text_stream_finish(host.revision(), 3, ElapsedTick(100), Vec::new(), snapshot()).is_err());
}

#[test]
fn unexecuted_unknown_append_or_finish_stays_charged_until_original_sealing() {
    let pinned = recipe();
    for close in [false, true] {
        let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
        let first = message(&mut host, 1);
        let input = dispatched(&mut host, &reviewer, 1, &first);
        if close { publish(&mut host, 1, &input, 2); host.reconcile(host.revision(), 1).unwrap(); }
        let pending = if close { closing(&mut host, 2) } else { first.clone() };
        let attempt = if close { 2 } else { 1 };
        if close { dispatched(&mut host, &reviewer, attempt, &pending); }
        let charged = host.inspect().control.ledger.charged; drop(host);
        let (mut host, reviewer) = reopen(&root, &pinned); resume(&mut host, 3);
        assert_eq!(host.inspect().control.ledger.charged, charged);
        assert_eq!(host.stream_snapshot().unwrap().pending, Some(attempt));
        assert!(host.propose_learned_text_stream_message(host.revision(), 3, ElapsedTick(100), Vec::new(), snapshot()).is_err());
        assert!(host.propose_learned_text_stream_finish(host.revision(), 3, ElapsedTick(100), Vec::new(), snapshot()).is_err());
        assert_eq!(host.reconcile(host.revision(), attempt).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.charged, charged);
        host.seal_unexecuted(host.revision(), attempt).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, charged - pending.spec().units);
        assert_eq!(host.stream_snapshot().unwrap().pending, None);
        let retry = if close { closing(&mut host, 3) } else { message(&mut host, 3) };
        let input = dispatched(&mut host, &reviewer, 3, &retry); publish(&mut host, 3, &input, 4);
        host.reconcile(host.revision(), 3).unwrap();
        let state = host.stream_snapshot().unwrap();
        assert_eq!(state.published.visible(), b"OK"); assert_eq!(state.published.message_count(), 1);
        assert_eq!(state.published.finished(), close);
        assert_eq!(state.publication.executions, if close { 2 } else { 1 });
    }
}

#[test]
fn recovery_fences_old_automatic_and_human_keys_but_fresh_review_can_publish() {
    let pinned = recipe(); let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned);
    generate(&mut host); let first = message(&mut host, 1);
    let (input, permit, request) = prepared(&mut host, 1, &first);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, first.spec().units); drop(host);
    let (mut host, reviewer) = reopen(&root, &pinned); resume(&mut host, 2);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    let before = host.revision(); let files = root.files();
    assert!(host.dispatch(before, &permit, &human, &first, &input, snapshot()).is_err());
    assert_eq!(host.revision(), before); assert_eq!(root.files(), files);
    let fresh = message(&mut host, 2); let input = dispatched(&mut host, &reviewer, 2, &fresh);
    publish(&mut host, 2, &input, 3); host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"OK");
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn raw_text_or_changed_stream_contract_cannot_reinterpret_or_mutate_a_retained_image() {
    let pinned = recipe(); let root = Directory::new(); let (mut host, _) = owner(&root, &pinned);
    step(&mut host); drop(host); let before = root.files();
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let raw = FileLearnedConfig::new_text(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()).unwrap();
    let mut alternatives = vec![raw];
    for stream in [StreamProfile::new(2, 1, 4, 64, 128).unwrap(),
        StreamProfile::new(1, 2, 4, 64, 128).unwrap(), StreamProfile::new(1, 1, 3, 64, 128).unwrap(),
        StreamProfile::new(1, 1, 4, 65, 128).unwrap(), StreamProfile::new(1, 1, 4, 64, 127).unwrap()] {
        alternatives.push(FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), config(&model),
            LearnedDecoderBindingLimits::default(), stream).unwrap());
    }
    for wrong in alternatives {
        assert_ne!(wrong, pinned);
        assert!(FileOversight::open_with_learned_text_stream(root.store(), profile(), &wrong).is_err());
        assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &wrong).is_err());
        assert!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &wrong).is_err());
        assert_eq!(root.files(), before);
    }
    let valid = FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &pinned).unwrap();
    assert_eq!(valid.confirmed.profile(), stream()); assert_eq!(root.files(), before);
    let (mut host, _) = reopen(&root, &pinned); resume(&mut host, 2); generate(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}

#[test]
fn stale_revisions_and_generic_substitutions_cannot_write_or_bypass_the_stream_builder() {
    let pinned = recipe(); let root = Directory::new(); let (mut host, _) = owner(&root, &pinned);
    generate(&mut host); drop(host); let (mut host, _) = reopen(&root, &pinned); resume(&mut host, 2);
    let valid = host.stream_message_spec("OK", ElapsedTick(100)).unwrap();
    let before = host.revision(); let files = root.files();
    assert_eq!(host.propose_learned_text_stream_message(before - 1, 1, ElapsedTick(100), Vec::new(), snapshot()).err(),
        Some(JournalError::Contract(Error::Stale)));
    for case in 0..4 {
        let mut changed = valid.clone();
        match case {
            0 => changed.payload = b"OK".to_vec(),
            1 => changed.payload = host.stream_message_spec("KO", ElapsedTick(100)).unwrap().payload,
            2 => changed.units = 2,
            _ => changed.target.as_mut().unwrap().expected_version += 1,
        }
        assert!(host.propose(before, 1, changed, snapshot()).is_err());
        assert_eq!(host.revision(), before); assert_eq!(root.files(), files);
    }
    assert_eq!(host.propose(before, 1, valid.clone(), snapshot()).unwrap().spec(), &valid);
}

#[test]
fn pending_final_token_requires_original_outcome_ack_before_any_stream_frame() {
    let pinned = recipe(); let root = Directory::new(); let (mut host, _) = owner(&root, &pinned);
    for _ in 0..3 { step(&mut host); }
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); drop(host);
    let (mut host, _) = reopen(&root, &pinned); resume(&mut host, 2);
    let before = host.revision();
    assert_eq!(host.propose_learned_text_stream_message(before, 1, ElapsedTick(100), Vec::new(), snapshot()).err(),
        Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before);
    host.complete_learned_step(before, n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 3);
    message(&mut host, 1); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn failed_or_undecodable_generation_survives_recovery_without_an_output_fallback() {
    for case in 0..3 {
        let model = model(if case == 2 { &[OTHER_CONTROL, END] } else { &[b'O' as u32, b'K' as u32, END] });
        let mut source = config(&model);
        if case == 0 { source.policy = policy(&model, true); }
        if case == 1 { source.telemetry.source_check_values = model.cache_profile().values_per_token() as u64; }
        let pinned = FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), source,
            LearnedDecoderBindingLimits::default(), stream()).unwrap();
        let root = Directory::new(); let (mut host, _) = owner(&root, &pinned); step(&mut host);
        if case < 2 {
            let n = host.learned_generation_inspection().unwrap().numerical;
            let result = host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
            if case == 0 { assert!(result.unwrap().accepted().is_none()); } else { assert_eq!(result.err(), Some(Error::Limit)); }
        } else { generate(&mut host); }
        let before = host.learned_generation_inspection().unwrap().numerical; drop(host);
        let (mut host, _) = reopen(&root, &pinned); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        let resumed = host.resume_learned_generation(host.revision(), before.actor_revision, before.position);
        assert_eq!(resumed.is_ok(), case == 2);
        assert!(host.propose_learned_text_stream_message(host.revision(), 1, ElapsedTick(100), Vec::new(), snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0); assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
    }
}

#[test]
fn actual_stream_bootstrap_and_output_capacity_are_checked_before_installation() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let small = StreamProfile::new(1, 1, 4, 2, 2).unwrap();
    let mut source = config(&model); source.output.max_bytes = 3;
    assert_eq!(FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), source.clone(),
        LearnedDecoderBindingLimits::default(), small).err(), Some(Error::Limit));
    source.output.max_bytes = 2;
    let exact = FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), source,
        LearnedDecoderBindingLimits::default(), small).unwrap();
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_stream(root.store(), profile(), stream()).unwrap();
    let before = host.revision(); let files = root.files();
    assert_eq!(host.enable_learned_generation(before, exact.clone()).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), before); assert_eq!(root.files(), files);
    assert!(!host.learned_generation_required()); drop(host);
    let valid = Directory::new(); let (mut host, _) = owner(&valid, &exact);
    generate(&mut host); message(&mut host, 1);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn pending_publication_still_seals_on_missing_inputs_or_late_witness_changes() {
    for missing in [false, true] {
        let pinned = recipe(); let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned);
        generate(&mut host); let action = message(&mut host, 1);
        let input = dispatched(&mut host, &reviewer, 1, &action);
        let mut current = snapshot();
        if !missing { current.values.insert(7, b"changed".to_vec()); }
        let result = host.publish_checked(host.revision(), 1, if missing { None } else { Some(&input) },
            current, ElapsedTick(2)).unwrap();
        assert!(!matches!(result.outcome, EndpointOutcome::Executed { .. }));
        assert_eq!(host.inspect().executions, 0);
        assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
        // Only the original receipt transition releases the charge. A missing
        // fresh check never converts the undisclosed text to a completed stream.
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert!(!host.stream_snapshot().unwrap().confirmed.finished());
        let retry = message(&mut host, 2); let input = dispatched(&mut host, &reviewer, 2, &retry);
        publish(&mut host, 2, &input, 3); host.reconcile(host.revision(), 2).unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"OK");
    }
}
