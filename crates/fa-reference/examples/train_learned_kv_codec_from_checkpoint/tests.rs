use super::*;
use fa_reference::action::consequence::activation::{
    monitor::MonitorOutcome,
    probe::training::decoder::kv::{KvMonitorArtifact, MAX_KV_MONITOR_ARTIFACT_BYTES},
    tensor::kv::{decoder::{DecoderModel, sampling::{SampleBudget, SamplingBudget,
        monitored::{GenerationBudget, GenerationStatus, GenerationStop}}},
        model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
            replay::{LearnedKvFitCheckpoint, archive::{LearnedKvFitArchive, MAX_FIT_ARCHIVE_BYTES},
                training::{CodecTrainingBudget, CodecTrainingInput}}}}};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fs;

#[path = "../../tests/support/bound_kv_monitor.rs"]
#[allow(dead_code)]
mod original;
#[path = "../train_learned_kv_probes_from_checkpoint/fixture.rs"]
#[allow(dead_code)]
mod probe_fixture;
#[path = "../supervise_publication/workflow/actor_service/learned/monitor.rs"]
#[allow(dead_code)]
mod operator_monitor;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::{Root, files, monitor_args, rows, selected, replace, set_number};

fn replay(args: &[OsString]) -> (LearnedKvCodec, LearnedKvFitCheckpoint) {
    let binding = fit::binding(&fs::read(&args[3]).unwrap()).unwrap();
    let archive = LearnedKvFitArchive::decode(&fs::read(&args[4]).unwrap(), &binding, MAX_FIT_ARCHIVE_BYTES).unwrap();
    archive.replay(binding.budget).unwrap()
}

#[test]
fn codec_files_drive_the_actual_probe_command_and_operator_with_independent_quiet_hold_oracles() {
    let root = Root::new(); let args = files(&root); let mut output = Vec::new();
    run(&args, &mut output).unwrap();
    let (codec, checkpoint) = replay(&args);
    let (plan, inputs) = assets::load(&args).unwrap();
    let model = original::model(); let tokenizer = original::tokenizer(&model);
    let expected = fixture::independent(&model, &plan);
    assert_eq!(fs::read(&args[4]).unwrap(), expected.1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
    assert_eq!(checkpoint.binding(), expected.1.binding());
    let log = rows(&args);
    assert_eq!(selected(&log, "planned_document").len(), 1);
    assert_eq!(selected(&log, "document_captured").len(), 1);
    assert_eq!(selected(&log, "fit_group").len(), 4);
    assert_eq!(selected(&log, "training_outcome")[0].get("status").unwrap().as_str(), Some("fitted"));
    let fitted = selected(&log, "fit_completed_work");
    for (field, value) in [("training_values", inputs.estimate.fit.source_values as u64),
        ("parameter_values", inputs.estimate.fit.parameter_values as u64),
        ("scratch_values_reserved", inputs.estimate.fit.scratch_values as u64),
        ("work_units_reserved", inputs.estimate.fit.work_units)] {
        assert_eq!(fitted[0].get(field).unwrap().as_u64(), Some(value));
    }
    let work = selected(&log, "input_work");
    assert_eq!(work[0].get("auxiliary_bytes").unwrap().as_u64(), Some(fs::metadata(&args[0]).unwrap().len() + fs::metadata(&args[2]).unwrap().len()));
    assert_eq!(work[0].get("weight_bytes").unwrap().as_u64(), Some(fs::metadata(&args[1]).unwrap().len()));
    assert!(selected(&log, "codec_artifacts").last().unwrap().get("saved").unwrap().as_bool().unwrap());

    // These are precisely the files just created by the new command, with no
    // regenerated codec fixture or alternate campaign implementation between.
    let probe_args = monitor_args(&root);
    assert!(crate::probe_command::run_for_codec_test(&probe_args, &mut Vec::new()).unwrap());
    let bytes = fs::read(&probe_args[7]).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let expected_policy = original::campaign().policy(expected.0, &probe_fixture::settings()).unwrap();
    artifact.verify(&model, &tokenizer, &expected_policy).unwrap();
    let loaded = operator_monitor::MonitorInput::decode(&bytes).unwrap()
        .bind_with_tokenizer(&model, &tokenizer, codec).unwrap();
    for (seed, candidate) in [(4, 0), (5, 1)] {
        let spec = original::spec(seed);
        let mut oracle = model.recompute_sampled(2000, spec.prompt(), original::inference(), spec.sampling().clone()).unwrap();
        let choice = oracle.advance_sampled(1, SampleBudget { decoder: original::inference(), sampling: SamplingBudget { vocabulary: 260 } }).unwrap();
        assert_eq!(choice.choice.token, candidate);
        let mut generation = model.monitored_generation(2000, 2001, spec, loaded.policy.clone(), GenerationBudget::default()).unwrap();
        assert!(generation.advance(0).unwrap().audit().complete_quiet());
        let cache = generation.accepted_cache_image().unwrap().encode().unwrap();
        let sampler = generation.sampler_state(); let logits = generation.accepted_logits().unwrap().to_vec();
        let event = generation.advance(1).unwrap();
        if candidate == 0 {
            assert!(event.audit().complete_quiet()); assert_eq!(event.sample(), Some(&choice.choice));
            assert!(generation.advance(2).unwrap().audit().complete_quiet());
            assert_eq!(generation.status(), GenerationStatus::Finished(GenerationStop::StopToken(256)));
            assert_eq!(generation.generated_tokens(), &[0, 256]);
        } else {
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(event.accepted().is_none()); assert!(event.sample().is_none());
            assert!(generation.generated_tokens().is_empty());
            assert_eq!(generation.accepted_cache_image().unwrap().encode().unwrap(), cache);
            assert_eq!(generation.accepted_logits().unwrap(), logits);
            assert_eq!(generation.sampler_state(), sampler);
        }
    }
}

#[test]
fn each_document_has_its_own_original_cache_and_estimated_fit_work_matches_completed_capture() {
    let root = Root::new(); let args = files(&root);
    replace(&args, "\"documents\":[{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]}]",
        "\"documents\":[{\"origin\":1020,\"stream\":1001,\"tokens\":[4,5]},{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]}]");
    fixture::fund(&args);
    run(&args, &mut Vec::new()).unwrap();
    let (plan, inputs) = assets::load(&args).unwrap();
    let (actual, saved) = replay(&args); let expected = fixture::independent(&inputs.model, &plan);
    assert_eq!(saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap(), expected.1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
    assert_eq!(actual.fit_report(), expected.0.fit_report());
    for source in saved.binding().sources.values() {
        for layer in source.layers().values() {
            assert_eq!((layer.token_count, layer.first_position, layer.first_sequence), (2, 0, 1));
        }
    }
    let log = rows(&args); let outcomes = selected(&log, "document_outcome");
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|row| row.get("accepted").unwrap().get("tokens").unwrap().as_u64() == Some(2)));
    assert_eq!(selected(&log, "training_admission")[0].get("tokens").unwrap().as_u64(), Some(4));
}

#[test]
fn all_original_work_dimensions_and_output_reservations_refuse_one_less_before_missing_weights() {
    let root = Root::new(); let args = files(&root); fixture::fund_exact_outputs(&args);
    run(&args, &mut Vec::new()).unwrap();
    let cases = [("budget", "tokens"), ("budget", "source_values"), ("budget", "decoder_products"),
        ("fitting", "source_values"), ("fitting", "parameter_values"), ("fitting", "scratch_values"),
        ("fitting", "work_units"), ("output_limits", "binding_bytes"), ("output_limits", "archive_bytes"),
        ("output_limits", "report_bytes")];
    for (section, field) in cases {
        let root = Root::new(); let args = files(&root); fixture::fund_exact_outputs(&args);
        let value = fixture::number(&args, section, field); assert!(value > 0);
        set_number(&args, section, field, value - 1); fs::remove_file(&args[1]).unwrap();
        let error = run(&args, &mut Vec::new()).unwrap_err().to_string();
        assert!(error.contains("codec training contract: Limit"), "{section}.{field}: {error}");
        assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    }
}

#[test]
fn malformed_and_mislabeled_corpora_refuse_before_checkpoint_access() {
    let root = Root::new(); let args = files(&root); fs::remove_file(&args[1]).unwrap();
    let source = fs::read_to_string(&args[2]).unwrap();
    for (before, after) in [("fa.learned-kv-codec-training/1", "unrecognized"),
        ("\"origin\":1010", "\"origin\":0"), ("\"stream\":1000", "\"stream\":0"),
        ("\"tokens\":[0,1]", "\"tokens\":[0,260]"), ("\"tokens\":[0,1]", "\"tokens\":[]"),
        ("\"origin\":1010", "\"label\":\"safe\",\"origin\":1010"),
        ("\"rank\":1", "\"rank\":3"), ("\"context\":8", "\"context\":8,\"context\":8") ] {
        fs::write(&args[2], &source).unwrap(); replace(&args, before, after);
        let error = run(&args, &mut Vec::new()).unwrap_err().to_string();
        assert!(!error.contains("No such file"), "{before}: {error}");
        assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    }
    fs::write(&args[2], &source).unwrap();
    replace(&args, "\"documents\":[{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]}]",
        "\"documents\":[{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]},{\"origin\":1011,\"stream\":1000,\"tokens\":[1,0]}]");
    assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("unique original"));
}

#[test]
fn changed_real_training_source_changes_the_codec_and_old_result_witness_cannot_certify_it() {
    let base = Root::new(); let original_args = files(&base); run(&original_args, &mut Vec::new()).unwrap();
    let changed = Root::new(); let changed_args = files(&changed);
    replace(&changed_args, "\"tokens\":[0,1]", "\"tokens\":[0,0]");
    run(&changed_args, &mut Vec::new()).unwrap();
    assert_eq!(fs::read(&original_args[3]).unwrap(), fs::read(&changed_args[3]).unwrap());
    let (original_codec, _) = replay(&original_args); let (changed_codec, _) = replay(&changed_args);
    let model = original::model(); let tokenizer = original::tokenizer(&model);
    let policy = original::campaign().policy(original_codec.clone(), &probe_fixture::settings()).unwrap();
    let bytes = original::campaign().monitor_artifact(&tokenizer, &original_codec,
        &probe_fixture::settings(), MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap().verify(&model, &tokenizer, &policy).unwrap();
    assert!(operator_monitor::MonitorInput::decode(&bytes).unwrap().bind_with_tokenizer(&model, &tokenizer, changed_codec).is_err());
    let mut archive = fs::read(&original_args[4]).unwrap(); let changed_bytes = fs::read(&changed_args[4]).unwrap();
    assert_ne!(archive, changed_bytes); assert_eq!(archive.len(), changed_bytes.len());
    let (_, inputs) = assets::load(&original_args).unwrap();
    let scalar_start = archive.len() - inputs.estimate.source_values * 4;
    archive[scalar_start..].copy_from_slice(&changed_bytes[scalar_start..]);
    let expected = fit::binding(&fs::read(&original_args[3]).unwrap()).unwrap();
    let unverified = LearnedKvFitArchive::decode(&archive, &expected, MAX_FIT_ARCHIVE_BYTES).unwrap();
    assert!(matches!(unverified.replay(expected.budget), Err(Error::Binding)));
}

#[test]
fn numeric_failure_keeps_attempted_work_and_unfinished_population_without_any_codec() {
    let root = Root::new(); let args = files(&root); probe_fixture::weights(Path::new(&args[1]), false, true);
    assert!(run(&args, &mut Vec::new()).is_err());
    assert!(!Path::new(&args[3]).exists()); assert!(!Path::new(&args[4]).exists());
    let log = rows(&args); let outcome = selected(&log, "training_outcome");
    assert_eq!(outcome[0].get("status").unwrap().as_str(), Some("failed"));
    assert_eq!(outcome[0].get("all_inference_reported").unwrap().as_bool(), Some(false));
    assert_eq!(outcome[0].get("fit_attempted").unwrap().as_bool(), Some(false));
    let document = selected(&log, "document_outcome");
    assert_eq!(document[0].get("attempted_tokens").unwrap().as_u64(), Some(1));
    assert_eq!(document[0].get("accepted").unwrap().get("tokens").unwrap().as_u64(), Some(0));
    assert_eq!(document[0].get("complete").unwrap().as_bool(), Some(false));
    assert_eq!(selected(&log, "codec_artifacts").len(), 0);
}

#[test]
fn closed_progress_output_cancels_before_fitting_and_preserves_unstarted_documents() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Root::new(); let args = files(&root);
    replace(&args, "\"documents\":[{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]}]",
        "\"documents\":[{\"origin\":1010,\"stream\":1000,\"tokens\":[0,1]},{\"origin\":1020,\"stream\":1001,\"tokens\":[4,5]}]");
    fixture::fund(&args);
    assert!(run(&args, &mut Broken).is_err());
    assert!(!Path::new(&args[3]).exists()); assert!(!Path::new(&args[4]).exists());
    let log = rows(&args); let outcome = selected(&log, "training_outcome");
    assert_eq!(outcome[0].get("status").unwrap().as_str(), Some("cancelled"));
    assert_eq!(outcome[0].get("fit_attempted").unwrap().as_bool(), Some(false));
    let documents = selected(&log, "document_outcome"); assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].get("attempted_tokens").unwrap().as_u64(), Some(2));
    assert_eq!(documents[1].get("attempted_tokens").unwrap().as_u64(), Some(0));
    assert_eq!(selected(&log, "fit_completed_work").len(), 0);
}

#[test]
fn original_failed_fit_and_cancelled_owners_never_emit_a_successful_runner_result() {
    let model = original::model(); let mut allowance = CodecTrainingBudget::default();
    allowance.fitting.parameter_values = 0;
    let mut owner = model.begin_codec_training(vec![CodecTrainingInput { origin: 1, stream: 2, tokens: vec![0, 1] }],
        LearnedKvPolicy::new(1, 1, 1, 8).unwrap(), allowance).unwrap();
    let mut log = Vec::new();
    assert!(drive(&mut owner, &mut log, &mut Vec::new()).is_err());
    assert_eq!(owner.status(), Status::Failed(Error::Limit));
    assert!(owner.report().fit_attempted); assert!(owner.report().documents[&1].complete);
    assert!(owner.report().fit.is_none()); assert!(owner.advance(owner.revision()).is_err());
    let mut cancelled = model.begin_codec_training(vec![CodecTrainingInput { origin: 1, stream: 2, tokens: vec![0, 1] }],
        LearnedKvPolicy::new(1, 1, 1, 8).unwrap(), CodecTrainingBudget::default()).unwrap();
    cancelled.cancel(cancelled.revision()).unwrap();
    let mut log = Vec::new(); assert!(drive(&mut cancelled, &mut log, &mut Vec::new()).is_err());
    assert_eq!(cancelled.status(), Status::Cancelled); assert!(!cancelled.report().fit_attempted);
    assert_eq!(cancelled.report().documents[&1].attempted_tokens, 0);
}

#[test]
fn existing_or_raced_outputs_are_preserved_without_repeating_the_completed_fit() {
    for existing in [3, 4, 5] {
        let root = Root::new(); let args = files(&root); fs::write(&args[existing], b"prior artifact").unwrap();
        fs::remove_file(&args[1]).unwrap();
        assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("output already exists"));
        assert_eq!(fs::read(&args[existing]).unwrap(), b"prior artifact");
        assert!([3, 4, 5].into_iter().filter(|index| *index != existing).all(|index| !Path::new(&args[index]).exists()));
    }
    struct Racer { path: std::path::PathBuf, bytes: Vec<u8>, created: bool }
    impl Write for Racer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if !self.created && self.bytes.windows(b"codec_ready".len()).any(|part| part == b"codec_ready") {
                fs::write(&self.path, b"concurrent artifact")?; self.created = true;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    for raced in [3, 4] {
        let root = Root::new(); let args = files(&root);
        let mut racer = Racer { path: Path::new(&args[raced]).to_path_buf(), bytes: Vec::new(), created: false };
        assert!(run(&args, &mut racer).is_err());
        assert_eq!(fs::read(&args[raced]).unwrap(), b"concurrent artifact");
        let log = rows(&args); assert_eq!(selected(&log, "fit_completed_work").len(), 1);
        assert!(selected(&log, "codec_artifacts").iter().all(|row| row.get("saved").unwrap().as_bool() == Some(false)));
        let before = fs::read(&args[5]).unwrap();
        assert!(run(&args, &mut Vec::new()).is_err()); assert_eq!(fs::read(&args[5]).unwrap(), before);
    }
}

#[test]
fn truncated_nonregular_and_underfunded_checkpoint_inputs_never_create_a_report_or_codec() {
    for variant in 0..4 {
        let root = Root::new(); let args = files(&root);
        match variant {
            0 => { let mut bytes = fs::read(&args[1]).unwrap(); bytes.pop(); fs::write(&args[1], bytes).unwrap(); },
            1 => { fs::remove_file(&args[1]).unwrap(); fs::create_dir(&args[1]).unwrap(); },
            2 => set_number(&args, "input_limits", "weight_read_calls", 1),
            _ => set_number(&args, "input_limits", "auxiliary_bytes", 1),
        }
        assert!(run(&args, &mut Vec::new()).is_err());
        assert!([3, 4, 5].into_iter().all(|index| !Path::new(&args[index]).exists()));
    }
    #[cfg(unix)] {
        let root = Root::new(); let args = files(&root); let original_path = root.0.join("original.weights");
        fs::rename(&args[1], &original_path).unwrap();
        std::os::unix::fs::symlink(&original_path, &args[1]).unwrap();
        assert!(run(&args, &mut Vec::new()).unwrap_err().to_string().contains("regular non-symlink"));
        assert!(!Path::new(&args[5]).exists());
    }
}

#[test]
fn original_profile_metadata_matches_identity_gqa_and_static_rotary_models_before_weights() {
    for model in [original::model(), original::scaled_model(),
        original::model_with_identity(fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity {
            tenant: 11, model: 12, model_generation: 13, tokenizer_generation: 14, profile_generation: 15 })] {
        assert_eq!(&model.profile().cache_profile().unwrap(), model.cache_profile());
        let estimate = LearnedKvPolicy::new(1, 1, 1, 8).unwrap().estimate_fit(model.cache_profile(), 2, FitBudget::default()).unwrap();
        let sources = BTreeMap::from([(1010, model.recompute(1000, &[0, 1], original::inference()).unwrap().cache_image().unwrap())]);
        let (codec, _) = LearnedKvCodec::fit_with_checkpoint(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(), &sources, FitBudget::default()).unwrap();
        assert_eq!(estimate.work_units, codec.fit_report().work_units_reserved);
        assert_eq!(estimate.parameter_values, codec.fit_report().parameter_values);
        assert_eq!(estimate.groups, codec.groups().len());
    }
    let root = Root::new(); let args = files(&root);
    let configuration = fs::read_to_string(&args[0]).unwrap();
    fs::write(&args[0], configuration.replacen("\"rope_theta\":10000.0", "\"rope_theta\":10000.0,\"rope_scaling\":{\"rope_type\":\"linear\",\"factor\":2.0}", 1)).unwrap();
    run(&args, &mut Vec::new()).unwrap();
    let (plan, inputs) = assets::load(&args).unwrap();
    assert_eq!(inputs.model.profile(), original::scaled_model().profile());
    let (_, checkpoint) = replay(&args); let expected = fixture::independent(&original::scaled_model(), &plan);
    assert_eq!(checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap(), expected.1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
    assert_eq!(selected(&rows(&args), "rotary_profile")[0].get("scaling").unwrap().as_str(), Some("linear"));
}

#[test]
fn source_origin_overlap_and_actual_imported_identity_are_checked_by_the_existing_monitor_consumer() {
    let root = Root::new(); let args = files(&root); run(&args, &mut Vec::new()).unwrap();
    let source = fs::read_to_string(&args[3]).unwrap();
    assert!(source.contains("\"origin\":1010"));
    fs::write(&args[3], source.replacen("\"origin\":1010", "\"origin\":101", 1)).unwrap();
    fs::remove_file(&args[4]).unwrap();
    let probe = monitor_args(&root);
    let error = crate::probe_command::run_for_codec_test(&probe, &mut Vec::new()).unwrap_err().to_string();
    assert!(error.contains("campaign contract: Duplicate"), "{error}");
    assert!(!Path::new(&probe[6]).exists()); assert!(!Path::new(&probe[7]).exists());

    let root = Root::new(); let args = files(&root);
    for (name, value) in [("tenant", 11), ("model", 12), ("model_generation", 13),
        ("tokenizer_generation", 14), ("profile_generation", 15)] {
        set_number(&args, "identity", name, value);
    }
    run(&args, &mut Vec::new()).unwrap();
    let (plan, inputs) = assets::load(&args).unwrap();
    let independent = original::model_with_identity(plan.identity);
    assert_eq!(inputs.model.profile(), independent.profile());
    assert_eq!(&inputs.model.profile().cache_profile().unwrap(), independent.cache_profile());
    let (_, checkpoint) = replay(&args); let expected = fixture::independent(&independent, &plan);
    assert_eq!(checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap(), expected.1.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
    let probe = monitor_args(&root); fs::remove_file(&args[4]).unwrap();
    let error = crate::probe_command::run_for_codec_test(&probe, &mut Vec::new()).unwrap_err().to_string();
    assert!(error.contains("campaign contract: Binding"), "{error}");
    assert!(!Path::new(&probe[6]).exists()); assert!(!Path::new(&probe[7]).exists());
}
