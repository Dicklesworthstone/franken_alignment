//! Two actual learned messages in ONE finite original sampled generation.
//! Congress ballots here are synthetic; the executable's continuation test
//! separately exercises original native inference and the second human key.
use super::*;
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::monitored::{GenerationStatus, GenerationStop}},
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use fa_reference::action::consequence::delivery::persistent::requests::{
    FileRequestDisposition, actor::{FileActorSupervisor, FileLearnedTextStreamActorPort, LearnedTextRelease},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorError, ActorOutcome, Knowledge}, learned_source::text::LearnedTextConfig,
};
use std::collections::BTreeSet;
#[path = "continuation/forecast.rs"]
mod forecast;

fn finite_model() -> DecoderModel {
    // Real attention retains earlier positions. The four distinct continuations
    // are selected by actual decoder logits, with no canned source output.
    let dimensions = 6;
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 259, hidden: dimensions, intermediate: dimensions, layers: 1,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let diagonal = |scale| {
        let mut matrix = vec![0.0; dimensions * dimensions];
        for index in 0..dimensions { matrix[index * dimensions + index] = scale; }
        matrix
    };
    let mut embedding = vec![0.0; 259 * dimensions];
    embedding[fixture::PROMPT as usize * dimensions] = 1.0;
    embedding[fixture::MERGED_PROMPT as usize * dimensions] = 1.0;
    let mut head = vec![0.0; 259 * dimensions];
    for (index, token) in [b'A' as u32, END, b'B' as u32, OTHER_CONTROL].into_iter().enumerate() {
        head[token as usize * dimensions + index] = 8.0;
        embedding[token as usize * dimensions + index + 1] = 1.0;
    }
    DecoderModel::new(profile, embedding, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; dimensions], queries: diagonal(0.1), keys: diagonal(0.1),
        values: diagonal(1.0), attention_output: diagonal(0.01),
        feed_forward_norm: vec![1.0; dimensions], gate: vec![0.0; dimensions * dimensions],
        up: vec![0.0; dimensions * dimensions], down: vec![0.0; dimensions * dimensions],
    }], vec![1.0; dimensions], head).unwrap()
}

fn finite_text(model: &DecoderModel, max_new_tokens: usize, hold_second: bool) -> LearnedTextConfig {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[fixture::PROMPT, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            // Only B introduces this value coordinate. The exact original
            // monitor stays quiet for prompt, A and the first control stop.
            if hold_second && side == KvSide::Value { weights[3] = 1.0; }
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, 0.5).unwrap();
            taps.insert(KvTap { layer: *layer, side }, LearnedRefinementMonitor::new(
                vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let mut text = config(model);
    text.max_new_tokens = max_new_tokens;
    text.stop_tokens = BTreeSet::from([END, OTHER_CONTROL]);
    text.policy = LearnedDecoderPolicy::new(codec,
        LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap(),
        LearnedStreamRetention::All, LearnedAuditPreparationBudget::default(), inference).unwrap();
    text
}

fn finite_recipe(max_new_tokens: usize, hold_second: bool, enabled: bool) -> FileLearnedConfig {
    let model = finite_model();
    let pinned = FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model),
        finite_text(&model, max_new_tokens, hold_second), LearnedDecoderBindingLimits::default(), stream()).unwrap();
    if enabled { pinned.with_text_stream_continuation().unwrap() } else { pinned }
}

fn request(host: &mut FileOversight, request: u64, message: Option<&str>) -> (u64, FrozenAction) {
    let spec = match message {
        Some(text) => host.stream_message_spec(text, ElapsedTick(100)).unwrap(),
        None => host.stream_finish_spec(ElapsedTick(100)).unwrap(),
    };
    // The original request intake rechecks this against the original learned
    // source builder before admission; the test cannot supply different output.
    let status = host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
    let FileRequestDisposition::Admitted { attempt, stage: ActionState::Reviewing } = status.disposition
        else { panic!("original request not admitted: {status:?}"); };
    (attempt, host.request_action(request).unwrap().clone())
}

fn confirmed_message(host: &mut FileOversight, reviewer: &FileHumanReviewer, request_id: u64,
    text: &str, tick: u64) -> u64 {
    let (attempt, action) = request(host, request_id, Some(text));
    let input = dispatched(host, reviewer, attempt, &action);
    publish(host, attempt, &input, tick);
    host.reconcile(host.revision(), attempt).unwrap();
    action.spec().units
}

fn continue_message(host: &mut FileOversight, after: u64, request: u64) {
    let command = host.prepare_learned_text_stream_continuation(after, request).unwrap();
    host.begin_learned_text_stream_continuation(host.revision(), command).unwrap();
}

#[test]
fn finite_messages_keep_original_sampler_cache_work_and_full_receipt_history_through_finish() {
    let model = finite_model();
    let mut uninterrupted = finite_text(&model, 4, false);
    // Keeping only the final stop lets the independent original engine audit
    // and accept the exact same four draws without any host message boundary.
    uninterrupted.stop_tokens = BTreeSet::from([OTHER_CONTROL]);
    let mut control = model.observed_learned_text_generation(tokenizer(&model), uninterrupted).unwrap();
    control.run_to_stop().unwrap();
    let pinned = finite_recipe(4, false, true); let root = Directory::new();
    let (mut host, reviewer) = owner(&root, &pinned);
    generate(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
    let first = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(first.sampled_draws, 2);
    let first_charge = confirmed_message(&mut host, &reviewer, 71, "A", 2);
    let ledger = host.inspect().control.ledger;
    continue_message(&mut host, 71, 72);
    let resumed = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(resumed.work, first.work); assert_eq!(resumed.telemetry, first.telemetry);
    assert_eq!(resumed.position, first.position); assert_eq!(resumed.sampled_draws, first.sampled_draws);
    assert_eq!(host.inspect().control.ledger, ledger);
    generate(&mut host);
    let second = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(second.bytes(), b"B");
    assert_eq!(second.evidence().tokens(), &[fixture::MERGED_PROMPT, b'A' as u32, END, b'B' as u32, OTHER_CONTROL]);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, control.position());
    assert_eq!(numerical.work, control.work()); assert_eq!(numerical.telemetry, control.telemetry_work());
    assert_eq!(numerical.sampled_draws as usize, control.accepted_tokens().len() - 1);
    assert_eq!(second.evidence().tokens(), control.accepted_tokens());
    assert_eq!(numerical.status, GenerationStatus::Finished(GenerationStop::StopToken(OTHER_CONTROL)));
    let second_charge = confirmed_message(&mut host, &reviewer, 72, "B", 3);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(host.prepare_learned_text_stream_continuation(72, 73).err(), Some(JournalError::Contract(Error::Limit)));
    let (attempt, close) = request(&mut host, 73, None);
    let input = dispatched(&mut host, &reviewer, attempt, &close);
    publish(&mut host, attempt, &input, 4); host.reconcile(host.revision(), attempt).unwrap();
    let completed = host.stream_snapshot().unwrap();
    assert_eq!(completed.confirmed, completed.published); assert!(completed.confirmed.finished());
    assert_eq!(completed.confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(completed.publication.executions, 3);
    assert_eq!(completed.publication.control.ledger.charged, first_charge + second_charge + close.spec().units);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    drop(host);
    let (host, _) = reopen(&root, &pinned);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, completed.confirmed);
    assert_eq!(host.stream_snapshot().unwrap().published, completed.published);
    assert_eq!(host.inspect().executions, completed.publication.executions);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}

#[test]
fn continuation_requires_the_latest_original_accepted_message_receipt() {
    let pinned = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
    let (attempt, first) = request(&mut host, 71, Some("A"));
    let (inputs, automatic, offer) = prepared(&mut host, attempt, &first);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &offer).unwrap();
    for cut in 0..3 {
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let revision = host.revision(); let files = root.files();
        assert!(host.prepare_learned_text_stream_continuation(71, 72).is_err());
        assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        match cut {
            0 => host.dispatch(host.revision(), &automatic, &human, &first, &inputs, snapshot()).unwrap(),
            1 => publish(&mut host, attempt, &inputs, 2),
            _ => {},
        }
    }
    host.reconcile(host.revision(), attempt).unwrap();
    for (after, next) in [(0, 72), (71, 0), (71, 71), (999, 72)] {
        assert!(host.prepare_learned_text_stream_continuation(after, next).is_err());
    }
    let command = host.prepare_learned_text_stream_continuation(71, 72).unwrap();
    let revision = host.revision(); let files = root.files();
    assert_eq!(host.begin_learned_text_stream_continuation(revision - 1, command).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
    host.begin_learned_text_stream_continuation(revision, command).unwrap(); generate(&mut host);
    confirmed_message(&mut host, &reviewer, 72, "B", 3);
    let files = root.files(); let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.prepare_learned_text_stream_continuation(71, 73).is_err());
    assert!(host.prepare_learned_text_stream_continuation(71, 71).is_err());
    assert!(host.prepare_learned_text_stream_continuation(999, 72).is_err());
    assert_eq!(root.files(), files); assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}

#[test]
fn opt_in_is_exact_recipe_identity_and_never_replenishes_the_last_generation_unit() {
    let legacy = finite_recipe(4, false, false);
    let enabled = finite_recipe(4, false, true);
    assert!(!legacy.allows_text_stream_continuation()); assert!(enabled.allows_text_stream_continuation());
    assert_ne!(legacy, enabled);
    assert_eq!(enabled.clone().with_text_stream_continuation().err(), Some(Error::Duplicate));
    for limit in [2, 4] {
        let pinned = finite_recipe(limit, false, limit == 2); let root = Directory::new();
        let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
        confirmed_message(&mut host, &reviewer, 71, "A", 2);
        let revision = host.revision(); let numerical = host.learned_generation_inspection().unwrap().numerical;
        let files = root.files();
        assert_eq!(host.prepare_learned_text_stream_continuation(71, 72).err(),
            Some(JournalError::Contract(if limit == 2 { Error::Limit } else { Error::Binding })));
        assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
    let root = Directory::new(); let (mut host, _) = owner(&root, &enabled); step(&mut host); drop(host);
    let files = root.files();
    for wrong in [legacy, finite_recipe(6, false, true), finite_recipe(4, true, true)] {
        assert!(FileOversight::open_with_learned_text_stream(root.store(), profile(), &wrong).is_err());
        assert!(FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile(), &wrong).is_err());
        assert_eq!(root.files(), files);
    }
}

#[test]
fn an_actual_second_message_monitor_hold_preserves_the_first_receipt_and_global_spend() {
    let pinned = finite_recipe(4, true, true); let root = Directory::new();
    let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
    let charge = confirmed_message(&mut host, &reviewer, 71, "A", 2);
    let first = host.learned_generation_inspection().unwrap().numerical;
    continue_message(&mut host, 71, 72);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    let held = host.learned_generation_inspection().unwrap().numerical;
    assert!(matches!(held.status, GenerationStatus::Held(_)));
    assert_eq!(held.sampled_draws, first.sampled_draws);
    assert!(held.work.admitted_tokens > first.work.admitted_tokens);
    assert!(held.work.reserved_decoder_products > first.work.reserved_decoder_products);
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    assert!(host.prepare_learned_text_stream_continuation(71, 73).is_err());
    assert!(host.propose_learned_text_stream_message(host.revision(), 2, ElapsedTick(100), Vec::new(), snapshot()).is_err());
    assert!(host.propose_learned_text_stream_finish(host.revision(), 2, ElapsedTick(100), Vec::new(), snapshot()).is_err());
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(stream.confirmed, stream.published); assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.control.ledger.charged, charge);
    drop(host);
    let (mut host, _) = reopen(&root, &pinned);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), held.actor_revision, held.position).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, held);
    assert!(host.prepare_learned_text_stream_continuation(71, 73).is_err());
    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
}

#[test]
fn continuation_and_every_second_message_cut_recover_without_new_draws_or_reissued_intent() {
    let pinned = finite_recipe(4, false, true);
    for cut in 0..=2 {
        let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
        confirmed_message(&mut host, &reviewer, 71, "A", 2);
        let command = host.prepare_learned_text_stream_continuation(71, 72).unwrap();
        host.begin_learned_text_stream_continuation(host.revision(), command).unwrap();
        for _ in 0..cut { step(&mut host); }
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let charged = host.inspect().control.ledger.charged; drop(host);
        let (mut host, reviewer) = reopen(&root, &pinned);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.current_learned_text_continuation().unwrap(), Some(command));
        assert_eq!(host.prepare_learned_text_stream_continuation(71, 72).unwrap(), command);
        let files = root.files(); let revision = host.revision();
        host.begin_learned_text_stream_continuation(0, command).unwrap();
        assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.inspect().control.ledger.charged, charged);
        resume(&mut host, 3); generate(&mut host);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 4);
        confirmed_message(&mut host, &reviewer, 72, "B", 4);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(host.inspect().executions, 2);
    }
}

#[test]
fn source_only_gateway_binds_the_new_message_to_its_original_continuation_request() {
    let pinned = finite_recipe(4, false, true); let root = Directory::new();
    let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
    confirmed_message(&mut host, &reviewer, 71, "A", 2);
    continue_message(&mut host, 71, 72); generate(&mut host);
    let files = root.files();
    let wrong = host.stream_message_spec("B", ElapsedTick(100)).unwrap();
    assert_eq!(host.submit_request(host.revision(), 73, wrong, snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.propose_learned_text_stream_message(host.revision(), 99,
        ElapsedTick(100), Vec::new(), snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(root.files(), files);
    let (port, mut supervisor): (FileLearnedTextStreamActorPort, FileActorSupervisor<FileOversight>) =
        host.into_learned_text_stream_actor_gateway().unwrap();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(port.submit(73, LearnedTextRelease::Message, ElapsedTick(100)).err(), Some(ActorError::IdempotencyConflict));
    assert_eq!(root.files(), files);
    // Failed derivation preserves the same single-use policy observation.
    let ticket = port.submit(72, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let (attempt, action) = {
        let host = supervisor.host().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(72).unwrap().disposition else { panic!(); };
        (attempt, host.request_action(72).unwrap().clone())
    };
    let mut host = supervisor.host_mut().unwrap();
    let input = dispatched(&mut host, &reviewer, attempt, &action);
    publish(&mut host, attempt, &input, 3);
    let revision = host.revision(); host.reconcile(revision, attempt).unwrap();
    drop(host);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let revision = supervisor.host().unwrap().revision(); let files = root.files();
    port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    port.submit(72, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert_eq!(supervisor.host().unwrap().revision(), revision); assert_eq!(root.files(), files);
}

#[test]
fn pending_second_message_stop_replays_the_original_intent_once_before_source_release() {
    let pinned = finite_recipe(4, false, true); let root = Directory::new();
    let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
    confirmed_message(&mut host, &reviewer, 71, "A", 2);
    continue_message(&mut host, 71, 72); step(&mut host);
    let pending = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), pending.actor_revision, pending.position).unwrap();
    drop(host);
    let files = root.files();
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &pinned).unwrap();
    while recovery.progress().replayed_events < recovery.progress().total_events {
        let at = recovery.progress().replayed_events; recovery.advance(at, 1).unwrap();
    }
    assert_eq!(root.files(), files);
    let (mut host, reviewer) = recovery.finish().unwrap(); resume(&mut host, 3);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, pending);
    assert!(host.propose_learned_text_stream_message(host.revision(), 2, ElapsedTick(100), Vec::new(), snapshot()).is_err());
    host.complete_learned_step(host.revision(), pending.actor_revision, pending.position).unwrap().unwrap();
    let completed = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(completed.sampled_draws, 4);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
    confirmed_message(&mut host, &reviewer, 72, "B", 4);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
    let files = root.files();
    assert!(host.complete_learned_step(host.revision(), pending.actor_revision, pending.position).is_err());
    assert_eq!(root.files(), files); assert_eq!(host.learned_generation_inspection().unwrap().numerical, completed);
}

#[test]
fn token_limit_completion_cannot_create_another_finite_message_even_if_text_policy_accepts_it() {
    use fa_reference::action::consequence::oversight::learned_source::text::LearnedTextCompletion;
    let model = finite_model(); let mut text = finite_text(&model, 3, false);
    text.stop_tokens = BTreeSet::from([END]); text.output.completion = LearnedTextCompletion::StopOrTokenLimit;
    let pinned = FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), text,
        LearnedDecoderBindingLimits::default(), stream()).unwrap().with_text_stream_continuation().unwrap();
    let root = Directory::new(); let (mut host, reviewer) = owner(&root, &pinned); generate(&mut host);
    confirmed_message(&mut host, &reviewer, 71, "A", 2); continue_message(&mut host, 71, 72);
    generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.status, GenerationStatus::Finished(GenerationStop::TokenLimit));
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
    confirmed_message(&mut host, &reviewer, 72, "B", 3);
    let files = root.files();
    assert_eq!(host.prepare_learned_text_stream_continuation(72, 73).err(), Some(JournalError::Contract(Error::WrongState)));
    assert_eq!(root.files(), files); assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}
