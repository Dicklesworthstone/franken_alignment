//! /5 operator execution using original raw/owned forecasts, numerical sources,
//! native reviewer inference, separate human custody and confirmed publication.
//! Neutral probabilities and synthetic model weights are deterministic controls;
//! green would not establish calibration or deployment model quality.
use super::*;
use super::continuation as native;
use super::super::{continuation, finish, prediction};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::activation::monitor::{
    MonitorOutcome, learned::LearnedMonitorWork,
};
use fa_reference::action::consequence::oversight::consistency::{
    ConsistencyDeadline, ConsistencyEventDomain,
};
use std::path::PathBuf;

fn fixture(root: &Root, owned: bool, age: u64) -> (Config, PathBuf, Loaded) {
    let config = configured(root);
    let path = native::write(root);
    super::forecast::install(root, &path, owned, age);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    (config, path, loaded)
}

fn start(config: &Config, loaded: &Loaded)
    -> (FileHumanReviewer, prediction::Session, FileSupervisedDriver, Wire) {
    let (host, reviewer, observer) = recovery::create_selected(config, loaded).unwrap();
    assert!(host.action_consistency_required());
    let prediction = prediction::Session::new(loaded.request, observer.unwrap());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (reviewer, prediction, FileSupervisedDriver::new(supervisor), ActorWire::new(port))
}

fn reopen(config: &Config, loaded: &Loaded, driver: FileSupervisedDriver)
    -> (FileHumanReviewer, prediction::Session, FileSupervisedDriver, Wire) {
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let (host, reviewer, observer) = recovery::open_selected(config, loaded).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    let prediction = prediction::Session::new(loaded.request, observer.unwrap());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (reviewer, prediction, FileSupervisedDriver::new(supervisor), ActorWire::new(port))
}

fn pending(driver: &FileSupervisedDriver, source: u64, samples: usize) -> ConsistencyDeadline {
    let host = driver.supervisor().host().unwrap();
    let deadline = host.action_consistency_deadline().unwrap().unwrap();
    assert_eq!(deadline.source_sequence, source);
    let snapshot = host.action_consistency_snapshot().unwrap();
    assert_eq!(snapshot.evidence.samples(), samples);
    assert_eq!(snapshot.pending_attempt, Some(deadline.attempt));
    assert!(!snapshot.coverage_lost && !snapshot.evidence.crossed());
    deadline
}

fn observed(driver: &FileSupervisedDriver, loaded: &Loaded, owned: bool,
    deadline: ConsistencyDeadline, event: bool, sample: usize) {
    let host = driver.supervisor().host().unwrap();
    let (frame, actual, domain, count, actor) = if owned {
        let observation = host.learned_action_consistency_observation(deadline.attempt).unwrap();
        let report = host.learned_action_consistency_report(deadline.attempt).unwrap();
        assert!(report.prediction().is_ok());
        assert!(report.work().probe_coordinates > 0);
        (observation.prediction().observation().frame(), observation.event(),
            observation.event_domain(), observation.sample(), observation.forecast_actor_revision())
    } else {
        let observation = host.action_consistency_observation(deadline.attempt).unwrap();
        (observation.prediction().observation().frame(), observation.event(),
            observation.event_domain(), observation.sample(), observation.forecast_actor_revision())
    };
    assert_eq!(frame.stream, 5); assert_eq!(frame.sequence, deadline.source_sequence);
    assert_eq!(frame.position + 1, deadline.source_sequence);
    assert_eq!(actual, event); assert_eq!(count, sample); assert_eq!(actor, deadline.actor_revision);
    assert_eq!(domain, ConsistencyEventDomain::StreamMessagePrefix(loaded.stream));
    let snapshot = host.action_consistency_snapshot().unwrap();
    assert_eq!(snapshot.evidence.samples(), sample); assert_eq!(snapshot.pending_attempt, None);
    assert!(!snapshot.coverage_lost && !snapshot.evidence.crossed());
    assert_eq!(host.action_consistency_deadline().unwrap(), None);
}

#[test]
fn loaded_required_forecasts_publish_two_native_reviewed_messages_and_separately_forecast_finish() {
    for owned in [false, true] {
        let root = Root::new();
        let (mut config, _, first) = fixture(&root, owned, 60_000);
        let (reviewer, mut prediction, mut driver, mut wire) = start(&config, &first);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction)).unwrap());
        let forecast = pending(&driver, 2, 0);
        let first_numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        assert_eq!(first_numerical.position, 4); assert_eq!(first_numerical.sampled_draws, 2);
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        submit(&mut driver, &mut wire, &mut config, &first, &deadline, &mut || ElapsedTick(1000)).unwrap();
        observed(&driver, &first, owned, forecast, true, 1);
        assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
            701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        let (mut driver, history) = native::review_current(&mut config, &first, &reviewer, driver, &deadline);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        let first_human = native::approve(&mut driver, &mut config, &reviewer, 1);
        native::publish(&mut driver, &mut config, &first_human); assert!(executed(&mut wire, 1));

        let (_, second) = native::next(&root, &config, 2);
        assert_eq!(first.generation, second.generation);
        let (reviewer, mut prediction, mut driver, mut wire) = reopen(&config, &second, driver);
        drop(history);
        let command = driver.supervisor().host().unwrap().prepare_learned_text_stream_continuation(1, 2).unwrap();
        let mut control = Control::new(&config, 2, None).unwrap();
        assert!(continuation::prepare(&mut driver, &reviewer, &mut config, command,
            &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
        {
            let host = driver.supervisor().host().unwrap();
            let current = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(current.position, first_numerical.position);
            assert_eq!(current.work, first_numerical.work);
            assert_eq!(current.cumulative_work, first_numerical.cumulative_work);
            assert_eq!(current.telemetry, first_numerical.telemetry);
            assert_eq!(current.sampled_draws, first_numerical.sampled_draws);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        }
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction)).unwrap());
        let forecast = pending(&driver, 4, 1);
        {
            let host = driver.supervisor().host().unwrap();
            let message = host.learned_text_message(second.evidence).unwrap();
            assert_eq!(message.bytes(), b"A");
            assert_eq!(message.evidence().tokens(), &[120, 121, 65, 256, 65, 256]);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 4);
            assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        }
        // The same ready output and request cannot spend its message forecast
        // as an explicit finish. The original pending obligation remains usable.
        assert!(submit_release(&mut driver, &mut wire, &mut config, &second,
            LearnedTextRelease::Finish, &deadline, &mut || ElapsedTick(1000)).is_err());
        assert_eq!(driver.supervisor().host().unwrap().action_consistency_deadline().unwrap(), Some(forecast));
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
        submit(&mut driver, &mut wire, &mut config, &second, &deadline, &mut || ElapsedTick(1000)).unwrap();
        observed(&driver, &second, owned, forecast, true, 2);
        let (mut driver, history) = native::review_current(&mut config, &second, &reviewer, driver, &deadline);
        let human = native::approve(&mut driver, &mut config, &reviewer, 2);
        assert!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), Some(&first_human)).result.is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
        native::publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 2));
        let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;

        let (path, last) = native::next(&root, &config, 3);
        assert_eq!(last.generation, second.generation);
        let (reviewer, mut prediction, mut driver, mut wire) = reopen(&config, &last, driver);
        drop(history);
        let mut control = Control::new(&config, 3, None).unwrap();
        assert!(finish::prepare(&mut driver, &reviewer, &mut config, &last, 2,
            &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            prediction.finish_request(&mut host).unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        }
        let forecast = pending(&driver, 6, 2);
        submit_release(&mut driver, &mut wire, &mut config, &last, LearnedTextRelease::Finish,
            &deadline, &mut || ElapsedTick(1000)).unwrap();
        observed(&driver, &last, owned, forecast, false, 3);
        let (mut driver, history) = native::review_current(&mut config, &last, &reviewer, driver, &deadline);
        let human = native::approve(&mut driver, &mut config, &reviewer, 3);
        native::publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 3));
        {
            let host = driver.supervisor().host().unwrap(); let stream = host.stream_snapshot().unwrap();
            assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
            assert!(stream.confirmed.finished()); assert_eq!(stream.confirmed, stream.published);
            assert_eq!(stream.publication.executions, 3); assert_eq!(stream.pending, None);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            let charge: u64 = (1..=3).map(|request| host.request_action(request).unwrap().spec().units).sum();
            assert_eq!(stream.publication.control.ledger.charged, charge);
            let frame = ReleaseFrame::decode(&host.request_action(2).unwrap().spec().payload).unwrap();
            assert_eq!(frame.prior_messages(), &["A"]); assert_eq!(frame.message(), Some("A"));
            assert_eq!(host.learned_text_stream_finish_request(2, 3).unwrap().deadline, deadline.logical);
            if owned {
                let work = host.learned_action_consistency_snapshot().unwrap();
                assert!(work.retained_source_bytes > 0 && work.work.probe_coordinates > 0);
                assert!(!work.has_unreported_work);
            }
        }
        assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
        for file in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(file)).unwrap();
        }
        let loaded = recipe::load(&path, &config, false).unwrap();
        let result = recovery::resume_finish(config, loaded, 2,
            || panic!("confirmed predictive finish must not forecast, sample or repeat reviews")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    }
}

#[test]
fn required_operator_generation_refuses_missing_observer_before_time_source_or_prompt_work() {
    for owned in [false, true] {
        let root = Root::new(); let (mut config, _, loaded) = fixture(&root, owned, 60_000);
        let (reviewer, mut prediction, mut driver, mut wire) = start(&config, &loaded);
        let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let disk = fs::read(config.store.join("delivery.bin")).unwrap();
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control, &deadline,
            &mut || panic!("missing forecast custody must fail before clock or source access")).is_err());
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, before);
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction)).unwrap());
        let forecast = pending(&driver, 2, 0);
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        observed(&driver, &loaded, owned, forecast, true, 1);
        assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn forecast_acquisition_yields_fresh_time_and_policy_before_any_expired_sample() {
    for owned in [false, true] {
        for expired in [false, true] {
            let root = Root::new(); let (mut config, _, loaded) = fixture(&root, owned, 1);
            let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
            let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
            let mut now = 1000;
            let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
                &deadline, &mut || { if expired { now += 1; } ElapsedTick(now) }, Some(&mut prediction));
            assert_eq!(generated.is_err(), expired, "{generated:?}");
            let host = driver.supervisor().host().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let forecast = host.action_consistency_deadline().unwrap().unwrap();
            assert_eq!(forecast.source_sequence, 2);
            assert_eq!(numerical.position, if expired { 2 } else { 4 });
            assert_eq!(numerical.sampled_draws, if expired { 0 } else { 2 });
            assert_eq!(numerical.work.sampling_attempts, if expired { 0 } else { 2 });
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.inspect().executions, 0);
            if expired {
                assert!(host.inspect().control.ledger.elapsed.unwrap() >= forecast.expires_at);
                assert_eq!(numerical.status, GenerationStatus::Generating);
            } else {
                assert!(generated.unwrap());
                assert!(matches!(numerical.status, GenerationStatus::Finished(GenerationStop::StopToken(_))));
            }
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

#[test]
fn owned_forecast_budget_exhaustion_retains_original_inconclusive_report_without_fallback_or_first_draw() {
    for exhausted in [false, true] {
        let root = Root::new(); let (mut config, path, _) = fixture(&root, true, 60_000);
        if exhausted {
            let path = root.0.join("forecast.json");
            let original = fs::read_to_string(&path).unwrap();
            assert!(original.contains("\"encoded_bytes\":1048576"));
            // A legal zero per-job allowance cannot pay even the coarse base.
            // The actual captured inventory still incurs its separate charge.
            fs::write(path, original.replacen("\"encoded_bytes\":1048576", "\"encoded_bytes\":0", 1)).unwrap();
        }
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction));
        assert_eq!(generated.is_err(), exhausted, "{generated:?}");
        assert_eq!(prediction.has_message(), !exhausted);
        assert_eq!(driver.phase(), FileDriverPhase::Idle);
        assert_eq!(driver.next_review_deadline(), None);
        let host = driver.supervisor().host().unwrap();
        assert!(host.owned_learned_action_consistency_required());
        assert_eq!(host.hosted_consistency_layer().unwrap(), None);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, if exhausted { 2 } else { 4 });
        assert_eq!(numerical.sampled_draws, if exhausted { 0 } else { 2 });
        assert_eq!(numerical.work.sampling_attempts, if exhausted { 0 } else { 2 });
        assert!(matches!(host.request_status(loaded.request), Err(JournalError::Contract(Error::Missing))));
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
        let report = host.learned_action_consistency_report(1).unwrap();
        let charged = host.learned_action_consistency_snapshot().unwrap();
        assert_eq!(report.monitor().frame().sequence, 2);
        assert_eq!(report.work(), charged.work);
        assert!(charged.retained_source_bytes > 0);
        assert!(!charged.has_unreported_work);
        assert_eq!(charged.consistency.evidence.samples(), 0);
        assert_eq!(charged.consistency.coverage_lost, exhausted);
        if exhausted {
            assert!(generated.unwrap_err().starts_with("original owned learned forecast is inconclusive:"));
            assert_eq!(report.prediction().unwrap_err(), Error::Limit);
            assert_eq!(report.monitor().outcome(), MonitorOutcome::BudgetExhausted);
            assert_eq!(report.work(), LearnedMonitorWork::default());
            assert_eq!(host.pending_forecast_request().unwrap(), None);
            assert_eq!(host.action_consistency_deadline().unwrap(), None);
            drop(host);
            let disk = fs::read(config.store.join("delivery.bin")).unwrap();
            {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                assert!(prediction.begin_message(&mut host).is_err());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
                assert_eq!(host.learned_action_consistency_snapshot().unwrap(), charged);
            }
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
            assert_eq!(cleanup(driver, 1000, 1), 0);
            let (host, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let recovered = host.learned_action_consistency_snapshot().unwrap();
            assert_eq!(recovered.work, charged.work);
            assert_eq!(recovered.retained_source_bytes, charged.retained_source_bytes);
            assert!(recovered.consistency.coverage_lost);
            assert_eq!(recovered.consistency.evidence, charged.consistency.evidence);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(host.learned_action_consistency_report(1).unwrap().prediction().unwrap_err(), Error::Limit);
        } else {
            assert!(generated.unwrap()); assert!(report.prediction().is_ok());
            assert!(report.work().encoded_bytes > 0 && report.work().probe_coordinates > 0);
            assert_eq!(host.pending_forecast_request().unwrap(), Some(1));
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

fn prompt_boundary(driver: &mut FileSupervisedDriver, config: &mut Config, deadline: &Deadline) {
    for _ in 0..2 {
        refresh(driver, config, deadline, &mut || ElapsedTick(1000)).unwrap();
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let current = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(current.status, GenerationStatus::Prefilling);
        let revision = host.revision();
        host.advance_learned_generation(revision, current.actor_revision, current.position).unwrap().unwrap();
    }
    let current = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(current.status, GenerationStatus::Generating);
    assert_eq!(current.position, 2); assert_eq!(current.sampled_draws, 0);
}

#[test]
fn pending_forecast_does_not_replace_a_missing_policy_source_at_the_first_sample_boundary() {
    for owned in [false, true] {
        for missing in [false, true] {
            let root = Root::new(); let (mut config, _, loaded) = fixture(&root, owned, 60_000);
            let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
            let deadline = deadline(); prompt_boundary(&mut driver, &mut config, &deadline);
            {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                prediction.begin_message(&mut host).unwrap();
            }
            let host = driver.supervisor().host().unwrap();
            let before = host.learned_generation_inspection().unwrap().numerical;
            let forecast = host.action_consistency_deadline().unwrap();
            let work = owned.then(|| host.learned_action_consistency_snapshot().unwrap());
            drop(host);
            if missing { fs::remove_file(root.0.join("evidence.json")).unwrap(); }
            let mut control = Control::new(&config, 1, None).unwrap();
            let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
                &deadline, &mut || ElapsedTick(1000), Some(&mut prediction));
            assert_eq!(generated.is_err(), missing, "{generated:?}");
            let host = driver.supervisor().host().unwrap();
            let after = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(host.action_consistency_deadline().unwrap(), forecast);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.pending_forecast_request().unwrap(), Some(1));
            assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
            assert!(matches!(host.request_status(1), Err(JournalError::Contract(Error::Missing))));
            assert_eq!(driver.phase(), FileDriverPhase::Idle);
            assert_eq!(driver.next_review_deadline(), None);
            if missing {
                assert_eq!(after.position, before.position);
                assert_eq!(after.sampled_draws, before.sampled_draws);
                assert_eq!(after.work, before.work); assert_eq!(after.telemetry, before.telemetry);
                if let Some(work) = work {
                    let current = host.learned_action_consistency_snapshot().unwrap();
                    assert_eq!(current.work, work.work);
                    assert_eq!(current.retained_source_bytes, work.retained_source_bytes);
                }
            } else {
                assert!(generated.unwrap()); assert_eq!(after.position, 4); assert_eq!(after.sampled_draws, 2);
            }
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

#[test]
fn predictive_recovery_retains_unanswered_forecast_and_spent_work_without_rearming_generation() {
    for owned in [false, true] {
        for sampled in 0..=1 {
            let root = Root::new(); let (mut config, _, loaded) = fixture(&root, owned, 60_000);
            let (_, mut prediction, mut driver, _) = start(&config, &loaded);
            let deadline = deadline(); prompt_boundary(&mut driver, &mut config, &deadline);
            {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                prediction.begin_message(&mut host).unwrap();
                for _ in 0..sampled { prediction.advance(&mut host).unwrap(); }
            }
            let host = driver.supervisor().host().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, sampled);
            let forecast = host.action_consistency_deadline().unwrap();
            let evidence = host.action_consistency_snapshot().unwrap().evidence;
            let learned = owned.then(|| host.learned_action_consistency_snapshot().unwrap());
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(prediction);
            let disk = fs::read(config.store.join("delivery.bin")).unwrap();
            assert!(recovery::open(&config, &loaded).is_err());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
            let (host, reviewer, observer) = recovery::open_selected(&config, &loaded).unwrap();
            assert!(host.learned_generation_inspection().unwrap().paused);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(host.action_consistency_deadline().unwrap(), forecast);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
            assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
            if let Some(learned) = learned {
                let recovered = host.learned_action_consistency_snapshot().unwrap();
                assert_eq!(recovered.work, learned.work);
                assert_eq!(recovered.retained_source_bytes, learned.retained_source_bytes);
                assert_eq!(recovered.has_unreported_work, learned.has_unreported_work);
            }
            let mut prediction = prediction::Session::new(loaded.request, observer.unwrap());
            let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
            let mut driver = FileSupervisedDriver::new(supervisor); let _wire = ActorWire::new(port);
            let disk = fs::read(config.store.join("delivery.bin")).unwrap();
            let mut control = Control::new(&config, loaded.request, None).unwrap();
            assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
                &deadline, &mut || panic!("lost forecast coverage cannot refresh or resume"),
                Some(&mut prediction)).is_err());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
            assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
            assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

#[test]
fn independent_predictor_changes_and_recipe_downgrade_refuse_before_recovery_writes() {
    for owned in [false, true] {
        let root = Root::new(); let (config, path, loaded) = fixture(&root, owned, 60_000);
        let (host, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical; drop(host);
        let disk = fs::read(config.store.join("delivery.bin")).unwrap();
        let file = root.0.join("forecast.json"); let original = fs::read_to_string(&file).unwrap();
        let mut changes = vec![
            ("\"bias_bits\":0", "\"bias_bits\":1065353216"),
            ("[1065353216,2147483648]", "[1065353216,0]"),
            ("\"null\":32768", "\"null\":32767"),
            ("\"event_prefix_hex\":\"41\"", "\"event_prefix_hex\":\"42\""),
            ("\"denominator\":100", "\"denominator\":101"),
            ("\"max_predictions\":8", "\"max_predictions\":9"),
            ("\"max_prediction_age_ticks\":60000", "\"max_prediction_age_ticks\":60001"),
        ];
        if owned {
            changes.extend([("\"encoded_bytes\":1048576", "\"encoded_bytes\":1048575"),
                ("\"max_retained_source_bytes\":8388608", "\"max_retained_source_bytes\":8388607")]);
        }
        for (before, after) in changes {
            assert!(original.contains(before));
            fs::write(&file, original.replacen(before, after, 1)).unwrap();
            let changed = recipe::load(&path, &config, false).unwrap();
            assert_ne!(changed.generation, loaded.generation);
            assert!(recovery::open_selected(&config, &changed).is_err(), "accepted {after}");
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        }
        fs::write(&file, &original).unwrap();
        let recipe = fs::read_to_string(&path).unwrap();
        fs::write(&path, recipe.replace("fa.learned-publication/5", "fa.learned-publication/4")
            .replace(&format!("\"forecast\":\"{}\",", file.display()), "")).unwrap();
        let downgraded = recipe::load(&path, &config, false).unwrap();
        assert!(recovery::open_selected(&config, &downgraded).is_err());
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
        fs::write(&path, recipe).unwrap();
        let original = recipe::load(&path, &config, false).unwrap();
        let (host, _, observer) = recovery::open_selected(&config, &original).unwrap();
        assert!(observer.is_some()); assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    }
}
