use super::*;
use super::super::tests::fixture::*;
use std::fs;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::OutputHead;
#[allow(dead_code)]
mod tied_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/tests/weights.rs"));
}

#[test]
fn generated_publication_recipe_loads_real_files_and_binds_every_native_configuration() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root);
    let loaded = load(&path, &config).unwrap();
    assert_eq!(loaded.text.prompt, b"x"); assert_eq!(loaded.request, 1); assert_eq!(loaded.generation, 7);
    assert_eq!(loaded.decoder.profile(), &model_profile());
    assert_eq!(loaded.tokenizer.to_bytes().unwrap(), tokenizer().to_bytes().unwrap());
    assert!(!config.store.exists()); // loading has no live owner, output or key
    let monitor = root.0.join("monitor.json");
    let old = fs::read(&monitor).unwrap();
    fs::write(&monitor, String::from_utf8(old).unwrap().replace("\"model\":9", "\"model\":10")).unwrap();
    assert!(load(&path, &config).is_err()); assert!(!config.store.exists());
}

#[test]
fn generated_publication_recipe_refuses_unknown_duplicate_oversized_and_malformed_fields() {
    let root = Root::new(); let path = write_recipe(&root); let text = fs::read_to_string(path).unwrap();
    assert!(Recipe::decode(text.as_bytes()).is_ok());
    for changed in [text.replacen("\"schema\":", "\"extra\":true,\"schema\":", 1),
        text.replacen("\"request\":1", "\"request\":1,\"request\":1", 1),
        text.replace("\"request\":1", "\"request\":0"),
        text.replace("\"max_new_tokens\":2", "\"max_new_tokens\":4097"),
        text.replace("\"stop_tokens\":[256]", "\"stop_tokens\":[]"),
        text.replace("\"prefix_controls\":[256]", "\"prefix_controls\":[4294967296]"),
        text.replace("\"scalar_products\":1000000", "\"scalar_products\":-1"),
        text.replace("/model.json", "/../model.json"),
        text.replace("\"ttl_ms\":100000", "\"ttl_ms\":3600001")]
    { assert!(Recipe::decode(changed.as_bytes()).is_err()); }
    assert!(Recipe::decode(&vec![b' '; MAX_RECIPE_BYTES + 1]).is_err());
    assert!(Recipe::decode(&text.as_bytes()[..text.len() - 1]).is_err());
}

#[test]
fn generated_publication_recipe_rejects_foreign_models_and_requires_actual_tied_weights() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root);
    let original = fs::read_to_string(&path).unwrap();
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    let foreign = original.replace("\"tenant\":1", "\"tenant\":2");
    fs::write(&path, foreign).unwrap();
    assert!(matches!(load(&path, &config), Err(error) if error.contains("tenant")));
    fs::write(&path, original).unwrap();
    let model = root.0.join("model.json"); let original = fs::read_to_string(&model).unwrap();
    fs::write(model, original.replacen('{', "{\"tie_word_embeddings\":true,", 1)).unwrap();
    let error = load(&path, &config).err().expect("missing weight file cannot become a model");
    assert!(!error.contains("independent output head"));
    fs::write(root.0.join("weights.safetensors"), tied_fixture::tied_weights_for(257, 120, false, false)).unwrap();
    assert_eq!(load(&path, &config).unwrap().decoder.output_head(), OutputHead::TiedEmbeddings);
    assert!(!config.store.exists());
}

#[test]
fn generated_tied_recipe_runs_original_monitored_inference_without_publishing() {
    use fa_reference::action::ElapsedTick;
    use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
    use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve, observed::FileOversight};
    for stored_head in [false, true] {
        let root = Root::new(); let mut config = configured(&root); let path = write_recipe(&root);
        let model_path = root.0.join("model.json"); let model = fs::read_to_string(&model_path).unwrap();
        fs::write(model_path, model.replacen('{', "{\"tie_word_embeddings\":true,", 1)).unwrap();
        fs::write(root.0.join("weights.safetensors"), tied_fixture::tied_weights_for(257, 120, stored_head, false)).unwrap();
        let loaded = load(&path, &config).unwrap();
        assert_eq!(loaded.decoder.output_head(), OutputHead::TiedEmbeddings);
        let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(&config.store,
            config.profile.clone(), loaded.stream, loaded.decoder.clone(), loaded.tokenizer.clone(),
            RecoveryReserve::terminal()).unwrap();
        host.enable_file_source(host.revision(), config.source_policy).unwrap();
        host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let command = FileTextGenerationCommand::new(loaded.generation, numerical.actor_revision,
            numerical.position, loaded.text.clone()).unwrap();
        let result = host.generate_decoder_text(host.revision(), command).unwrap();
        assert_eq!(result.result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(result.result().unwrap().generation().finish(), GenerationFinish::StopToken);
        assert_eq!(result.result().unwrap().generation().work().attempted_samples, 2);
        assert_eq!(host.inspect().executions, 0); // computation is not publication
        fs::write(root.0.join("weights.safetensors"), tied_fixture::tied_weights_for(257, 120, true, true)).unwrap();
        assert!(load(&path, &config).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}
