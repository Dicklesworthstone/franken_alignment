//! Imported checkpoint/tokenizer files reach the original actor and effect path.
//! Synthetic weights and same-process peers exercise enforcement, not calibration.
use super::*;
use crate::workflow::actor_service::tokenizer::fixture::raw_byte_level_json;
use fa_reference::strict_json;
use std::fs;
use std::path::PathBuf;

mod split {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/config/sharded/tests/split.rs"));
}

const JSON_INPUT: &str = r#"{"format":"huggingface_raw_bytelevel","path":"tokenizer.json"}"#;
const NATIVE_INPUT: &str = r#"{"format":"native_archive","path":"tokenizer.bbpe"}"#;

fn write_recipe(root: &Root, sharded: bool, json: bool) -> PathBuf {
    write_inputs(root);
    let path = root.0.join("recipe.json");
    let mut recipe = fs::read_to_string(&path).unwrap();
    if sharded {
        let (index, sources) = split::split(&fs::read(root.0.join("model.safetensors")).unwrap());
        fs::write(root.0.join("index.json"), index).unwrap();
        for (label, bytes) in sources { fs::write(root.0.join(label), bytes).unwrap(); }
        recipe = recipe.replace("\"weights\":\"model.safetensors\"", concat!(
            "\"weights\":{\"index\":\"index.json\",\"shards\":{",
            "\"left.safetensors\":\"left.safetensors\",\"right.safetensors\":\"right.safetensors\"}}"));
        fs::remove_file(root.0.join("model.safetensors")).unwrap();
    }
    if json { fs::write(root.0.join("tokenizer.json"), raw_byte_level_json(true)).unwrap(); }
    recipe = recipe.replace("fa.native-text-service/1",
        if sharded { "fa.native-text-service/4" } else { "fa.native-text-service/3" })
        .replace("\"tokenizer\":\"tokenizer.bbpe\"",
            &format!("\"tokenizer\":{}", if json { JSON_INPUT } else { NATIVE_INPUT }));
    fs::write(&path, recipe).unwrap();
    path
}

#[test]
fn native_huggingface_tokenizer_reaches_both_keys_and_exact_receipt_recovery() {
    for sharded in [false, true] {
        for approve in [false, true] {
            let root = Root::new(); let config = configured(&root);
            let path = write_recipe(&root, sharded, true);
            let inputs = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            assert_eq!(inputs.tokenizer.encode(b"ab", Default::default()).unwrap().tokens(), &[257]);
            assert_eq!(inputs.tokenizer.special_tokens()[&256].as_slice(), b"<eos>");
            let (actor, reviewer) = profiles(&root, &config);
            let store = config.store.clone(); let profile = config.profile.clone();
            let decoder = inputs.decoder.clone(); let tokenizer = inputs.tokenizer.clone();
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), profile.clone(),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
            let mut original = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve(config, &actor, &reviewer, inputs, || ElapsedTick(1000), &mut original);
            assert!(result.is_ok(), "{result:?}"); human.join().unwrap();
            let response = client.join().unwrap();
            assert_eq!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approve);
            let before = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(before.text.bytes().unwrap(), b"A");
            assert_eq!(before.text.generation_revision(), 4);
            assert_eq!(before.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
            assert_eq!(before.publication.executions, u64::from(approve));
            assert_eq!(before.publication.control.ledger.reserved, 0);
            if !approve {
                assert!(before.publication.payload.is_empty());
                assert_eq!(before.publication.control.ledger.charged, 0);
                continue;
            }
            assert_eq!(before.publication.payload, b"A");

            // This changes a real tokenizer behavior even though this prompt and
            // result happen to be unaffected. Exact bootstrap must still refuse.
            let json_path = root.0.join("tokenizer.json");
            let original_json = fs::read_to_string(&json_path).unwrap();
            fs::write(&json_path, original_json.replace("<eos>", "<stop>")).unwrap();
            let config = configured(&root);
            let changed = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            let disk = fs::read(store.join("delivery.bin")).unwrap();
            let mut output = Vec::new();
            assert!(serve_mode(config, &actor, &reviewer, changed, true,
                || ElapsedTick(20000), &mut output).is_err());
            assert!(output.is_empty()); assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), disk);
            assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
            fs::write(json_path, original_json).unwrap();

            let mut config = configured(&root);
            let inputs = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            config.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let mut recovered = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_mode(config, &actor, &reviewer, inputs, true,
                || ElapsedTick(20000), &mut recovered);
            assert!(result.is_ok(), "{result:?}");
            assert!(matches!(client.join().unwrap().result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            assert_eq!(recovered.bytes, original.bytes);
            let after = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(after.text.generation_revision(), before.text.generation_revision());
            assert_eq!(after.numerical.numerical.position, before.numerical.numerical.position);
            assert!(after.numerical.paused);
            assert_eq!(after.publication.executions, 1);
            assert_eq!(after.publication.payload, before.publication.payload);
            assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
            assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
        }
    }
}

#[test]
fn native_huggingface_hold_never_exposes_generated_output_or_launches_helpers() {
    let root = Root::new(); let mut config = configured(&root);
    let path = write_recipe(&root, false, true);
    let monitor = root.0.join("monitor.json"); let source = fs::read_to_string(&monitor).unwrap();
    fs::write(monitor, source.replace("3.0", "-3.0")).unwrap();
    let inputs = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone(); let profile = config.profile.clone();
    config.programs.clear(); let mut output = Vec::new();
    assert!(serve(config, &actor, &reviewer, inputs, || ElapsedTick(1000), &mut output).is_err());
    assert!(output.is_empty());
    let image = FileOversight::read_publication(&store, &profile).unwrap();
    assert!(image.stop.is_some()); assert_eq!(image.executions, 0);
    assert!(image.payload.is_empty()); assert_eq!(image.control.ledger.charged, 0);
    assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
}

#[test]
fn native_huggingface_recipe_preserves_exact_input_limits_and_native_versions() {
    for sharded in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = write_recipe(&root, sharded, true);
        let mut names = vec!["recipe.json", "model.json", "monitor.json", "sampling.json", "tokenizer.json", "prompt.txt"];
        if sharded { names.extend(["index.json", "left.safetensors", "right.safetensors"]); }
        else { names.push("model.safetensors"); }
        let total = names.iter().map(|name| fs::metadata(root.0.join(name)).unwrap().len() as usize).sum();
        assert!(recipe::load(&path, 1, total).is_ok());
        assert!(recipe::load(&path, 1, total - 1).is_err());
        assert!(!config.store.exists());

        let path = write_recipe(&root, sharded, false);
        let explicit = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let old = source.replace(if sharded { "fa.native-text-service/4" } else { "fa.native-text-service/3" },
            if sharded { "fa.native-text-service/2" } else { "fa.native-text-service/1" })
            .replace(NATIVE_INPUT, "\"tokenizer.bbpe\"");
        fs::write(&path, old).unwrap();
        let legacy = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
        assert_eq!(explicit.decoder, legacy.decoder);
        assert_eq!(explicit.tokenizer.to_bytes().unwrap(), legacy.tokenizer.to_bytes().unwrap());
        // Filename extensions do not select a parser, nor does a failed parser
        // fall back: the explicit archive selection rejects these JSON bytes.
        fs::write(root.0.join("tokenizer.bbpe"), raw_byte_level_json(true)).unwrap();
        assert!(recipe::load(&path, 1, config.profile.delivery.limits.bytes).err().unwrap()
            .starts_with("tokenizer input refused:"));
    }
}

#[test]
fn native_huggingface_settings_refuse_before_any_checkpoint_weight_read() {
    for sharded in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = write_recipe(&root, sharded, true);
        fs::remove_file(root.0.join(if sharded { "index.json" } else { "model.safetensors" })).unwrap();
        let valid = raw_byte_level_json(true);
        for changed in [valid.replacen("\"use_regex\":false", "\"use_regex\":true", 1),
            valid.replace("\"normalizer\":null", "\"normalizer\":{\"type\":\"Lowercase\"}"),
            valid.replace("\"normalized\":false", "\"normalized\":true")]
        {
            fs::write(root.0.join("tokenizer.json"), changed).unwrap();
            let error = recipe::load(&path, 1, config.profile.delivery.limits.bytes).err().unwrap();
            assert_eq!(error, "tokenizer input refused: Binding");
            assert!(!config.store.exists());
        }
        fs::write(root.0.join("tokenizer.json"), valid).unwrap();
        assert!(!recipe::load(&path, 1, config.profile.delivery.limits.bytes).err().unwrap()
            .starts_with("tokenizer input refused:")); // matching tokenizer reaches missing weight input
    }
}

#[test]
fn native_huggingface_format_is_explicit_before_any_model_or_tokenizer_input() {
    let root = Root::new(); let config = configured(&root); let path = write_recipe(&root, false, true);
    let original = fs::read_to_string(&path).unwrap();
    fs::remove_file(root.0.join("model.json")).unwrap();
    fs::remove_file(root.0.join("tokenizer.json")).unwrap();
    for (from, to, expected) in [
        ("huggingface_raw_bytelevel", "auto", "unknown tokenizer format"),
        (JSON_INPUT, "\"tokenizer.json\"", "tokenizer input must be a format/path object"),
        (JSON_INPUT, r#"{"path":"tokenizer.json"}"#, "missing tokenizer format"),
        (JSON_INPUT, r#"{"format":"native_archive","path":"tokenizer.bbpe","fallback":true}"#, "unknown tokenizer input field"),
        ("fa.native-text-service/3", "fa.native-text-service/1", "tokenizer path must be text"),
    ] {
        fs::write(&path, original.replace(from, to)).unwrap();
        assert_eq!(recipe::load(&path, 1, config.profile.delivery.limits.bytes).err().unwrap(), expected);
        assert!(!config.store.exists());
    }
}

#[test]
fn native_huggingface_direct_owner_recovers_and_requires_both_keys() {
    use crate::workflow::actor_service::tokenizer::direct;
    for sharded in [false, true] {
        for approve in [false, true] {
            let root = Root::new(); let mut config = configured(&root);
            let path = write_recipe(&root, sharded, true);
            let inputs = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            let selected = bootstrap::selection(&config, &inputs, None).unwrap();
            let (mut host, old_reviewer) = bootstrap::prepare(&config, &inputs, selected, false).unwrap();
            direct::begin(&mut host, &mut config, inputs.generation, inputs.request.clone());
            let pending = host.pending_decoder_text().unwrap().unwrap();
            let progress = host.decoder_text_progress(inputs.generation).unwrap();
            let work = progress.numerical().partial().unwrap().work();
            let numerical = host.decoder_inspection().unwrap().numerical;
            drop(host);

            // Independently read all files again; recovery uses the actual native
            // service opener, not a test importer or a reconstructed cursor.
            let inputs = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            let (mut host, reviewer) = bootstrap::prepare(&config, &inputs, selected, true).unwrap();
            assert_eq!(host.pending_decoder_text().unwrap().unwrap(), pending);
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            let replayed = host.decoder_text_progress(inputs.generation).unwrap();
            assert_eq!(replayed.numerical().partial().unwrap().work(), work);
            assert_eq!(replayed.bytes().unwrap(), progress.bytes().unwrap());
            let request = direct::complete(&mut host, &mut config, (&reviewer, &old_reviewer),
                (inputs.generation, 91), approve);
            drop(host);

            let json_path = root.0.join("tokenizer.json");
            let original = fs::read_to_string(&json_path).unwrap();
            fs::write(&json_path, original.replace("<eos>", "<stop>")).unwrap();
            let changed = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            let disk = fs::read(config.store.join("delivery.bin")).unwrap();
            assert!(bootstrap::prepare(&config, &changed, selected, true).is_err());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
            fs::write(&json_path, original).unwrap();
            fs::remove_file(root.0.join("evidence.json")).unwrap();
            config.programs.clear();
            let exact = recipe::load(&path, 1, config.profile.delivery.limits.bytes).unwrap();
            let (mut host, _) = bootstrap::prepare(&config, &exact, selected, true).unwrap();
            direct::receipt(&mut host, &config, &request, approve);
        }
    }
}
