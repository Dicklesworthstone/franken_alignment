//! Closed independent selection, legacy byte compatibility and early refusal.
use super::*;
use super::super::{processes, identity_tests};
use crate::publication::PublicationProfile;

const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":2,"max_member_outcomes":8}"#;

#[test]
fn checked_envelope_preserves_all_six_legacy_versions_and_original_identity_joint_order() {
    for (version, mode, identity) in [(1, None, false), (2, None, true), (3, None, false),
        (4, None, true), (5, Some(false), true), (5, Some(true), true), (6, None, false)] {
        let root = Root::new(); let config = configured(&root);
        let path = if matches!(version, 1 | 6) { fixture::write(&root, b"allow") } else { native::write(&root) };
        if matches!(version, 2 | 3) {
            fs::write(&path, fs::read_to_string(&path).unwrap().replace("fa.learned-publication/4",
                &format!("fa.learned-publication/{version}"))).unwrap();
        }
        if let Some(owned) = mode { super::super::forecast::install(&root, &path, owned, 60_000); }
        if version == 6 { fs::write(&path, processes::process_recipe(&root, "allow")).unwrap(); }
        if identity { identity_tests::provision(&root, &path, true); }
        let original_bytes = fs::read(&path).unwrap();
        let original = recipe::load(&path, &config, true).unwrap();
        assert!(original.generation.required_publication_validation().is_none());
        let producer = provision(&root, &config, &path, true, false);
        let selected = recipe::load(&path, &config, true).unwrap();
        let exact = selected.publication.as_ref().unwrap().learned_profile(&config.profile).unwrap();
        assert_eq!(selected.generation, original.generation.clone().with_required_publication_validation(exact).unwrap());
        assert_eq!(selected.generation.required_publication_validation(), Some(exact));
        assert!(exact.feed.unwrap().snapshot_fallback);
        assert_eq!(selected.request, original.request); assert_eq!(selected.rounds, original.rounds);
        assert_eq!(selected.native.is_some(), original.native.is_some());
        assert_eq!(selected.process.is_some(), original.process.is_some());
        // Neither live admission nor historical recipe loading obtains current
        // evidence: producer access occurs only at a real request boundary.
        drop(producer); fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
        if identity { fs::remove_file(root.0.join("observed.manifest")).unwrap(); }
        for name in ["native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let offline = recipe::load(&path, &config, false).unwrap();
        assert_eq!(offline.generation, selected.generation); assert!(offline.native.is_none());
        fs::write(&path, &original_bytes).unwrap();
        assert_eq!(recipe::load(&path, &config, false).unwrap().generation, original.generation);
        assert_eq!(fs::read(&path).unwrap(), original_bytes); assert!(!config.store.exists());
    }
}

pub(super) fn captures(root: &Root, version: u64) -> String {
    let feed = if version == 1 { String::new() } else {
        format!(r#", "feed":{{"path":"{}/feed.bin","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
            root.0.display())
    };
    let wait = if version == 4 { r#", "max_retries":3"# } else { "" };
    format!(r#"{{"schema":"fa.supervised-witnesses/{version}","source":91,"original":"{}/original.bin","current":"{}/current.bin"{feed}{wait},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), root.0.display())
}

#[test]
fn original_capture_feed_producer_and_whole_input_profiles_remain_explicit_and_stronger_unsupported_modes_refuse() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, envelope(&root, &original)).unwrap();
    let profiles = [(captures(&root, 1), false, false), (captures(&root, 2), true, false),
        (profile(&root, &config, false, false), true, false), (profile(&root, &config, true, false), true, true),
        (profile(&root, &config, false, true), true, false), (profile(&root, &config, true, true), true, true)];
    for (document, feed, fallback) in profiles {
        fs::write(root.0.join("publication.json"), &document).unwrap();
        let selected = recipe::load(&path, &config, false).unwrap();
        let native = selected.generation.required_publication_validation().unwrap();
        assert_eq!(native.feed.is_some(), feed);
        assert_eq!(native.feed.is_some_and(|value| value.snapshot_fallback), fallback);
        assert_eq!(native, PublicationProfile::decode(document.as_bytes()).unwrap()
            .learned_profile(&config.profile).unwrap());
        assert!(!config.store.exists());
    }
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    for (document, diagnostic) in [
        (captures(&root, 4), "finite-wait"),
        (format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{JOINT},"publication":{}}}"#,
            profile(&root, &config, false, false)), "learned joint qualification"),
        (profile(&root, &config, false, false).replacen("\"tenant\":1", "\"tenant\":2", 1), "Binding"),
    ] {
        fs::write(root.0.join("publication.json"), document).unwrap();
        let failure = recipe::load(&path, &config, false).err().unwrap();
        assert!(failure.contains(diagnostic), "{failure}");
        assert!(!failure.contains("No such file"), "unsupported publication reached model intake: {failure}");
        assert!(!config.store.exists());
    }
}

#[test]
fn checked_envelope_rejects_repetition_reversed_order_unknown_fields_and_unbounded_input_before_assets() {
    let root = Root::new(); let config = configured(&root); let path = native::write(&root);
    identity_tests::provision(&root, &path, true);
    let producer = provision(&root, &config, &path, false, false);
    let valid = fs::read_to_string(&path).unwrap();
    recipe::load(&path, &config, false).unwrap();
    drop(producer); fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    fs::remove_file(root.0.join("identity.passport")).unwrap();
    let invalid = [envelope(&root, &valid),
        format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{JOINT},"recipe":{valid}}}"#),
        valid.replacen("fa.learned-checked-publication/1", "fa.learned-checked-publication/2", 1),
        valid.replacen("\"publication\":", "\"bypass\":true,\"publication\":", 1),
        valid.replacen("\"publication\":", "\"publication\":null,\"publication\":", 1),
        valid.replacen(&root.0.join("publication.json").display().to_string(), "relative.json", 1),
        format!("{}{valid}", " ".repeat(65_536)),
    ];
    for document in invalid {
        fs::write(&path, document).unwrap();
        let failure = recipe::load(&path, &config, false).err().unwrap();
        assert!(!failure.contains("No such file"), "invalid envelope reached assets: {failure}");
        assert!(!config.store.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
