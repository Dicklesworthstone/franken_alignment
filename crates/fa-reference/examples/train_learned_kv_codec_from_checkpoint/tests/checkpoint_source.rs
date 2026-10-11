use super::*;
#[path = "../../support/checkpoint_source_fixture.rs"]
#[allow(dead_code)]
mod source_fixture;

#[test]
fn checkpoint_sources_produce_original_codec_files_and_complete_monitors_with_causal_sampled_verdicts() {
    for (sharded, adapted) in [(false, false), (true, false), (false, true), (true, true)] {
        let root = Root::new(); let args = files(&root);
        let source = source_fixture::create(&root.0, Path::new(&args[1]), sharded, adapted);
        run(&source.invocation(&args), &mut Vec::new()).unwrap();
        let selected_args = source.selected(&args);
        let (plan, inputs) = assets::load_with_source(&selected_args, true).unwrap();
        let model = original::model(); let tokenizer = original::tokenizer(&model);
        let expected = fixture::independent(&model, &plan);
        assert_eq!(fs::read(&args[4]).unwrap(), expected.1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
        let (codec, _) = replay(&args);
        let log = rows(&args); let metadata = selected(&log, "checkpoint_source");
        assert_eq!(metadata.len(), 1);
        for (key, value) in [("base_files", if sharded { 2 } else { 1 }),
            ("adapter_parameters", if adapted { 48 } else { 0 }),
            ("updated_parameters", if adapted { 32 } else { 0 }),
            ("merge_products", if adapted { 64 } else { 0 })] {
            assert_eq!(metadata[0].get(key).unwrap().as_u64(), Some(value));
        }
        let work = selected(&log, "input_work");
        let auxiliary = fs::metadata(&args[0]).unwrap().len() + fs::metadata(&args[2]).unwrap().len()
            + source.auxiliary_bytes() as u64;
        assert_eq!(work[0].get("auxiliary_bytes").unwrap().as_u64(), Some(auxiliary));
        assert_eq!(work[0].get("weight_bytes").unwrap().as_u64(), Some(source.weight_bytes() as u64));

        // The same new files feed the actual monitor command; no fixture codec
        // or adapter merger is substituted between these two commands.
        let probe_args = monitor_args(&root);
        assert!(crate::probe_command::run_for_codec_test(&source.invocation(&probe_args), &mut Vec::new()).unwrap());
        let bytes = fs::read(&probe_args[7]).unwrap();
        let policy = original::campaign().policy(expected.0, &probe_fixture::settings()).unwrap();
        KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap()
            .verify(&model, &tokenizer, &policy).unwrap();
        let loaded = operator_monitor::MonitorInput::decode(&bytes).unwrap()
            .bind_with_tokenizer(&inputs.model, &tokenizer, codec.clone()).unwrap();
        for (seed, candidate) in [(4, 0), (5, 1)] {
            let spec = original::spec(seed);
            let mut oracle = model.recompute_sampled(2000, spec.prompt(), original::inference(), spec.sampling().clone()).unwrap();
            let choice = oracle.advance_sampled(1, SampleBudget { decoder: original::inference(),
                sampling: SamplingBudget { vocabulary: 260 } }).unwrap();
            assert_eq!(choice.choice.token, candidate);
            let mut generation = inputs.model.monitored_generation(2000, 2001, spec,
                loaded.policy.clone(), GenerationBudget::default()).unwrap();
            assert!(generation.advance(0).unwrap().audit().complete_quiet());
            let accepted = generation.accepted_cache_image().unwrap().encode().unwrap();
            let sampler = generation.sampler_state();
            let event = generation.advance(1).unwrap();
            if candidate == 0 {
                assert!(event.audit().complete_quiet()); assert_eq!(event.sample(), Some(&choice.choice));
                assert!(generation.advance(2).unwrap().audit().complete_quiet());
                assert_eq!(generation.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
                assert_eq!(generation.generated_tokens(), &[0, 256]);
            } else {
                assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
                assert!(event.accepted().is_none()); assert!(event.sample().is_none());
                assert_eq!(generation.accepted_cache_image().unwrap().encode().unwrap(), accepted);
                assert_eq!(generation.sampler_state(), sampler); assert!(generation.generated_tokens().is_empty());
            }
        }
        if adapted {
            source.remove_final_value_signal();
            let (_, changed) = assets::load_with_source(&selected_args, true).unwrap();
            assert!(operator_monitor::MonitorInput::decode(&bytes).unwrap()
                .bind_with_tokenizer(&changed.model, &tokenizer, codec).is_err());
            assert_eq!(fs::read(&probe_args[7]).unwrap(), bytes);
        }
    }
}

#[test]
fn codec_checkpoint_source_shares_exact_reader_and_merge_allowances_across_all_files() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    let selected_args = source.selected(&args);
    let (_, initial) = assets::load_with_source(&selected_args, true).unwrap();
    let physical = source.weight_bytes(); let calls = initial.work.weight_read_calls;
    source.set_number("weight_bytes", physical as u64); source.set_number("weight_read_calls", calls as u64);
    set_number(&args, "input_limits", "weight_bytes", physical as u64);
    set_number(&args, "input_limits", "weight_read_calls", calls as u64);
    let (_, exact) = assets::load_with_source(&selected_args, true).unwrap();
    assert_eq!((exact.work.weight_bytes, exact.work.weight_read_calls, exact.work.merge_products), (physical, calls, 64));
    let manifest = fs::read(&source.manifest).unwrap(); let plan = fs::read(&args[2]).unwrap();
    for (at_source, dimension) in [(true, "weight_bytes"), (false, "weight_bytes"),
        (true, "weight_read_calls"), (false, "weight_read_calls")] {
        fs::write(&source.manifest, &manifest).unwrap(); fs::write(&args[2], &plan).unwrap();
        let short = (if dimension == "weight_bytes" { physical - 1 } else { calls - 1 }) as u64;
        if at_source { source.set_number(dimension, short); }
        else { set_number(&args, "input_limits", dimension, short); }
        let error = assets::load_with_source(&selected_args, true).err().unwrap();
        let failure = error.downcast_ref::<checkpoint_source::CheckpointLoadError>().unwrap();
        if dimension == "weight_bytes" {
            assert_eq!((failure.work.weight_bytes, failure.work.weight_read_calls, failure.work.merge_products), (0, 0, 0));
        } else {
            assert!(failure.work.weight_bytes > 0);
            assert_eq!(failure.work.weight_read_calls, calls - 1); assert_eq!(failure.work.merge_products, 0);
        }
        assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    }
    fs::write(&source.manifest, &manifest).unwrap(); fs::write(&args[2], &plan).unwrap();
    source.set_number("merge_products", 63); source.remove_tensors();
    let error = run(&source.invocation(&args), &mut Vec::new()).unwrap_err().to_string();
    assert!(error.contains("Limit"), "{error}"); assert!(!error.contains("No such file"), "{error}");
    assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
}

#[test]
fn codec_manifest_auxiliary_bytes_are_charged_and_bad_source_metadata_precedes_tensor_access() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    // The plan contains its own byte allowance. Settle that decimal-width
    // dependency, then admit precisely its complete physical auxiliary files.
    for _ in 0..4 {
        let required = fs::metadata(&args[0]).unwrap().len() + fs::metadata(&args[2]).unwrap().len()
            + source.auxiliary_bytes() as u64;
        set_number(&args, "input_limits", "auxiliary_bytes", required);
    }
    let (_, exact) = assets::load_with_source(&source.selected(&args), true).unwrap();
    assert_eq!(exact.work.auxiliary_bytes as u64, fixture::number(&args, "input_limits", "auxiliary_bytes"));
    let required = exact.work.auxiliary_bytes as u64;
    set_number(&args, "input_limits", "auxiliary_bytes", required - 1);
    source.remove_tensors();
    let error = run(&source.invocation(&args), &mut Vec::new()).unwrap_err().to_string();
    assert!(!error.contains("No such file"), "{error}");
    assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));

    for cause in ["index", "identity", "adapter", "training"] {
        let root = Root::new(); let args = files(&root);
        let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true); source.remove_tensors();
        match cause {
            "index" => {
                let path = source.index.as_ref().unwrap(); let text = fs::read_to_string(path).unwrap();
                fs::write(path, text.replace("part-a.safetensors", "unregistered.safetensors")).unwrap();
            }
            "identity" => source.replace("\"model_generation\":2", "\"model_generation\":3"),
            "adapter" => {
                let path = source.adapter_configuration.as_ref().unwrap();
                let text = fs::read_to_string(path).unwrap();
                fs::write(path, text.replace("\"inference_mode\":true", "\"inference_mode\":false")).unwrap();
            }
            "training" => set_number(&args, "budget", "decoder_products", 1),
            _ => unreachable!(),
        }
        let error = run(&source.invocation(&args), &mut Vec::new()).unwrap_err().to_string();
        assert!(!error.contains("No such file"), "{cause}: {error}");
        assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    }
}

#[test]
fn source_selection_is_explicit_and_the_original_codec_invocation_retains_its_meaning() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    assert!(run(&source.selected(&args), &mut Vec::new()).is_err());
    assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    let mut wrong = vec![OsString::from("--guess-checkpoint")]; wrong.extend(args.clone());
    assert!(run(&wrong, &mut Vec::new()).unwrap_err().to_string().contains("train_learned_kv_codec_from_checkpoint"));
    run(&args, &mut Vec::new()).unwrap();
    assert!(selected(&rows(&args), "checkpoint_source").is_empty());
    let (plan, _) = assets::load(&args).unwrap();
    assert_eq!(fs::read(&args[4]).unwrap(), fixture::independent(&original::model(), &plan).1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
}
