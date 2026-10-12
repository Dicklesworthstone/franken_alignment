//! Actual original raw/owned forecasts, journal transitions and sampled logits.
//! Neutral forecast tables are controls, not evidence of calibration quality.
use super::*;
use super::super::tests::{Directory, action_spec, config, model, owner, profile,
    read_machine, snapshot, step};
use super::super::{FileOwnedPreOutputForecast, FileOwnedPreOutputStart};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::{consistency::{BinaryForecast,
    ErrorBudget, ForecastRegistration}, monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::{experiment::KvSide, decoder::sampling::monitored::GenerationStatus}};
use crate::action::consequence::delivery::persistent::{JournalIo,
    observed::{journal, decoder::learned::FileLearnedConfig,
        consistency::{FileConsistencyConfig, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig}}};

#[derive(Debug)]
enum Original {
    Raw(Box<FilePreOutputForecast>),
    Owned(Box<FileOwnedPreOutputForecast>),
}
impl Original {
    fn advance(&mut self, host: &mut FileOversight)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError> {
        let revision = host.revision();
        match self {
            Self::Raw(run) => run.advance(host, revision),
            Self::Owned(run) => run.advance(host, revision),
        }
    }
    fn cooperative(self) -> Run {
        match self {
            Self::Raw(run) => Run::Raw(Box::new((*run).into_cooperative())),
            Self::Owned(run) => Run::Owned(Box::new((*run).into_cooperative())),
        }
    }
}
#[derive(Debug)]
enum Run {
    Raw(Box<FilePreOutputStepper>),
    Owned(Box<FileOwnedPreOutputStepper>),
}
impl Run {
    fn advance(&mut self, host: &mut FileOversight, revision: u64, quantum: usize)
        -> Result<FilePreOutputQuantum, JournalError> {
        match self {
            Self::Raw(run) => run.advance_quantum(host, revision, quantum),
            Self::Owned(run) => run.advance_quantum(host, revision, quantum),
        }
    }
    fn poll(&mut self, host: &mut FileOversight, quantum: usize)
        -> Result<FilePreOutputQuantum, JournalError> {
        let revision = host.revision();
        self.advance(host, revision, quantum)
    }
    fn numerical(&self) -> &HostedLearnedInspection {
        match self { Self::Raw(run) => run.numerical(), Self::Owned(run) => run.numerical() }
    }
}
fn start(root: &Directory, owned: bool, alarm: bool)
    -> (FileOversight, Original, FileLearnedConfig) {
    let config = config(alarm, 1);
    let (mut host, _) = owner(root, &config);
    let profile = if owned { model().cache_profile().layers()[&1].values().profile() }
        else { model().residual_contract(1).unwrap().profile() };
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let predictor = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile, weights: vec![0.0; 2],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"visible".to_vec(), negative: neutral,
            at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap();
    let observer = if owned {
        let predictor = FileLearnedConsistencyConfig::new(predictor, 1, KvSide::Value,
            LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap();
        host.enable_learned_action_consistency(host.revision(), predictor).unwrap()
    } else {
        host.enable_action_consistency(host.revision(),
            predictor.with_hosted_residual(1).unwrap()).unwrap()
    };
    step(&mut host).unwrap(); // Complete original prompt, with zero samples.
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    let original = if owned {
        match observer.begin_owned_pre_output_request(&mut host, revision, 71,
            numerical.actor_revision, numerical.position).unwrap() {
            FileOwnedPreOutputStart::Ready(run) => Original::Owned(run),
            other => panic!("neutral original owned forecast not ready: {other:?}"),
        }
    } else {
        Original::Raw(Box::new(observer.begin_pre_output_request(&mut host, revision, 71,
            numerical.actor_revision, numerical.position).unwrap().unwrap()))
    };
    // Force the initial cold path; subsequent quanta must retain their genuine
    // acknowledged predecessor rather than reconstructing from zero each time.
    host.clear_learned_replay_cache();
    (host, original, config)
}
fn disk(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
fn complete(run: &mut Run, host: &mut FileOversight, quantum: usize)
    -> Result<Rc<GenerationEvent>, Error> {
    for _ in 0..1000 {
        let event = run.poll(host, quantum).unwrap();
        assert!(event.replayed_events() <= quantum);
        if let FilePreOutputQuantum::Completed { result, .. } = event { return result; }
    }
    panic!("finite unchanged original history did not complete");
}
fn intent(run: &mut Run, host: &mut FileOversight) {
    for _ in 0..1000 {
        let event = run.poll(host, 1).unwrap();
        assert!(event.replayed_events() <= 1);
        match event {
            FilePreOutputQuantum::IntentAcknowledged { .. } => return,
            FilePreOutputQuantum::ReplayingIntent { .. } => {}
            other => panic!("intent did not yield separately: {other:?}"),
        }
    }
    panic!("finite intent did not acknowledge");
}

#[test]
fn raw_and_owned_quanta_preserve_original_samples_journal_bytes_and_lifetime_work() {
    for owned in [false, true] {
        for quantum in [1, 2, 32] {
            let root = Directory::new(); let control_root = Directory::new();
            let (mut host, original, config) = start(&root, owned, false);
            let (mut control, mut reference, _) = start(&control_root, owned, false);
            let before = disk(&host);
            let mut run = original.cooperative();
            assert_eq!(disk(&host), before); // Conversion is not a transaction.
            if let Run::Owned(run) = &run {
                assert_eq!(run.forecast_report().prediction().unwrap(), run.prediction());
            }
            while run.numerical().status.is_active() {
                let prior = host.learned_generation_inspection().unwrap().numerical;
                let revision = host.revision();
                intent(&mut run, &mut host);
                assert_eq!(host.revision(), revision + 1);
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, prior);
                assert!(host.learned_generation_inspection().unwrap().pending.is_some());
                let actual = complete(&mut run, &mut host, quantum).unwrap();
                let expected = reference.advance(&mut control).unwrap().unwrap();
                assert_eq!(actual.sample(), expected.sample());
                assert_eq!(actual.status(), expected.status());
                assert_eq!(run.numerical(), &control.learned_generation_inspection().unwrap().numerical);
                assert_eq!(disk(&host), journal::encode(&host.profile,
                    host.store.identity(), &control.events).unwrap());
                assert_eq!(read_machine(&host, &config).snapshot(host.events.len()), host.inspect());
            }
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.retained_requests(), 0); assert_eq!(host.inspect().executions, 0);
            let bytes = disk(&host);
            assert_eq!(run.poll(&mut host, quantum).err(), Some(Error::WrongState.into()));
            assert_eq!(disk(&host), bytes);
        }
    }
}

#[test]
fn appended_time_is_replayed_without_restarting_the_verified_prefix() {
    for owned in [false, true] {
        let root = Directory::new(); let control_root = Directory::new();
        let (mut host, original, _) = start(&root, owned, false);
        let (mut control, mut reference, _) = start(&control_root, owned, false);
        let mut run = original.cooperative();
        let before = disk(&host);
        let FilePreOutputQuantum::ReplayingIntent { progress, replayed_events } =
            run.poll(&mut host, 1).unwrap() else { panic!("cold replay must yield"); };
        assert_eq!(replayed_events, 1); assert_eq!(progress.replayed_events, 1);
        assert_eq!(disk(&host), before);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        control.observe_time(control.revision(), ElapsedTick(2)).unwrap();
        let FilePreOutputQuantum::ReplayingIntent { progress, replayed_events } =
            run.poll(&mut host, 1).unwrap() else { panic!("appended tail cannot erase the original prefix"); };
        assert_eq!(replayed_events, 1); assert_eq!(progress.replayed_events, 2);
        let actual = complete(&mut run, &mut host, 1).unwrap();
        let expected = reference.advance(&mut control).unwrap().unwrap();
        assert_eq!(actual.sample(), expected.sample());
        assert_eq!(disk(&host), journal::encode(&host.profile,
            host.store.identity(), &control.events).unwrap());
    }
}

#[test]
fn invalid_quanta_stale_revisions_and_foreign_owners_preserve_pending_replay() {
    for owned in [false, true] {
        let root = Directory::new(); let foreign_root = Directory::new();
        let (mut host, original, _) = start(&root, owned, false);
        let (mut foreign, _, _) = start(&foreign_root, owned, false);
        let mut run = original.cooperative(); let _ = run.poll(&mut host, 1).unwrap();
        let bytes = disk(&host); let foreign_bytes = disk(&foreign); let revision = host.revision();
        assert_eq!(run.advance(&mut host, revision, 0).err(), Some(Error::InvalidInput.into()));
        assert_eq!(run.advance(&mut host, revision, MAX_JOURNAL_EVENTS + 1).err(), Some(Error::Limit.into()));
        assert_eq!(run.advance(&mut host, revision - 1, 1).err(), Some(Error::Stale.into()));
        assert_eq!(run.poll(&mut foreign, 1).err(), Some(Error::Binding.into()));
        assert_eq!(disk(&host), bytes); assert_eq!(disk(&foreign), foreign_bytes);
        let FilePreOutputQuantum::ReplayingIntent { progress, .. } = run.poll(&mut host, 1).unwrap()
            else { panic!("unchanged cold prefix must still be pending"); };
        assert_eq!(progress.replayed_events, 2);
        assert!(complete(&mut run, &mut host, 1).unwrap().sample().is_some());
    }
}

#[test]
fn expiry_between_quanta_cannot_start_or_complete_an_original_sample() {
    for owned in [false, true] {
        for acknowledged_intent in [false, true] {
            for expired in [false, true] {
                let root = Directory::new(); let (mut host, original, _) = start(&root, owned, false);
                let mut run = original.cooperative();
                if acknowledged_intent { intent(&mut run, &mut host); }
                else { let _ = run.poll(&mut host, 1).unwrap(); }
                let prior = run.numerical().clone();
                host.observe_time(host.revision(), ElapsedTick(if expired { 11 } else { 10 })).unwrap();
                let bytes = disk(&host);
                if expired {
                    assert!(run.poll(&mut host, 1).is_err());
                    assert_eq!(disk(&host), bytes);
                    assert_eq!(host.learned_generation_inspection().unwrap().numerical, prior);
                    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
                } else { assert!(complete(&mut run, &mut host, 1).unwrap().sample().is_some()); }
                assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
                assert_eq!(host.inspect().executions, 0);
            }
        }
    }
}

#[test]
fn another_numerical_step_or_consumed_forecast_cannot_be_adopted_by_catch_up() {
    for owned in [false, true] {
        for consume in [false, true] {
            let root = Directory::new(); let (mut host, original, _) = start(&root, owned, false);
            let mut run = original.cooperative(); let _ = run.poll(&mut host, 1).unwrap();
            if consume {
                let spec = action_spec(&host);
                host.submit_request(host.revision(), 71, spec, snapshot()).unwrap();
            } else { step(&mut host).unwrap(); }
            let bytes = disk(&host); let before = host.learned_generation_inspection().unwrap();
            assert_eq!(run.poll(&mut host, 1).err(),
                Some(if consume { Error::Binding } else { Error::Stale }.into()));
            assert_eq!(disk(&host), bytes);
            assert_eq!(host.learned_generation_inspection().unwrap(), before);
        }
    }
}

#[test]
fn dropping_preparation_preserves_forecast_and_only_acknowledged_intents() {
    for owned in [false, true] {
        for acknowledged_intent in [false, true] {
            let root = Directory::new(); let (mut host, original, _) = start(&root, owned, false);
            let mut run = original.cooperative();
            if acknowledged_intent { intent(&mut run, &mut host); }
            else { let _ = run.poll(&mut host, 1).unwrap(); }
            let bytes = disk(&host); let before = host.action_consistency_snapshot().unwrap();
            drop(run);
            assert_eq!(disk(&host), bytes); assert_eq!(host.action_consistency_snapshot().unwrap(), before);
            assert!(before.pending_attempt.is_some());
            let numerical = host.learned_generation_inspection().unwrap();
            assert_eq!(numerical.pending.is_some(), acknowledged_intent);
            assert_eq!(numerical.numerical.sampled_draws, 0);
        }
    }
}

#[test]
fn held_original_sample_is_not_released_or_retried_by_a_later_quantum() {
    for owned in [false, true] {
        let root = Directory::new(); let (mut host, original, _) = start(&root, owned, true);
        let mut run = original.cooperative();
        let event = complete(&mut run, &mut host, 1).unwrap();
        assert!(matches!(event.status(), GenerationStatus::Held(_)));
        assert!(event.sample().is_none()); assert!(event.accepted().is_none());
        assert_eq!(run.numerical().cumulative_work.sampling_attempts, 1);
        let bytes = disk(&host);
        assert_eq!(run.poll(&mut host, 1).err(), Some(Error::WrongState.into()));
        assert_eq!(disk(&host), bytes);
    }
}

#[test]
fn both_finalizers_preserve_unknown_storage_outcomes_and_never_cold_retry() {
    for owned in [false, true] {
        for completion in [false, true] {
            for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
                JournalIo::Rename, JournalIo::DirectorySync] {
                let root = Directory::new(); let (mut host, original, config) = start(&root, owned, false);
                let mut run = original.cooperative();
                if completion { intent(&mut run, &mut host); }
                let before = host.inspect(); host.store.fail_once(barrier);
                let mut failure = None;
                for _ in 0..1000 {
                    match run.poll(&mut host, 1) {
                        Err(error) => { failure = Some(error); break; }
                        Ok(event) => assert!(matches!(event,
                            FilePreOutputQuantum::ReplayingIntent { .. }
                            | FilePreOutputQuantum::ReplayingCompletion { .. })),
                    }
                }
                assert!(matches!(failure, Some(JournalError::Io(error)) if error.operation == barrier));
                assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
                assert_eq!(run.poll(&mut host, 1).err(), Some(JournalError::Unavailable));
                drop(host);
                let (mut recovered, _) = FileOversight::open_with_learned_generation(
                    root.store(), profile(), &config).unwrap();
                assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
                assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence.samples(), 0);
                assert!(recovered.learned_generation_inspection().unwrap().paused);
                assert_eq!(recovered.inspect().executions, 0);
                assert_eq!(run.poll(&mut recovered, 1).err(), Some(Error::Binding.into()));
            }
        }
    }
}
