//! The actual predictive stream and its original terminal recovery allowance.
//! Raw and owned forecasts use the same numerical, custody and stop contracts.
use super::*;
use fa_reference::action::consequence::delivery::{StopRequest, persistent::{
    RecoveryReserve, observed::{
        consistency::FileConsistencyObserver,
        decoder::learned::{FileLearnedRecoveryProgress, FileLearnedRecoveryStatus},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            predictive::{FilePredictiveRequirements, FilePredictiveRoles,
                learned::FilePredictiveLearnedRecovery,
                owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements}}},
    },
}};

fn configured(owned: bool) -> FileLearnedConfig {
    Forecast::new(owned).pin(finite_recipe(4, false, false)
        .with_required_sidecar().unwrap()).unwrap()
        .with_forecast_text_stream_continuation().unwrap()
}

fn requirements(host: &FileOversight, profile: &FileOversightProfile,
    generation: &FileLearnedConfig) -> FileRecoveryRequirements
{
    let control = host.inspect().control;
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: generation.text_stream_profile(), decoder: None,
            decoder_stop: None, source: generation.required_policy_source(), identity: None,
            campaigns: None, credential: None },
        effective_policy: profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    }
}

enum Recovery { Raw(FilePredictiveLearnedRecovery), Owned(FileOwnedPredictiveRecovery) }
impl Recovery {
    fn require(self, reserve: RecoveryReserve) -> Result<Self, JournalError> {
        match self {
            Self::Raw(run) => run.require_recovery_reserve(reserve).map(Self::Raw),
            Self::Owned(run) => run.require_recovery_reserve(reserve).map(Self::Owned),
        }
    }
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Raw(run) => run.progress(), Self::Owned(run) => run.progress() }
    }
    fn advance(&mut self, expected: usize, count: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        match self {
            Self::Raw(run) => run.advance(expected, count),
            Self::Owned(run) => run.advance(expected, count),
        }
    }
    fn finish(self) -> Result<(FileOversight, FilePredictiveRoles), JournalError> {
        match self { Self::Raw(run) => run.finish(), Self::Owned(run) => run.finish() }
    }
}

fn recover(root: &Directory, profile: &FileOversightProfile, generation: &FileLearnedConfig,
    expected: &FileRecoveryRequirements) -> Recovery
{
    if let Some(prediction) = generation.required_pre_output_forecast() {
        Recovery::Raw(FileOversight::begin_open_predictive_guarded_with_learned_generation(
            root.store(), profile.clone(), &FilePredictiveRequirements {
                oversight: expected.clone(), prediction: prediction.clone(), evaluation: None,
            }, generation).unwrap())
    } else {
        Recovery::Owned(FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile.clone(), &FileOwnedPredictiveRequirements {
                oversight: expected.clone(),
                prediction: generation.required_owned_pre_output_forecast().unwrap().clone(),
                evaluation: None,
            }, generation).unwrap())
    }
}

fn predict(host: &mut FileOversight, observer: &FileConsistencyObserver, owned: bool) {
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    let sequence = if owned {
        observer.forecast_owned_learned_request(host, revision, 71, numerical.actor_revision)
            .unwrap().unwrap().prediction().unwrap().observation().frame().sequence
    } else {
        observer.forecast_hosted_request(host, revision, 71, numerical.actor_revision)
            .unwrap().unwrap().observation().frame().sequence
    };
    assert_eq!(sequence, numerical.position);
}

#[test]
fn reserved_predictive_bootstrap_matches_original_forecasts_and_numerical_output() {
    for owned in [false, true] {
        let root = Directory::new(); let control_root = Directory::new();
        let config = configured(owned);
        let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
            root.store(), profile(), config.clone(), RecoveryReserve::terminal()).unwrap();
        let (mut control, _, control_observer) = FileOversight::create_with_pre_output_forecast(
            control_root.store(), profile(), config.clone()).unwrap();
        assert_eq!(host.revision(), 3); assert_eq!(control.revision(), 2);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert_eq!(control.journal_capacity().unwrap().reserve(), None);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            control.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
        assert!(host.action_consistency_required());
        assert_eq!(host.owned_learned_action_consistency_required(), owned);
        assert!(!host.clock_ready());
        let before = root.files();
        FileOversight::read_stream_publication_with_learned_generation(
            root.store(), &profile(), &config).unwrap();
        assert_eq!(root.files(), before);
        for (current, role) in [(&mut host, &observer), (&mut control, &control_observer)] {
            current.observe_time(current.revision(), ElapsedTick(1)).unwrap(); step(current);
            let numerical = current.learned_generation_inspection().unwrap().numerical;
            let revision = current.revision();
            assert_eq!(current.advance_learned_generation(revision, numerical.actor_revision,
                numerical.position).err(), Some(Error::Incomplete.into()));
            assert_eq!(current.revision(), revision);
            predict(current, role, owned); generate(current);
            request(current, 71, Some("A"));
            assert_eq!(current.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert_eq!(current.inspect().executions, 0);
        }
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            control.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence,
            control.action_consistency_snapshot().unwrap().evidence);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    }
}

#[test]
fn invalid_predictive_reserve_and_bootstrap_io_failure_return_no_owner() {
    let root = Directory::new();
    assert_eq!(FileOversight::create_with_pre_output_forecast_with_reserve(root.store(), profile(),
        finite_recipe(4, false, false).with_required_sidecar().unwrap(),
        RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
    assert!(!root.store().exists());
    for owned in [false, true] {
        for reserve in [RecoveryReserve { events: 2, bytes: 50 }, RecoveryReserve { events: 3, bytes: 49 }] {
            let root = Directory::new();
            assert_eq!(FileOversight::create_with_pre_output_forecast_with_reserve(root.store(),
                profile(), configured(owned), reserve).err(), Some(Error::InvalidInput.into()));
            assert!(!root.store().join("delivery.bin").exists());
        }
        let root = Directory::new(); let mut limited = profile(); limited.delivery.limits.events = 5;
        assert_eq!(FileOversight::create_with_pre_output_forecast_with_reserve(root.store(), limited,
            configured(owned), RecoveryReserve::terminal()).err(), Some(Error::Limit.into()));
        assert!(!root.store().join("delivery.bin").exists());
        let root = Directory::new();
        std::fs::write(root.store(), b"existing non-directory input").unwrap();
        assert!(FileOversight::create_with_pre_output_forecast_with_reserve(root.store(), profile(),
            configured(owned), RecoveryReserve::terminal()).is_err());
        assert_eq!(std::fs::read(root.store()).unwrap(), b"existing non-directory input");
    }
}

#[test]
fn predictive_recovery_checks_exact_reserve_before_replay_or_storage_cleanup() {
    for owned in [false, true] {
        let config = configured(owned);
        for reserve in [None, Some(RecoveryReserve { events: 4, bytes: 60 })] {
            let root = Directory::new();
            let (host, _, _) = match reserve {
                None => FileOversight::create_with_pre_output_forecast(root.store(), profile(), config.clone()),
                Some(reserve) => FileOversight::create_with_pre_output_forecast_with_reserve(
                    root.store(), profile(), config.clone(), reserve),
            }.unwrap();
            let expected = requirements(&host, &profile(), &config); drop(host);
            std::fs::write(root.store().join("delivery.pending"), b"unconfirmed evidence").unwrap();
            let before = root.files();
            let run = recover(&root, &profile(), &config, &expected);
            assert_eq!(run.progress().replayed_events, 0);
            assert_eq!(run.require(RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
            assert_eq!(root.files(), before);
        }
        let root = Directory::new();
        let (host, _, _) = FileOversight::create_with_pre_output_forecast_with_reserve(
            root.store(), profile(), config.clone(), RecoveryReserve::terminal()).unwrap();
        let expected = requirements(&host, &profile(), &config); drop(host);
        std::fs::write(root.store().join("delivery.pending"), b"unconfirmed evidence").unwrap();
        let before = root.files(); let mut run = recover(&root, &profile(), &config, &expected);
        run.advance(0, 1).unwrap();
        assert_eq!(run.require(RecoveryReserve::terminal()).err(), Some(Error::WrongState.into()));
        assert_eq!(root.files(), before);
        let mut wrong = expected.clone(); wrong.minimum.journal_revision += 1;
        let mut run = recover(&root, &profile(), &config, &wrong)
            .require(RecoveryReserve::terminal()).unwrap();
        let total = run.progress().total_events;
        assert!(run.advance(0, total).is_err()); assert!(run.finish().is_err());
        assert_eq!(root.files(), before);
        let mut run = recover(&root, &profile(), &config, &expected)
            .require(RecoveryReserve::terminal()).unwrap();
        run.advance(0, run.progress().total_events).unwrap();
        let (host, _) = run.finish().unwrap();
        assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert!(!root.store().join("delivery.pending").exists());
    }
}

#[test]
fn reserved_predictive_recovery_returns_new_observer_custody_without_new_numerical_work() {
    for owned in [false, true] {
        let root = Directory::new(); let config = configured(owned);
        let (mut host, _, old) = FileOversight::create_with_pre_output_forecast_with_reserve(
            root.store(), profile(), config.clone(), RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let expected = requirements(&host, &profile(), &config); drop(host);
        let before = root.files();
        let mut run = recover(&root, &profile(), &config, &expected)
            .require(RecoveryReserve::terminal()).unwrap();
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            let progress = run.progress(); run.advance(progress.replayed_events, 1).unwrap();
            assert_eq!(root.files(), before);
        }
        let (mut host, roles) = run.finish().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        let revision = host.revision();
        let foreign = if owned {
            old.forecast_owned_learned_request(&mut host, revision, 71, numerical.actor_revision).err()
        } else {
            old.forecast_hosted_request(&mut host, revision, 71, numerical.actor_revision).err()
        };
        assert_eq!(foreign, Some(Error::Binding.into()));
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        predict(&mut host, &roles.consistency_observer, owned); generate(&mut host);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
        request(&mut host, 71, Some("A"));
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn reserved_tail_fences_and_stops_with_unanswered_forecast_and_exhausted_ordinary_capacity() {
    for owned in [false, true] {
        let root = Directory::new(); let config = configured(owned); let mut limited = profile();
        limited.delivery.limits.events = 10;
        let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
            root.store(), limited.clone(), config.clone(), RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        predict(&mut host, &observer, owned);
        assert_eq!(host.revision(), 7);
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let expected = requirements(&host, &limited, &config);
        let before = root.files();
        assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)).err(), Some(Error::Limit.into()));
        assert_eq!(root.files(), before); drop(host);
        let mut run = recover(&root, &limited, &config, &expected)
            .require(RecoveryReserve::terminal()).unwrap();
        run.advance(0, run.progress().total_events).unwrap();
        let (mut host, roles) = run.finish().unwrap();
        assert_eq!(host.revision(), 8);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
        let before = root.files(); let revision = host.revision();
        let refused = if owned {
            roles.consistency_observer.forecast_owned_learned_request(&mut host, revision,
                72, numerical.actor_revision).is_err()
        } else {
            roles.consistency_observer.forecast_hosted_request(&mut host, revision,
                72, numerical.actor_revision).is_err()
        };
        assert!(refused); assert_eq!(root.files(), before);
        let control = host.inspect().control;
        host.request_stop(host.revision(), StopRequest { operation: 91,
            expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch }).unwrap();
        host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.revision(), 10);
        assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().stop.is_some());
        assert!(host.stop_progress().unwrap().drained());
    }
}
