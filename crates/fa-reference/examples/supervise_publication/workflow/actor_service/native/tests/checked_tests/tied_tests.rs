//! The native service consumes real tied weights through its existing checked
//! workflow. Model and helper quality remain outside these synthetic fixtures.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::OutputHead;
mod tied_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/tests/weights.rs"));
}
fn tied_inputs(root: &Root, config: &Config, stored_head: bool) -> Inputs {
    write_inputs(root);
    let path = root.0.join("model.json");
    let model = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, model.replace("\"tie_word_embeddings\":false", "\"tie_word_embeddings\":true")).unwrap();
    std::fs::write(root.0.join("model.safetensors"), tied_fixture::tied_weights(stored_head, false)).unwrap();
    let native = recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap();
    assert_eq!(native.decoder.output_head(), OutputHead::TiedEmbeddings);
    native
}

#[test]
fn native_tied_recipe_requires_explicit_tying_equal_heads_and_the_original_input_bound() {
    for stored_head in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let native = tied_inputs(&root, &config, stored_head);
        let names = ["recipe.json", "model.json", "model.safetensors", "monitor.json", "sampling.json", "tokenizer.bbpe", "prompt.txt"];
        let total: usize = names.iter().map(|name| std::fs::metadata(root.0.join(name)).unwrap().len() as usize).sum();
        let path = root.0.join("recipe.json");
        assert_eq!(recipe::load(&path, 1, total).unwrap().decoder, native.decoder);
        assert!(recipe::load(&path, 1, total - 1).is_err());
        let model_path = root.0.join("model.json"); let model = std::fs::read_to_string(&model_path).unwrap();
        std::fs::write(&model_path, model.replace("\"tie_word_embeddings\":true", "\"tie_word_embeddings\":false")).unwrap();
        let independent = recipe::load(&path, 1, total + 1);
        assert_eq!(independent.is_ok(), stored_head);
        if let Ok(independent) = independent {
            assert_eq!(independent.decoder.output_head(), OutputHead::Independent);
            assert_ne!(independent.decoder, native.decoder);
        }
        std::fs::write(&model_path, model).unwrap();
        std::fs::write(root.0.join("model.safetensors"), tied_fixture::tied_weights(true, true)).unwrap();
        assert!(recipe::load(&path, 1, config.profile.delivery.limits.bytes).is_err());
        assert!(!config.store.exists());
    }
}

#[test]
fn native_tied_checked_creation_and_recovery_still_require_independent_human_approval() {
    for steps in [None, Some(1), Some(4)] {
        for approve in [true, false] {
            let root = Root::new(); let mut config = configured(&root);
            let native = tied_inputs(&root, &config, false);
            let (actor, reviewer) = profiles(&root, &config);
            let (publication, _producer) = publication(&root, &config, false);
            if let Some(steps) = steps { seed(&mut config, &native, &publication, steps); }
            let store = config.store.clone(); let bootstrap = config.profile.clone();
            let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
            let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_selected(config, &actor, &reviewer, native,
                (steps.is_some(), Some(&publication)), || ElapsedTick(1001), &mut output);
            assert!(result.is_ok(), "steps={steps:?}, approve={approve}: {result:?}");
            human.join().unwrap(); assert_eq!(executed(&client.join().unwrap()), approve);
            let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(image.text.generation_revision(), 4);
            assert_eq!(image.numerical.numerical.position, 4);
            assert_eq!(image.text.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(image.text.bytes().unwrap(), b"A");
            assert_eq!(image.publication.executions, u64::from(approve));
            assert_eq!(image.publication.control.ledger.reserved, 0);
            if approve {
                assert_eq!(ReleaseFrame::decode(&image.publication.payload).unwrap().message(), Some("A"));
                assert_eq!(image.publication.control.ledger.charged, image.publication.payload.len() as u64);
            } else {
                assert!(image.publication.payload.is_empty()); assert_eq!(image.publication.control.ledger.charged, 0);
            }
            assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
        }
    }
}

#[test]
fn native_tied_receipt_recovery_never_republishes_and_refuses_mode_only_substitution() {
    let root = Root::new(); let config = configured(&root);
    let native = tied_inputs(&root, &config, true); // equal physical matrices permit both imports
    let (actor, reviewer) = profiles(&root, &config);
    let (publication, producer) = publication(&root, &config, false);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(), ReviewDecision::Approve);
    let mut original = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (false, Some(&publication)),
        || ElapsedTick(1000), &mut original);
    assert!(result.is_ok(), "{result:?}"); human.join().unwrap(); assert!(executed(&client.join().unwrap()));
    drop(producer);
    let mut config = configured(&root);
    let native = recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap();
    config.programs.clear();
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let mut recovered = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (true, Some(&publication)),
        || ElapsedTick(20000), &mut recovered);
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&client.join().unwrap()));
    assert_eq!(recovered.bytes, original.bytes);
    let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(image.text.generation_revision(), 4); assert!(image.numerical.paused);
    assert_eq!(image.publication.executions, 1); assert_eq!(image.publication.control.ledger.reserved, 0);

    // Same exact weights, tokenizer, prompt and output, but a different declared
    // model contract: this must refuse before recovery mutates the journal.
    let model_path = root.0.join("model.json"); let model = std::fs::read_to_string(&model_path).unwrap();
    std::fs::write(model_path, model.replace("\"tie_word_embeddings\":true", "\"tie_word_embeddings\":false")).unwrap();
    let config = configured(&root);
    let changed = recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap();
    assert_eq!(changed.decoder.output_head(), OutputHead::Independent);
    let mut output = Vec::new();
    assert!(serve_selected(config, &actor, &reviewer, changed, (true, Some(&publication)),
        || ElapsedTick(20001), &mut output).is_err());
    assert!(output.is_empty());
    let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(after.revision, image.publication.revision);
    assert_eq!(after.executions, 1); assert_eq!(after.payload, image.publication.payload);
    assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
}

#[test]
fn native_tied_held_and_limited_generations_never_emit_publication_references() {
    for held in [true, false] {
        let root = Root::new(); let config = configured(&root);
        let _native = tied_inputs(&root, &config, false);
        if held {
            let path = root.0.join("monitor.json"); let monitor = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, monitor.replace("3.0", "-3.0")).unwrap();
        } else {
            let path = root.0.join("recipe.json"); let recipe = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, recipe.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1")).unwrap();
        }
        let native = recipe::load(&root.0.join("recipe.json"), 1, config.profile.delivery.limits.bytes).unwrap();
        let (actor, reviewer) = profiles(&root, &config);
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let mut output = Vec::new();
        assert!(serve_mode(config, &actor, &reviewer, native, false,
            || ElapsedTick(1000), &mut output).is_err());
        assert!(output.is_empty());
        let image = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(image.executions, 0); assert_eq!(image.control.ledger.charged, 0);
        assert!(image.stop.is_some()); assert!(image.payload.is_empty());
        assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
    }
}
