//! Original loaded models and publication owners; no synthetic output source.
//! Fixture weights and forecasts are controls, not deployment-quality evidence.
use super::*;
use super::continuation as native;
use super::super::{generation, prediction};

struct Run {
    config: Config,
    loaded: Loaded,
    reviewer: FileHumanReviewer,
    prediction: Option<prediction::Session>,
    driver: FileSupervisedDriver,
    wire: Wire,
}
impl Run {
    fn new(root: &Root, forecast: Option<bool>) -> Self {
        let config = configured(root);
        let path = native::write(root);
        if let Some(owned) = forecast { super::forecast::install(root, &path, owned, 60_000); }
        evidence(root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer, observer, _identity) = recovery::create_selected(&config, &loaded).unwrap();
        let prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        Self { config, loaded, reviewer, prediction,
            driver: FileSupervisedDriver::new(supervisor), wire: ActorWire::new(port) }
    }
    fn cold(&mut self) {
        refresh(&mut self.driver, &mut self.config, &deadline(), &mut || ElapsedTick(1000)).unwrap();
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        for _ in 0..prediction::MAX_REPLAY_EVENTS_PER_QUANTUM * 4 {
            let revision = host.revision(); host.observe_time(revision, ElapsedTick(1000)).unwrap();
        }
        host.clear_learned_replay_cache();
    }
    fn generate(&mut self, tick: u64) -> Result<bool, String> {
        let mut control = Control::new(&self.config, self.loaded.request, None).unwrap();
        generate_selected(&mut self.driver, &self.reviewer, &mut self.config,
            &mut control, &deadline(), &mut || ElapsedTick(tick), self.prediction.as_mut())
    }
    fn quantum(&mut self, session: &mut generation::Session) -> Result<usize, String> {
        session.advance_quantum(&mut self.driver.supervisor_mut().host_mut().unwrap())
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.config.store.join("delivery.bin")).unwrap() }
}

fn acknowledge_intent(run: &mut Run, session: &mut generation::Session) {
    let original = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    for _ in 0..1000 {
        assert!(run.quantum(session).unwrap() <= prediction::MAX_REPLAY_EVENTS_PER_QUANTUM);
        let current = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, original, "intent must return before numerical completion");
        if current.pending.is_some() { return; }
    }
    panic!("finite unchanged prompt history never acknowledged its intent");
}

#[test]
fn all_original_prompt_and_output_steps_match_the_synchronous_owner_with_separate_intent_yields() {
    let root = Root::new(); let control_root = Root::new();
    let mut run = Run::new(&root, None); let mut control = Run::new(&control_root, None);
    run.cold(); control.cold();
    let mut session = generation::Session::new();
    loop {
        let previous = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        if !previous.status.is_active() { break; }
        let revision = run.driver.supervisor().host().unwrap().revision();
        acknowledge_intent(&mut run, &mut session);
        assert_eq!(run.driver.supervisor().host().unwrap().revision(), revision + 1);
        let mut completed = false;
        for _ in 0..1000 {
            assert!(run.quantum(&mut session).unwrap() <= prediction::MAX_REPLAY_EVENTS_PER_QUANTUM);
            let current = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
            if current.numerical.position != previous.position {
                assert!(current.pending.is_none()); completed = true; break;
            }
        }
        assert!(completed, "unchanged original completion must progress");
        {
            let mut host = control.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.advance_learned_generation(revision, previous.actor_revision, previous.position).unwrap().unwrap();
        }
        let host = run.driver.supervisor().host().unwrap();
        let reference = control.driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            reference.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.revision(), revision + 2);
        assert_eq!(host.inspect().control.ledger, reference.inspect().control.ledger);
        assert_eq!(host.inspect().executions, 0);
    }
    let host = run.driver.supervisor().host().unwrap();
    assert_eq!(host.learned_text_message(run.loaded.evidence).unwrap().bytes(), b"A");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    drop(host);
    let before = run.bytes(); assert!(run.quantum(&mut session).is_err()); assert_eq!(run.bytes(), before);
    assert_eq!(cleanup(run.driver, 1000, 1), 0); assert_eq!(cleanup(control.driver, 1000, 1), 0);
}

#[test]
fn the_actual_nonforecast_operator_completes_cold_generation_and_both_publication_keys() {
    let root = Root::new(); let mut run = Run::new(&root, None); run.cold();
    let mut session = generation::Session::new();
    let before = run.bytes();
    let numerical = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(run.quantum(&mut session).unwrap(), prediction::MAX_REPLAY_EVENTS_PER_QUANTUM);
    assert_eq!(run.bytes(), before);
    assert_eq!(run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, numerical);
    // Abandoning an unacknowledged private replay does not publish an intent.
    // The actual command must now drive the same cold original history itself.
    drop(session);
    assert!(run.generate(1000).unwrap());
    submit(&mut run.driver, &mut run.wire, &mut run.config, &run.loaded,
        &deadline(), &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut run.config, &run.loaded,
        &run.reviewer, run.driver, &deadline());
    assert!(matches!(driver.step_computed_from_policy_file(&mut run.config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { .. }));
    let human = native::approve(&mut driver, &mut run.config, &run.reviewer, run.loaded.request);
    native::publish(&mut driver, &mut run.config, &human);
    assert!(executed(&mut run.wire, run.loaded.request));
    let host = driver.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert_eq!(view.confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(view.confirmed, view.published); assert_eq!(view.publication.executions, 1);
    assert_eq!(view.publication.control.ledger.charged,
        host.request_action(run.loaded.request).unwrap().spec().units);
    drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn raw_and_owned_prompt_replay_cannot_be_used_for_sampling_even_with_a_pending_forecast() {
    for owned in [false, true] {
        let root = Root::new(); let mut run = Run::new(&root, Some(owned)); run.cold();
        let mut session = generation::Session::new(); let mut ready = false;
        for _ in 0..1000 {
            let current = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
            assert_eq!(current.sampled_draws, 0);
            if current.status == GenerationStatus::Generating { ready = true; break; }
            assert!(run.quantum(&mut session).unwrap() <= prediction::MAX_REPLAY_EVENTS_PER_QUANTUM);
        }
        assert!(ready); let original = run.bytes();
        assert!(run.quantum(&mut session).is_err()); assert_eq!(run.bytes(), original);
        {
            let mut host = run.driver.supervisor_mut().host_mut().unwrap();
            run.prediction.as_mut().unwrap().begin_message(&mut host).unwrap();
        }
        let original = run.bytes();
        assert!(run.quantum(&mut session).is_err()); assert_eq!(run.bytes(), original);
        assert_eq!(run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
        // The original separately owned forecast lane is still usable.
        assert!(run.generate(1000).unwrap());
        let host = run.driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.learned_text_message(run.loaded.evidence).unwrap().bytes(), b"A");
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
        assert_eq!(host.inspect().executions, 0);
        drop(host); assert_eq!(cleanup(run.driver, 1000, 1), 0);
    }
}

#[test]
fn missing_policy_and_expired_workflow_after_prompt_intent_prevent_numerical_completion() {
    for forecast in [None, Some(false), Some(true)] {
        for failure in 0..3 {
            let root = Root::new(); let mut run = Run::new(&root, forecast); run.cold();
            let mut session = generation::Session::new(); acknowledge_intent(&mut run, &mut session);
            let before = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
            assert_eq!(before.numerical.position, 0); assert_eq!(before.numerical.sampled_draws, 0);
            assert!(before.pending.is_some());
            if failure == 1 { fs::remove_file(root.0.join("evidence.json")).unwrap(); }
            let result = run.generate(if failure == 2 { 61_001 } else { 1000 });
            let host = run.driver.supervisor().host().unwrap();
            if failure == 0 {
                assert!(result.unwrap());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            } else {
                assert!(result.is_err());
                let after = host.learned_generation_inspection().unwrap();
                assert_eq!(after.numerical, before.numerical); assert_eq!(after.pending, before.pending);
            }
            assert_eq!(host.inspect().executions, 0); assert_eq!(host.retained_requests(), 0);
            drop(host); assert_eq!(cleanup(run.driver, 1000, 1), 0);
        }
    }
}

#[test]
fn another_original_numerical_step_cannot_be_adopted_by_a_partially_replayed_session() {
    let root = Root::new(); let mut run = Run::new(&root, None); run.cold();
    let mut session = generation::Session::new(); run.quantum(&mut session).unwrap();
    {
        let mut host = run.driver.supervisor_mut().host_mut().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let revision = host.revision();
        host.advance_learned_generation(revision, numerical.actor_revision, numerical.position).unwrap().unwrap();
    }
    let before = run.bytes();
    let numerical = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert!(run.quantum(&mut session).is_err());
    assert_eq!(run.bytes(), before);
    assert_eq!(run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(cleanup(run.driver, 1000, 1), 0);
}

#[test]
fn interrupted_prompt_intent_recovers_and_completes_once_without_resetting_the_numerical_recipe() {
    let root = Root::new(); let mut run = Run::new(&root, None); run.cold();
    let mut session = generation::Session::new(); acknowledge_intent(&mut run, &mut session);
    let before = run.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
    drop(session); assert_eq!(cleanup(run.driver, 1000, 1), 0);
    let (host, reviewer) = recovery::open(&run.config, &run.loaded).unwrap();
    let recovered = host.learned_generation_inspection().unwrap();
    assert!(recovered.paused); assert_eq!(recovered.pending, before.pending);
    assert_eq!(recovered.numerical, before.numerical);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    run.reviewer = reviewer; run.driver = FileSupervisedDriver::new(supervisor); run.wire = ActorWire::new(port);
    assert!(run.generate(1000).unwrap());
    let host = run.driver.supervisor().host().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
    assert_eq!(host.learned_text_message(run.loaded.evidence).unwrap().bytes(), b"A");
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.retained_requests(), 0);
    drop(host); assert_eq!(cleanup(run.driver, 1000, 1), 0);
}
