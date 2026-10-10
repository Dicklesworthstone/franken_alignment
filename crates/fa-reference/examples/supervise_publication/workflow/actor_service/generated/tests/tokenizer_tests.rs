//! Explicit tokenizer.json on the actual create-generated and receipt workflows.
use super::*;
use crate::workflow::actor_service::tokenizer::fixture::raw_byte_level_json;
use std::path::PathBuf;

fn json_recipe(root: &Root) -> PathBuf {
    let path = write_recipe(root); let source = fs::read_to_string(&path).unwrap();
    fs::write(root.0.join("tokenizer.json"), raw_byte_level_json(false)).unwrap();
    let source = source.replace("fa.generated-publication/1", "fa.generated-publication/2")
        .replace(&format!("\"tokenizer\":\"{}/tokenizer.bbpe\"", root.0.display()),
            &format!(r#""tokenizer":{{"format":"huggingface_raw_bytelevel","path":"{}/tokenizer.json"}}"#,
                root.0.display()));
    fs::write(&path, source).unwrap(); path
}

#[test]
fn generated_huggingface_recipe_publishes_only_with_both_keys_and_recovers_exactly() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let config = configured(&root); let peers = peers(&root, &config);
        let path = json_recipe(&root); let loaded = recipe::load(&path, &config).unwrap();
        assert_eq!(loaded.tokenizer.special_tokens()[&256].as_slice(), b"<eos>");
        assert_eq!(loaded.tokenizer.encode(b"x", Default::default()).unwrap().tokens(), &[120]);
        let decoder = loaded.decoder.clone(); let tokenizer = loaded.tokenizer.clone();
        let store = config.store.clone(); let profile = config.profile.clone();
        let human = review(peers.clone(), decision);
        let result = create(config, loaded, &peers, || ElapsedTick(1000)).unwrap();
        let packet = human.join().unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(executed(&result), decision == ReviewDecision::Approve);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let before = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(before.text.bytes().unwrap(), b"A");
        assert_eq!(before.text.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(before.text.generation_revision(), 4);
        assert_eq!(before.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
        assert_eq!(before.publication.control.ledger.reserved, 0);
        if decision == ReviewDecision::Reject {
            assert_eq!(before.publication.executions, 0); assert!(before.publication.payload.is_empty());
            assert_eq!(before.publication.control.ledger.charged, 0);
            continue;
        }
        assert_eq!(before.publication.executions, 1);
        assert_eq!(before.publication.control.ledger.charged, packet.action().spec().payload.len() as u64);
        let json = root.0.join("tokenizer.json"); let original = fs::read_to_string(&json).unwrap();
        fs::write(&json, original.replace("<eos>", "<stop>")).unwrap();
        let config = configured(&root); let altered = recipe::load(&path, &config).unwrap();
        let disk = fs::read(store.join("delivery.bin")).unwrap();
        assert!(recovery::resume(config, altered, || panic!("mismatched tokenizer must precede clock work")).is_err());
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), disk);
        fs::write(json, original).unwrap();

        let mut config = configured(&root); let same = recipe::load(&path, &config).unwrap();
        config.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
        let recovered = recovery::resume(config, same, || ElapsedTick(20000)).unwrap();
        assert!(recovered.failure.is_none(), "{:?}", recovered.failure);
        assert!(executed(&recovered)); assert_eq!(recovered.cleanup_pending, 0);
        let after = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(after.text.generation_revision(), before.text.generation_revision());
        assert_eq!(after.numerical.numerical.position, before.numerical.numerical.position);
        assert!(after.numerical.paused);
        assert_eq!(after.publication.executions, 1);
        assert_eq!(after.publication.payload, before.publication.payload);
        assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    }
}

#[test]
fn generated_huggingface_hold_retains_no_publication_or_partial_output() {
    let root = Root::new(); let mut config = configured(&root); let peers = peers(&root, &config);
    let path = json_recipe(&root); fs::write(root.0.join("monitor.json"), monitor(true)).unwrap();
    let loaded = recipe::load(&path, &config).unwrap();
    let store = config.store.clone(); let profile = config.profile.clone();
    config.programs.clear();
    let result = create(config, loaded, &peers, || ElapsedTick(1000)).unwrap();
    assert!(result.failure.as_deref().unwrap().contains("native generation did not complete"));
    assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let mut output = Vec::new(); assert!(emit(result, &mut output).is_err());
    assert!(!output.windows(b"payload".len()).any(|part| part == b"payload"));
    let image = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(image.executions, 0); assert!(image.payload.is_empty());
    assert_eq!(image.control.ledger.charged, 0); assert_eq!(image.control.ledger.reserved, 0);
    assert!(!peers.socket(1).exists());
}

#[test]
fn generated_huggingface_format_settings_and_paths_refuse_before_weight_or_store_io() {
    let root = Root::new(); let config = configured(&root); let path = json_recipe(&root);
    let source = fs::read_to_string(&path).unwrap();
    let valid = raw_byte_level_json(false);
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    for changed in [valid.replacen("\"use_regex\":false", "\"use_regex\":true", 1),
        valid.replace("\"normalizer\":null", "\"normalizer\":{\"type\":\"Lowercase\"}"),
        valid.replace("\"normalized\":false", "\"normalized\":true")]
    {
        fs::write(root.0.join("tokenizer.json"), changed).unwrap();
        assert_eq!(recipe::load(&path, &config).err().unwrap(), "tokenizer input refused: Binding");
        assert!(!config.store.exists());
    }
    fs::write(root.0.join("tokenizer.json"), valid).unwrap();
    assert!(!recipe::load(&path, &config).err().unwrap().starts_with("tokenizer input refused:"));
    fs::remove_file(root.0.join("model.json")).unwrap();
    for (from, to, expected) in [
        ("huggingface_raw_bytelevel", "auto", "unknown tokenizer format"),
        ("fa.generated-publication/2", "fa.generated-publication/1", "tokenizer path must be text"),
        ("/tokenizer.json", "/../tokenizer.json", "tokenizer must be an absolute normalized file path"),
        ("\"format\":\"huggingface_raw_bytelevel\",", "", "missing tokenizer format"),
    ] {
        fs::write(&path, source.replace(from, to)).unwrap();
        assert_eq!(recipe::load(&path, &config).err().unwrap(), expected);
        assert!(!config.store.exists());
    }
}

#[test]
fn generated_explicit_native_tokenizer_preserves_legacy_binding_without_format_fallback() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root);
    let legacy = recipe::load(&path, &config).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let source = source.replace("fa.generated-publication/1", "fa.generated-publication/2")
        .replace(&format!("\"tokenizer\":\"{}/tokenizer.bbpe\"", root.0.display()),
            &format!(r#""tokenizer":{{"format":"native_archive","path":"{}/tokenizer.bbpe"}}"#, root.0.display()));
    fs::write(&path, &source).unwrap();
    let explicit = recipe::load(&path, &config).unwrap();
    assert_eq!(explicit.decoder, legacy.decoder);
    assert_eq!(explicit.tokenizer.to_bytes().unwrap(), legacy.tokenizer.to_bytes().unwrap());
    assert_eq!(explicit.text, legacy.text);
    fs::write(root.0.join("tokenizer.bbpe"), raw_byte_level_json(false)).unwrap();
    assert!(recipe::load(&path, &config).err().unwrap().starts_with("tokenizer input refused:"));
    fs::write(&path, source.replace("native_archive", "huggingface_raw_bytelevel")).unwrap();
    assert!(recipe::load(&path, &config).is_ok()); // declared format, despite the .bbpe filename
    assert!(!config.store.exists());
}

#[test]
fn generated_huggingface_direct_owner_recovers_and_requires_both_keys() {
    use crate::workflow::actor_service::tokenizer::direct;
    for approve in [false, true] {
        let root = Root::new(); let mut config = configured(&root);
        let path = json_recipe(&root); let loaded = recipe::load(&path, &config).unwrap();
        // Exactly the original constructor and source enablement selected by
        // create-generated, without the separately tested socket transport.
        let (mut host, old_reviewer) = FileOversight::create_generated_text_stream_with_reserve(
            &config.store, config.profile.clone(), loaded.stream, loaded.decoder.clone(),
            loaded.tokenizer.clone(), RecoveryReserve::terminal()).unwrap();
        host.enable_file_source(host.revision(), config.source_policy).unwrap();
        direct::begin(&mut host, &mut config, loaded.generation, loaded.text.clone());
        let pending = host.pending_decoder_text().unwrap().unwrap();
        let progress = host.decoder_text_progress(loaded.generation).unwrap();
        let work = progress.numerical().partial().unwrap().work();
        let numerical = host.decoder_inspection().unwrap().numerical;
        drop(host);

        let loaded = recipe::load(&path, &config).unwrap();
        let (mut host, reviewer) = recovery::open(&config, &loaded).unwrap();
        assert_eq!(host.pending_decoder_text().unwrap().unwrap(), pending);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        let replayed = host.decoder_text_progress(loaded.generation).unwrap();
        assert_eq!(replayed.numerical().partial().unwrap().work(), work);
        assert_eq!(replayed.bytes().unwrap(), progress.bytes().unwrap());
        let request = direct::complete(&mut host, &mut config, (&reviewer, &old_reviewer),
            (loaded.generation, loaded.request), approve);
        drop(host);

        let json = root.0.join("tokenizer.json"); let original = fs::read_to_string(&json).unwrap();
        fs::write(&json, original.replace("<eos>", "<stop>")).unwrap();
        let changed = recipe::load(&path, &config).unwrap();
        let disk = fs::read(config.store.join("delivery.bin")).unwrap();
        assert!(recovery::open(&config, &changed).is_err());
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        fs::write(&json, original).unwrap();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        config.programs.clear();
        let exact = recipe::load(&path, &config).unwrap();
        let (mut host, _) = recovery::open(&config, &exact).unwrap();
        direct::receipt(&mut host, &config, &request, approve);
    }
}
