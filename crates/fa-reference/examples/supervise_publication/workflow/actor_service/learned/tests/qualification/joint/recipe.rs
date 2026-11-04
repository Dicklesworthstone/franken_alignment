//! Envelope compatibility, strict input admission and complete sequence pins.
use super::*;
use super::super::super::{forecast, processes, peer_intake};
use super::super::super::super::sequence as resident;

#[test]
fn joint_wrapper_preserves_every_existing_learned_recipe_and_forecast_mode() {
    for (version, mode) in [(1, None), (2, None), (3, None), (4, None),
        (5, Some(false)), (5, Some(true)), (6, None)] {
        let root = Root::new(); let config = configured(&root);
        let path = if version == 1 || version == 6 { fixture::write(&root, b"allow") }
            else { native::write(&root) };
        if matches!(version, 2 | 3) {
            fs::write(&path, fs::read_to_string(&path).unwrap()
                .replace("fa.learned-publication/4", &format!("fa.learned-publication/{version}"))).unwrap();
        }
        if let Some(owned) = mode { forecast::install(&root, &path, owned, 60_000); }
        if version == 6 { fs::write(&path, processes::process_recipe(&root, "allow")).unwrap(); }
        let bytes = fs::read(&path).unwrap();
        let legacy = recipe::load(&path, &config, true).unwrap();
        assert_eq!(legacy.generation.required_held_out_joint(), None);
        wrap(&path);
        let selected = recipe::load(&path, &config, true).unwrap();
        assert_eq!(selected.generation.required_held_out_joint(), Some(policy()));
        assert_eq!(selected.generation, legacy.generation.clone().with_required_held_out_joint(policy()).unwrap());
        assert_eq!(selected.request, legacy.request); assert_eq!(selected.ttl_ms, legacy.ttl_ms);
        assert_eq!(selected.stream, legacy.stream); assert_eq!(selected.rounds, legacy.rounds);
        assert_eq!(selected.native.is_some(), legacy.native.is_some());
        assert_eq!(selected.process.is_some(), legacy.process.is_some());
        // Receipt/terminal recipe admission pins the SAME policy without any
        // current native helper intake. Independent actor/fit assets remain.
        for name in ["native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let historical = recipe::load(&path, &config, false).unwrap();
        assert_eq!(historical.generation, selected.generation); assert!(historical.native.is_none());
        fs::write(&path, &bytes).unwrap();
        let unchanged = recipe::load(&path, &config, false).unwrap();
        assert_eq!(unchanged.generation, legacy.generation);
        assert_eq!(unchanged.generation.required_held_out_joint(), None);
        assert_eq!(fs::read(&path).unwrap(), bytes); assert!(!config.store.exists());
    }
}

#[test]
fn joint_wrapper_rejects_unknown_duplicate_nested_and_unbounded_fields_before_assets() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    let inner = fs::read_to_string(&path).unwrap(); let valid = envelope(&inner);
    fs::write(&path, &valid).unwrap(); recipe::load(&path, &config, false).unwrap();
    fs::remove_file(root.0.join("fit-binding.json")).unwrap();
    let mut invalid = vec![
        valid.replacen("fa.learned-joint-publication/1", "fa.learned-joint-publication/2", 1),
        valid.replace("fa.learned-publication/4", "fa.learned-publication/99"),
        valid.replacen("\"joint\":", "\"automatic_approval\":true,\"joint\":", 1),
        valid.replacen("\"joint\":", "\"joint\":{},\"joint\":", 1),
        valid.replacen("\"recipe\":", "\"recipe\":{},\"recipe\":", 1),
        valid.replacen("\"request\":1", "\"request\":1,\"bypass_joint\":true", 1),
        valid.replacen("\"max_cases\":3", "\"max_cases\":3,\"extra\":true", 1),
        valid.replacen("\"max_cases\":3,", "", 1),
        valid.replacen("\"generation\":1", "\"generation\":1,\"generation\":2", 1),
        envelope(&valid),
    ];
    for (old, replacement) in [
        ("\"id\":71", "\"id\":0"),
        ("\"generation\":1", "\"generation\":0"),
        ("\"minimum_safe_roots\":1", "\"minimum_safe_roots\":0"),
        ("\"minimum_violation_roots\":2", "\"minimum_violation_roots\":0"),
        ("\"maximum_escape_ppm\":0", "\"maximum_escape_ppm\":1000001"),
        ("\"maximum_false_stop_ppm\":0", "\"maximum_false_stop_ppm\":1000001"),
        ("\"max_cases\":3", "\"max_cases\":18446744073709551615"),
        ("\"max_member_outcomes\":12", "\"max_member_outcomes\":18446744073709551615"),
        ("\"id\":71", "\"id\":-1"),
        ("\"id\":71", "\"id\":71.0"),
    ] { invalid.push(valid.replacen(old, replacement, 1)); }
    for document in invalid {
        fs::write(&path, document).unwrap();
        let failure = recipe::load(&path, &config, false).err().unwrap();
        assert!(!failure.contains("No such file"), "parser reached missing fit asset: {failure}");
        assert!(!config.store.exists());
    }
    fs::write(&path, format!("{valid}{}", " ".repeat(64 * 1024))).unwrap();
    assert!(recipe::load(&path, &config, false).is_err()); assert!(!config.store.exists());
}

#[test]
fn resident_joint_policy_must_match_every_independently_selected_entry_before_any_clock_or_store() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        if let Some(owned) = mode { forecast::install(&root, &path, owned, 60_000); }
        wrap(&path);
        let (second, _) = native::next(&root, &config, 2);
        let original = fs::read_to_string(&second).unwrap();
        for changed in [
            original.replacen("\"id\":71", "\"id\":72", 1),
            original.replacen("\"max_member_outcomes\":12", "\"max_member_outcomes\":16", 1),
        ] {
            fs::write(&second, changed).unwrap();
            let first = recipe::load(&path, &config, true).unwrap();
            let next = recipe::load(&second, &config, true).unwrap();
            assert_ne!(first.generation, next.generation);
            let entries = [first, next].into_iter().map(|loaded| {
                let mut actor = peer_intake::actor_profile(&root, &config, loaded.request);
                actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
                resident::Entry { loaded, actor, release: LearnedTextRelease::Message }
            }).collect();
            let peers = fixture::peers(&root, &config);
            let failure = resident::run(configured(&root), entries, &peers, false, None, None,
                || panic!("mixed joint policies must refuse before any clock or startup")).err().unwrap();
            assert!(failure.contains("exact original finite generation"), "{failure}");
            assert!(!config.store.exists()); assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
        fs::write(&second, original).unwrap();
        assert_eq!(recipe::load(&path, &config, true).unwrap().generation,
            recipe::load(&second, &config, true).unwrap().generation);
    }
}

#[test]
fn resident_activation_option_reaches_only_explicit_live_recovery() {
    let root = Root::new(); let missing = root.0.join("unread-config.json");
    let base = ["serve-learned-generated".to_owned(), missing.to_str().unwrap().to_owned(),
        "sequence".to_owned(), "reviewer".to_owned()];
    let mut create = base.to_vec();
    create.extend(["--credibility-activation".to_owned(), "capsule".to_owned()]);
    assert_eq!(crate::command(create).err().as_deref(),
        Some("sequence credibility activation requires explicit live recovery"));
    for option in [vec!["--continue-generation"], vec!["--after-message", "1"]] {
        let mut args = base.to_vec(); args.extend(option.into_iter().map(str::to_owned));
        args.extend(["--credibility-activation".to_owned(), "capsule".to_owned()]);
        let error = crate::command(args).unwrap_err();
        assert!(!error.contains("credibility activation requires"), "{error}");
        assert!(!error.contains("serve-learned-generated CONFIG"), "{error}");
        assert!(error.contains("No such file"), "{error}");
    }
    assert!(!missing.exists()); assert!(!root.0.join("store").exists());
}
