//! Recovery uses actual original inference and canonical files, never imported
//! cursors/results. Socket/helper peers remain synthetic same-process fixtures.
use super::*;

fn seed(config: &mut Config, native: &Inputs, begin: bool, steps: u64) {
    let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(
        &config.store, config.profile.clone(), native.stream, native.decoder.clone(),
        native.tokenizer.clone(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    if begin {
        let numerical = host.decoder_inspection().unwrap().numerical;
        let command = FileTextGenerationCommand::new(native.generation, numerical.actor_revision,
            numerical.position, native.request.clone()).unwrap();
        host.begin_decoder_text(host.revision(), command).unwrap();
        for revision in 0..steps {
            host.advance_decoder_generation(host.revision(), native.generation, revision).unwrap();
        }
        assert_eq!(host.decoder_generation_progress(native.generation).unwrap().generation_revision(), steps);
    }
    assert_eq!(host.inspect().executions, 0);
    // Dropping models a stopped process at an acknowledged cut, NOT a Stop
    // consequence. Ordinary service failure deliberately records Stop instead.
}

#[test]
fn native_recovery_continues_partial_and_complete_intents_with_independent_human_review() {
    for steps in [1, 4] {
        for approve in [true, false] {
            let root = Root::new(); let mut config = configured(&root);
            let (actor, reviewer) = profiles(&root, &config); let native = inputs(&root, &config);
            let store = config.store.clone(); let bootstrap = config.profile.clone();
            let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
            seed(&mut config, &native, true, steps);
            let before = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(before.text.generation_revision(), steps);
            let (sender, receiver) = mpsc::channel();
            let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
            let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1001), &mut output);
            assert!(result.is_ok(), "{result:?}");
            human.join().unwrap(); let response = client.join().unwrap();
            let after = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(after.text.command(), before.text.command());
            assert_eq!(after.text.generation_revision(), 4); // no duplicate prefix
            assert_eq!(after.numerical.numerical.position, 4);
            assert_eq!(after.text.bytes().unwrap(), b"A");
            assert_eq!(after.text.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(after.publication.executions, if approve { 1 } else { 0 });
            assert_eq!(after.publication.control.ledger.reserved, 0);
            if approve {
                assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
                assert_eq!(ReleaseFrame::decode(&after.publication.payload).unwrap().message(), Some("A"));
            } else {
                assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
                assert!(after.publication.payload.is_empty());
                assert_eq!(after.publication.control.ledger.charged, 0);
            }
            assert_eq!(std::str::from_utf8(&output.bytes).unwrap().lines().count(), 1);
            assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
        }
    }
}

#[test]
fn native_recovery_rejects_changed_prompt_budget_length_and_generation_without_advancing() {
    for changed in 0..4 {
        let root = Root::new(); let mut config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config); let mut native = inputs(&root, &config);
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
        seed(&mut config, &native, true, 1);
        match changed {
            0 => native.request.prompt = b"ba".to_vec(),
            1 => native.request.generation.sampling_entries -= 1,
            2 => native.request.max_new_tokens += 1,
            _ => native.generation = 8,
        }
        let mut output = Vec::new();
        assert!(serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1001), &mut output).is_err());
        assert!(output.is_empty());
        let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(image.text.generation_revision(), 1);
        assert_eq!(image.numerical.numerical.position, 1);
        assert!(image.numerical.paused);
        assert_eq!(image.publication.executions, 0);
        assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
    }
}

// Produce a genuinely reviewed/published message before testing reply recovery.
fn publish(root: &Root) -> Vec<u8> {
    let config = configured(root); let native = inputs(root, &config);
    let (actor, reviewer) = profiles(root, &config);
    let (sender, receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), receiver);
    let human = reviewing(reviewer.clone(), config.store.clone(), config.profile.clone(), ReviewDecision::Approve);
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve(config, &actor, &reviewer, native, || ElapsedTick(1000), &mut output);
    assert!(result.is_ok(), "{result:?}");
    human.join().unwrap();
    assert!(matches!(client.join().unwrap().result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    output.bytes
}

#[test]
fn native_recovery_of_expired_published_request_preserves_document_and_never_republishes() {
    let root = Root::new(); let original = publish(&root);
    let mut config = configured(&root); let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    let before = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    // New-work paths would fail: the receipt path must not need either consumer.
    config.programs.clear(); std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), receiver);
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(20000), &mut output);
    assert!(result.is_ok(), "{result:?}");
    assert!(matches!(client.join().unwrap().result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(output.bytes, original); // includes original, now-expired deadline
    let after = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.numerical.numerical.position, before.numerical.numerical.position);
    assert!(after.numerical.paused); // receipt replay never resumes numerical work
    assert_eq!(after.publication.executions, 1);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
}

#[test]
fn native_recovery_source_loss_cannot_resume_or_publish_the_saved_prefix() {
    let root = Root::new(); let mut config = configured(&root);
    let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    seed(&mut config, &native, true, 1);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Vec::new();
    assert!(serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1001), &mut output).is_err());
    assert!(output.is_empty());
    let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(image.numerical.numerical.position, 1);
    assert_eq!(image.publication.executions, 0);
    assert!(image.publication.stop.is_some());
}

#[test]
fn native_recovery_never_invents_an_intent_or_restarts_held_or_limited_work() {
    for mode in ["missing", "held", "limited"] {
        let root = Root::new(); let mut config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config); write_inputs(&root);
        if mode == "held" {
            let path = root.0.join("monitor.json");
            let original = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, original.replace("3.0", "-3.0")).unwrap();
        }
        let mut native = recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap();
        if mode == "limited" { native.request.max_new_tokens = 1; }
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
        let steps = match mode { "held" => 1, "limited" => 3, _ => 0 };
        seed(&mut config, &native, mode != "missing", steps);
        let mut output = Vec::new();
        assert!(serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1001), &mut output).is_err());
        assert!(output.is_empty());
        assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap().executions, 0);
        let progress = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7);
        if mode == "missing" { assert!(matches!(progress, Err(JournalError::Contract(Error::Missing)))); }
        else { assert_eq!(progress.unwrap().text.generation_revision(), steps); }
        assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
    }
}

#[test]
fn native_recovery_failed_receipt_output_neither_stops_nor_reissues_the_effect() {
    struct BrokenOutput;
    impl Write for BrokenOutput {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Root::new(); let _original = publish(&root);
    let mut config = configured(&root); let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let before = FileOversight::read_publication(&store, &bootstrap).unwrap();
    config.programs.clear(); std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(20000), &mut BrokenOutput);
    assert!(result.unwrap_err().contains("source reference output failed"));
    let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(after.executions, 1); assert_eq!(after.payload, before.payload);
    assert_eq!(after.stop.is_some(), before.stop.is_some());
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
}

#[test]
fn native_recovery_cannot_skip_a_later_pending_intent_even_before_its_first_token() {
    let root = Root::new(); let mut config = configured(&root);
    let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
    seed(&mut config, &native, true, 4);
    let (mut host, _) = FileOversight::open_generated_text_stream_with_reserve(
        &config.store, config.profile.clone(), native.stream, &native.decoder,
        &native.tokenizer, RecoveryReserve::terminal()).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1001)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let next = FileTextGenerationCommand::new(8, numerical.actor_revision,
        numerical.position, native.request.clone()).unwrap();
    host.begin_decoder_text(host.revision(), next.clone()).unwrap();
    // The old result still has the exact numerical position; only the genuine
    // outstanding intent distinguishes this case from a valid recovery control.
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 4);
    assert!(recovery::select(&host, actor.request, &native).unwrap_err().contains("outstanding generation"));
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    drop(host);
    let mut output = Vec::new();
    assert!(serve_mode(config, &actor, &reviewer, native, true, || ElapsedTick(1002), &mut output).is_err());
    assert!(output.is_empty());
    let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 8).unwrap();
    assert_eq!(image.text.command(), &next);
    assert_eq!(image.text.generation_revision(), 0);
    assert_eq!(image.numerical.numerical.position, 4);
    assert_eq!(image.publication.executions, 0);
}
