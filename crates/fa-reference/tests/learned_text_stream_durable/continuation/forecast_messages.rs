//! Required forecasts at two real original message boundaries and their finish.
//! Decoder, retained residual/KV source, journal, sidecar and endpoint are real;
//! the deliberately neutral forecast table and committee ballots are controls.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyObserver;
use fa_reference::action::consequence::oversight::{learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};

struct Scenario {
    root: Directory,
    forecast: Forecast,
    config: FileLearnedConfig,
    host: FileOversight,
    reviewer: FileHumanReviewer,
    observer: FileConsistencyObserver,
}
impl Scenario {
    fn new(owned: bool, max_new_tokens: usize) -> Self {
        let root = Directory::new(); let forecast = Forecast::new(owned);
        let config = forecast.pin(finite_recipe(max_new_tokens, false, false)
            .with_required_sidecar().unwrap()).unwrap()
            .with_forecast_text_stream_continuation().unwrap();
        let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(
            root.store(), profile(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        Self { root, forecast, config, host, reviewer, observer }
    }
    fn predict(&mut self, request: u64) -> Result<Result<u64, Error>, JournalError> {
        let numerical = self.host.learned_generation_inspection()?.numerical;
        let revision = self.host.revision();
        match &self.forecast {
            Forecast::Residual(_) => self.observer.forecast_hosted_request(&mut self.host,
                revision, request, numerical.actor_revision)
                .map(|result| result.map(|prediction| prediction.observation().frame().sequence)),
            Forecast::Learned(_) => self.observer.forecast_owned_learned_request(&mut self.host,
                revision, request, numerical.actor_revision)
                .map(|result| result.and_then(|report| report.prediction()
                    .map(|prediction| prediction.observation().frame().sequence))),
        }
    }
    fn samples(&self) -> usize { self.host.action_consistency_snapshot().unwrap().evidence.samples() }
    fn event(&self, attempt: u64) -> bool {
        match &self.forecast {
            Forecast::Residual(_) => self.host.action_consistency_observation(attempt).unwrap().event(),
            Forecast::Learned(_) => self.host.learned_action_consistency_observation(attempt).unwrap().event(),
        }
    }
    fn dispatch(&mut self, request_id: u64, text: Option<&str>) -> (u64, FrozenAction, CommitteeInput) {
        let (attempt, action) = request(&mut self.host, request_id, text);
        let input = sidecar_dispatch(&mut self.host, &self.reviewer, attempt, &action);
        (attempt, action, input)
    }
    fn confirm(&mut self, request_id: u64, text: Option<&str>, tick: u64) -> (u64, FrozenAction) {
        let (attempt, action, input) = self.dispatch(request_id, text);
        publish(&mut self.host, attempt, &input, tick);
        self.host.reconcile(self.host.revision(), attempt).unwrap();
        (attempt, action)
    }
    fn first(&mut self) {
        step(&mut self.host); // Only the original complete prompt; zero draws.
        assert_eq!(self.predict(71).unwrap().unwrap(), 1);
        generate(&mut self.host);
        self.confirm(71, Some("A"), 2);
        assert_eq!(self.samples(), 1);
    }
}

fn sidecar_dispatch(host: &mut FileOversight, reviewer: &FileHumanReviewer,
    attempt: u64, action: &FrozenAction) -> CommitteeInput
{
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let packet = host.begin_learned_sidecar(host.revision(), attempt, numerical.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + attempt,
            generation: 1, transform_id: 7 }, priority: Vec::new(),
            budget: SidecarCongressBudget::default() }).unwrap();
    let input = packet.packet.input().clone();
    let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), 1000 + attempt,
        attempt, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &offer).unwrap();
    host.dispatch(host.revision(), &automatic, &human, action, &input, snapshot()).unwrap();
    input
}

#[test]
fn explicit_recipe_forecasts_both_messages_and_finish_without_resetting_original_work() {
    let model = finite_model(); let mut text = finite_text(&model, 4, false);
    text.stop_tokens = BTreeSet::from([OTHER_CONTROL]);
    let mut control = model.observed_learned_text_generation(tokenizer(&model), text).unwrap();
    control.run_to_stop().unwrap();
    for owned in [false, true] {
        let mut case = Scenario::new(owned, 4);
        assert!(case.config.allows_text_stream_continuation());
        assert!(case.config.requires_forecast_text_stream_continuation());
        step(&mut case.host);
        assert_eq!(case.predict(71).unwrap().unwrap(), 1);
        assert_eq!(case.samples(), 0);
        generate(&mut case.host);
        let (first_attempt, first_action) = case.confirm(71, Some("A"), 2);
        assert!(case.event(first_attempt)); assert_eq!(case.samples(), 1);
        let first = case.host.learned_generation_inspection().unwrap().numerical;
        let ledger = case.host.inspect().control.ledger;
        continue_message(&mut case.host, 71, 72);
        let continued = case.host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(continued.work, first.work); assert_eq!(continued.telemetry, first.telemetry);
        assert_eq!(continued.cumulative_work, first.cumulative_work);
        assert_eq!(continued.position, first.position); assert_eq!(continued.sampled_draws, first.sampled_draws);
        assert_eq!(case.host.inspect().control.ledger, ledger);
        assert_eq!(case.predict(72).unwrap().unwrap(), first.position);
        let boundary = case.host.action_consistency_deadline().unwrap().unwrap();
        assert_eq!(boundary.actor_revision, first.actor_revision);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, continued);
        generate(&mut case.host);
        let message = case.host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
        assert_eq!(message.bytes(), b"B"); assert_eq!(message.evidence().tokens(), control.accepted_tokens());
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, control.position()); assert_eq!(numerical.work, control.work());
        assert_eq!(numerical.telemetry, control.telemetry_work()); assert_eq!(numerical.sampled_draws, 4);
        // The second message's pre-output forecast cannot be spent as a finish.
        let finish = case.host.stream_finish_spec(ElapsedTick(100)).unwrap();
        let files = case.root.files(); let revision = case.host.revision();
        assert_eq!(case.host.submit_request(revision, 72, finish, snapshot()).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
        assert_eq!(case.host.action_consistency_deadline().unwrap(), Some(boundary));
        let (second_attempt, second_action) = case.confirm(72, Some("B"), 3);
        assert!(!case.event(second_attempt)); assert_eq!(case.samples(), 2);
        assert_eq!(case.host.action_consistency_deadline().unwrap(), None);
        assert_eq!(case.host.prepare_learned_text_stream_continuation(72, 73).err(),
            Some(JournalError::Contract(Error::Limit)));
        assert_eq!(case.predict(73).unwrap().unwrap(), numerical.position);
        let (finish_attempt, finish_action) = case.confirm(73, None, 4);
        assert!(!case.event(finish_attempt)); assert_eq!(case.samples(), 3);
        let completed = case.host.stream_snapshot().unwrap();
        assert_eq!(completed.confirmed, completed.published); assert!(completed.confirmed.finished());
        assert_eq!(completed.confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(completed.publication.executions, 3);
        assert_eq!(completed.publication.control.ledger.charged,
            first_action.spec().units + second_action.spec().units + finish_action.spec().units);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        drop(case.host);
        let (host, _) = reopen(&case.root, &case.config);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, completed.confirmed);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        let evidence = host.action_consistency_snapshot().unwrap();
        assert_eq!(evidence.evidence.samples(), 3); assert!(!evidence.coverage_lost);
        assert_eq!(evidence.pending_attempt, None);
    }
}

#[test]
fn missing_wrong_request_and_late_forecasts_cannot_move_the_original_message_boundary() {
    for owned in [false, true] {
        let mut case = Scenario::new(owned, 4); case.first();
        continue_message(&mut case.host, 71, 72);
        let command = case.host.current_learned_text_continuation().unwrap().unwrap();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        let files = case.root.files(); let revision = case.host.revision();
        assert_eq!(case.host.advance_learned_generation(revision, numerical.actor_revision,
            numerical.position).err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(case.predict(73).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.current_learned_text_continuation().unwrap(), Some(command));
        assert_eq!(case.samples(), 1);
        assert_eq!(case.predict(72).unwrap().unwrap(), command.position());
        let pending = case.host.action_consistency_deadline().unwrap();
        step(&mut case.host);
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        let files = case.root.files(); let revision = case.host.revision();
        assert_eq!(numerical.sampled_draws, 3);
        assert_eq!(case.predict(72).err(), Some(JournalError::Contract(Error::WrongState)));
        assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
        assert_eq!(case.host.action_consistency_deadline().unwrap(), pending);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        generate(&mut case.host); case.confirm(72, Some("B"), 3);
        assert_eq!(case.samples(), 2);
    }
}

#[test]
fn a_forecast_at_the_previous_receipt_cut_remains_a_finish_and_cannot_fund_continuation() {
    for owned in [false, true] {
        let mut case = Scenario::new(owned, 4); case.first();
        let command = case.host.prepare_learned_text_stream_continuation(71, 72).unwrap();
        assert_eq!(case.predict(72).unwrap().unwrap(), command.position());
        let pending = case.host.action_consistency_deadline().unwrap();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        let evidence = case.host.action_consistency_snapshot().unwrap();
        let files = case.root.files(); let revision = case.host.revision();
        assert_eq!(case.host.begin_learned_text_stream_continuation(revision, command).err(),
            Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(case.host.prepare_learned_text_stream_continuation(71, 73).err(),
            Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.action_consistency_snapshot().unwrap(), evidence);
        assert_eq!(case.host.action_consistency_deadline().unwrap(), pending);
        assert_eq!(case.host.current_learned_text_continuation().unwrap(), None);
        // Its original intended finish is still usable with fresh independent keys.
        case.confirm(72, None, 3);
        assert!(case.host.stream_snapshot().unwrap().confirmed.finished());
        assert_eq!(case.host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.samples(), 2);
    }
}

#[test]
fn raw_forecast_handle_binds_the_exact_second_message_cut_and_keeps_its_original_draws() {
    let mut case = Scenario::new(false, 4); case.first();
    continue_message(&mut case.host, 71, 72);
    let numerical = case.host.learned_generation_inspection().unwrap().numerical;
    let revision = case.host.revision(); let files = case.root.files();
    for (actor, position) in [(numerical.actor_revision + 1, numerical.position),
        (numerical.actor_revision, numerical.position + 1)] {
        assert_eq!(case.observer.begin_pre_output_request(&mut case.host,
            revision, 72, actor, position).err(), Some(JournalError::Contract(Error::Stale)));
    }
    assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
    let mut run = case.observer.begin_pre_output_request(&mut case.host, revision, 72,
        numerical.actor_revision, numerical.position).unwrap().unwrap();
    assert_eq!(run.request(), 72); assert_eq!(run.numerical(), &numerical);
    assert_eq!(run.prediction().observation().frame().sequence, numerical.position);
    let revision = case.host.revision(); run.advance(&mut case.host, revision).unwrap().unwrap();
    let after_sample = case.host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(after_sample.sampled_draws, 3);
    let revision = case.host.revision(); let files = case.root.files();
    assert_eq!(case.observer.begin_pre_output_request(&mut case.host, revision, 72,
        after_sample.actor_revision, after_sample.position).err(), Some(JournalError::Contract(Error::WrongState)));
    assert_eq!(case.root.files(), files);
    run.advance(&mut case.host, revision).unwrap().unwrap();
    assert_eq!(run.numerical().sampled_draws, 4);
    let pending = case.host.action_consistency_deadline().unwrap();
    drop(run);
    assert_eq!(case.host.action_consistency_deadline().unwrap(), pending);
    case.confirm(72, Some("B"), 3); assert_eq!(case.samples(), 2);
}

#[test]
fn unconfirmed_output_cannot_supply_a_later_message_or_finish_forecast() {
    for owned in [false, true] {
        let mut case = Scenario::new(owned, 4);
        step(&mut case.host); case.predict(71).unwrap().unwrap(); generate(&mut case.host);
        let (attempt, _, input) = case.dispatch(71, Some("A"));
        for published in [false, true] {
            if published { publish(&mut case.host, attempt, &input, 2); }
            let numerical = case.host.learned_generation_inspection().unwrap().numerical;
            let revision = case.host.revision(); let files = case.root.files();
            assert!(case.host.prepare_learned_text_stream_continuation(71, 72).is_err());
            assert!(case.predict(72).is_err());
            assert_eq!(case.root.files(), files); assert_eq!(case.host.revision(), revision);
            assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(case.samples(), 1);
        }
        case.host.reconcile(case.host.revision(), attempt).unwrap();
        continue_message(&mut case.host, 71, 72);
        case.predict(72).unwrap().unwrap(); generate(&mut case.host);
        case.confirm(72, Some("B"), 3); assert_eq!(case.samples(), 2);
    }
}

#[test]
fn expiry_of_the_second_forecast_retains_the_first_sample_and_all_spent_work() {
    for owned in [false, true] {
        let mut case = Scenario::new(owned, 4); case.first();
        continue_message(&mut case.host, 71, 72); case.predict(72).unwrap().unwrap();
        step(&mut case.host);
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        let deadline = case.host.action_consistency_deadline().unwrap().unwrap();
        let revision = case.host.revision();
        assert!(case.observer.expire_forecast(&mut case.host, revision,
            deadline, deadline.expires_at).unwrap().unwrap());
        let evidence = case.host.action_consistency_snapshot().unwrap();
        assert!(evidence.coverage_lost); assert_eq!(evidence.evidence.samples(), 1);
        assert_eq!(case.host.action_consistency_deadline().unwrap(), Some(deadline));
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        let revision = case.host.revision(); let files = case.root.files();
        assert_eq!(case.host.advance_learned_generation(revision, numerical.actor_revision,
            numerical.position).err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(case.root.files(), files);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    }
}

#[test]
fn recovery_preserves_pending_message_obligation_and_refuses_a_different_recipe_without_writes() {
    for owned in [false, true] {
        for sampled in 0..=1 {
            let mut case = Scenario::new(owned, 4); case.first();
            continue_message(&mut case.host, 71, 72); case.predict(72).unwrap().unwrap();
            for _ in 0..sampled { step(&mut case.host); }
            let numerical = case.host.learned_generation_inspection().unwrap().numerical;
            let deadline = case.host.action_consistency_deadline().unwrap();
            let command = case.host.current_learned_text_continuation().unwrap();
            let evidence = case.host.action_consistency_snapshot().unwrap().evidence;
            let wrong = case.forecast.pin(finite_recipe(4, false, false)
                .with_required_sidecar().unwrap()).unwrap();
            assert_ne!(case.config, wrong);
            drop(case.host);
            let files = case.root.files();
            assert_eq!(FileOversight::read_stream_publication_with_learned_generation(
                case.root.store(), &profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
            assert_eq!(FileOversight::open_with_learned_text_stream(
                case.root.store(), profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
            assert_eq!(case.root.files(), files);
            let (mut host, _) = reopen(&case.root, &case.config);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(host.current_learned_text_continuation().unwrap(), command);
            assert_eq!(host.action_consistency_deadline().unwrap(), deadline);
            let recovered = host.action_consistency_snapshot().unwrap();
            assert_eq!(recovered.evidence, evidence); assert!(recovered.coverage_lost);
            assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
            let revision = host.revision(); let files = case.root.files();
            assert!(host.advance_learned_generation(revision, numerical.actor_revision, numerical.position).is_err());
            assert_eq!(case.root.files(), files);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        }
    }
}
