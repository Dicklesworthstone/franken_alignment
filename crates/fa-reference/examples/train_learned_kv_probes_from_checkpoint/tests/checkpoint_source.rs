use super::*;
#[path = "../../support/checkpoint_source_fixture.rs"]
#[allow(dead_code)]
mod source_fixture;

fn set_input(args: &[OsString], field: &str, value: u64) {
    let text = fs::read_to_string(&args[3]).unwrap();
    let parsed = strict_json::parse(text.as_bytes(), Limits::default()).unwrap();
    let old = parsed.get("input_limits").unwrap().get(field).unwrap().as_u64().unwrap();
    replace(args, &format!("\"{field}\":{old}"), &format!("\"{field}\":{value}"));
}

#[test]
fn manifest_checkpoint_campaigns_preserve_complete_effective_model_binding_and_original_reports() {
    for (sharded, adapted) in [(false, false), (true, false), (false, true), (true, true)] {
        let root = Root::new(); let args = files(&root);
        let source = source_fixture::create(&root.0, Path::new(&args[1]), sharded, adapted);
        assert!(run(&source.invocation(&args), &mut Vec::new()).unwrap());
        let model = original::model(); let tokenizer = original::tokenizer(&model); let codec = original::codec(&model);
        let bytes = fs::read(&args[7]).unwrap();
        let policy = original::campaign().policy(codec.clone(), &fixture::settings()).unwrap();
        KvMonitorArtifact::decode(&bytes, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap()
            .verify(&model, &tokenizer, &policy).unwrap();
        let loaded = operator_monitor::MonitorInput::decode(&bytes).unwrap()
            .bind_with_tokenizer(&model, &tokenizer, codec).unwrap();
        assert_eq!(loaded.policy.monitor().budget(), fixture::settings().audit);
        let log = rows(&args); let work = selected(&log, "input_work");
        let auxiliary: u64 = [0, 2, 3, 4].into_iter().map(|index| fs::metadata(&args[index]).unwrap().len()).sum();
        assert_eq!(work[0].get("auxiliary_bytes").unwrap().as_u64(), Some(auxiliary + source.auxiliary_bytes() as u64));
        assert_eq!(work[0].get("weight_bytes").unwrap().as_u64(), Some(source.weight_bytes() as u64));
        let source_row = selected(&log, "checkpoint_source");
        assert_eq!(source_row.len(), 1);
        assert_eq!(source_row[0].get("base_files").unwrap().as_u64(), Some(if sharded { 2 } else { 1 }));
        assert_eq!(source_row[0].get("merge_products").unwrap().as_u64(), Some(if adapted { 64 } else { 0 }));
        assert_eq!(selected(&log, "planned_case").len(), 6);
        assert_eq!(selected(&log, "evaluation").len(), 4);
        assert!(selected(&log, "evaluation").iter().all(|row| row.get("accepted").unwrap().as_bool() == Some(true)));
    }
}

#[test]
fn changing_only_the_last_value_adapter_keeps_other_taps_and_refuses_the_old_bound_monitor() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    assert!(run(&source.invocation(&args), &mut Vec::new()).unwrap());
    let control = rows(&args); let artifact = fs::read(&args[7]).unwrap();
    let failed = Root::new(); let bad = files(&failed);
    let changed = source_fixture::create(&failed.0, Path::new(&bad[1]), true, true);
    changed.remove_final_value_signal();
    assert!(!run(&changed.invocation(&bad), &mut Vec::new()).unwrap());
    assert!(!Path::new(&bad[7]).exists());
    let failure = rows(&bad);
    let good_taps = selected(&control, "evaluation"); let bad_taps = selected(&failure, "evaluation");
    assert_eq!(&good_taps[..3], &bad_taps[..3]);
    assert_eq!(bad_taps[3].get("evaluated").unwrap().as_bool(), Some(false));
    assert_eq!(bad_taps[3].get("accepted").unwrap().as_bool(), Some(false));
    assert_eq!(bad_taps[3].get("planned_benign").unwrap().as_u64(), Some(1));
    assert_eq!(bad_taps[3].get("planned_violation").unwrap().as_u64(), Some(1));
    assert!(bad_taps[3].get("counts").unwrap().is_null());
    assert_eq!(selected(&control, "campaign_admitted_work"), selected(&failure, "campaign_admitted_work"));
    assert_eq!(selected(&failure, "checkpoint_source")[0].get("merge_products").unwrap().as_u64(), Some(64));
    let (_, actual) = assets::load_with_source(&changed.selected(&bad), true).unwrap();
    let codec = original::codec(&original::model());
    assert!(operator_monitor::MonitorInput::decode(&artifact).unwrap()
        .bind_with_tokenizer(&actual.model, &actual.tokenizer, codec).is_err());
    assert_eq!(fs::read(&args[7]).unwrap(), artifact);
}

#[test]
fn campaign_and_manifest_limits_conjoin_for_all_base_and_adapter_reads() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    let selected_args = source.selected(&args);
    let (_, initial) = assets::load_with_source(&selected_args, true).unwrap();
    let bytes = source.weight_bytes(); let calls = initial.work.weight_read_calls;
    source.set_number("weight_bytes", bytes as u64); source.set_number("weight_read_calls", calls as u64);
    set_input(&args, "weight_bytes", bytes as u64); set_input(&args, "weight_read_calls", calls as u64);
    let (_, exact) = assets::load_with_source(&selected_args, true).unwrap();
    assert_eq!((exact.work.weight_bytes, exact.work.weight_read_calls, exact.work.merge_products), (bytes, calls, 64));
    let manifest = fs::read(&source.manifest).unwrap(); let campaign = fs::read(&args[3]).unwrap();
    for (at_source, field) in [(true, "weight_bytes"), (false, "weight_bytes"),
        (true, "weight_read_calls"), (false, "weight_read_calls")] {
        fs::write(&source.manifest, &manifest).unwrap(); fs::write(&args[3], &campaign).unwrap();
        let value = (if field == "weight_bytes" { bytes - 1 } else { calls - 1 }) as u64;
        if at_source { source.set_number(field, value); } else { set_input(&args, field, value); }
        let error = assets::load_with_source(&selected_args, true).err().unwrap();
        let failure = error.downcast_ref::<checkpoint_source::CheckpointLoadError>().unwrap();
        assert_eq!(failure.work.merge_products, 0);
        if field == "weight_bytes" { assert_eq!((failure.work.weight_bytes, failure.work.weight_read_calls), (0, 0)); }
        else { assert_eq!(failure.work.weight_read_calls, calls - 1); assert!(failure.work.weight_bytes > 0); }
        assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());
    }
    fs::write(&source.manifest, &manifest).unwrap(); fs::write(&args[3], &campaign).unwrap();
    assert!(run(&source.invocation(&args), &mut Vec::new()).unwrap());
}

#[test]
fn campaign_source_auxiliary_limit_and_tokenizer_preflight_cover_the_new_manifest_path() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    for _ in 0..4 {
        let required: u64 = [0, 2, 3, 4].into_iter().map(|index| fs::metadata(&args[index]).unwrap().len()).sum();
        set_input(&args, "auxiliary_bytes", required + source.auxiliary_bytes() as u64);
    }
    let (plan, exact) = assets::load_with_source(&source.selected(&args), true).unwrap();
    assert_eq!(exact.work.auxiliary_bytes, plan.input_limits.auxiliary_bytes);
    set_input(&args, "auxiliary_bytes", exact.work.auxiliary_bytes as u64 - 1); source.remove_tensors();
    let error = run(&source.invocation(&args), &mut Vec::new()).unwrap_err().to_string();
    assert!(!error.contains("No such file"), "{error}");
    assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());

    for cause in ["tokenizer", "capture", "merge", "schema"] {
        let root = Root::new(); let args = files(&root);
        let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true); source.remove_tensors();
        match cause {
            "tokenizer" => { let mut bytes = fs::read(&args[2]).unwrap(); bytes[15] ^= 1; fs::write(&args[2], bytes).unwrap(); },
            "capture" => decrement(&args, "capture_budget", "scalar_products"),
            "merge" => source.set_number("merge_products", 63),
            "schema" => source.replace("fa.checkpoint-weights/1", "unsupported"),
            _ => unreachable!(),
        }
        let error = run(&source.invocation(&args), &mut Vec::new()).unwrap_err().to_string();
        assert!(!error.contains("No such file"), "{cause}: {error}");
        assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());
    }
}

#[test]
fn an_unselected_manifest_never_changes_the_original_campaign_weight_interpretation() {
    let root = Root::new(); let args = files(&root);
    let source = source_fixture::create(&root.0, Path::new(&args[1]), true, true);
    assert!(run(&source.selected(&args), &mut Vec::new()).is_err());
    assert!(!Path::new(&args[6]).exists()); assert!(!Path::new(&args[7]).exists());
    assert!(run(&args, &mut Vec::new()).unwrap());
    assert!(selected(&rows(&args), "checkpoint_source").is_empty());
}
