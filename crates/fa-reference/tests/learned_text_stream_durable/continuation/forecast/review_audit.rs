//! Actual finite model/KV, sidecar, forecast and two-key publication histories.
//! The neutral likelihood table and manual congress ballots are explicit controls.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::delivery::persistent::observed::replay::{
    FileReviewReplay, FileReviewReplayCursor, FileReviewReplayStatus,
};
use fa_reference::action::consequence::oversight::{
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity},
};

struct Case {
    root: Directory,
    forecast: Forecast,
    config: FileLearnedConfig,
    host: FileOversight,
    reviewer: FileHumanReviewer,
    observer: FileConsistencyObserver,
}
impl Case {
    fn new(owned: bool, allowance: usize) -> Self {
        let root = Directory::new();
        let forecast = Forecast::new(owned);
        let config = forecast.pin(finite_recipe(allowance, false, false)
            .with_required_sidecar().unwrap()).unwrap()
            .with_forecast_text_stream_continuation().unwrap();
        let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(
            root.store(), profile(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        Self { root, forecast, config, host, reviewer, observer }
    }
    fn predict(&mut self, request: u64) {
        let numerical = self.host.learned_generation_inspection().unwrap().numerical;
        let revision = self.host.revision();
        match &self.forecast {
            Forecast::Residual(_) => { self.observer.forecast_hosted_request(&mut self.host,
                revision, request, numerical.actor_revision).unwrap().unwrap(); }
            Forecast::Learned(_) => { self.observer.forecast_owned_learned_request(&mut self.host,
                revision, request, numerical.actor_revision).unwrap().unwrap().prediction().unwrap(); }
        }
    }
    fn confirm(&mut self, request_id: u64, text: Option<&str>, tick: u64) -> u64 {
        let (attempt, action) = request(&mut self.host, request_id, text);
        let numerical = self.host.learned_generation_inspection().unwrap().numerical;
        let packet = self.host.begin_learned_sidecar(self.host.revision(), attempt,
            numerical.actor_revision, LearnedSidecarRequest {
                identity: SidecarIdentity { object_id: 1000 + attempt, generation: 1, transform_id: 7 },
                priority: Vec::new(), budget: SidecarCongressBudget::default(),
            }).unwrap();
        let inputs = packet.packet.input().clone();
        let round = 100 + attempt;
        self.host.begin_review(self.host.revision(), attempt, round, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
        }, snapshot()).unwrap();
        let anchor = self.host.review_anchor(round).unwrap();
        let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
        self.host.commit_review(self.host.revision(), round, "reviewer", digest).unwrap();
        self.host.open_reveals(self.host.revision(), round).unwrap();
        self.host.reveal_review(self.host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        let application = self.host.finish_review(self.host.revision(), round, Some(&inputs), snapshot())
            .unwrap().unwrap();
        let completed = self.host.inspect();
        let automatic = self.host.authorize(self.host.revision(), attempt, &inputs, snapshot()).unwrap();
        let offer = self.host.request_human_approval(self.host.revision(), 1000 + attempt,
            attempt, &inputs, ElapsedTick(40)).unwrap();
        let revision = self.host.revision();
        let human = self.reviewer.approve(&mut self.host, revision, &offer).unwrap();
        self.host.dispatch(self.host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        publish(&mut self.host, attempt, &inputs, tick);
        self.host.reconcile(self.host.revision(), attempt).unwrap();
        let replay = self.host.review_replay(round).unwrap();
        replay.archive().verify_receipt(&anchor, &application).unwrap();
        assert_eq!(replay.application(), &Ok(application));
        assert_eq!(replay.completed_snapshot(), &completed);
        assert_eq!(replay.archive().inputs.as_ref(), &inputs);
        round
    }
    fn first(&mut self) -> u64 {
        step(&mut self.host);
        self.predict(71);
        generate(&mut self.host);
        self.confirm(71, Some("A"), 2)
    }
    fn cursor(&self, round: u64) -> FileReviewReplayCursor {
        match &self.forecast {
            Forecast::Residual(_) => FileOversight::begin_read_review_replay_with_learned_generation(
                self.root.store(), &profile(), &self.config, round).unwrap(),
            Forecast::Learned(predictor) => FileOversight::begin_read_review_replay_with_owned_learned_consistency(
                self.root.store(), &profile(), &self.config, predictor, round).unwrap(),
        }
    }
    fn read(&self, round: u64) -> Result<FileReviewReplay, JournalError> {
        match &self.forecast {
            Forecast::Residual(_) => FileOversight::read_review_replay_with_learned_generation(
                self.root.store(), &profile(), &self.config, round),
            Forecast::Learned(predictor) => FileOversight::read_review_replay_with_owned_learned_consistency(
                self.root.store(), &profile(), &self.config, predictor, round),
        }
    }
}

fn finish(mut cursor: FileReviewReplayCursor, quantum: usize) -> FileReviewReplay {
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        let before = cursor.progress();
        let after = cursor.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(cursor.progress().attempted_events, cursor.progress().total_events);
    cursor.finish().unwrap()
}

#[test]
fn replay_both_forecast_representations_after_two_messages_and_independent_finish() {
    for owned in [false, true] {
        let mut case = Case::new(owned, 4);
        let first = case.first();
        let frozen = case.cursor(first);
        let first_cut = case.host.inspect();
        continue_message(&mut case.host, 71, 72);
        case.predict(72); generate(&mut case.host);
        let second = case.confirm(72, Some("B"), 3);
        case.predict(73);
        let terminal = case.confirm(73, None, 4);
        let before = case.host.inspect();
        let numerical = case.host.learned_generation_inspection().unwrap();
        let evidence = case.host.action_consistency_snapshot().unwrap();
        let files = case.root.files();
        assert_eq!(before.executions, 3);
        assert_eq!(evidence.evidence.samples(), 3);
        assert_eq!(finish(frozen, 1).journal_snapshot(), &first_cut);
        for round in [first, second, terminal] {
            let expected = case.host.review_replay(round).unwrap();
            for quantum in [1, 7] { assert_eq!(finish(case.cursor(round), quantum), expected); }
            assert_eq!(case.read(round).unwrap(), expected);
            // Recipe-owned predictors are also fully bound by the independently
            // supplied generation recipe, without a standalone fallback lane.
            assert_eq!(FileOversight::read_review_replay_with_learned_generation(
                case.root.store(), &profile(), &case.config, round).unwrap(), expected);
            assert!(FileOversight::read_review_replay(case.root.store(), &profile(), round).is_err());
        }
        assert_eq!(case.host.inspect(), before);
        assert_eq!(case.host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(case.host.action_consistency_snapshot().unwrap(), evidence);
        assert_eq!(case.root.files(), files);
    }
}

#[test]
fn same_first_message_does_not_allow_a_different_original_generation_allowance() {
    for owned in [false, true] {
        let mut case = Case::new(owned, 4); let round = case.first();
        let mut other = Case::new(owned, 3); other.first();
        assert_eq!(case.host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(other.host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        let files = case.root.files();
        assert_eq!(FileOversight::begin_read_review_replay_with_learned_generation(
            case.root.store(), &profile(), &other.config, round).unwrap_err(),
            JournalError::Contract(Error::Binding));
        assert_eq!(case.root.files(), files);
        assert_eq!(finish(case.cursor(round), 1), case.host.review_replay(round).unwrap());
    }
}

#[test]
fn read_only_audit_survives_owner_drop_and_preserves_staging_and_original_receipt() {
    for owned in [false, true] {
        let mut case = Case::new(owned, 4); let round = case.first();
        let expected = case.host.review_replay(round).unwrap();
        let cursor = case.cursor(round);
        let Case { root, config, forecast, host, .. } = case;
        drop(host);
        let staged = root.store().join("delivery.pending");
        std::fs::write(&staged, b"retained incomplete write").unwrap();
        let files = root.files();
        assert_eq!(finish(cursor, 1), expected);
        let read = match &forecast {
            Forecast::Residual(_) => FileOversight::read_review_replay_with_learned_generation(
                root.store(), &profile(), &config, round),
            Forecast::Learned(predictor) => FileOversight::read_review_replay_with_owned_learned_consistency(
                root.store(), &profile(), &config, predictor, round),
        }.unwrap();
        assert_eq!(read, expected);
        assert_eq!(root.files(), files);
        assert_eq!(std::fs::read(staged).unwrap(), b"retained incomplete write");
        assert_eq!(read.journal_snapshot().executions, 1);
    }
}

#[test]
fn cancellation_during_native_history_exports_no_review_and_changes_no_live_numerics() {
    for owned in [false, true] {
        let mut case = Case::new(owned, 4); let round = case.first();
        let files = case.root.files();
        let before = case.host.inspect();
        let numerical = case.host.learned_generation_inspection().unwrap();
        let mut cursor = case.cursor(round);
        let mid = cursor.progress().total_events / 2;
        let progress = cursor.advance(0, mid).unwrap();
        assert_eq!(progress.status, FileReviewReplayStatus::Replaying);
        let cancelled = cursor.cancel();
        assert_eq!(cancelled.attempted_events, progress.attempted_events);
        assert_eq!(cancelled.status, FileReviewReplayStatus::Cancelled);
        assert_eq!(case.root.files(), files);
        assert_eq!(case.host.inspect(), before);
        assert_eq!(case.host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(finish(case.cursor(round), 1), case.host.review_replay(round).unwrap());
    }
}

#[test]
fn owned_predictor_generation_is_independently_bound_before_any_native_replay() {
    let mut case = Case::new(true, 4); let round = case.first();
    let model = finite_model(); let neutral = BinaryForecast::new(32768, 32768).unwrap();
    let parameters = FileConsistencyParameters { probe_id: 91, probe_generation: 2,
        profile: model.cache_profile().layers()[&1].values().profile(),
        weights: vec![0.0; 6], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"A".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10 };
    let config = FileConsistencyConfig::new(parameters).unwrap().with_stream_messages(stream()).unwrap();
    let changed = FileLearnedConsistencyConfig::new(config, 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
    let before = case.root.files();
    assert_eq!(FileOversight::begin_read_review_replay_with_owned_learned_consistency(
        case.root.store(), &profile(), &case.config, &changed, round).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(case.root.files(), before);
    assert_eq!(finish(case.cursor(round), 1), case.host.review_replay(round).unwrap());
}
