//! Real original learned generation, topology-source reads, native congress and
//! independent human approval precede this sealed governance composition.
use super::*;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::{FileLearnedPolicyPromotionRequest, FileLearnedPolicyPromotionReview},
    guarded::mediated::learned::{FileMediatedPolicyPromotionHistory, FileMediatedPolicyPromotionRecovery},
};

fn candidate() -> Policy { Policy::new(2, vec![Predicate::PayloadAtMost(1024)]).unwrap() }

fn request(history: &History) -> FileLearnedPolicyPromotionRequest {
    let floor = history.expected.oversight.minimum;
    FileLearnedPolicyPromotionRequest::new(history.anchor.revision(), PolicyUpdate::new(
        901, floor.control_sequence, floor.authority_epoch, candidate()).unwrap()).unwrap()
}

fn begin(history: &History, expected: &FileMediatedRequirements,
    saved: &TopologyCheckpoint, selected: FileLearnedPolicyPromotionRequest)
    -> Result<FileMediatedPolicyPromotionRecovery, JournalError>
{
    FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        history.root.store(), history.profile.clone(), expected, &history.config,
        saved.history_anchor())?.require_policy_promotion_checkpoint(selected, saved)
}

fn ready(mut run: FileMediatedPolicyPromotionRecovery)
    -> Result<FileMediatedPolicyPromotionRecovery, JournalError>
{
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1)?;
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    Ok(run)
}

fn original(history: &History) -> FileMediatedPolicyPromotionRecovery {
    ready(begin(history, &history.expected, history.topology.as_ref().unwrap(), request(history)).unwrap()).unwrap()
}

fn promote(history: &History) -> (FileLearnedPolicyPromotionReview, FileMediatedPolicyPromotionHistory) {
    let run = original(history);
    let reviewed = run.policy_promotion_review().unwrap().clone();
    let result = run.finish_promoted_policy(&reviewed, false).unwrap();
    (reviewed, result)
}

fn preserved(saved: &TopologyCheckpoint, result: &FileMediatedPolicyPromotionHistory) {
    let next = result.topology_checkpoint();
    let promoted = result.promotion();
    assert_eq!(next.history_anchor(), promoted.history_anchor());
    assert_eq!(next.expected(), saved.expected());
    assert_eq!(next.topology_bytes(), saved.topology_bytes());
    assert_eq!(next.inventory_bytes(), saved.inventory_bytes());
    assert_eq!(next.work(), saved.work());
    assert_eq!(next.last_operation(), saved.last_operation());
    assert_eq!(next.available(), saved.available());
    assert_eq!(next.minimum().journal_revision, promoted.snapshot().revision);
    assert_eq!(next.minimum().control_sequence, promoted.snapshot().control.sequence);
    assert_eq!(next.minimum().authority_epoch, promoted.snapshot().control.ledger.epoch);
    let mut archive = Vec::new();
    next.write_to(&mut archive, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    assert_eq!(TopologyCheckpoint::read_trusted(&mut archive.as_slice(),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(), *next);
}

#[test]
fn real_mediated_promotion_preserves_source_numerics_and_unknown_charges_but_cancels_reservations() {
    for cut in [Cut::Confirmed, Cut::Unknown, Cut::FinishPending] {
        let history = History::with_source(Lane::Plain, cut);
        let saved = history.topology.as_ref().unwrap();
        assert!(saved.work().checks > 0 && saved.available());
        let directory = history.root.store().parent().unwrap().to_owned();
        std::fs::remove_file(directory.join("topology.json")).unwrap();
        std::fs::remove_file(directory.join("inventory.json")).unwrap();
        let (reviewed, result) = promote(&history);
        let promoted = result.promotion();
        assert_eq!(reviewed.snapshot().revision, history.anchor.revision());
        assert_eq!(reviewed.report().cases().len(), if cut == Cut::FinishPending { 3 } else { 2 });
        assert_eq!(promoted.appended_events(), 3);
        assert_eq!(promoted.snapshot().revision, history.anchor.revision() + 3);
        assert_eq!(promoted.current_policy(), &candidate());
        assert_eq!(promoted.snapshot().control.ledger.charged, history.charge);
        assert_eq!(promoted.snapshot().executions, u64::from(cut != Cut::Unknown));
        assert!(promoted.snapshot().control.sequence > reviewed.snapshot().control.sequence);
        assert!(promoted.snapshot().control.ledger.epoch > reviewed.snapshot().control.ledger.epoch);
        if cut == Cut::Unknown {
            assert_eq!(reviewed.snapshot().control.ledger.stages[&history.attempt], ActionState::Unknown);
            assert_eq!(promoted.snapshot().control.ledger.stages[&history.attempt], ActionState::Unknown);
            assert!(promoted.receipt().change().cancelled.is_empty());
        } else if cut == Cut::FinishPending {
            assert!(reviewed.snapshot().control.ledger.reserved > 0);
            assert_eq!(promoted.snapshot().control.ledger.reserved, 0);
            assert_eq!(promoted.receipt().change().cancelled.len(), 1);
            assert_ne!(promoted.receipt().change().cancelled[0], history.attempt);
        }
        preserved(saved, &result);
        let mut expected = history.expected.clone();
        expected.oversight.effective_policy = candidate();
        expected.oversight.minimum = result.topology_checkpoint().minimum();
        let mut run = history.begin_with(&expected, promoted.history_anchor()).unwrap();
        run.drive().unwrap();
        let (host, _) = run.finish().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, history.numerical.numerical);
        assert_eq!(host.inspect().control.ledger.charged, history.charge);
    }
}

#[test]
fn exact_lost_export_retries_require_current_policy_and_preserve_original_review_and_pair() {
    let history = History::with_source(Lane::Plain, Cut::Confirmed);
    let saved = history.topology.as_ref().unwrap();
    let (reviewed, first) = promote(&history);
    let canonical = history.files();
    assert!(ready(begin(&history, &history.expected, saved, request(&history)).unwrap()).is_err());
    assert_eq!(history.files(), canonical);
    let mut expected = history.expected.clone();
    expected.oversight.effective_policy = candidate();
    for selected in [saved, first.topology_checkpoint()] {
        let run = ready(begin(&history, &expected, selected, request(&history)).unwrap()).unwrap();
        assert_eq!(run.policy_promotion_review().unwrap(), &reviewed);
        let retry = run.finish_promoted_policy(&reviewed, false).unwrap();
        assert_eq!(retry.promotion().appended_events(), 0);
        assert_eq!(retry.promotion().receipt(), first.promotion().receipt());
        assert_eq!(retry.topology_checkpoint(), first.topology_checkpoint());
        preserved(saved, &retry);
        assert_eq!(history.files(), canonical);
    }
    let run = ready(begin(&history, &expected, saved, request(&history)).unwrap()).unwrap();
    assert_eq!(run.finish_promoted_policy(&reviewed, true).unwrap_err(), Error::Binding.into());
    assert_eq!(history.files(), canonical);
    // Even a genuine later Fence cannot inherit the old source counters through
    // this operation-specific suffix. Its checkpoint must be retained separately.
    let (host, _) = FileOversight::open_with_learned_generation(history.root.store(),
        history.profile.clone(), &history.config).unwrap();
    expected.topology.available = false;
    drop(host);
    let later = history.files();
    assert!(begin(&history, &expected, saved, request(&history)).is_err());
    assert_eq!(history.files(), later);
}

#[test]
fn every_original_guard_and_complete_source_cut_remain_required_before_any_write() {
    let history = History::build_configured(Lane::Plain, Cut::Confirmed, None, true, true, true);
    let saved = history.topology.as_ref().unwrap();
    let before = history.files();
    for field in 0..12 {
        let mut wrong = history.expected.clone();
        match field {
            0 => wrong.oversight.guards.campaigns = None,
            1 => wrong.oversight.guards.stream = None,
            2 => wrong.oversight.guards.identity = None,
            3 => wrong.oversight.credential_epoch = None,
            4 => wrong.oversight.effective_policy = candidate(),
            5 => wrong.oversight.minimum.journal_revision += 1,
            6 => wrong.oversight.minimum.control_sequence += 1,
            7 => wrong.oversight.minimum.authority_epoch += 1,
            8 => wrong.topology.initial = graph(2, false),
            9 => wrong.topology.current = graph(1, true),
            10 => wrong.topology.available = !wrong.topology.available,
            _ => wrong.evaluation = None,
        }
        assert!(begin(&history, &wrong, saved, request(&history)).and_then(ready).is_err(), "{field}");
        assert_eq!(history.files(), before);
    }
    let foreign = History::with_source(Lane::Plain, Cut::Confirmed);
    for wrong in [history.early_topology.as_ref().unwrap(), foreign.topology.as_ref().unwrap()] {
        assert!(begin(&history, &history.expected, wrong, request(&history)).is_err());
        assert_eq!(history.files(), before);
    }
    let run = original(&history);
    let reviewed = run.policy_promotion_review().unwrap().clone();
    let other = original(&foreign).policy_promotion_review().unwrap().clone();
    assert_eq!(run.finish_promoted_policy(&other, false).unwrap_err(), Error::Binding.into());
    assert_eq!(history.files(), before);
    let result = original(&history).finish_promoted_policy(&reviewed, false).unwrap();
    preserved(saved, &result);
}

#[test]
fn mediated_promotion_spends_only_ordinary_capacity_and_keeps_terminal_reserve() {
    for remaining in 0..=3 {
        let history = History::build_configured(Lane::Plain, Cut::Unknown, Some(remaining), true, false, true);
        let before = history.files();
        let run = original(&history);
        let reviewed = run.policy_promotion_review().unwrap().clone();
        let promoted = run.finish_promoted_policy(&reviewed, false);
        if remaining < 3 {
            assert_eq!(promoted.unwrap_err(), Error::Limit.into());
            assert_eq!(history.files(), before);
        } else {
            let result = promoted.unwrap();
            assert_eq!(result.promotion().appended_events(), 3);
            assert_eq!(result.promotion().snapshot().control.ledger.charged, history.charge);
            preserved(history.topology.as_ref().unwrap(), &result);
        }
    }
}

#[test]
fn changed_canonical_or_failed_storage_returns_no_pair_and_exact_retry_recovers() {
    let history = History::with_source(Lane::Plain, Cut::Confirmed);
    let run = original(&history);
    let reviewed = run.policy_promotion_review().unwrap().clone();
    let canonical = history.root.bytes();
    let path = history.root.store().join("delivery.bin");
    let mut changed = canonical.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert_eq!(run.finish_promoted_policy(&reviewed, false).unwrap_err(), Error::Binding.into());
    assert_eq!(history.root.bytes(), changed);
    std::fs::write(&path, &canonical).unwrap();
    let pending = history.root.store().join("delivery.pending");
    std::fs::remove_file(&pending).unwrap();
    std::fs::create_dir(&pending).unwrap();
    assert_eq!(original(&history).finish_promoted_policy(&reviewed, false).unwrap_err(), JournalError::InvalidFile);
    assert_eq!(history.root.bytes(), canonical);
    assert!(pending.is_dir());
    std::fs::remove_dir(&pending).unwrap();
    let result = original(&history).finish_promoted_policy(&reviewed, false).unwrap();
    preserved(history.topology.as_ref().unwrap(), &result);
}

#[test]
fn promotion_custody_must_be_selected_before_replay_and_requires_original_terminal_reserve() {
    let history = History::with_source(Lane::Plain, Cut::Confirmed);
    let saved = history.topology.as_ref().unwrap();
    let before = history.files();
    let run = begin(&history, &history.expected, saved, request(&history)).unwrap();
    assert_eq!(run.policy_promotion_review().unwrap_err(), Error::Incomplete.into());
    drop(run);
    let mut late = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        history.root.store(), history.profile.clone(), &history.expected, &history.config,
        &history.anchor).unwrap();
    late.advance(0, 1).unwrap();
    assert_eq!(late.require_policy_promotion_checkpoint(request(&history), saved).unwrap_err(), Error::WrongState.into());
    assert_eq!(history.files(), before);
    let unreserved = History::build_configured(Lane::Plain, Cut::Confirmed, None, true, false, false);
    assert!(begin(&unreserved, &unreserved.expected, unreserved.topology.as_ref().unwrap(), request(&unreserved)).is_err());
    let (_, result) = promote(&history);
    preserved(saved, &result);
}
