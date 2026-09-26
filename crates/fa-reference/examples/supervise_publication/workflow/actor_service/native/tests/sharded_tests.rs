//! Physical multi-file inputs through the original numerical and checked effect
//! owners. Synthetic model/helper/credential fixtures do not prove isolation.
use super::*;
use fa_reference::strict_json;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput, producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use std::path::PathBuf;

mod split {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/sharded/tests/split.rs"));
}
#[allow(dead_code)]
mod weights {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/tests/weights.rs"));
}
const WEIGHT_OBJECT: &str = r#"{"index":"index.json","shards":{"left.safetensors":"left.bin","right.safetensors":"right.bin"}}"#;

fn setup(root: &Root, config: &Config, tied: bool) -> (PathBuf, Inputs) {
    write_inputs(root);
    if tied {
        std::fs::write(root.0.join("model.safetensors"), weights::tied_weights(false, false)).unwrap();
        let path = root.0.join("model.json"); let json = std::fs::read_to_string(&path).unwrap();
        std::fs::write(path, json.replace("\"tie_word_embeddings\":false", "\"tie_word_embeddings\":true")).unwrap();
    }
    let path = root.0.join("recipe.json");
    let original = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
    let raw = std::fs::read(root.0.join("model.safetensors")).unwrap();
    let (index, sources) = split::split(&raw);
    std::fs::write(root.0.join("index.json"), index).unwrap();
    for (label, bytes) in sources {
        // Actual paths differ from index labels, demonstrating explicit mapping.
        let selected = if label == "left.safetensors" { "left.bin" } else { "right.bin" };
        std::fs::write(root.0.join(selected), bytes).unwrap();
    }
    let json = std::fs::read_to_string(&path).unwrap();
    let next = json.replace("fa.native-text-service/1", "fa.native-text-service/2")
        .replace("\"weights\":\"model.safetensors\"", &format!("\"weights\":{WEIGHT_OBJECT}"));
    assert_ne!(next, json); std::fs::write(&path, next).unwrap();
    std::fs::remove_file(root.0.join("model.safetensors")).unwrap(); // no fallback source remains
    (path, original)
}
fn observed(scope: Scope) -> FilePublicationInputs {
    let projection = ProjectionKey { source: 40, branch: scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key: projection, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(projection, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(1, 1, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, projection), DomainClosure::Closed(close)),
        vec![SnapshotEntry::new(0, 1, b"original".to_vec()).unwrap()], &frontiers).unwrap()), None)
}
fn publication(root: &Root, config: &Config) -> (PublicationProfile, FilePublicationProducer) {
    let s = config.profile.delivery.scope;
    let identity = PublicationProducerProfile { source: 91, scope: s, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity,
        observed(s), ElapsedTick(1000)).unwrap();
    let json = format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), s.tenant, s.principal, s.run, s.branch, s.authority);
    (PublicationProfile::decode(json.as_bytes()).unwrap(), producer)
}
fn seed(config: &mut Config, native: &Inputs, publication: &PublicationProfile) {
    let selected = publication.generated_profile(&config.profile, native.stream, Some(RecoveryReserve::terminal())).unwrap();
    let (mut host, _) = FileOversight::create_generated_text_stream_checked(&config.store,
        config.profile.clone(), native.decoder.clone(), native.tokenizer.clone(), selected).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let intent = FileTextGenerationCommand::new(native.generation, n.actor_revision, n.position, native.request.clone()).unwrap();
    host.begin_decoder_text(host.revision(), intent).unwrap();
    host.advance_decoder_text(host.revision(), native.generation, 0).unwrap();
    assert_eq!(host.decoder_generation_progress(native.generation).unwrap().generation_revision(), 1);
}
fn executed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn native_sharded_recipe_charges_all_physical_inputs_and_preserves_legacy_mode() {
    for tied in [false, true] {
        let root = Root::new(); let config = configured(&root); let (path, single) = setup(&root, &config, tied);
        let names = ["recipe.json", "model.json", "index.json", "left.bin", "right.bin",
            "monitor.json", "sampling.json", "tokenizer.bbpe", "prompt.txt"];
        let total = names.iter().map(|name| std::fs::metadata(root.0.join(name)).unwrap().len() as usize).sum();
        let native = recipe::load(&path, 1, total).unwrap();
        assert!(native.decoder.is_sharded()); assert!(!single.decoder.is_sharded());
        assert_ne!(native.decoder, single.decoder);
        assert_eq!(native.decoder.profile(), single.decoder.profile());
        assert_eq!(native.decoder.output_head(), single.decoder.output_head());
        assert_eq!(native.request, single.request);
        let physical: usize = ["index.json", "left.bin", "right.bin", "monitor.json", "sampling.json"]
            .iter().map(|name| std::fs::metadata(root.0.join(name)).unwrap().len() as usize).sum();
        assert_eq!(native.decoder.input_bytes(), physical);
        assert!(recipe::load(&path, 1, total - 1).is_err());
        assert!(!root.0.join("left.safetensors").exists()); assert!(!root.0.join("right.safetensors").exists());
        assert!(!config.store.exists());
    }
}

#[test]
fn native_sharded_schema_and_index_admission_precede_shard_io_without_fallback() {
    let root = Root::new(); let config = configured(&root); let (path, _) = setup(&root, &config, false);
    let original = std::fs::read_to_string(&path).unwrap(); let limit = config.profile.delivery.limits.bytes;
    for changed in [original.replace("fa.native-text-service/2", "fa.native-text-service/1"),
        original.replace(WEIGHT_OBJECT, "\"model.safetensors\""),
        original.replace(WEIGHT_OBJECT, r#"{"index":"index.json","shards":{}}"#),
        original.replace(WEIGHT_OBJECT, r#"{"index":"index.json","shards":{"left.safetensors":"left.bin"},"fallback":"model.safetensors"}"#),
        original.replace("\"left.safetensors\":\"left.bin\"", "\"left.safetensors\":\"left.bin\",\"left.safetensors\":\"absent.bin\"")] {
        std::fs::write(&path, changed).unwrap(); assert!(recipe::load(&path, 1, limit).is_err());
    }
    std::fs::write(&path, &original).unwrap();
    let index = root.0.join("index.json"); let valid = std::fs::read_to_string(&index).unwrap();
    std::fs::remove_file(root.0.join("left.bin")).unwrap();
    std::fs::write(&index, valid.replace("left.safetensors", "../left.safetensors")).unwrap();
    assert!(recipe::load(&path, 1, limit).err().unwrap().starts_with("native shard index/labels refused:"));
    std::fs::write(&index, valid).unwrap();
    // Near-identical valid index reaches the actual missing-file refusal instead.
    assert!(!recipe::load(&path, 1, limit).err().unwrap().starts_with("native shard index/labels refused:"));
    let extra = original.replace("\"right.safetensors\":\"right.bin\"",
        "\"right.safetensors\":\"right.bin\",\"extra.safetensors\":\"absent.bin\"");
    std::fs::write(&path, extra).unwrap();
    assert!(recipe::load(&path, 1, limit).err().unwrap().starts_with("native shard index/labels refused:"));
    assert!(!config.store.exists());
}

#[test]
fn native_sharded_checked_create_and_resume_keep_witnesses_and_independent_human_decisions() {
    for tied in [false, true] {
        for open in [false, true] {
            for approve in [false, true] {
                let root = Root::new(); let mut config = configured(&root);
                let (path, _) = setup(&root, &config, tied);
                let native = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
                let (actor, reviewer) = profiles(&root, &config); let (publication, _producer) = publication(&root, &config);
                if open { seed(&mut config, &native, &publication); }
                let store = config.store.clone(); let bootstrap = config.profile.clone();
                let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
                let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
                let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(),
                    if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
                let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
                let result = serve_selected(config, &actor, &reviewer, native, (open, Some(&publication)),
                    || ElapsedTick(1001), &mut output);
                assert!(result.is_ok(), "tied={tied} open={open} approve={approve}: {result:?}");
                human.join().unwrap(); assert_eq!(executed(&client.join().unwrap()), approve);
                let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
                assert_eq!(image.text.generation_revision(), 4); assert_eq!(image.numerical.numerical.position, 4);
                assert_eq!(image.text.bytes().unwrap(), b"A");
                assert_eq!(image.text.finish(), Some(Ok(GenerationFinish::StopToken)));
                assert_eq!(image.publication.executions, u64::from(approve));
                assert_eq!(image.publication.control.ledger.reserved, 0);
                if approve {
                    assert_eq!(ReleaseFrame::decode(&image.publication.payload).unwrap().message(), Some("A"));
                    assert_eq!(image.publication.control.ledger.charged, image.publication.payload.len() as u64);
                } else { assert!(image.publication.payload.is_empty()); assert_eq!(image.publication.control.ledger.charged, 0); }
                assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
            }
        }
    }
}

#[test]
fn native_sharded_receipt_recovery_pins_the_index_and_never_republishes() {
    let root = Root::new(); let config = configured(&root); let (path, _) = setup(&root, &config, true);
    let native = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
    let (actor, reviewer) = profiles(&root, &config); let (publication, producer) = publication(&root, &config);
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let human = reviewing(reviewer.clone(), store.clone(), bootstrap.clone(), ReviewDecision::Approve);
    let mut original = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (false, Some(&publication)),
        || ElapsedTick(1000), &mut original);
    assert!(result.is_ok(), "{result:?}"); human.join().unwrap(); assert!(executed(&client.join().unwrap()));
    drop(producer);
    // Semantically harmless index whitespace is still a different pinned input.
    let index = root.0.join("index.json"); let valid = std::fs::read(&index).unwrap();
    let mut changed = valid.clone(); changed.push(b' '); std::fs::write(&index, changed).unwrap();
    let config = configured(&root); let native = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
    let before = FileOversight::read_publication(&store, &bootstrap).unwrap(); let mut output = Vec::new();
    assert!(serve_selected(config, &actor, &reviewer, native, (true, Some(&publication)),
        || ElapsedTick(20000), &mut output).is_err()); assert!(output.is_empty());
    assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap().revision, before.revision);
    std::fs::write(index, valid).unwrap();
    let mut config = configured(&root); let native = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
    config.programs.clear(); std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
    let mut recovered = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_selected(config, &actor, &reviewer, native, (true, Some(&publication)),
        || ElapsedTick(20000), &mut recovered);
    assert!(result.is_ok(), "{result:?}"); assert!(executed(&client.join().unwrap()));
    assert_eq!(recovered.bytes, original.bytes); // original, now-expired deadline
    let image = FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(image.text.generation_revision(), 4); assert!(image.numerical.paused);
    assert_eq!(image.publication.executions, 1);
    assert_eq!(image.publication.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(image.publication.control.ledger.reserved, 0);
    assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
}

#[test]
fn native_sharded_held_and_limited_generations_never_emit_a_publication_reference() {
    for hold in [false, true] {
        let root = Root::new(); let config = configured(&root); let (path, _) = setup(&root, &config, true);
        if hold {
            let monitor = root.0.join("monitor.json"); let json = std::fs::read_to_string(&monitor).unwrap();
            std::fs::write(monitor, json.replace("3.0", "-3.0")).unwrap();
        } else {
            let json = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, json.replace("\"max_new_tokens\":2", "\"max_new_tokens\":1")).unwrap();
        }
        let native = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
        let (actor, reviewer) = profiles(&root, &config); let store = config.store.clone(); let bootstrap = config.profile.clone();
        let mut output = Vec::new();
        assert!(serve(config, &actor, &reviewer, native, || ElapsedTick(1000), &mut output).is_err());
        assert!(output.is_empty()); let image = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(image.executions, 0); assert!(image.payload.is_empty()); assert!(image.stop.is_some());
        assert_eq!(image.control.ledger.charged, 0); assert_eq!(image.control.ledger.reserved, 0);
        assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
    }
}
