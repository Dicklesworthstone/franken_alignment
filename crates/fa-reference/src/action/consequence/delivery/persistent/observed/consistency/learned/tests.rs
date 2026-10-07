//! Real-codec publication, replay, request-routing and deterministic Store faults.
//! These controls do not claim hardware durability or detector qualification.
use super::*;
use super::super::super::{FileHumanReviewer, FileOversightProfile, Machine, journal};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::probe::learned::ResidualRetention;
use crate::action::consequence::oversight::consistency::ConsistencyStopPolicy;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::observed::containment::FileStateUpdate;
use crate::action::consequence::delivery::persistent::observed::publication::CheckedCompletion;
use std::collections::BTreeMap;
mod fixture;
use fixture::*;

#[test]
fn learned_forecasts_keep_the_original_congress_human_and_publication_boundary() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 1.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![0.0, 1.0], LearnedMonitorBudget::default());
    let (mut host, reviewer, observer) = owner(&root, config.clone());
    assert!(host.publication_guard_required());
    let report = predict(&mut host, &observer, &checked).unwrap().unwrap();
    report.prediction().unwrap(); assert!(report.work().refinements > 0);
    let spec = proposal(&host);
    let action = host.propose_consistent(host.revision(), 1, spec, snapshot()).unwrap().unwrap();
    let unreviewed = inputs(&host, &action);
    assert!(host.authorize(host.revision(), 1, &unreviewed, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    let input = review(&mut host, &action);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    assert_eq!(host.inspect().executions, 0);
    let request = host.request_human_approval(host.revision(), 201, 1, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.complete_checked_publication(host.revision(), CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &input,
        snapshot: snapshot(), now: ElapsedTick(2),
    }).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"risk");
    assert_eq!(host.learned_action_consistency_report(1).unwrap().work(), report.work());
    assert!(host.action_consistency_observation(1).is_err());
    assert_eq!(host.learned_action_consistency_observation(1).unwrap().sample(), 1);
    let before = host.learned_action_consistency_snapshot().unwrap();
    let sources = disk_sources(&root, &checked);
    assert_eq!(FileOversight::read_learned_action_consistency(root.store(), &profile(), &config, &sources).unwrap(), before);
    drop(host);
    assert!(FileOversight::open(root.store(), profile()).is_err());
    let (mut recovered, _, _) = FileOversight::open_with_learned_action_consistency(
        root.store(), profile(), &config, &sources).unwrap();
    let after = recovered.learned_action_consistency_snapshot().unwrap();
    assert_eq!(after.work, before.work); assert_eq!(after.consistency.evidence, before.consistency.evidence);
    assert!(!after.consistency.coverage_lost); assert!(!recovered.clock_ready());
    assert_eq!(recovered.inspect().executions, 1);
    let revision = recovered.revision(); let actor = recovered.actor_snapshot().unwrap().actor_revision;
    assert!(matches!(observer.forecast_learned_action(&mut recovered, revision, 2, actor, &checked, row()),
        Err(JournalError::Contract(Error::Binding))));
    assert!(reviewer.approve(&mut recovered, revision, &request).is_err());
    assert!(recovered.publish(revision, 1).is_err());
    assert_eq!(recovered.inspect().executions, 1);
}

#[test]
fn consumed_refusal_and_lifetime_remainder_survive_recovery_and_a_new_capture() {
    let root = Directory::new();
    let (first, _) = source([1.0, 1.0], ResidualRetention::All, 1);
    let (second, image) = source([1.0, 1.0], ResidualRetention::All, 2);
    let second_row = KvRow { position: 1, ..row() };
    let mut lifetime = LearnedMonitorBudget::default();
    lifetime.encoded_bytes = first.report().base_encoded_bytes + second.report().base_encoded_bytes - 1;
    let config = configuration(&first, vec![0.0, 0.0], lifetime);
    let (mut host, _, observer) = owner(&root, config.clone());
    let first_report = predict(&mut host, &observer, &first).unwrap().unwrap();
    first_report.prediction().unwrap();
    let mut denied = proposal(&host); denied.units = 17;
    assert!(host.propose_consistent(host.revision(), 1, denied, snapshot()).unwrap().is_err());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    let mut sources = disk_sources(&root, &first);
    drop(host);
    let (mut host, _, observer) = FileOversight::open_with_learned_action_consistency(
        root.store(), profile(), &config, &sources).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, first_report.work());
    host.record_actor_state(host.revision(), FileStateUpdate {
        operation: 501, expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch, state: actor(2, image.encode().unwrap()),
    }).unwrap();
    let revision = host.revision(); let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    let report = observer.forecast_learned_action(&mut host, revision, 2, actor_revision, &second, second_row).unwrap().unwrap();
    assert_eq!(report.monitor().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(report.prediction().unwrap_err(), Error::Limit);
    assert_eq!(report.work(), LearnedMonitorWork::default());
    let after = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(after.work, first_report.work()); assert!(after.consistency.coverage_lost);
    assert_eq!(after.consistency.evidence.samples(), 1); assert_eq!(after.consistency.pending_attempt, None);
    sources.insert(host.revision(), second);
    assert_eq!(FileOversight::read_learned_action_consistency(root.store(), &profile(), &config, &sources).unwrap(), after);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn pending_forecast_is_not_erased_or_rearmed_by_the_recovery_fence() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
    let (mut host, _, observer) = owner(&root, config.clone());
    predict(&mut host, &observer, &checked).unwrap().unwrap().prediction().unwrap();
    let before = host.learned_action_consistency_snapshot().unwrap();
    let sources = disk_sources(&root, &checked); drop(host);
    let (mut host, _, observer) = FileOversight::open_with_learned_action_consistency(
        root.store(), profile(), &config, &sources).unwrap();
    let after = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(after.work, before.work); assert_eq!(after.retained_source_bytes, before.retained_source_bytes);
    assert_eq!(after.consistency.pending_attempt, Some(1)); assert!(after.consistency.coverage_lost);
    assert_eq!(after.consistency.evidence.samples(), 0);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    assert!(observer.forecast_learned_action(&mut host, revision, 2, actor, &checked, row()).unwrap().is_err());
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, before.work);
    assert!(host.enable_learned_action_consistency(host.revision(), config).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn numerical_refusal_receipts_never_become_request_bindings_or_likelihood_samples() {
    for missing in [false, true] {
        let root = Directory::new();
        let retention = if missing { ResidualRetention::None } else { ResidualRetention::All };
        let (checked, _) = source([1.0, 1.0], retention, 1);
        let mut lifetime = LearnedMonitorBudget::default();
        if !missing { lifetime.refinements = 0; }
        let base = configuration(&checked, vec![0.0, 1.0], lifetime);
        let stopped = base.consistency().clone().with_terminal_stop(ConsistencyStopPolicy::new(11, 1, 7007).unwrap()).unwrap();
        let config = FileLearnedConsistencyConfig::new(stopped, 1, KvSide::Key,
            LearnedMonitorBudget::default(), lifetime, MAX_CHECKED_KV_BYTES).unwrap();
        let (mut host, _, observer) = owner(&root, config.clone());
        let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
        let report = observer.forecast_learned_request(&mut host, revision, 42, actor, &checked, row()).unwrap().unwrap();
        assert_eq!(report.prediction().unwrap_err(), if missing { Error::Incomplete } else { Error::Limit });
        assert!(report.work().probe_coordinates > 0);
        assert_eq!(host.pending_forecast_request().unwrap(), None);
        let before = host.learned_action_consistency_snapshot().unwrap();
        assert!(before.consistency.coverage_lost); assert_eq!(before.consistency.evidence.samples(), 0);
        assert_eq!(before.work, report.work()); assert!(!before.has_unreported_work);
        assert!(host.inspect().stop.is_some()); assert!(host.inspect().control.suspended);
        let sources = disk_sources(&root, &checked); drop(host);
        let (host, _, _) = FileOversight::open_with_learned_action_consistency(
            root.store(), profile(), &config, &sources).unwrap();
        assert_eq!(host.learned_action_consistency_report(1).unwrap().work(), report.work());
        assert_eq!(host.learned_action_consistency_report(1).unwrap().prediction().unwrap_err(), report.prediction().unwrap_err());
        assert_eq!(host.pending_forecast_request().unwrap(), None);
        assert!(host.inspect().stop.is_some()); assert!(host.inspect().control.suspended);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn only_the_original_external_request_can_consume_a_learned_forecast() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
    let (mut host, _, observer) = owner(&root, config.clone());
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_learned_request(&mut host, revision, 42, actor, &checked, row()).unwrap().unwrap().prediction().unwrap();
    assert_eq!(host.pending_forecast_request().unwrap(), Some(42));
    let revision = host.revision(); let spec = proposal(&host);
    assert!(host.propose_consistent(revision, 1, spec.clone(), snapshot()).is_err());
    assert!(host.submit_request(revision, 43, spec.clone(), snapshot()).is_err());
    assert_eq!(host.revision(), revision); assert!(host.storage_failure().is_none());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    let mut denied = spec; denied.units = 17;
    let status = host.submit_request(revision, 42, denied.clone(), snapshot()).unwrap();
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.submit_request(revision, 42, denied, snapshot()).unwrap(), status);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    let sources = disk_sources(&root, &checked); drop(host);
    let (host, _, _) = FileOversight::open_with_learned_action_consistency(root.store(), profile(), &config, &sources).unwrap();
    assert_eq!(host.request_status(42).unwrap(), status);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn every_storage_barrier_returns_no_candidate_and_recovery_follows_the_actual_cut() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
        let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
        let (mut host, _, observer) = owner(&root, config.clone());
        let before = host.inspect(); host.store.fail_once(stage);
        let JournalError::Io(failure) = predict(&mut host, &observer, &checked).unwrap_err()
            else { panic!("missing injected Store failure"); };
        assert_eq!(failure.operation, stage);
        assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.learned_action_consistency_snapshot(), Err(JournalError::Unavailable));
        let sources = disk_sources(&root, &checked);
        let disk = FileOversight::read_learned_action_consistency(root.store(), &profile(), &config, &sources).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(disk.consistency.pending_attempt, visible.then_some(1));
        assert_eq!(disk.consistency.evidence.samples(), 0); assert!(!disk.consistency.coverage_lost);
        assert_eq!(disk.work.encoded_bytes > 0, visible);
        drop(host);
        let (host, _, _) = FileOversight::open_with_learned_action_consistency(
            root.store(), profile(), &config, &sources).unwrap();
        let recovered = host.learned_action_consistency_snapshot().unwrap();
        assert_eq!(recovered.work, disk.work); assert_eq!(recovered.retained_source_bytes, disk.retained_source_bytes);
        assert_eq!(recovered.consistency.pending_attempt, disk.consistency.pending_attempt);
        assert_eq!(recovered.consistency.coverage_lost, visible);
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn missing_changed_extra_or_unchecked_recovery_inputs_cannot_create_a_live_owner() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
    let (different, _) = source([1.0, 1.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
    let (mut host, _, observer) = owner(&root, config.clone());
    predict(&mut host, &observer, &checked).unwrap().unwrap().prediction().unwrap();
    let sources = disk_sources(&root, &checked);
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let identity = host.store.identity().to_path_buf();
    let events = journal::decode(&profile(), &identity, &canonical).unwrap();
    assert!(Machine::replay(&profile(), &events).is_err());
    drop(host);
    let key = *sources.keys().next().unwrap();
    let mut extra = sources.clone(); extra.insert(key + 100, checked.clone());
    for wrong in [BTreeMap::new(), BTreeMap::from([(key, different)]), extra] {
        assert!(FileOversight::open_with_learned_action_consistency(root.store(), profile(), &config, &wrong).is_err());
    }
    let different_config = configuration(&checked, vec![0.0, 0.0], LearnedMonitorBudget::default());
    assert!(FileOversight::open_with_learned_action_consistency(root.store(), profile(), &different_config, &sources).is_err());
    let store = super::super::super::super::storage::Store::open(&root.store()).unwrap();
    assert_eq!(store.read(profile().delivery.limits.bytes).unwrap(), canonical); drop(store);
    let mut events = events;
    recovery::bind_events(&mut events, &config, &sources).unwrap();
    if let Event::Consistency(ConsistencyEvent::ForecastLearned(_, _, _, capture)) = &mut events[(key - 1) as usize] {
        let mut bad = capture.outcome.as_ref().unwrap().to_vec();
        bad[8] ^= 1; capture.outcome = Some(Rc::from(bad));
    } else { panic!("missing learned forecast event"); }
    assert!(matches!(Machine::replay(&profile(), &events), Err(Error::Binding)));
    let (host, _, _) = FileOversight::open_with_learned_action_consistency(root.store(), profile(), &config, &sources).unwrap();
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
}

#[test]
fn learned_wire_is_canonical_and_cannot_parse_a_missing_outcome_or_imported_runtime() {
    let root = Directory::new();
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1);
    let config = configuration(&checked, vec![1.0, 0.0], LearnedMonitorBudget::default());
    for end in 0..config.encoded().len() {
        assert!(FileLearnedConsistencyConfig::decode(&config.encoded()[..end]).is_err());
    }
    assert_eq!(FileLearnedConsistencyConfig::decode(config.encoded()).unwrap(), config);
    let hosted = config.consistency().clone().with_hosted_residual(1).unwrap();
    assert!(matches!(FileLearnedConsistencyConfig::new(hosted, 1, KvSide::Key,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES), Err(Error::Binding)));
    let (mut host, _, observer) = owner(&root, config);
    predict(&mut host, &observer, &checked).unwrap().unwrap();
    let original = host.events.last().unwrap();
    let Event::Consistency(original) = original else { panic!("missing forecast"); };
    let mut w = Writer::new(100_000); super::super::write(&mut w, original).unwrap(); let bytes = w.finish();
    for end in 0..bytes.len() {
        assert!(super::super::read(&mut Reader::new(&bytes[..end])).is_err());
    }
    let mut r = Reader::new(&bytes); let event = super::super::read(&mut r).unwrap(); r.end().unwrap();
    let ConsistencyEvent::ForecastLearned(_, _, _, capture) = event else { panic!("wrong tag"); };
    assert!(matches!(capture.runtime(), Err(Error::Incomplete)));
    let uncompleted = ConsistencyEvent::ForecastLearned(2, 0, row(), Capture::new(&checked).unwrap());
    let mut w = Writer::new(100_000); super::super::write(&mut w, &uncompleted).unwrap();
    assert!(super::super::read(&mut Reader::new(&w.finish())).is_err());
}

mod publication;
