//! Legacy compatibility, exact wrapper composition and resident admission.
use super::*;
use super::super::{processes, peer_intake};
use super::super::super::sequence as resident;

#[test]
fn identity_wrapper_preserves_legacy_recipes_and_canonical_joint_composition_without_live_manifest_reads() {
    for (version, mode, joint) in [(1, None, false), (2, None, true), (3, None, false),
        (4, None, true), (5, Some(false), true), (5, Some(true), true), (6, None, true)] {
        let root = Root::new(); let config = configured(&root);
        let path = if version == 1 || version == 6 { fixture::write(&root, b"allow") }
            else { native::write(&root) };
        if matches!(version, 2 | 3) {
            fs::write(&path, fs::read_to_string(&path).unwrap().replace("fa.learned-publication/4",
                &format!("fa.learned-publication/{version}"))).unwrap();
        }
        if let Some(owned) = mode { super::super::forecast::install(&root, &path, owned, 60_000); }
        if version == 6 { fs::write(&path, processes::process_recipe(&root, "allow")).unwrap(); }
        let bytes = fs::read(&path).unwrap(); let original = recipe::load(&path, &config, true).unwrap();
        assert!(!original.generation.requires_computed_identity()); assert!(original.identity.is_none());
        provision(&root, &path, joint);
        let selected = recipe::load(&path, &config, true).unwrap();
        let registration = selected.generation.required_computed_identity_registration().unwrap().clone();
        let mut expected = original.generation.clone();
        if let Some(policy) = selected.generation.required_held_out_joint() {
            expected = expected.with_required_held_out_joint(policy).unwrap();
        }
        assert_eq!(selected.generation, expected.with_required_computed_identity_registration(registration).unwrap());
        assert_eq!(selected.request, original.request); assert_eq!(selected.rounds, original.rounds);
        assert_eq!(selected.native.is_some(), original.native.is_some());
        assert_eq!(selected.process.is_some(), original.process.is_some());
        let wrapped = fs::read_to_string(&path).unwrap();
        // A new invocation supplies fresh measurements while retaining exactly
        // the original model, registration and all finite generation budgets.
        fs::write(&path, wrapped.replacen("\"check\":901", "\"check\":902", 1)
            .replacen("\"measurement_sequence\":1", "\"measurement_sequence\":2", 1)
            .replacen("observed.manifest", "not-yet-observed.manifest", 1)).unwrap();
        fs::remove_file(root.0.join("observed.manifest")).unwrap();
        for name in ["native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let historical = recipe::load(&path, &config, false).unwrap();
        assert_eq!(historical.generation, selected.generation);
        assert_eq!(historical.identity.as_ref().unwrap().check, 902);
        assert!(historical.native.is_none());
        fs::write(&path, &bytes).unwrap();
        assert_eq!(recipe::load(&path, &config, false).unwrap().generation, original.generation);
        assert_eq!(fs::read(&path).unwrap(), bytes); assert!(!config.store.exists());
    }
}

#[test]
fn identity_envelope_rejects_nested_reversed_and_unbounded_inputs_before_asset_or_store_access() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    provision(&root, &path, true); let valid = fs::read_to_string(&path).unwrap();
    recipe::load(&path, &config, false).unwrap();
    fs::remove_file(root.0.join("identity.passport")).unwrap();
    let mut invalid = vec![envelope(&root, &valid, 902, 2),
        format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{JOINT},"recipe":{valid}}}"#),
        valid.replacen("\"identity\":", "\"identity\":{},\"identity\":", 1),
        valid.replacen("\"identity\":", "\"skip_identity\":true,\"identity\":", 1),
        valid.replacen("fa.learned-identity-publication/1", "fa.learned-identity-publication/2", 1)];
    for (old, new) in [("\"observer_id\":99", "\"observer_id\":0"),
        ("\"timeout_ticks\":10000", "\"timeout_ticks\":0"),
        ("\"validity_ticks\":60000", "\"validity_ticks\":9999"),
        ("\"max_checks\":8", "\"max_checks\":65"),
        ("\"check\":901", "\"check\":0"),
        ("\"check\":901", "\"check\":901.0"),
        ("\"measurement_sequence\":1", "\"measurement_sequence\":0"),
        ("\"measurement_sequence\":1", "\"measurement_sequence\":1,\"extra\":false")] {
        invalid.push(valid.replacen(old, new, 1));
    }
    invalid.push(valid.replacen(&format!("\"scalar_products\":{MAX_DECODER_PRODUCTS}"),
        "\"scalar_products\":18446744073709551615", 1));
    for document in invalid {
        fs::write(&path, document).unwrap();
        let error = recipe::load(&path, &config, false).err().unwrap();
        assert!(!error.contains("No such file"), "malformed envelope reached passport I/O: {error}");
        assert!(!config.store.exists());
    }
}

#[test]
fn resident_identity_registration_and_fresh_check_sequence_are_admitted_before_time_or_startup() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let config = configured(&root); let path = native::write(&root);
        if let Some(owned) = mode { super::super::forecast::install(&root, &path, owned, 60_000); }
        provision(&root, &path, true);
        let (next, _) = native::next(&root, &config, 2);
        let original = fs::read_to_string(&next).unwrap();
        let fresh = original.replacen("\"check\":901", "\"check\":902", 1)
            .replacen("\"measurement_sequence\":1", "\"measurement_sequence\":2", 1);
        for (document, diagnostic) in [
            (original.clone(), "identity checks must be distinct"),
            (fresh.replacen("\"measurement_sequence\":2", "\"measurement_sequence\":1", 1), "identity checks must be distinct"),
            (fresh.replacen("\"observer_id\":99", "\"observer_id\":100", 1), "exact original finite generation"),
        ] {
            fs::write(&next, document).unwrap();
            let first = recipe::load(&path, &config, true).unwrap();
            let second = recipe::load(&next, &config, true).unwrap();
            let entries = [first, second].into_iter().map(|loaded| {
                let mut actor = peer_intake::actor_profile(&root, &config, loaded.request);
                actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
                resident::Entry { loaded, actor, release: LearnedTextRelease::Message }
            }).collect();
            let peers = fixture::peers(&root, &config);
            let error = resident::run(configured(&root), entries, &peers, false, None, None,
                || panic!("invalid identity sequence reached startup time")).err().unwrap();
            assert!(error.contains(diagnostic), "{error}");
            assert!(!config.store.exists()); assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
        fs::write(&next, fresh).unwrap();
        assert_eq!(recipe::load(&path, &config, true).unwrap().generation,
            recipe::load(&next, &config, true).unwrap().generation);
    }
}
