//! Near-identical invalid inputs and stale observations against successful imports.
use super::*;
use fa_reference::action::consequence::oversight::policy_state::StateFreshness;

#[test]
fn learned_command_capture_horizon_and_closed_schema_refuse_before_asset_reads_or_startup() {
    let root = Root::new(); let config = configured(&root); let path = fixture::write(&root, b"allow");
    let valid = fs::read_to_string(&path).unwrap();
    let loaded = recipe::load(&path, &config, true).unwrap();
    assert_eq!(loaded.evidence.token_ids, 4096); assert!(!config.store.exists());
    fs::remove_file(root.0.join("fit-binding.json")).unwrap();
    for invalid in [valid.replace("\"token_ids\":4096", "\"token_ids\":4097"),
        valid.replace("\"token_ids\":4096", "\"token_ids\":0")] {
        fs::write(&path, invalid).unwrap();
        let failure = recipe::load(&path, &config, true).err().unwrap();
        assert!(failure.contains("capture token limit"), "{failure}"); // No missing-file read.
        assert!(!config.store.exists());
    }
    for (old, replacement, expected) in [
        ("fa.learned-publication/1", "fa.learned-publication/2", "unsupported"),
        ("huggingface_raw_bytelevel", "automatic", "tokenizer format"),
        ("\"completion\":\"stop_required\"", "\"completion\":\"token_limit\"", "monitored control stop"),
        ("\"rounds\":1", "\"rounds\":2", "one bounded review round"),
        ("\"request\":1", "\"request\":1,\"unexpected\":true", "unsupported"),
    ] {
        fs::write(&path, valid.replace(old, replacement)).unwrap();
        let failure = recipe::load(&path, &config, true).err().unwrap();
        assert!(failure.contains(expected), "{failure}");
        assert!(!config.store.exists());
    }
}

#[test]
fn learned_command_malformed_fit_probes_and_native_roster_never_create_durable_state() {
    let root = Root::new(); let config = configured(&root); let path = fixture::write(&root, b"allow");
    recipe::load(&path, &config, true).unwrap(); assert!(!config.store.exists());
    for (name, old, replacement) in [
        ("fit-binding.json", "\"rank\":1", "\"rank\":0"),
        ("fit-binding.json", "\"origin\":101", "\"origin\":102"),
        ("learned-monitor.json", "\"retention\":\"all\"", "\"retention\":\"guess\""),
        ("learned-monitor.json", "\"side\":\"value\"", "\"side\":\"residual\""),
        ("learned-monitor.json", "\"side\":\"value\"", "\"side\":\"key\""),
        ("learned-monitor.json", "\"weights\":[1.0,0.0]", "\"weights\":[]"),
        ("learned-monitor.json", "\"bias\":0.0", "\"bias\":1e999"),
        ("native-roster.json", "fa.learned-native-roster/1", "fa.learned-native-roster/2"),
        ("native-roster.json", "\"max_new_tokens\":2", "\"max_new_tokens\":0"),
        ("helper-monitor.json", "\"weights\":[1.0,0.0]", "\"weights\":[1.0]"),
        ("tokenizer.json", "\"use_regex\":false", "\"use_regex\":true"),
    ] {
        let file = root.0.join(name); let original = fs::read_to_string(&file).unwrap();
        assert!(original.contains(old));
        fs::write(&file, original.replacen(old, replacement, 1)).unwrap();
        assert!(recipe::load(&path, &config, true).is_err(), "accepted {name}: {replacement}");
        assert!(!config.store.exists());
        fs::write(file, original).unwrap();
    }
    let file = root.0.join("fit.fakv"); let mut bytes = fs::read(&file).unwrap();
    // A valid archive boundary does not turn modified scalar/witness data into
    // an imported usable codec. The original recomputed fit must agree.
    let last = bytes.len() - 1; bytes[last] ^= 1;
    fs::write(&file, bytes).unwrap();
    assert!(recipe::load(&path, &config, false).is_err());
    assert!(!config.store.exists());
}

#[test]
fn learned_command_exact_reopen_refuses_changed_independent_recipe_without_journal_writes() {
    let root = Root::new(); let config = configured(&root); let path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&path, &config, false).unwrap();
    let (host, _reviewer) = recovery::create(&config, &loaded).unwrap(); drop(host);
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    for (name, old, replacement) in [
        ("tokenizer.json", "<eos>", "<stop>"),
        ("prompt.txt", "xy", "xz"),
        ("sampling.json", "\"seed\":9", "\"seed\":10"),
        ("learned-monitor.json", "\"threshold\":100", "\"threshold\":99"),
    ] {
        let file = root.0.join(name); let original = fs::read_to_string(&file).unwrap();
        fs::write(&file, original.replace(old, replacement)).unwrap();
        let changed = recipe::load(&path, &config, false).unwrap();
        assert!(recovery::open(&config, &changed).is_err(), "reopened changed {name}");
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
        fs::write(file, original).unwrap();
    }
    let file = root.0.join("weights.safetensors"); let original = fs::read(&file).unwrap();
    let mut changed = original.clone(); let last = changed.len() - 1; changed[last] ^= 1;
    fs::write(&file, changed).unwrap();
    let changed = recipe::load(&path, &config, false).unwrap();
    assert!(recovery::open(&config, &changed).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    fs::write(file, original).unwrap();
    let loaded = recipe::load(&path, &config, false).unwrap();
    let (host, _) = recovery::open(&config, &loaded).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn learned_command_missing_or_full_context_source_blocks_the_first_original_token() {
    for contexts in [false, true] {
        let root = Root::new(); let mut config = configured(&root);
        let path = fixture::write(&root, b"allow");
        if contexts { evidence(&root, &config, 1, true, true); }
        let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
        let (_port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &deadline(), &mut || ElapsedTick(1000)).is_err());
        let host = driver.supervisor().host().unwrap();
        let state = host.learned_generation_inspection().unwrap();
        assert_eq!(state.numerical.position, 0); assert_eq!(state.numerical.sampled_draws, 0);
        assert_eq!(state.numerical.work.admitted_tokens, 0); assert!(state.pending.is_none());
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
    }
}

#[test]
fn learned_command_post_read_lease_boundary_blocks_numerics_without_renewing_saved_time() {
    for gap in [9, 10] {
        let root = Root::new(); let mut config = configured(&root);
        config.source_policy.freshness = StateFreshness::new(10).unwrap();
        let path = fixture::write(&root, b"allow"); evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
        let (_port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut control = Control::new(&config, 1, None).unwrap(); let mut calls = 0;
        let result = generate(&mut driver, &reviewer, &mut config, &mut control, &deadline(), &mut || {
            calls += 1;
            ElapsedTick(if calls == 1 { 1000 } else if calls < 5 { 1000 + gap } else { 1000 + 2 * gap })
        });
        assert_eq!(result.is_ok(), gap == 9, "{result:?}");
        let host = driver.supervisor().host().unwrap(); let state = host.learned_generation_inspection().unwrap();
        assert_eq!(state.numerical.position, if gap == 9 { 4 } else { 0 });
        assert_eq!(state.numerical.sampled_draws, if gap == 9 { 2 } else { 0 });
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
    }
}

#[test]
fn learned_command_router_and_options_have_no_weaker_fallback() {
    assert_eq!(crate::command(vec!["create-learned-generated".into()]).unwrap_err(), USAGE);
    for args in [
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after"],
        vec!["create-learned-generated", "config", "recipe", "--resume", "--continue-generation"],
        vec!["create-learned-generated", "config", "recipe", "--native-text"],
    ] { assert_eq!(command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE); }
    let args = ["create-learned-generated", "config", "recipe", "reviewer"].map(str::to_owned);
    assert_eq!(command(&args, Some(Path::new("qualification"))).unwrap_err(), USAGE);
}
