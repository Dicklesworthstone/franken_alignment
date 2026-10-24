//! Fresh measurements after original recovery, exact offline registration pins,
//! and joint credibility composition without renewed numerical/effect budgets.
use super::*;
use super::super::super::{audit as audit_command, stopped as command_stop};
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;

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
fn recovered_joint_credibility_and_computed_identity_both_need_fresh_evidence_before_the_second_publication() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut case = Case::new(&root, mode, false, true);
        assert!(case.measure().unwrap()); let mut case = case.publish();
        {
            let mut host = case.driver.supervisor_mut().host_mut().unwrap();
            let state = host.inspect();
            let activation = super::super::qualification::activation(&case.config,
                state.control.sequence, state.control.ledger.epoch, 20, 2);
            let revision = host.revision();
            host.activate_credibility(revision, activation, &case.config.profile.committee).unwrap();
            assert!(host.held_out_joint_report(20).unwrap().unwrap().qualified());
            host.check_credibility().unwrap();
        }
        let old = case.numerical(); let prior_cost = case.cost();
        let (path, _) = native::next(&root, &case.config, 2);
        fs::write(&path, fs::read_to_string(&path).unwrap().replacen("\"check\":901", "\"check\":902", 1)).unwrap();
        let loaded = recipe::load(&path, &case.config, true).unwrap();
        assert_eq!(loaded.generation, case.loaded.generation);
        let Case { mut config, driver, wire, prediction, identity, histories, .. } = case;
        assert_eq!(cleanup(driver, 1000, 1), 0);
        drop(wire); drop(prediction); drop(identity); drop(histories);
        let (host, reviewer, forecast, observer) = recovery::open_selected(&config, &loaded).unwrap();
        assert!(host.check_credibility().is_err());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, old.numerical);
        assert!(host.identity_installation(901).unwrap().is_some(), "old result remains historical");
        let state = host.inspect();
        let capsule = super::super::qualification::activation(&config,
            state.control.sequence, state.control.ledger.epoch, 21, 3);
        let capsule_path = root.0.join("fresh-qualification.bin");
        fs::write(&capsule_path, capsule.encode_reference().unwrap()).unwrap();
        let prediction = forecast.map(|observer| prediction::Session::new(2, observer));
        let identity = Some(check::Session::new(observer.unwrap()));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let deadline = deadline(); let mut control = Control::new(&config, 2, None).unwrap();
        assert!(super::super::super::qualification::prepare(&mut driver, &config,
            Some(&capsule_path), (&mut control, &reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        let command = driver.supervisor().host().unwrap()
            .prepare_learned_text_stream_continuation(1, 2).unwrap();
        assert!(super::super::super::continuation::prepare(&mut driver, &reviewer, &mut config,
            command, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
        let mut case = Case { config, path, loaded, reviewer, driver, wire: ActorWire::new(port),
            prediction, identity, histories: Vec::new() };
        assert_eq!(case.driver.supervisor().host().unwrap().identity_status().unwrap(), IdentityStatus::Missing);
        let before = case.numerical(); let cost = case.cost();
        // A fresh ID alone cannot reuse measurement sequence1 from the original
        // matched check. Native comparison refuses after all five tokens; the
        // acknowledged work stays retained while only this basis is withdrawn.
        assert!(case.measure().is_err());
        let history = case.identity.as_ref().unwrap().history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status(), ComputedIdentityStatus::Committed);
        assert_eq!(history[0].work().completed_tokens, 5);
        assert_eq!(history[0].result().unwrap().observation.measurement.as_ref().unwrap_err(),
            &fa_reference::Error::Stale);
        assert_eq!(case.driver.supervisor().host().unwrap().identity_report(902).unwrap().outcome,
            IdentityOutcome::Unavailable);
        let selection = case.loaded.identity.as_mut().unwrap();
        selection.check = 903; selection.measurement_sequence = 2;
        fs::remove_file(root.0.join("observed.manifest")).unwrap();
        assert!(case.measure().is_err(), "historical match cannot replace fresh measurement input");
        fs::write(root.0.join("observed.manifest"), identity_wire::encode_manifest(&manifest())).unwrap();
        assert!(case.measure().unwrap());
        assert_eq!(case.numerical().numerical, before.numerical);
        assert_eq!(case.numerical().pending, before.pending); assert_eq!(case.cost(), cost);
        let history = case.identity.as_ref().unwrap().history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].work().completed_tokens, 5);
        assert_eq!(history.iter().map(|run| run.work().completed_tokens).sum::<u64>(), 10);
        if let (Some(original), Some(current)) = (prior_cost, cost) {
            assert_eq!(current.work, original.work);
            assert_eq!(current.retained_source_bytes, original.retained_source_bytes);
            assert_eq!(current.consistency.evidence, original.consistency.evidence);
        }
        let case = case.publish();
        let host = case.driver.supervisor().host().unwrap();
        assert!(host.held_out_joint_report(21).unwrap().unwrap().qualified());
        assert_eq!(host.identity_report(903).unwrap().outcome, IdentityOutcome::Matched);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
        assert_eq!(host.inspect().executions, 2);
        let charge = host.request_action(1).unwrap().spec().units + host.request_action(2).unwrap().spec().units;
        assert_eq!(host.inspect().control.ledger.charged, charge);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        if mode.is_some() { assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2); }
        drop(host); case.close();
    }
}

#[test]
fn historical_identity_registration_is_exact_before_cleanup_and_needs_no_live_manifest_helpers_or_source() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut case = Case::new(&root, mode, false, false);
        assert!(case.measure().unwrap()); let case = case.publish();
        let anchor = case.driver.supervisor().host().unwrap().history_anchor().unwrap();
        let before = case.driver.supervisor().host().unwrap().inspect();
        let cost = case.cost();
        let Case { config, path, mut loaded, driver, wire, prediction, identity, histories, .. } = case;
        assert_eq!(cleanup(driver, 1000, 1), 0);
        drop(wire); drop(prediction); drop(identity); drop(histories);
        loaded.recovery_anchor = Some(anchor.clone());
        let original = fs::read_to_string(&path).unwrap();
        let registered = fs::read(root.0.join("identity.passport")).unwrap();
        let passport = identity_wire::decode_passport(&registered).unwrap();
        let journal = fs::read(config.store.join("delivery.bin")).unwrap();
        let pending = config.store.join("delivery.pending");
        fs::write(&pending, b"identity refusal must preserve staging").unwrap();
        for name in ["observed.manifest", "evidence.json", "native-roster.json", "helper-model.json",
            "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        fs::remove_dir(root.0.join("peers")).unwrap();
        let reads = config.source.status().read_attempts;
        let mut variants = vec![(original.split_once("\"recipe\":").unwrap().1
            .strip_suffix('}').unwrap().to_owned(), registered.clone())];
        for (old, new) in [("\"observer_id\":99", "\"observer_id\":100"),
            ("\"timeout_ticks\":10000", "\"timeout_ticks\":10001"),
            ("\"validity_ticks\":60000", "\"validity_ticks\":60001"),
            ("\"max_checks\":8", "\"max_checks\":9")] {
            variants.push((original.replacen(old, new, 1), registered.clone()));
        }
        let anchors = passport.anchors().values().cloned().collect::<Vec<_>>();
        let mut wrong_manifest = passport.manifest().clone(); wrong_manifest.weights = [9; 32];
        let mut wide = anchors.clone();
        let first = &wide[0];
        wide[0] = IdentityAnchor::new(first.id(), first.profile(), first.stream(),
            first.stimulus().to_vec(), &[[-1.0, 1.0], [0.0, 0.0]]).unwrap();
        for wrong in [
            ModelPassport::new(52, 1, passport.manifest().clone(), anchors.clone()).unwrap(),
            ModelPassport::new(51, 2, passport.manifest().clone(), anchors.clone()).unwrap(),
            ModelPassport::new(51, 1, wrong_manifest, anchors).unwrap(),
            ModelPassport::new(51, 1, passport.manifest().clone(), wide).unwrap(),
        ] { variants.push((original.clone(), identity_wire::encode_passport(&wrong).unwrap())); }
        for (document, passport) in variants {
            fs::write(&path, document).unwrap(); fs::write(root.0.join("identity.passport"), passport).unwrap();
            let mut wrong = recipe::load(&path, &config, false).unwrap();
            wrong.recovery_anchor = Some(anchor.clone());
            assert!(recovery::open_selected(&config, &wrong).is_err());
            assert!(recovery::readonly::read(&config, &wrong, None, &mut || Duration::ZERO).is_err());
            let mut report = Vec::new();
            assert!(audit_command::run(&config, &wrong, 1, &anchor, &mut report, &mut || Ok(())).is_err());
            assert!(report.is_empty());
            assert!(command_stop::run(&config, &wrong, 801,
                || panic!("changed identity registration reached terminal clock"), &mut Vec::new()).is_err());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), journal);
            assert_eq!(fs::read(&pending).unwrap(), b"identity refusal must preserve staging");
        }
        fs::write(&path, original).unwrap(); fs::write(root.0.join("identity.passport"), registered).unwrap();
        let mut exact = recipe::load(&path, &config, false).unwrap();
        exact.recovery_anchor = Some(anchor.clone());
        assert_eq!(exact.generation, loaded.generation); assert!(exact.native.is_none());
        let response = recovery::readonly::read(&config, &exact, None, &mut || Duration::ZERO).unwrap();
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let mut report = Vec::new();
        audit_command::run(&config, &exact, 1, &anchor, &mut report, &mut || Ok(())).unwrap();
        let report = fa_reference::strict_json::parse(&report, fa_reference::strict_json::Limits::default()).unwrap();
        assert_eq!(report.get("authority").unwrap().as_str(), Some("none"));
        let value = report.get("knowledge").unwrap().get("value").unwrap();
        assert_eq!(value.get("application").unwrap().get("status").unwrap().as_str(), Some("applied"));
        assert_eq!(value.get("application_inputs").unwrap().as_str(), Some("equal_to_reviewed"));
        assert_eq!(value.get("historical_effects").unwrap().get("executions_at_journal_cut").unwrap().as_str(), Some("1"));
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), journal);
        assert_eq!(fs::read(&pending).unwrap(), b"identity refusal must preserve staging");
        let mut clocks = 0;
        command_stop::run(&config, &exact, 801, || { clocks += 1; ElapsedTick(1001) }, &mut Vec::new()).unwrap();
        assert_eq!(clocks, 1); assert!(!pending.exists());
        let stopped = publication(&config, &exact);
        assert_eq!(stopped.executions, before.executions); assert_eq!(stopped.payload, before.payload);
        assert_eq!(stopped.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(stopped.control.ledger.reserved, 0);
        if let Some(cost) = cost {
            let predictor = exact.generation.required_owned_pre_output_forecast().unwrap();
            let current = FileOversight::read_publication_with_owned_learned_consistency(&config.store,
                &config.profile, &exact.generation, predictor).unwrap().consistency;
            assert_eq!(current.work, cost.work); assert_eq!(current.retained_source_bytes, cost.retained_source_bytes);
            assert_eq!(current.consistency.evidence, cost.consistency.evidence);
        }
        let final_bytes = fs::read(config.store.join("delivery.bin")).unwrap();
        command_stop::run(&config, &exact, 801,
            || panic!("completed identity-pinned stop must be clock free"), &mut Vec::new()).unwrap();
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), final_bytes);
        assert_eq!(config.source.status().read_attempts, reads); assert!(!root.0.join("peers").exists());
    }
}
