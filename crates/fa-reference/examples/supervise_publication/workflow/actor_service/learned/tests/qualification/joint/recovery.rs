//! Exact independently retained policy selection for historical commands. No
//! fresh evaluator capsule, current evidence, helper process or socket is needed.
use super::*;
use super::super::super::super::stopped as command_stop;
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use std::cell::Cell;

fn publication(config: &Config, loaded: &Loaded) -> FileDeliverySnapshot {
    if let Some(predictor) = loaded.generation.required_owned_pre_output_forecast() {
        FileOversight::read_publication_with_owned_learned_consistency(&config.store,
            &config.profile, &loaded.generation, predictor).unwrap().publication
    } else {
        FileOversight::read_publication_with_learned_generation(&config.store,
            &config.profile, &loaded.generation).unwrap()
    }
}

#[test]
fn joint_policy_recovery_pins_every_field_before_cleanup_and_keeps_receipts_and_stop_offline() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let seed = seeded(&root, mode);
        let mut loaded = recipe::load(&seed.first_recipe, &seed.config, false).unwrap();
        let (host, human, observer) = recovery::open_selected(&seed.config, &loaded).unwrap();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
        assert!(host.held_out_joint_report(20).unwrap().unwrap().qualified());
        assert!(host.check_credibility().is_err(), "historical success cannot qualify recovered live work");
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.sampled_draws, 2); assert_eq!(numerical.position, 4);
        let original_cost = owned_cost(&host);
        let anchor = host.history_anchor().unwrap();
        loaded.recovery_anchor = Some(anchor.clone());
        drop(observer); drop(human); drop(host);
        let before = publication(&seed.config, &loaded);
        let journal = fs::read(seed.config.store.join("delivery.bin")).unwrap();
        let pending = seed.config.store.join("delivery.pending");
        fs::write(&pending, b"joint recovery staging must survive refused profiles").unwrap();
        let original = fs::read_to_string(&seed.first_recipe).unwrap();
        let prefix = format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{POLICY},"recipe":"#);
        let legacy = original.strip_prefix(&prefix).unwrap().strip_suffix('}').unwrap().to_owned();
        for name in ["evidence.json", "native-roster.json", "helper-model.json",
            "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe",
            "helper-sampling.json", "qualification-20.bin"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let reads = seed.config.source.status().read_attempts;
        let mut changed = vec![legacy];
        for (old, replacement) in [
            ("\"id\":71", "\"id\":72"),
            ("\"generation\":1", "\"generation\":2"),
            ("\"minimum_safe_roots\":1", "\"minimum_safe_roots\":2"),
            ("\"minimum_violation_roots\":2", "\"minimum_violation_roots\":3"),
            ("\"maximum_escape_ppm\":0", "\"maximum_escape_ppm\":1"),
            ("\"maximum_false_stop_ppm\":0", "\"maximum_false_stop_ppm\":1"),
            ("\"max_cases\":3", "\"max_cases\":4"),
            ("\"max_member_outcomes\":12", "\"max_member_outcomes\":16"),
        ] { changed.push(original.replacen(old, replacement, 1)); }
        for document in changed {
            fs::write(&seed.first_recipe, document).unwrap();
            let mut wrong = recipe::load(&seed.first_recipe, &seed.config, false).unwrap();
            wrong.recovery_anchor = Some(anchor.clone());
            assert!(recovery::open_selected(&seed.config, &wrong).is_err());
            assert!(recovery::readonly::read(&seed.config, &wrong, None,
                &mut || Duration::ZERO).is_err());
            assert!(command_stop::run(&seed.config, &wrong, 801,
                || panic!("wrong joint policy must refuse before terminal time"), &mut Vec::new()).is_err());
            assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), journal);
            assert_eq!(fs::read(&pending).unwrap(), b"joint recovery staging must survive refused profiles");
            assert_eq!(publication(&seed.config, &loaded), before);
        }
        fs::write(&seed.first_recipe, original).unwrap();
        let exact = recipe::load(&seed.first_recipe, &seed.config, false).unwrap();
        assert_eq!(exact.generation, loaded.generation); assert!(exact.native.is_none());
        let response = recovery::readonly::read(&seed.config, &loaded, None,
            &mut || Duration::ZERO).unwrap();
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), journal);
        assert_eq!(fs::read(&pending).unwrap(), b"joint recovery staging must survive refused profiles");
        let calls = Cell::new(0); let mut output = Vec::new();
        command_stop::run(&seed.config, &loaded, 801, || {
            calls.set(calls.get() + 1); ElapsedTick(1001)
        }, &mut output).unwrap();
        assert_eq!(calls.get(), 1);
        assert!(String::from_utf8(output).unwrap().contains("\"status\":\"stopped_drained\""));
        let stopped = publication(&seed.config, &loaded);
        assert_eq!(stopped.executions, before.executions);
        assert_eq!(stopped.payload, before.payload);
        assert_eq!(stopped.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(stopped.control.ledger.reserved, 0);
        assert_eq!(stopped.stop.as_ref().unwrap().request().operation, 801);
        assert!(!pending.exists());
        if let Some(prior) = original_cost {
            let predictor = loaded.generation.required_owned_pre_output_forecast().unwrap();
            let current = FileOversight::read_publication_with_owned_learned_consistency(&seed.config.store,
                &seed.config.profile, &loaded.generation, predictor).unwrap().consistency;
            assert_eq!(current.work, prior.work);
            assert_eq!(current.retained_source_bytes, prior.retained_source_bytes);
            assert_eq!(current.consistency.evidence, prior.consistency.evidence);
        }
        let completed = fs::read(seed.config.store.join("delivery.bin")).unwrap();
        command_stop::run(&seed.config, &loaded, 801,
            || panic!("exact completed joint-policy stop retry is clock free"), &mut Vec::new()).unwrap();
        assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), completed);
        assert_eq!(seed.config.source.status().read_attempts, reads);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
