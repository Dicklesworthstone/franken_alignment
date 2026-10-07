//! Original numerical generation and Store faults, not synthetic event outcomes.
//! The fixture's model/probability table is not a calibrated detector.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::{KvRow, MAX_CHECKED_KV_BYTES};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{
    consistency::{FileConsistencyConfig, FileConsistencyParameters},
    publication::CheckedCompletion};
use crate::action::consequence::delivery::EndpointOutcome;
// Reuse exact original generator, journal and whole-input/two-key fixtures.
// Some fixture utilities target other profiles and are not called here.
#[allow(dead_code)]
#[path = "../../../decoder/learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];

fn predictor(owned: bool, bytes: usize) -> FileLearnedConsistencyConfig {
    let model = model();
    let tensor = model.cache_profile().layers()[&1].values();
    let neutral = BinaryForecast::new(32768, 32768).unwrap();
    let base = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: tensor.profile(), weights: vec![0.0; tensor.dimensions()],
        bias: 0.0, threshold: 0.0, forecast: ForecastRegistration {
            domain: 71, generation: 1, policy_generation: 1, event_prefix: b"risk".to_vec(),
            negative: neutral, at_threshold: neutral, positive: neutral,
        }, alpha: ErrorBudget::new(1, 4).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 8,
    }).unwrap();
    let limits = LearnedMonitorBudget { encoded_bytes: bytes, ..LearnedMonitorBudget::default() };
    let predictor = FileLearnedConsistencyConfig::new(base, 1, KvSide::Value,
        limits, LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES).unwrap();
    if owned { predictor.with_owned_generation().unwrap() } else { predictor }
}
fn setup(root: &Directory, generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver)
{
    let (mut host, human) = owner(root, generation);
    let observer = host.enable_learned_action_consistency(host.revision(), predictor.clone()).unwrap();
    assert!(host.owned_learned_action_consistency_required());
    (host, human, observer)
}
fn predict(host: &mut FileOversight, observer: &FileConsistencyObserver, request: Option<u64>)
    -> Result<Result<LearnedForecastReport, Error>, JournalError>
{
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    match request {
        Some(request) => observer.forecast_owned_learned_request(host, revision, request, actor),
        None => observer.forecast_owned_learned_action(host, revision, 1, actor),
    }
}
fn disk(root: &Directory, generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig)
    -> FileLearnedPublicationSnapshot
{
    FileOversight::read_publication_with_owned_learned_consistency(root.store(), &profile(), generation, predictor).unwrap()
}
fn canonical(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }

#[test]
fn keyed_owned_forecast_survives_reconstruction_without_any_supplied_capture_inventory() {
    let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
    let (mut host, _, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
    let numerical = host.learned_generation_inspection().unwrap();
    let forecast = predict(&mut host, &observer, Some(71)).unwrap().unwrap(); forecast.prediction().unwrap();
    let after = host.learned_generation_inspection().unwrap();
    assert_eq!(after.numerical, numerical.numerical); assert_eq!(after.pending, numerical.pending);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
    let action = action_spec(&host);
    assert!(host.submit_request(host.revision(), 72, action.clone(), snapshot()).is_err());
    assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
    let submitted = host.submit_request(host.revision(), 71, action.clone(), snapshot()).unwrap();
    let revision = host.revision();
    assert_eq!(host.submit_request(0, 71, action, snapshot()).unwrap(), submitted);
    assert_eq!(host.revision(), revision);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().consistency.evidence.samples(), 1);
    assert_eq!(host.learned_action_consistency_report(1).unwrap().work(), forecast.work());
    let historical = disk(&root, &generation, &predictor);
    assert_eq!(historical.consistency, host.learned_action_consistency_snapshot().unwrap());
    assert!(FileOversight::learned_consistency_recovery_requirements(root.store(), &profile()).unwrap().is_empty());
    drop(host);
    assert!(FileOversight::open(root.store(), profile()).is_err());
    assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &generation).is_err());
    let (mut host, _, recovered) = FileOversight::open_with_owned_learned_consistency(
        root.store(), profile(), &generation, &predictor).unwrap();
    assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, forecast.work());
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().consistency.evidence, historical.consistency.consistency.evidence);
    assert!(matches!(predict(&mut host, &observer, Some(72)), Err(JournalError::Contract(Error::Binding))));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    step(&mut host).unwrap();
    predict(&mut host, &recovered, Some(72)).unwrap().unwrap().prediction().unwrap();
    let total = host.learned_action_consistency_snapshot().unwrap().work;
    assert!(total.encoded_bytes > forecast.work().encoded_bytes);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn original_two_key_publication_and_receipt_are_reconstructed_from_one_owned_source_cut() {
    let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
    let (mut host, human, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
    predict(&mut host, &observer, None).unwrap().unwrap().prediction().unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    let stale = host.revision() - 1;
    assert!(human.approve(&mut host, stale, &request).is_err());
    let revision = host.revision(); let human_key = human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human_key, &action, &inputs, snapshot()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    let published = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert!(matches!(published.outcome, EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), 1).unwrap();
    let expected = disk(&root, &generation, &predictor);
    assert_eq!(expected.publication.payload, b"visible"); assert_eq!(expected.publication.executions, 1);
    assert_eq!(expected.publication.control.ledger.charged, 16);
    drop(host);
    let (mut host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &human_key, &action, &inputs, snapshot()).is_err());
    let learned = host.learned_generation_inspection().unwrap();
    host.reconcile_attempt_at(host.revision(), 1, ElapsedTick(2)).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, learned.numerical);
}
#[test]
fn pending_recovery_preserves_coverage_loss_and_refuses_old_roles_and_standin_sources() {
    let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
    let (mut host, _, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
    let source = host.machine.broker.hosted_learned_original().unwrap().last_event().unwrap().audit().source().clone();
    let before = canonical(&host); let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert!(matches!(observer.forecast_learned_request(&mut host, revision, 71, actor, &source,
        KvRow { layer: 1, side: KvSide::Value, position: 0 }), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&host), before);
    predict(&mut host, &observer, Some(71)).unwrap().unwrap().prediction().unwrap();
    let history = disk(&root, &generation, &predictor);
    assert_eq!(history.pending_request, Some(71)); assert!(!history.consistency.consistency.coverage_lost);
    drop(host);
    let (mut host, _, next) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
    let restored = host.learned_action_consistency_snapshot().unwrap();
    assert!(restored.consistency.coverage_lost); assert_eq!(restored.consistency.pending_attempt, Some(1));
    assert_eq!(restored.work, history.consistency.work); assert_eq!(restored.consistency.evidence.samples(), 0);
    assert!(predict(&mut host, &observer, Some(72)).is_err());
    assert!(predict(&mut host, &next, Some(72)).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn every_forecast_write_fault_returns_no_candidate_and_recovers_only_the_canonical_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
        let (mut host, _, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
        let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        assert!(matches!(predict(&mut host, &observer, Some(71)), Err(JournalError::Io(ref e)) if e.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(host.learned_action_consistency_snapshot().is_err());
        let canonical = disk(&root, &generation, &predictor);
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(canonical.pending_request, visible.then_some(71));
        assert_eq!(canonical.consistency.consistency.pending_attempt, visible.then_some(1));
        assert_eq!(canonical.consistency.consistency.evidence.samples(), 0);
        drop(host);
        let (host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
        let restored = host.learned_action_consistency_snapshot().unwrap();
        assert_eq!(restored.consistency.pending_attempt, canonical.consistency.consistency.pending_attempt);
        assert_eq!(restored.consistency.coverage_lost, visible); assert_eq!(restored.work, canonical.consistency.work);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn atomic_publication_faults_stay_query_only_after_owned_forecast_recovery() {
    for barrier in BARRIERS {
        let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
        let (mut host, human, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
        predict(&mut host, &observer, None).unwrap().unwrap().prediction().unwrap();
        let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = human.approve(&mut host, revision, &request).unwrap();
        host.store.fail_once(barrier);
        let result = host.complete_checked_publication(host.revision(), CheckedCompletion {
            automatic: &automatic, human: &human, action: &action, current: &inputs,
            snapshot: snapshot(), now: ElapsedTick(1),
        });
        assert!(matches!(result, Err(JournalError::Io(ref e)) if e.operation == barrier));
        let read = disk(&root, &generation, &predictor);
        let published = barrier == JournalIo::DirectorySync;
        assert_eq!(read.publication.executions, u64::from(published));
        assert_eq!(read.consistency.consistency.evidence.samples(), 1);
        drop(host);
        let (host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
        assert_eq!(host.inspect().executions, u64::from(published));
        assert_eq!(host.inspect().control.ledger.charged, if published { 16 } else { 0 });
        assert!(host.learned_generation_inspection().unwrap().paused);
    }
    for published in [false, true] {
        let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, MAX_CHECKED_KV_BYTES);
        let (mut host, human, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
        predict(&mut host, &observer, None).unwrap().unwrap().prediction().unwrap();
        let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = human.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if published { host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap(); }
        drop(host);
        let (mut host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let result = host.reconcile_attempt_at(host.revision(), 1, ElapsedTick(2)).unwrap();
        if published { assert!(matches!(result, Reconciliation::Resolved(EndpointOutcome::Executed { .. }))); }
        else { assert_eq!(result, Reconciliation::AwaitingResolution); }
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
}

#[test]
fn mismatched_recipe_source_mode_and_saved_outcome_cannot_be_imported_as_success() {
    let root = Directory::new(); let generation = config(false, 1); let good = predictor(true, MAX_CHECKED_KV_BYTES);
    let (mut host, _, observer) = setup(&root, &generation, &good); step(&mut host).unwrap();
    predict(&mut host, &observer, Some(71)).unwrap().unwrap().prediction().unwrap();
    let before = canonical(&host);
    assert!(FileOversight::read_publication_with_owned_learned_consistency(root.store(), &profile(),
        &config(false, 3), &good).is_err());
    assert!(FileOversight::read_publication_with_owned_learned_consistency(root.store(), &profile(),
        &generation, &predictor(false, MAX_CHECKED_KV_BYTES)).is_err());
    assert!(FileOversight::read_publication_with_owned_learned_consistency(root.store(), &profile(),
        &generation, &predictor(true, 0)).is_err());
    assert_eq!(canonical(&host), before);
    let mut events = journal::decode(&host.profile, host.store.identity(), &before).unwrap();
    bind_owned_history(&mut events, &generation, &good).unwrap();
    let saved = match events.last_mut().unwrap() {
        Event::Consistency(ConsistencyEvent::ForecastOwnedLearnedRequest(_, _, saved)) => saved,
        _ => panic!("last original event must be keyed forecast"),
    };
    let mut wrong = saved.as_ref().unwrap().to_vec(); let last = wrong.len() - 1; wrong[last] ^= 1;
    *saved = Some(Rc::from(wrong));
    assert!(Machine::replay(&profile(), &events).is_err());
    assert_eq!(disk(&root, &generation, &good).pending_request, Some(71));
}

#[test]
fn owned_configuration_and_forecast_records_are_exact_framed_and_do_not_contain_source_exports() {
    let supplied = predictor(false, MAX_CHECKED_KV_BYTES);
    let owned = supplied.clone().with_owned_generation().unwrap();
    assert_eq!(&owned.encoded()[..8], b"FALCPRD\x02");
    assert_eq!(&owned.encoded()[8..], &supplied.encoded()[8..]);
    assert_eq!(super::super::config::FileLearnedConsistencyConfig::decode(owned.encoded()).unwrap(), owned);
    assert!(owned.clone().with_owned_generation().is_err());
    for end in 0..owned.encoded().len() {
        assert!(super::super::config::FileLearnedConsistencyConfig::decode(&owned.encoded()[..end]).is_err());
    }
    let root = Directory::new(); let generation = config(false, 1);
    let (mut host, _, observer) = setup(&root, &generation, &owned); step(&mut host).unwrap();
    predict(&mut host, &observer, Some(71)).unwrap().unwrap().prediction().unwrap();
    let Event::Consistency(event) = host.events.last().unwrap() else { panic!("forecast event"); };
    let mut writer = super::super::super::super::super::codec::shared::Writer::new(1000);
    super::super::super::write(&mut writer, event).unwrap(); let encoded = writer.finish();
    assert_eq!(encoded[0], 11); assert!(encoded.len() <= 21 + outcome::MAX_BYTES);
    for end in 0..encoded.len() {
        let mut reader = Reader::new(&encoded[..end]);
        assert!(super::super::super::read(&mut reader).is_err());
    }
    let mut extra = encoded.clone(); extra.push(0); let mut reader = Reader::new(&extra);
    super::super::super::read(&mut reader).unwrap(); assert!(reader.end().is_err());
}

#[test]
fn numerical_refusal_and_interrupted_source_never_create_keyed_forecasts_or_refill_after_recovery() {
    let root = Directory::new(); let generation = config(false, 1); let predictor = predictor(true, 0);
    let (mut host, _, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
    let report = predict(&mut host, &observer, Some(71)).unwrap().unwrap();
    assert_eq!(report.prediction().unwrap_err(), Error::Limit);
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    let charged = host.learned_action_consistency_snapshot().unwrap();
    drop(host);
    let (host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor).unwrap();
    let restored = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(restored.work, charged.work); assert_eq!(restored.retained_source_bytes, charged.retained_source_bytes);
    assert!(restored.consistency.coverage_lost); assert_eq!(restored.consistency.evidence.samples(), 0);
    drop(host);
    let root = Directory::new(); let predictor = self::predictor(true, MAX_CHECKED_KV_BYTES);
    let (mut host, _, observer) = setup(&root, &generation, &predictor); step(&mut host).unwrap();
    host.source_interrupted = true;
    let before = canonical(&host);
    assert!(predict(&mut host, &observer, Some(71)).is_err());
    assert_eq!(canonical(&host), before); assert_eq!(host.pending_forecast_request().unwrap(), None);
}
