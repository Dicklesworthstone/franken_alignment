//! Sealed terminal recovery of real learned and predictive publication histories.
//! Decoder, retained forecast sources, sidecar, journal and endpoint are original;
//! neutral forecast coefficients and committee ballots are synthetic controls.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::{decoder::learned::FileLearnedRecovery,
        consistency::learned::FileLearnedConsistencySnapshot,
        guarded::{anchored::FileHistoryAnchor, learned_recovery::FileGuardedLearnedRecovery},
        shutdown::recovery::FileStoppedRecovery}};
use fa_reference::action::consequence::oversight::{learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Learned, Guarded, Raw, Owned }
const LANES: [Lane; 4] = [Lane::Learned, Lane::Guarded, Lane::Raw, Lane::Owned];

enum Run {
    Learned(FileLearnedRecovery), Guarded(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery), Owned(FileOwnedPredictiveRecovery),
}
impl Run {
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Learned(run) => run.progress(), Self::Guarded(run) => run.progress(),
            Self::Raw(run) => run.progress(), Self::Owned(run) => run.progress() }
    }
    fn advance(&mut self, expected: usize, count: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        match self { Self::Learned(run) => run.advance(expected, count),
            Self::Guarded(run) => run.advance(expected, count),
            Self::Raw(run) => run.advance(expected, count), Self::Owned(run) => run.advance(expected, count) }
    }
    fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        match self { Self::Learned(run) => run.stop_request(operation),
            Self::Guarded(run) => run.stop_request(operation),
            Self::Raw(run) => run.stop_request(operation), Self::Owned(run) => run.stop_request(operation) }
    }
    fn finish_stopped(self, request: StopRequest, clock: impl FnOnce() -> ElapsedTick)
        -> Result<FileStoppedRecovery, JournalError>
    {
        match self { Self::Learned(run) => run.finish_stopped(request, clock),
            Self::Guarded(run) => run.finish_stopped(request, clock),
            Self::Raw(run) => run.finish_stopped(request, clock),
            Self::Owned(run) => run.finish_stopped(request, clock) }
    }
    // Only an independent fixture author uses ordinary finish to produce a
    // different, fully valid canonical successor for the conflict controls.
    fn ordinary_fixture_owner(self) -> FileOversight {
        match self { Self::Learned(run) => run.finish().unwrap().0,
            Self::Guarded(run) => run.finish().unwrap().0,
            Self::Raw(run) => run.finish().unwrap().0, Self::Owned(run) => run.finish().unwrap().0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut { Prompt, Reserved, Sent, Executed, Expired }

struct History {
    root: Directory, lane: Lane, profile: FileOversightProfile, generation: FileLearnedConfig,
    expected: FileRecoveryRequirements, anchor: FileHistoryAnchor,
    before: FileDeliverySnapshot, request: StopRequest, attempt: Option<u64>, units: u64,
    owned_source: Option<FileLearnedConsistencySnapshot>,
}
impl History {
    fn new(lane: Lane, cut: Cut, event_limit: Option<usize>) -> Self {
        let root = Directory::new(); let mut profile = profile();
        if let Some(limit) = event_limit { profile.delivery.limits.events = limit; }
        let generation = match lane {
            Lane::Learned | Lane::Guarded => finite_recipe(4, false, false).with_required_sidecar().unwrap(),
            Lane::Raw => configured(false), Lane::Owned => configured(true),
        };
        let (mut host, reviewer, observer) = match lane {
            Lane::Learned | Lane::Guarded => {
                let (host, reviewer) = FileOversight::create_with_learned_text_stream_with_reserve(
                    root.store(), profile.clone(), generation.clone(), RecoveryReserve::terminal()).unwrap();
                (host, reviewer, None)
            }
            Lane::Raw | Lane::Owned => {
                let (host, reviewer, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
                    root.store(), profile.clone(), generation.clone(), RecoveryReserve::terminal()).unwrap();
                (host, reviewer, Some(observer))
            }
        };
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
        if let Some(observer) = observer.as_ref() { predict(&mut host, observer, lane == Lane::Owned); }
        let (attempt, units) = if cut == Cut::Prompt { (None, 0) } else {
            generate(&mut host);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
            let (attempt, action) = request(&mut host, 71, Some("A"));
            if cut != Cut::Reserved {
                let input = dispatch_sidecar(&mut host, &reviewer, attempt, &action);
                if cut == Cut::Executed { publish(&mut host, attempt, &input, 2); }
            }
            (Some(attempt), action.spec().units)
        };
        if event_limit.is_some() {
            assert_eq!(cut, Cut::Prompt);
            while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            }
        }
        let before = host.inspect();
        let request = StopRequest { operation: 501, expected_control_sequence: before.control.sequence,
            expected_authority_epoch: before.control.ledger.epoch };
        let expected = requirements(&host, &profile, &generation);
        let anchor = host.history_anchor().unwrap();
        let owned_source = (lane == Lane::Owned).then(|| host.learned_action_consistency_snapshot().unwrap());
        drop(host);
        Self { root, lane, profile, generation, expected, anchor, before, request, attempt, units, owned_source }
    }
    fn begin_with(&self, expected: &FileRecoveryRequirements, reserve: RecoveryReserve)
        -> Result<Run, JournalError>
    {
        match self.lane {
            Lane::Learned => FileOversight::begin_open_with_learned_generation(self.root.store(),
                self.profile.clone(), &self.generation).map(Run::Learned),
            Lane::Guarded => FileOversight::begin_open_guarded_anchored_with_learned_generation(
                self.root.store(), self.profile.clone(), expected, &self.generation, &self.anchor)?
                .require_recovery_reserve(reserve).map(Run::Guarded),
            Lane::Raw => FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                self.root.store(), self.profile.clone(), &FilePredictiveRequirements {
                    oversight: expected.clone(), prediction: self.generation.required_pre_output_forecast().unwrap().clone(),
                    evaluation: None }, &self.generation, &self.anchor)?
                .require_recovery_reserve(reserve).map(Run::Raw),
            Lane::Owned => FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                self.root.store(), self.profile.clone(), &FileOwnedPredictiveRequirements {
                    oversight: expected.clone(), prediction: self.generation.required_owned_pre_output_forecast().unwrap().clone(),
                    evaluation: None }, &self.generation, &self.anchor)?
                .require_recovery_reserve(reserve).map(Run::Owned),
        }
    }
    fn begin(&self) -> Run { self.begin_with(&self.expected, RecoveryReserve::terminal()).unwrap() }
    fn ready(&self) -> Run {
        let before = self.root.files(); let mut run = self.begin();
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1).unwrap();
            assert_eq!(self.root.files(), before, "replay itself may not write or clean up");
        }
        assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready); run
    }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.root.store().join("delivery.bin")).unwrap() }
    fn pending(&self) { std::fs::write(self.root.store().join("delivery.pending"), b"unconfirmed candidate").unwrap(); }
    fn assert_observed(&self, result: &FileStoppedRecovery) {
        let disk = if self.lane == Lane::Owned {
            let observed = FileOversight::read_predictive_guarded_anchored_with_owned_learned_consistency(
                self.root.store(), &self.profile, &FileOwnedPredictiveRequirements {
                    oversight: self.expected.clone(),
                    prediction: self.generation.required_owned_pre_output_forecast().unwrap().clone(), evaluation: None,
                }, &self.generation, &self.anchor).unwrap();
            let source = self.owned_source.as_ref().unwrap();
            assert_eq!(observed.journal.consistency.work, source.work);
            assert_eq!(observed.journal.consistency.retained_source_bytes, source.retained_source_bytes);
            assert_eq!(observed.journal.consistency.consistency.evidence, source.consistency.evidence);
            observed.journal.publication
        } else {
            FileOversight::read_publication_with_learned_generation(
                self.root.store(), &self.profile, &self.generation).unwrap()
        };
        assert_eq!(&disk, result.snapshot());
    }
}

fn dispatch_sidecar(host: &mut FileOversight, reviewer: &FileHumanReviewer,
    attempt: u64, action: &FrozenAction) -> CommitteeInput
{
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let packet = host.begin_learned_sidecar(host.revision(), attempt, numerical.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + attempt, generation: 1,
            transform_id: 7 }, priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let input = packet.packet.input().clone(); let round = 100 + attempt;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&input), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &input, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &offer).unwrap();
    host.dispatch(host.revision(), &automatic, &human, action, &input, snapshot()).unwrap(); input
}

#[test]
fn stop_selection_requires_complete_original_replay_and_exact_predecessors() {
    for lane in LANES {
        let history = History::new(lane, Cut::Reserved, None); history.pending();
        let before = history.root.files(); let run = history.begin();
        assert_eq!(run.stop_request(501), Err(Error::Incomplete.into()));
        assert_eq!(run.finish_stopped(history.request, || panic!("incomplete replay has no clock")),
            Err(Error::Incomplete.into()));
        assert_eq!(history.root.files(), before);
        let run = history.ready();
        assert_eq!(run.stop_request(0), Err(Error::InvalidInput.into()));
        assert_eq!(run.stop_request(501).unwrap(), history.request);
        drop(run);
        for authority in [false, true] {
            let run = history.ready(); let mut stale = history.request;
            if authority { stale.expected_authority_epoch += 1; } else { stale.expected_control_sequence += 1; }
            assert_eq!(run.finish_stopped(stale, || panic!("stale predecessor has no clock")), Err(Error::Stale.into()));
            assert_eq!(history.root.files(), before);
        }
        let run = history.ready();
        let result = run.finish_stopped(history.request, || ElapsedTick(2)).unwrap();
        assert_eq!(result.snapshot().revision, history.before.revision + 3);
        assert_eq!(result.progress().receipt.request(), history.request);
        assert_eq!(result.progress().receipt.cancelled(), &[history.attempt.unwrap()]);
        assert_eq!(result.progress().receipt.refunded_units(), history.units);
        assert!(result.progress().drained()); history.assert_observed(&result);
    }
}

#[test]
fn direct_stop_spends_only_the_original_terminal_tail_and_completed_retry_needs_no_clock() {
    for lane in LANES {
        let history = History::new(lane, Cut::Prompt, Some(10));
        assert_eq!(history.before.revision, 7);
        let original = history.bytes(); let run = history.ready();
        assert_eq!(run.stop_request(501).unwrap(), history.request);
        let calls = Cell::new(0);
        let first = run.finish_stopped(history.request, || { calls.set(calls.get() + 1); ElapsedTick(2) }).unwrap();
        assert_eq!(calls.get(), 1); assert!(matches!(first, FileStoppedRecovery::Advanced { .. }));
        assert_eq!(first.snapshot().revision, 10); assert!(first.progress().drained());
        assert_eq!(first.snapshot().executions, 0); history.assert_observed(&first);
        let canonical = history.bytes();
        for _ in 0..2 {
            std::fs::write(history.root.store().join("delivery.pending"), &original).unwrap();
            let before = history.root.files(); let run = history.ready();
            assert_eq!(run.stop_request(502), Err(Error::Duplicate.into()));
            let mut foreign = history.request; foreign.operation = 502;
            assert_eq!(run.finish_stopped(foreign, || panic!("foreign stop has no clock")), Err(Error::Duplicate.into()));
            assert_eq!(history.root.files(), before);
            let run = history.ready(); let mut different = history.request; different.expected_authority_epoch += 1;
            assert_eq!(run.finish_stopped(different, || panic!("changed stop has no clock")), Err(Error::Binding.into()));
            assert_eq!(history.root.files(), before);
            let run = history.ready(); assert_eq!(run.stop_request(501).unwrap(), history.request);
            let again = run.finish_stopped(history.request, || panic!("drained retry has no clock or capacity cost")).unwrap();
            assert!(matches!(again, FileStoppedRecovery::AlreadyDrained { .. }));
            assert_eq!(again.snapshot(), first.snapshot()); assert_eq!(again.progress(), first.progress());
            assert_eq!(history.bytes(), canonical); assert!(!history.root.store().join("delivery.pending").exists());
        }
    }
}

#[test]
fn terminal_recovery_distinguishes_unspent_sent_executed_and_expired_original_effects() {
    for lane in LANES {
        for cut in [Cut::Reserved, Cut::Sent, Cut::Executed, Cut::Expired] {
            let history = History::new(lane, cut, None); let attempt = history.attempt.unwrap();
            assert_eq!(history.before.executions, u64::from(cut == Cut::Executed));
            assert_eq!(history.before.control.ledger.reserved, if cut == Cut::Reserved { history.units } else { 0 });
            assert_eq!(history.before.control.ledger.charged, if cut == Cut::Reserved { 0 } else { history.units });
            let calls = Cell::new(0); let run = history.ready();
            let result = run.finish_stopped(history.request, || {
                calls.set(calls.get() + 1); ElapsedTick(if cut == Cut::Expired { 1001 } else { 2 })
            }).unwrap();
            assert_eq!(calls.get(), 1); assert_eq!(result.snapshot().revision, history.before.revision + 3);
            assert_eq!(result.snapshot().executions, history.before.executions);
            assert_eq!(result.progress().reserved_units, 0);
            assert_eq!(result.progress().charged_units,
                if matches!(cut, Cut::Executed | Cut::Expired) { history.units } else { 0 });
            assert_eq!(result.progress().drained(), cut != Cut::Expired);
            if cut == Cut::Expired {
                let FileStoppedRecovery::Advanced { sweep, .. } = &result else { panic!("first stop advances"); };
                assert_eq!(sweep.outcomes[&attempt], Ok(Reconciliation::RetentionExpired));
                assert_eq!(sweep.progress.unresolved, vec![attempt]);
                assert_eq!(sweep.progress.irrecoverable, vec![attempt]);
                assert_eq!(result.snapshot().control.ledger.stages[&attempt], ActionState::IrrecoverablyUnknown);
            } else {
                assert!(result.progress().unresolved.is_empty());
                assert!(result.progress().irrecoverable.is_empty());
                if cut == Cut::Executed {
                    assert_eq!(result.snapshot().control.ledger.stages[&attempt], ActionState::Confirmed);
                } else if cut == Cut::Sent {
                    assert_eq!(result.snapshot().control.ledger.stages[&attempt], ActionState::ConfirmedNotExecuted);
                }
            }
            history.assert_observed(&result);
            let canonical = history.bytes(); let run = history.ready();
            let again = run.finish_stopped(history.request, || {
                assert_eq!(cut, Cut::Expired, "drained histories never acquire new time");
                calls.set(calls.get() + 1); ElapsedTick(1002)
            }).unwrap();
            assert_eq!(calls.get(), if cut == Cut::Expired { 2 } else { 1 });
            assert_eq!(again.snapshot().executions, history.before.executions);
            assert_eq!(again.progress().charged_units, result.progress().charged_units);
            assert_eq!(again.progress().drained(), result.progress().drained());
            if cut == Cut::Expired {
                assert_eq!(again.snapshot().revision, result.snapshot().revision + 3);
                assert_eq!(again.progress().unresolved, vec![attempt]);
                assert_eq!(again.progress().irrecoverable, vec![attempt]);
            } else { assert_eq!(again.snapshot(), result.snapshot()); assert_eq!(history.bytes(), canonical); }
            history.assert_observed(&again);
        }
    }
}

#[test]
fn complete_guard_floors_and_exact_reserve_remain_required_for_sealed_stop() {
    for lane in [Lane::Guarded, Lane::Raw, Lane::Owned] {
        let history = History::new(lane, Cut::Sent, None); history.pending(); let before = history.root.files();
        assert_eq!(history.begin_with(&history.expected, RecoveryReserve { events: 4, bytes: 60 }).err(),
            Some(Error::Binding.into()));
        assert_eq!(history.root.files(), before);
        let mut stale = history.expected.clone(); stale.minimum.journal_revision += 1;
        let mut run = history.begin_with(&stale, RecoveryReserve::terminal()).unwrap();
        assert_eq!(run.advance(0, run.progress().total_events), Err(Error::Stale.into()));
        assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Failed(Error::Stale));
        assert_eq!(run.stop_request(501), Err(Error::Stale.into()));
        assert_eq!(run.finish_stopped(history.request, || panic!("failed guard has no clock")), Err(Error::Stale.into()));
        assert_eq!(history.root.files(), before);
        let result = history.ready().finish_stopped(history.request, || ElapsedTick(2)).unwrap();
        assert!(result.progress().drained()); assert_eq!(result.snapshot().executions, 0);
        history.assert_observed(&result);
    }
}

#[test]
fn canonical_change_before_or_during_stop_clock_keeps_all_foreign_bytes_and_staging() {
    for lane in LANES {
        let history = History::new(lane, Cut::Sent, None); let original = history.bytes();
        let mut successor = history.ready().ordinary_fixture_owner();
        successor.observe_time(successor.revision(), ElapsedTick(2)).unwrap();
        let alternate = history.bytes(); assert_ne!(alternate, original); drop(successor);
        std::fs::write(history.root.store().join("delivery.bin"), &original).unwrap();
        for during_clock in [false, true] {
            history.pending(); let run = history.ready(); let calls = Cell::new(0);
            if !during_clock { std::fs::write(history.root.store().join("delivery.bin"), &alternate).unwrap(); }
            let error = run.finish_stopped(history.request, || {
                calls.set(calls.get() + 1);
                assert!(during_clock, "a changed canonical predecessor must refuse before the clock");
                std::fs::write(history.root.store().join("delivery.bin"), &alternate).unwrap(); ElapsedTick(2)
            }).unwrap_err();
            assert_eq!(error, Error::Binding.into()); assert_eq!(calls.get(), usize::from(during_clock));
            assert_eq!(history.bytes(), alternate);
            assert_eq!(std::fs::read(history.root.store().join("delivery.pending")).unwrap(), b"unconfirmed candidate");
            std::fs::write(history.root.store().join("delivery.bin"), &original).unwrap();
        }
        let result = history.ready().finish_stopped(history.request, || ElapsedTick(2)).unwrap();
        assert_eq!(result.snapshot().revision, history.before.revision + 3);
        assert_eq!(result.snapshot().executions, 0); assert!(result.progress().drained());
        history.assert_observed(&result);
    }
}

#[test]
fn stale_or_unavailable_stop_clock_cannot_publish_a_recovery_fence_or_clean_staging() {
    for lane in LANES {
        let history = History::new(lane, Cut::Sent, None); history.pending(); let before = history.root.files();
        assert_eq!(history.ready().finish_stopped(history.request, || ElapsedTick(0)), Err(Error::Stale.into()));
        assert_eq!(history.root.files(), before);
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            history.ready().finish_stopped(history.request, || panic!("trusted clock unavailable"))
        }));
        assert!(interrupted.is_err()); assert_eq!(history.root.files(), before);
        let result = history.ready().finish_stopped(history.request, || ElapsedTick(2)).unwrap();
        assert_eq!(result.snapshot().revision, history.before.revision + 3);
        assert!(result.progress().drained()); assert_eq!(result.snapshot().executions, 0);
        history.assert_observed(&result);
    }
}

#[test]
fn an_original_unfenced_stop_reuses_its_historical_request_and_finishes_the_terminal_transaction() {
    for lane in LANES {
        let history = History::new(lane, Cut::Sent, None);
        let mut host = history.ready().ordinary_fixture_owner();
        let control = host.inspect().control;
        let original = StopRequest { operation: 501, expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch };
        host.request_stop(host.revision(), original).unwrap();
        assert!(!host.stop_progress().unwrap().drained());
        let before = host.inspect();
        assert_ne!(original.expected_authority_epoch, before.control.ledger.epoch);
        drop(host);
        let run = history.ready();
        assert_eq!(run.stop_request(501).unwrap(), original);
        assert_eq!(run.stop_request(502), Err(Error::Duplicate.into()));
        let result = run.finish_stopped(original, || ElapsedTick(2)).unwrap();
        assert!(matches!(result, FileStoppedRecovery::Advanced { .. }));
        assert_eq!(result.snapshot().revision, before.revision + 3);
        assert_eq!(result.progress().receipt.request(), original);
        assert!(result.progress().drained()); assert_eq!(result.progress().charged_units, 0);
        assert_eq!(result.snapshot().executions, 0); history.assert_observed(&result);
    }
}

#[test]
fn anchored_stop_rejects_an_original_equal_counter_fork_without_cleanup_and_accepts_its_saved_cut() {
    for lane in [Lane::Guarded, Lane::Raw, Lane::Owned] {
        let mut history = History::new(lane, Cut::Sent, None); let common = history.bytes();
        let mut left = history.ready().ordinary_fixture_owner();
        left.observe_time(left.revision(), ElapsedTick(2)).unwrap();
        let left_view = left.inspect(); let selected = left.history_anchor().unwrap();
        let left_expected = requirements(&left, &history.profile, &history.generation);
        let left_bytes = history.bytes(); drop(left);
        std::fs::write(history.root.store().join("delivery.bin"), &common).unwrap();
        let mut right = history.ready().ordinary_fixture_owner();
        right.observe_time(right.revision(), ElapsedTick(3)).unwrap();
        let right_view = right.inspect(); drop(right);
        assert_eq!(left_view.revision, right_view.revision);
        assert_eq!(left_view.control.sequence, right_view.control.sequence);
        assert_eq!(left_view.control.ledger.epoch, right_view.control.ledger.epoch);
        assert_ne!(left_bytes, history.bytes());
        history.anchor = selected; history.expected = left_expected; history.pending();
        let before = history.root.files();
        assert_eq!(history.begin_with(&history.expected, RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
        assert_eq!(history.root.files(), before);
        std::fs::write(history.root.store().join("delivery.bin"), left_bytes).unwrap();
        let run = history.ready(); let request = run.stop_request(501).unwrap();
        assert_eq!(request.expected_control_sequence, left_view.control.sequence);
        assert_eq!(request.expected_authority_epoch, left_view.control.ledger.epoch);
        let result = run.finish_stopped(request, || ElapsedTick(4)).unwrap();
        assert_eq!(result.snapshot().revision, left_view.revision + 3);
        assert!(result.progress().drained()); assert_eq!(result.snapshot().executions, 0);
        history.assert_observed(&result);
    }
}

#[path = "stop_recovery/phases.rs"]
mod phases;
