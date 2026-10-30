//! Exact recipe-bound situations from real numerical publication histories.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "learned_situation/fixture.rs"]
mod fixture;
use fixture::{Directory, Lane, generate, message, owner, profile, recipe, step};
use fa_reference::{Error, action::{ActionState, ElapsedTick}};
use fa_reference::action::consequence::delivery::persistent::{JournalError,
    observed::{FileOversight, FileOversightProfile, decoder::learned::FileLearnedConfig,
        guarded::anchored::FileHistoryAnchor,
        replay::situation::{FileSituation, FileSituationPage, FileSituationBasis,
            FileSituationOrigin, FileSituationReplay, FileSituationReplayStatus}}};
use fa_reference::action::consequence::oversight::actor::Knowledge;

fn begin(root: &Directory, recipe: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileSituationReplay, JournalError>
{
    match recipe.required_owned_pre_output_forecast() {
        Some(predictor) => FileOversight::begin_read_situation_with_owned_learned_consistency(
            root.store(), &profile(), recipe, predictor, anchor),
        None => FileOversight::begin_read_situation_with_learned_generation(
            root.store(), &profile(), recipe, anchor),
    }
}
fn page(report: &FileSituation) -> (FileSituationPage, FileSituationBasis) {
    let Knowledge::Known { value, basis } = report.page(None, 64).unwrap() else { panic!("known historical situation") };
    (value, basis)
}
fn agrees(host: &FileOversight, report: &FileSituation, anchored: Option<u64>) {
    let (live, live_basis) = page(&host.situation().unwrap());
    let (replayed, basis) = page(report);
    assert_eq!(replayed.summary, live.summary);
    assert_eq!(replayed.attempts, live.attempts);
    assert_eq!(replayed.offset, 0); assert!(replayed.next.is_none());
    assert_eq!(basis.origin, FileSituationOrigin::CanonicalImage);
    assert_eq!(basis.anchored_through, anchored);
    assert_eq!(basis.journal_revision, live_basis.journal_revision);
    assert_eq!(basis.control_sequence, live_basis.control_sequence);
    assert_eq!(basis.recorded_at, live_basis.recorded_at);
    assert_eq!(basis.scope, live_basis.scope);
}

#[test]
fn every_plain_numerical_cut_replays_without_advancing_the_live_sampler_or_journal() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, _, _) = owner(&root, &recipe);
    for _ in 0..=4 {
        let before = host.inspect(); let numeric = host.learned_generation_inspection().unwrap();
        let files = root.files(); let anchor = host.history_anchor().unwrap();
        let mut cursor = begin(&root, &recipe, Some(&anchor)).unwrap();
        assert_eq!(cursor.advance(0).unwrap().checked_events, 0);
        while cursor.progress().status == FileSituationReplayStatus::Reading {
            let previous = cursor.progress().checked_events;
            let next = cursor.advance(1).unwrap(); assert_eq!(next.checked_events, previous + 1);
        }
        agrees(&host, &cursor.finish().unwrap(), Some(anchor.revision()));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.learned_generation_inspection().unwrap(), numeric);
        assert_eq!(root.files(), files);
        if !numeric.numerical.status.is_active() { break; }
        step(&mut host);
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
}

#[test]
fn raw_and_owned_forecasts_replay_from_their_exact_recipes_without_spending_live_evidence() {
    for lane in [Lane::Raw, Lane::Owned] {
        let root = Directory::new(); let recipe = recipe(lane, false);
        let (mut host, _, observer) = owner(&root, &recipe);
        // One real prefill step finishes the fixture's merged prompt; forecast
        // the next release before sampling. No external capture is substituted.
        step(&mut host);
        let revision = host.revision();
        let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let observer = observer.unwrap();
        match lane {
            Lane::Raw => { observer.forecast_hosted_request(&mut host, revision, 9001, actor_revision).unwrap().unwrap(); }
            Lane::Owned => { observer.forecast_owned_learned_request(&mut host, revision, 9001, actor_revision)
                .unwrap().unwrap().prediction().unwrap(); }
            Lane::Plain => unreachable!(),
        }
        assert_eq!(host.pending_forecast_request().unwrap(), Some(9001));
        let before = host.inspect(); let numeric = host.learned_generation_inspection().unwrap();
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let files = root.files(); let anchor = host.history_anchor().unwrap();
        agrees(&host, &begin(&root, &recipe, Some(&anchor)).unwrap().complete().unwrap(), Some(anchor.revision()));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.learned_generation_inspection().unwrap(), numeric);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.pending_forecast_request().unwrap(), Some(9001));
        assert_eq!(root.files(), files);
    }
}

#[test]
fn generated_effects_keep_original_pending_and_unresolved_dispositions() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, human, _) = owner(&root, &recipe); generate(&mut host);
    let action = message(&mut host, 1);
    let pending = begin(&root, &recipe, None).unwrap().complete().unwrap();
    agrees(&host, &pending, None);
    assert_eq!(page(&pending).0.summary.pending_attempts, 1);
    let inputs = fixture::dispatch(&mut host, &human, 1, &action);
    let dispatched = begin(&root, &recipe, None).unwrap().complete().unwrap();
    let (value, _) = page(&dispatched);
    assert_eq!(value.attempts[0].stage, ActionState::Dispatching);
    assert_eq!(value.summary.unresolved_attempts, 1);
    assert_eq!(value.summary.charged_units, action.spec().units);
    assert_eq!(value.summary.executions, 0);
    host.publish_checked(host.revision(), 1, Some(&inputs), fixture::snapshot(), ElapsedTick(1)).unwrap();
    let files = root.files();
    let published = begin(&root, &recipe, None).unwrap().complete().unwrap();
    agrees(&host, &published, None);
    assert_eq!(page(&published).0.summary.unresolved_attempts, 1);
    assert_eq!(page(&published).0.summary.executions, 1);
    assert_eq!(root.files(), files);
    host.reconcile(host.revision(), 1).unwrap();
    let confirmed = begin(&root, &recipe, None).unwrap().complete().unwrap();
    assert_eq!(page(&confirmed).0.attempts[0].stage, ActionState::Confirmed);
    agrees(&host, &confirmed, None);
    // Already captured situations do not follow a later reconciliation.
    assert_eq!(page(&dispatched).0.summary.executions, 0);
    assert_eq!(page(&published).0.summary.unresolved_attempts, 1);
    assert_eq!(page(&pending).0.summary.pending_attempts, 1);
}

#[test]
fn missing_changed_and_cross_mode_recipes_never_downgrade_to_plain_replay() {
    for lane in [Lane::Plain, Lane::Raw, Lane::Owned] {
        let root = Directory::new(); let original = recipe(lane, false);
        let (mut host, _, _) = owner(&root, &original); step(&mut host);
        let files = root.files();
        assert!(FileOversight::read_situation(root.store(), &profile(), None).is_err());
        assert!(begin(&root, &recipe(lane, true), None).is_err());
        for wrong_lane in [Lane::Plain, Lane::Raw, Lane::Owned] {
            if wrong_lane != lane { assert!(begin(&root, &recipe(wrong_lane, false), None).is_err()); }
        }
        if lane == Lane::Owned {
            assert!(FileOversight::begin_read_situation_with_learned_generation(
                root.store(), &profile(), &original, None).and_then(FileSituationReplay::complete).is_err());
            let wrong = recipe(Lane::Owned, true);
            assert!(FileOversight::begin_read_situation_with_owned_learned_consistency(
                root.store(), &profile(), &wrong, original.required_owned_pre_output_forecast().unwrap(), None).is_err());
        }
        agrees(&host, &begin(&root, &original, None).unwrap().complete().unwrap(), None);
        assert_eq!(root.files(), files);
    }
}

#[test]
fn no_learned_prefix_can_escape_and_cancellation_never_acquires_the_writer_lock() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, _, _) = owner(&root, &recipe); generate(&mut host); message(&mut host, 1);
    let events = host.revision() as usize; let files = root.files();
    for cut in 0..events {
        let mut cursor = begin(&root, &recipe, None).unwrap();
        for _ in 0..cut { cursor.advance(1).unwrap(); }
        assert_eq!(cursor.finish().err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(root.files(), files);
    }
    let mut cursor = begin(&root, &recipe, None).unwrap();
    assert_eq!(cursor.advance(65).err(), Some(JournalError::Contract(Error::Limit)));
    assert_eq!(cursor.progress().checked_events, 0);
    drop(cursor);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    agrees(&host, &begin(&root, &recipe, None).unwrap().complete().unwrap(), None);
}

#[test]
fn anchored_successors_are_read_once_and_rollback_and_foreign_custody_are_refused() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, _, _) = owner(&root, &recipe); step(&mut host);
    let canonical = root.store().join("delivery.bin");
    let first = std::fs::read(&canonical).unwrap(); let anchor = host.history_anchor().unwrap();
    step(&mut host); let successor = std::fs::read(&canonical).unwrap();
    let future_anchor = host.history_anchor().unwrap();
    let cursor = begin(&root, &recipe, Some(&anchor)).unwrap();
    // A later on-disk replacement cannot change the already selected image.
    std::fs::write(&canonical, &first).unwrap();
    agrees(&host, &cursor.complete().unwrap(), Some(anchor.revision()));
    assert!(begin(&root, &recipe, Some(&future_anchor)).is_err());
    std::fs::write(&canonical, &successor).unwrap();
    let fork = Directory::new();
    let (mut other, _, _) = owner(&fork, &recipe); step(&mut other);
    assert!(begin(&root, &recipe, Some(&other.history_anchor().unwrap())).is_err());
    assert_eq!(std::fs::read(&canonical).unwrap(), successor);
    agrees(&host, &begin(&root, &recipe, Some(&anchor)).unwrap().complete().unwrap(), Some(anchor.revision()));
}

#[test]
fn incomplete_generation_intent_is_historical_not_automatically_completed() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, _, _) = owner(&root, &recipe);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let before = host.learned_generation_inspection().unwrap(); assert!(before.pending.is_some());
    let files = root.files();
    agrees(&host, &begin(&root, &recipe, None).unwrap().complete().unwrap(), None);
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn wrong_bootstrap_and_corrupt_suffix_never_become_an_empty_learned_situation() {
    let root = Directory::new(); let recipe = recipe(Lane::Plain, false);
    let (mut host, _, _) = owner(&root, &recipe); step(&mut host);
    let mut wrong = profile(); wrong.delivery.total += 1;
    assert!(FileOversight::begin_read_situation_with_learned_generation(root.store(), &wrong, &recipe, None).is_err());
    let canonical = root.store().join("delivery.bin"); let bytes = std::fs::read(&canonical).unwrap();
    let mut corrupt = bytes.clone(); corrupt.push(0);
    std::fs::write(&canonical, &corrupt).unwrap();
    assert!(begin(&root, &recipe, None).is_err());
    assert_eq!(std::fs::read(&canonical).unwrap(), corrupt);
    std::fs::write(&canonical, &bytes).unwrap();
    agrees(&host, &begin(&root, &recipe, None).unwrap().complete().unwrap(), None);
}
