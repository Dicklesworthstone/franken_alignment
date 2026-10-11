//! Source manifests reach the actual learned actor, native congress and journal.
#[path = "checkpoint_source/fixture.rs"]
mod source_assets;
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderModel;

#[test]
fn checkpoint_source_operator_adapted_generation_repack_recovery_native_and_human_keys() {
    for approve in [true, false] {
        let root = Root::new(); let mut config = source_assets::configured(&root);
        let assets = source_assets::write(&root);
        let (tokens, _logits) = assets.oracle(); assert_eq!(tokens, [65, 256]);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&assets.path, &config, true).unwrap();
        let original_generation = loaded.generation.clone();
        // A stale independently supplied host cannot adopt the final-gen2
        // source even though all model/codec/tokenizer assets admit correctly.
        let stale_host = configured(&root);
        assert_eq!(recovery::create(&stale_host, &loaded).err().unwrap(), "Contract(Binding)");
        assert!(!config.store.exists());
        let (mut host, _) = recovery::create(&config, &loaded).unwrap();
        for position in 0..3 {
            host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
            let before = host.learned_generation_inspection().unwrap().numerical;
            host.advance_learned_generation(host.revision(), before.actor_revision, position).unwrap().unwrap();
        }
        let before = host.learned_generation_inspection().unwrap();
        assert_eq!(before.numerical.position, 3); assert_eq!(before.numerical.sampled_draws, 1);
        assert!(host.learned_text_message(loaded.evidence).is_err());
        assert!(host.inspect().payload.is_empty()); assert_eq!(host.inspect().executions, 0);
        let epoch = host.inspect().control.ledger.epoch;
        drop(host);

        // Physical packaging can change while complete effective parameters,
        // tokenizer, codec and original numerical state remain exactly equal.
        assets.repack();
        let packed = recipe::load(&assets.path, &config, true).unwrap();
        assert_eq!(packed.generation, original_generation);
        let (host, _) = recovery::open(&config, &packed).unwrap();
        let recovered = host.learned_generation_inspection().unwrap();
        assert_eq!(recovered.numerical, before.numerical);
        assert_eq!(recovered.journal_revision, before.journal_revision + 1);
        assert_eq!(host.inspect().control.ledger.epoch, epoch + 1);
        assert!(recovered.paused && !host.clock_ready());
        assert!(host.learned_text_message(loaded.evidence).is_err());
        assert!(host.inspect().payload.is_empty()); assert_eq!(host.inspect().executions, 0);
        drop(host);
        // Successful recovery appends the original fence. The next refusal
        // must preserve this complete POST-fence journal byte for byte.
        let journal = fs::read(config.store.join("delivery.bin")).unwrap();

        // This one unused embedding changes neither selected prompt's logits
        // nor output. It still changes the effective model and must invalidate
        // recovery before any journal event or numerical continuation.
        let mut changed = assets.base.clone();
        source_assets::replace_value(&mut changed, "model.embed_tokens.weight", 0, 1.25);
        fs::write(&assets.single, changed).unwrap();
        let mut expected_changed = assets.effective_bytes.clone();
        source_assets::replace_value(&mut expected_changed, "model.embed_tokens.weight", 0, 1.25);
        let changed_oracle = DecoderModel::from_llama_safetensors(source_assets::final_identity(), 16,
            &fs::read(root.0.join("model.json")).unwrap(), &expected_changed).unwrap().0;
        assert_eq!(changed_oracle.recompute(5, &[120, 121], source_assets::read_budget()).unwrap().logits().unwrap(),
            assets.effective.recompute(5, &[120, 121], source_assets::read_budget()).unwrap().logits().unwrap());
        let changed = recipe::load(&assets.path, &config, true).unwrap();
        assert_ne!(changed.generation, original_generation);
        assert!(recovery::open(&config, &changed).is_err());
        assert!(fs::read(config.store.join("delivery.bin")).unwrap() == journal,
            "mismatched effective parameters changed the recovered journal");
        fs::write(&assets.single, &assets.base).unwrap();

        let loaded = recipe::load(&assets.path, &config, true).unwrap();
        let (host, reviewer) = recovery::open(&config, &loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000)).unwrap());
        let complete = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        assert_eq!(complete.numerical.position, 4); assert_eq!(complete.numerical.sampled_draws, 2);
        assert_eq!(complete.numerical.work.admitted_tokens, 4);
        assert!(complete.numerical.telemetry.compression_source_values > 0);
        assert_eq!(driver.supervisor().host().unwrap().learned_text_message(loaded.evidence).unwrap().bytes(), b"A");
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
            &deadline, &mut || ElapsedTick(1000));
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        let mut driver = reviewed.driver; let history = reviewed.history.unwrap(); assert_native(&history);
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
            701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
        if approve {
            let human = {
                let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
                reviewer.approve(&mut host, revision, &offer).unwrap()
            };
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
        } else {
            let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            reviewer.reject(&mut host, revision, &offer).unwrap();
            let revision = host.revision(); host.cancel_request(revision, 1).unwrap();
        }
        assert_eq!(executed(&mut wire, 1), approve);
        let state = driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.executions, u64::from(approve)); assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.payload, if approve { b"A".to_vec() } else { Vec::new() });
        assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
        let loaded = recipe::load(&assets.path, &config, false).unwrap();
        let result = recovery::resume(config, loaded, || panic!("receipt recovery must not consult time")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approve);
    }
}

#[test]
fn checkpoint_source_operator_closed_selection_keeps_old_versions_and_never_sniffs_weights() {
    let root = Root::new(); let config = source_assets::configured(&root); let assets = source_assets::write(&root);
    let original = fs::read_to_string(&assets.path).unwrap();
    for version in [1, 2] {
        fs::write(&assets.path, original.replace("fa.learned-publication/3", &format!("fa.learned-publication/{version}"))).unwrap();
        let error = recipe::load(&assets.path, &config, true).err().unwrap();
        assert!(error.contains("single weight path"), "{error}"); assert!(!config.store.exists());
    }
    for replacement in ["{\"format\":\"infer\",\"path\":\"/missing\"}",
        "{\"format\":\"checkpoint_source\",\"path\":\"/missing\",\"fallback\":true}"] {
        fs::write(&assets.path, original.replace(&source_assets::selection(&assets.manifest), replacement)).unwrap();
        let error = recipe::load(&assets.path, &config, true).err().unwrap();
        assert!(!error.contains("No such file"), "{error}"); assert!(!config.store.exists());
    }
    let direct = root.0.join("effective.bin"); fs::write(&direct, &assets.effective_bytes).unwrap();
    fs::write(&assets.path, original.replace(&source_assets::selection(&assets.manifest),
        &source_assets::quote(direct.to_str().unwrap()))).unwrap();
    let legacy_path = recipe::load(&assets.path, &config, true).unwrap();
    fs::write(&assets.path, &original).unwrap();
    let explicit = recipe::load(&assets.path, &config, true).unwrap();
    assert_eq!(legacy_path.generation, explicit.generation);
    fs::write(&assets.path, original.replace(&source_assets::selection(&assets.manifest), &source_assets::selection(&direct))).unwrap();
    assert!(recipe::load(&assets.path, &config, true).is_err()); assert!(!config.store.exists());

    fs::write(&assets.path, original).unwrap();
    let roster = root.0.join("native-roster.json"); let text = fs::read_to_string(&roster).unwrap();
    fs::write(&roster, text.replace("fa.learned-native-roster/2", "fa.learned-native-roster/1")).unwrap();
    let error = recipe::load(&assets.path, &config, true).err().unwrap();
    assert!(error.contains("single weight path"), "{error}"); assert!(!config.store.exists());
    fs::write(roster, text).unwrap();
    assert!(recipe::load(&assets.path, &config, true).is_ok());
}

#[test]
fn checkpoint_source_operator_preflight_and_one_common_weight_allowance_refuse_before_store() {
    for defect in 0..6 {
        let root = Root::new(); let config = source_assets::configured(&root); let assets = source_assets::write(&root);
        let original = fs::read_to_string(&assets.manifest).unwrap();
        let changed = match defect {
            0 => original.replace("\"model_generation\":1", "\"model_generation\":2"),
            1 => original.replace("\"merge_products\":4", "\"merge_products\":3"),
            2 => original.replace("\"weight_read_calls\":4096", "\"weight_read_calls\":1"),
            3 => {
                let total: u64 = assets.actor_shards.iter().chain(std::iter::once(&assets.adapter_weights))
                    .map(|path| fs::metadata(path).unwrap().len()).sum();
                original.replace("\"weight_bytes\":65536", &format!("\"weight_bytes\":{}", total - 1))
            }
            4 => original.replace("\"schema\":\"fa.checkpoint-weights/1\"", "\"schema\":\"fa.checkpoint-weights/2\""),
            _ => {
                let index = root.0.join("actor-index.json");
                let text = fs::read_to_string(&index).unwrap().replace("part-1.safetensors", "unregistered.safetensors");
                fs::write(index, text).unwrap(); original.clone()
            }
        };
        fs::write(&assets.manifest, changed).unwrap();
        if matches!(defect, 0 | 1 | 4 | 5) {
            for path in assets.actor_shards.iter().chain(std::iter::once(&assets.adapter_weights)) { fs::remove_file(path).unwrap(); }
        }
        let error = recipe::load(&assets.path, &config, true).err().unwrap();
        assert!(!config.store.exists());
        match defect {
            0 => assert!(error.contains("Identity"), "{error}"),
            1 => assert!(error.contains("Limit"), "{error}"),
            2 => assert!(error.contains("weight_read_calls: 1") && error.contains("merge_products: 0"), "{error}"),
            3 => assert!(error.contains("weight_bytes: 0") && error.contains("merge_products: 0"), "{error}"),
            4 => assert!(error.contains("unsupported checkpoint source schema"), "{error}"),
            _ => assert!(!error.contains("No such file"), "{error}"),
        }
    }
    let root = Root::new(); let config = source_assets::configured(&root); let assets = source_assets::write(&root);
    let original = fs::read_to_string(&assets.manifest).unwrap();
    let total: u64 = assets.actor_shards.iter().chain(std::iter::once(&assets.adapter_weights))
        .map(|path| fs::metadata(path).unwrap().len()).sum();
    fs::write(&assets.manifest, original.replace("\"weight_bytes\":65536", &format!("\"weight_bytes\":{total}"))).unwrap();
    assert!(recipe::load(&assets.path, &config, true).is_ok());
}

#[test]
fn checkpoint_source_operator_native_source_cannot_fall_back_when_later_helper_shard_is_absent() {
    let root = Root::new(); let config = source_assets::configured(&root); let assets = source_assets::write(&root);
    assert!(recipe::load(&assets.path, &config, true).is_ok());
    fs::remove_file(&assets.helper_shards[1]).unwrap();
    let error = recipe::load(&assets.path, &config, true).err().unwrap();
    assert!(error.contains("checkpoint work"), "{error}"); assert!(!config.store.exists());
    // Terminal recovery deliberately needs no new native source or fresh vote.
    let loaded = recipe::load(&assets.path, &config, false).unwrap(); assert!(loaded.native.is_none());
}
