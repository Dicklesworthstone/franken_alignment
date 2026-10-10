//! Actual original generations, fit replay, native model work and durable effects.
use super::*;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluationStatus;

#[test]
fn learned_command_pending_generation_recovery_native_congress_and_both_keys() {
    for approve in [true, false] {
        let root = Root::new(); let mut config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (mut host, obsolete_reviewer) = recovery::create(&config, &loaded).unwrap();
        for position in 0..3 {
            host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, position);
            host.advance_learned_generation(host.revision(), numerical.actor_revision, position).unwrap().unwrap();
        }
        let before = host.learned_generation_inspection().unwrap();
        assert_eq!(before.numerical.position, 3);
        assert_eq!(before.numerical.sampled_draws, 1);
        assert!(host.learned_text_message(loaded.evidence).is_err()); // Partial A is withheld.
        assert!(host.inspect().payload.is_empty()); assert_eq!(host.inspect().executions, 0);
        host.begin_learned_step(host.revision(), before.numerical.actor_revision, 3).unwrap();
        let pending = host.learned_generation_inspection().unwrap();
        assert!(pending.pending.is_some()); drop(host);

        let loaded = recipe::load(&path, &config, true).unwrap();
        let (mut host, reviewer) = recovery::open(&config, &loaded).unwrap();
        let recovered = host.learned_generation_inspection().unwrap();
        assert!(recovered.paused);
        assert_eq!(recovered.pending, pending.pending);
        assert_eq!(recovered.numerical, before.numerical);
        let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
        assert!(host.advance_learned_generation(host.revision(), recovered.numerical.actor_revision, 3).is_err());
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control, &deadline,
            &mut || ElapsedTick(1000)).unwrap());
        {
            let host = driver.supervisor().host().unwrap();
            let complete = host.learned_generation_inspection().unwrap();
            assert_eq!(complete.numerical.position, 4);
            assert_eq!(complete.numerical.sampled_draws, 2);
            assert_eq!(complete.numerical.work.admitted_tokens, 4);
            assert!(complete.numerical.telemetry.compression_source_values > 0);
            assert!(complete.pending.is_none() && !complete.paused);
            assert_eq!(host.learned_text_message(loaded.evidence).unwrap().bytes(), b"A");
            assert!(host.inspect().payload.is_empty()); assert_eq!(host.inspect().executions, 0);
        }
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        // A constant real observation is legal: complete native commitments and
        // reveals may finish early without inventing a later clock.
        let reads_before_native = config.source.status().read_attempts;
        let events_before_native = driver.supervisor().host().unwrap()
            .file_source_status().unwrap().capture.retained_events;
        let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
            &deadline, &mut || ElapsedTick(1000));
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        let mut driver = reviewed.driver; let history = reviewed.history.unwrap(); assert_native(&history);
        // Every quantum reacquires and durably records policy, even when the
        // producer's bytes are equal. Startup adds exactly one observation.
        assert_eq!(config.source.status().read_attempts - reads_before_native,
            history.polls() as u64 + 1);
        assert_eq!(driver.supervisor().host().unwrap().file_source_status().unwrap()
            .capture.retained_events - events_before_native, history.polls() + 1);
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
            701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
        let action = driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            assert!(matches!(obsolete_reviewer.approve(&mut host, revision, &offer),
                Err(JournalError::Contract(Error::Binding))));
            assert_eq!(host.revision(), revision);
            assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
        }
        if approve {
            let human = {
                let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
                reviewer.approve(&mut host, revision, &offer).unwrap()
            };
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
            assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
            let reads = config.source.status().read_attempts;
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
            assert_eq!(config.source.status().read_attempts, reads);
        } else {
            let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            reviewer.reject(&mut host, revision, &offer).unwrap();
            let revision = host.revision();
            host.cancel_request(revision, 1).unwrap();
        }
        assert_eq!(executed(&mut wire, 1), approve);
        {
            let host = driver.supervisor().host().unwrap(); let state = host.inspect();
            assert_eq!(state.executions, u64::from(approve));
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(state.control.ledger.charged, if approve { action.spec().payload.len() as u64 } else { 0 });
            assert_eq!(state.payload, if approve { b"A".to_vec() } else { Vec::new() });
            // Finite original source history remains sufficient through both
            // independent reviews, reservation, and the final publication read.
            let reads_bound = NATIVE_INPUT_BYTES + NATIVE_POLL_OVERHEAD + 32;
            assert!(config.source.status().read_attempts <= reads_bound as u64);
            let captured = host.file_source_status().unwrap().capture;
            assert!(captured.retained_events <= reads_bound);
            assert!(captured.retained_events < config.source_policy.limits.events);
            assert!(captured.fault.is_none());
        }
        assert_eq!(cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
        drop(history);
        // Exact numerical recipe remains required, but a historical receipt has
        // no live evidence/native-helper dependency and cannot renew a deadline.
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        for name in ["native-roster.json", "helper-model.json", "helper-weights.safetensors",
            "helper-tokenizer.bbpe", "helper-sampling.json", "helper-monitor.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let mut loaded = recipe::load(&path, &config, false).unwrap(); loaded.ttl_ms = 1;
        assert!(loaded.native.is_none());
        let generation = loaded.generation.clone(); let profile = config.profile.clone(); let store = config.store.clone();
        let result = recovery::resume(config, loaded, || panic!("terminal recovery must not read time")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approve);
        let before_output = fs::read(store.join("delivery.bin")).unwrap();
        let mut output = Vec::new();
        assert_eq!(emit(result, &mut output).is_ok(), approve);
        assert!(!output.windows(7).any(|bytes| bytes == b"payload"));
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), before_output);
        let disk = FileOversight::read_publication_with_learned_generation(&store, &profile, &generation).unwrap();
        assert_eq!(disk.executions, u64::from(approve)); assert_eq!(disk.control.ledger.reserved, 0);
    }
}

#[test]
fn learned_command_native_deny_cannot_become_a_permitting_job() {
    let root = Root::new();
    let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"deny");
    let mut control = Control::new(&config, 1, None).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver; let history = reviewed.history.unwrap(); assert_native(&history);
    assert!(!matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
    assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
        701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
    assert!(!executed(&mut wire, 1));
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.charged, 0);
    assert!(state.payload.is_empty());
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn learned_command_partial_native_cancel_retains_work_and_original_actor_ticket() {
    let root = Root::new();
    let (mut config, loaded, _reviewer, driver, mut wire, deadline) = prepared(&root, b"allow");
    let mut run = review::start(driver, &mut config, &loaded, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    let reservation = run.review().reservation();
    for _ in 0..loaded.native_limits.polls {
        if native_records(run.review()).any(|record| record.progress.native.work.decoder.tokens > 0
            && record.progress.native.status == NativeEvaluationStatus::Running) { break; }
        run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    }
    let work: Vec<_> = native_records(run.review()).map(|record|
        (record.progress.completed_probe_work, record.progress.native.work)).collect();
    assert!(work.iter().any(|(_, work)| work.decoder.tokens > 0));
    assert!(native_records(run.review()).all(|record| record.progress.basis != Some(SidecarDecisionBasis::NativeModel)));
    run.cancel(run.review().revision()).unwrap();
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(native_records(run.review()).map(|record|
        (record.progress.completed_probe_work, record.progress.native.work)).collect::<Vec<_>>(), work);
    let handoff = run.into_handoff().unwrap();
    assert_eq!(handoff.review.status(), NativeReviewStatus::Cancelled);
    assert!(matches!(response(&mut wire, 1).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(cleanup(handoff.driver, 1000, 1), 0); drop(handoff.review);
}

#[test]
fn learned_command_changed_policy_after_both_reviews_cannot_publish_old_approval() {
    let root = Root::new();
    let (mut config, loaded, reviewer, driver, _wire, deadline) = prepared(&root, b"allow");
    let mut control = Control::new(&config, 1, None).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver; let history = reviewed.history.unwrap(); assert_native(&history);
    let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
        701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
    let human = {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        reviewer.approve(&mut host, revision, &offer).unwrap()
    };
    evidence(&root, &config, 2, false, false);
    let event = driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1001), Some(&human)).result;
    assert!(!matches!(event, Ok(FileDriverEvent::Dispatched { .. })));
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 2);
    drop(host);
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn learned_command_idle_phase_wait_uses_real_time_without_spending_original_polls() {
    let root = Root::new();
    let (mut config, loaded, _reviewer, driver, _wire, deadline) = prepared(&root, b"unknown");
    config.timing.commit_ms = 30; config.timing.reveal_ms = 60;
    let mut run = review::start(driver, &mut config, &loaded, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    for _ in 0..loaded.native_limits.polls {
        if native_records(run.review()).all(|record| record.failure.is_some()) { break; }
        run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Running);
    assert_eq!(native_records(run.review()).count(), 2);
    assert!(native_records(run.review()).all(|record|
        record.failure.is_some() && record.progress.native.work.decoder.tokens > 2
            && record.progress.native.work.sampled_draws == 2));
    let mut control = Control::new(&config, 1, None).unwrap();
    let polls = run.review().polls(); let reserve = run.review().reservation();
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    let reads = config.source.status().read_attempts;
    let started = Instant::now();
    let mut clock = || ElapsedTick(1000 + started.elapsed().as_millis() as u64);
    assert!(!review::wait_for_phase(&mut run, &mut control, config.timing.poll_ms,
        &deadline, &mut clock).unwrap());
    assert!(started.elapsed() >= Duration::from_millis(30));
    assert_eq!(run.review().polls(), polls); assert_eq!(run.review().reservation(), reserve);
    assert_eq!(config.source.status().read_attempts, reads);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    run.step_from_policy_file(&mut config.source, &mut clock, None).result.unwrap();
    if run.review().status() == NativeReviewStatus::Running {
        assert!(!review::wait_for_phase(&mut run, &mut control, config.timing.poll_ms,
            &deadline, &mut clock).unwrap());
        run.step_from_policy_file(&mut config.source, &mut clock, None).result.unwrap();
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().polls() <= polls + 2);
    let handoff = run.into_handoff().unwrap();
    assert!(!matches!(handoff.driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
    assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(cleanup(handoff.driver, 1000, 1), 0); drop(handoff.review);
}
