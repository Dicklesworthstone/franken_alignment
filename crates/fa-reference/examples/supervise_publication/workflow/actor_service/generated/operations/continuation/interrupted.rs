//! Real interrupted numerical work reaches the same independent review pipeline.
//! No saved approval, fake model result or replacement generation cursor is used.
use super::*;

fn recorded(root: &Root, recipe: &Loaded, steps: u64) -> FileOversight {
    let mut config = configured(root);
    let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(
        &config.store, config.profile.clone(), recipe.stream, recipe.decoder.clone(),
        recipe.tokenizer.clone(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(recipe.generation, n.actor_revision,
        n.position, recipe.text.clone()).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for _ in 0..steps {
        let revision = host.decoder_text_progress(recipe.generation).unwrap().generation_revision();
        host.advance_decoder_text(host.revision(), recipe.generation, revision).unwrap();
    }
    host
}

#[test]
fn every_interruption_cut_and_complete_unsubmitted_output_reach_one_original_publication() {
    for cut in 0..=4 {
        let root = Root::new(); let mut recipe = loaded(false);
        drop(recorded(&root, &recipe, cut));
        // No future publication request was bound by the numerical intent.
        // Select a fresh ID explicitly; retain ALL original generation inputs.
        recipe.request = 11;
        let config = continuation_config(&root); let audience = peers(&root, &config);
        let human = review_request(audience.clone(), 11, ReviewDecision::Approve);
        let result = continue_generation(config, recipe, &audience, || ElapsedTick(1001)).unwrap();
        let packet = human.join().unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert!(executed(&result)); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(packet.binding().request, 11);
        assert_eq!(packet.action().spec().policy_epoch, 1);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let config = configured(&root); let recipe = loaded(false);
        let state = FileOversight::read_decoder_text_message(&config.store, &config.profile,
            &recipe.decoder, &recipe.tokenizer, recipe.stream, 11).unwrap();
        assert_eq!(state.source.generation, 7); assert_eq!(state.source.generation_revision, 4);
        assert_eq!(state.stream.confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(state.stream.confirmed, state.stream.published);
        assert_eq!(state.stream.publication.executions, 1);
        assert_eq!(state.stream.publication.control.ledger.charged, packet.action().spec().units);
        assert_eq!(state.stream.publication.control.ledger.reserved, 0);
        let progress = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &recipe.decoder, &recipe.tokenizer, 7).unwrap();
        assert_eq!(progress.numerical.numerical.position, 4);
        assert_eq!(progress.numerical.numerical.sampled_draws, 2);
        assert_eq!(progress.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
        let mut receipt_recipe = loaded(false); receipt_recipe.request = 11;
        let receipt = recovery::resume(config, receipt_recipe, || panic!("terminal receipt must not infer")).unwrap();
        assert!(executed(&receipt));
    }
}

#[test]
fn recovered_computation_is_not_consent_and_rejection_keeps_output_private() {
    let root = Root::new(); drop(recorded(&root, &loaded(false), 3));
    let config = continuation_config(&root); let audience = peers(&root, &config);
    let human = review_request(audience.clone(), 1, ReviewDecision::Reject);
    let result = continue_generation(config, loaded(false), &audience, || ElapsedTick(1001)).unwrap();
    human.join().unwrap();
    assert!(result.failure.is_none()); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let mut output = Vec::new(); assert!(emit(result, &mut output).is_err());
    assert!(!output.windows(7).any(|part| part == b"payload"));
    let config = configured(&root); let recipe = loaded(false);
    let state = FileOversight::read_decoder_text_message(&config.store, &config.profile,
        &recipe.decoder, &recipe.tokenizer, recipe.stream, 1).unwrap();
    assert_eq!(state.stream.publication.executions, 0); assert!(state.stream.publication.payload.is_empty());
    assert_eq!(state.stream.publication.control.ledger.charged, 0);
    assert_eq!(state.stream.publication.control.ledger.reserved, 0);
    assert_eq!(state.generation.bytes().unwrap(), b"A");
}

#[test]
fn wrong_or_absent_retained_intents_refuse_before_source_read_or_additional_tokens() {
    for field in 0..5 {
        let root = Root::new(); drop(recorded(&root, &loaded(false), 1));
        let mut config = configured(&root); config.programs.clear();
        let audience = peers(&root, &config); let mut recipe = loaded(false);
        match field {
            0 => recipe.generation = 99,
            1 => recipe.text.prompt.push(b'y'),
            2 => recipe.text.max_new_tokens += 1,
            3 => recipe.text.generation.scalar_products -= 1,
            _ => recipe.text.tokenization.pair_lookups -= 1,
        }
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(continue_generation(config, recipe, &audience, || ElapsedTick(1001)).is_err());
        assert!(!audience.socket(1).exists());
        let config = configured(&root); let recipe = loaded(false);
        let state = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &recipe.decoder, &recipe.tokenizer, 7).unwrap();
        assert_eq!(state.text.generation_revision(), 1);
        assert_eq!(state.numerical.numerical.position, 1); assert_eq!(state.publication.executions, 0);
    }
    let root = Root::new(); let config = configured(&root); let audience = peers(&root, &config);
    assert!(continue_generation(config, loaded(false), &audience, || ElapsedTick(1001)).is_err());
    assert!(!root.0.join("store").exists());
}

#[test]
fn fresh_request_ids_cannot_repackage_previously_submitted_native_output() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); initial(&root, decision);
        let mut config = configured(&root); config.programs.clear(); let audience = peers(&root, &config);
        let original = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let mut recipe = loaded(false); recipe.request = 2;
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(continue_generation(config, recipe, &audience, || ElapsedTick(1001)).is_err());
        let config = configured(&root); let recipe = loaded(false);
        let state = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &recipe.decoder, &recipe.tokenizer, 7).unwrap();
        assert_eq!(state.numerical.numerical.position, 4);
        assert_eq!(state.publication.executions, original.executions);
        assert_eq!(state.publication.control.ledger.charged, original.control.ledger.charged);
        assert!(FileOversight::read_decoder_text_message(&config.store, &config.profile,
            &recipe.decoder, &recipe.tokenizer, recipe.stream, 2).is_err());
    }
}

#[test]
fn missing_fresh_source_stops_without_advancing_recovered_numerical_position() {
    let root = Root::new(); drop(recorded(&root, &loaded(false), 3));
    let mut config = configured(&root); config.programs.clear(); let audience = peers(&root, &config);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = continue_generation(config, loaded(false), &audience, || ElapsedTick(1001)).unwrap();
    assert!(result.failure.is_some()); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    assert!(!audience.socket(1).exists());
    let config = configured(&root); let recipe = loaded(false);
    let state = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
        &recipe.decoder, &recipe.tokenizer, 7).unwrap();
    assert_eq!(state.numerical.numerical.position, 3);
    assert_eq!(state.numerical.numerical.sampled_draws, 1);
    assert_eq!(state.publication.executions, 0); assert!(state.publication.stop.is_some());
    assert!(state.publication.payload.is_empty());
}

#[test]
fn held_cancelled_and_token_limited_generations_cannot_restart_as_new_work() {
    for reason in 0..3 {
        let root = Root::new(); let mut recipe = loaded(reason == 0);
        if reason == 2 { recipe.text.max_new_tokens = 1; }
        let mut host = recorded(&root, &recipe, if reason == 0 { 1 } else if reason == 1 { 0 } else { 3 });
        if reason == 1 { host.cancel_decoder_text(host.revision(), 7, 0).unwrap(); }
        let before = host.decoder_inspection().unwrap().numerical; drop(host);
        let mut config = configured(&root); config.programs.clear(); let audience = peers(&root, &config);
        let decoder = recipe.decoder.clone(); let tokenizer = recipe.tokenizer.clone();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(continue_generation(config, recipe, &audience, || ElapsedTick(1001)).is_err());
        let config = configured(&root);
        let state = FileOversight::read_decoder_text_progress(&config.store, &config.profile,
            &decoder, &tokenizer, 7).unwrap();
        assert_eq!(state.numerical.numerical, before);
        assert_eq!(state.publication.executions, 0); assert!(!audience.socket(1).exists());
    }
}

#[test]
fn command_modes_are_explicit_exclusive_and_validate_before_loading_any_files() {
    for tail in [vec!["--resume", "--continue-generation"],
        vec!["peer", "--continue-generation", "1"], vec!["peer", "--after"],
        vec!["peer", "--continue-generation", "--after", "1"], vec!["peer", "--unknown"],
        vec!["--continue-generation"]] {
        let args = [vec!["create-generated", "missing-config", "missing-recipe"], tail].concat();
        assert_eq!(command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE);
    }
    let args = ["create-generated", "missing-config", "missing-recipe", "peer", "--continue-generation"]
        .map(str::to_owned);
    assert_eq!(command(&args, Some(Path::new("qualification"))).unwrap_err(), USAGE);
    // A valid flag is routed past option parsing, not interpreted as a filename.
    assert_ne!(crate::command(args.to_vec()).unwrap_err(), USAGE);
}
