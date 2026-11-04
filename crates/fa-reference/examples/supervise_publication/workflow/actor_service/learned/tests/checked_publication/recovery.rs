//! Exact offline contract selection and original uncertainty/receipt semantics.
use super::*;
use super::super::super::{audit as audit_command, stopped as stop_command};

#[test]
fn checked_history_requires_the_exact_profile_before_cleanup_and_audits_or_stops_without_live_sources() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    super::super::forecast::install(&root, &path, true, 60_000);
    let producer = provision(&root, &config, &path, true, false);
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.anchor_output = Some(root.0.join("history.bin"));
    let reader = Reader::new(&config, &loaded);
    let (result, _) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000));
    assert!(confirmed(&result)); assert!(result.failure.is_none());
    let before = reader.snapshot();
    let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
    let predictor = loaded.generation.required_owned_pre_output_forecast().unwrap();
    let cost = FileOversight::read_publication_with_owned_learned_consistency(&config.store,
        &config.profile, &loaded.generation, predictor).unwrap().consistency;
    let anchor = anchor_files::load(&root.0.join("history.bin")).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let selected_path = root.0.join("publication.json");
    let selected = fs::read_to_string(&selected_path).unwrap();
    let canonical = config.store.join("delivery.bin"); let journal = fs::read(&canonical).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"selection failure must preserve existing staging").unwrap();
    drop(producer); fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
        "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let reads = config.source.status().read_attempts;
    let omitted = original.split_once("\"recipe\":").unwrap().1.strip_suffix('}').unwrap().to_owned();
    let mut variants = vec![(omitted, selected.clone()),
        (original.clone(), profile(&root, &config, false, false))];
    for (old, replacement) in [
        ("\"bindings\":8", "\"bindings\":9"),
        ("\"limits\":{\"bindings\":8,\"steps\":10000", "\"limits\":{\"bindings\":8,\"steps\":10001"),
        ("\"value_bytes\":1048576", "\"value_bytes\":1048577"),
        ("\"source\":41", "\"source\":42"),
        ("\"after\":0", "\"after\":1"),
        ("\"max_age_ms\":5000", "\"max_age_ms\":5001"),
        ("\"lookup\":{\"steps\":10000", "\"lookup\":{\"steps\":10001"),
        ("\"bytes\":1048576", "\"bytes\":1048577"),
    ] {
        assert!(selected.contains(old));
        variants.push((original.clone(), selected.replacen(old, replacement, 1)));
    }
    for (document, profile) in variants {
        fs::write(&path, document).unwrap(); fs::write(&selected_path, profile).unwrap();
        let mut wrong = recipe::load(&path, &config, false).unwrap();
        wrong.recovery_anchor = Some(anchor.clone());
        assert!(recovery::open_selected(&config, &wrong).is_err());
        assert!(recovery::readonly::read(&config, &wrong, None, &mut || Duration::ZERO).is_err());
        let mut output = Vec::new();
        assert!(audit_command::run(&config, &wrong, 1, &anchor, &mut output, &mut || Ok(())).is_err());
        assert!(output.is_empty());
        assert!(stop_command::run(&config, &wrong, 801,
            || panic!("changed witness contract reached terminal clock"), &mut Vec::new()).is_err());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
        assert_eq!(fs::read(&pending).unwrap(), b"selection failure must preserve existing staging");
    }
    fs::write(&path, original).unwrap(); fs::write(&selected_path, selected).unwrap();
    let mut exact = recipe::load(&path, &config, false).unwrap();
    exact.recovery_anchor = Some(anchor.clone());
    assert_eq!(exact.generation, loaded.generation); assert!(exact.native.is_none());
    let receipt = recovery::readonly::read(&config, &exact, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, result.response.result);
    let mut report = Vec::new();
    audit_command::run(&config, &exact, 1, &anchor, &mut report, &mut || Ok(())).unwrap();
    let report = fa_reference::strict_json::parse(&report, fa_reference::strict_json::Limits::default()).unwrap();
    let value = report.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(value.get("application").unwrap().get("status").unwrap().as_str(), Some("applied"));
    assert_eq!(value.get("application_inputs").unwrap().as_str(), Some("equal_to_reviewed"));
    assert_eq!(value.get("historical_effects").unwrap().get("executions_at_journal_cut").unwrap().as_str(), Some("1"));
    assert_eq!(fs::read(&canonical).unwrap(), journal);
    assert_eq!(fs::read(&pending).unwrap(), b"selection failure must preserve existing staging");
    let mut clocks = 0;
    stop_command::run(&config, &exact, 801, || { clocks += 1; ElapsedTick(1001) }, &mut Vec::new()).unwrap();
    assert_eq!(clocks, 1); assert!(!pending.exists());
    let stopped = reader.snapshot();
    assert_eq!(stopped.executions, before.executions); assert_eq!(stopped.payload, before.payload);
    assert_eq!(stopped.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(stopped.control.ledger.reserved, 0);
    let current = FileOversight::read_publication_with_owned_learned_consistency(&config.store,
        &config.profile, &exact.generation, exact.generation.required_owned_pre_output_forecast().unwrap()).unwrap().consistency;
    assert_eq!(current.work, cost.work); assert_eq!(current.retained_source_bytes, cost.retained_source_bytes);
    assert_eq!(current.consistency.evidence, cost.consistency.evidence);
    let final_bytes = fs::read(&canonical).unwrap();
    stop_command::run(&config, &exact, 801,
        || panic!("completed checked Stop must be clock free"), &mut Vec::new()).unwrap();
    assert_eq!(fs::read(&canonical).unwrap(), final_bytes);
    assert_eq!(config.source.status().read_attempts, reads); assert!(!root.0.join("peers").exists());
}

#[test]
fn missing_producer_prevents_review_and_a_quiet_reread_cannot_extend_its_original_lease() {
    // Missing original evidence cannot reach even the human offer. Numerical
    // work already completed remains charged to its original generation.
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    let producer = provision(&root, &config, &path, false, false);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap(); let reader = Reader::new(&config, &loaded);
    drop(producer); fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000));
    assert!(!confirmed(&result)); assert!(result.failure.is_some()); assert!(packet.is_none());
    let state = reader.snapshot();
    assert_eq!(state.executions, 0); assert!(state.payload.is_empty());
    assert_eq!(state.control.ledger.charged, 0); assert_eq!(state.control.ledger.reserved, 0);
    let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    assert!(host.inspect().stop.is_some()); drop(host);

    for expired in [false, true] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        let _producer = provision(&root, &config, &path, false, false);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap(); let reader = Reader::new(&config, &loaded);
        let mut dispatched = false;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve, &mut |reviewed| {
            if reviewed && !dispatched {
                let current = reader.snapshot();
                dispatched = current.control.ledger.charged != 0 && current.executions == 0;
            }
            // The workflow deadline and action deadline are still far away.
            // Freshness is anchored to producer1000, never this reread time.
            ElapsedTick(if dispatched { if expired { 6000 } else { 5999 } } else { 1000 })
        });
        assert!(dispatched); let packet = packet.unwrap();
        assert_eq!(confirmed(&result), !expired, "{:?}", result.failure);
        let current = reader.snapshot();
        assert_eq!(current.executions, u64::from(!expired));
        assert_eq!(current.control.ledger.charged, if expired { 0 } else { packet.action().spec().units });
        assert_eq!(current.control.ledger.reserved, 0);
        if !expired { assert!(result.failure.is_none(), "{:?}", result.failure); }
    }
}
