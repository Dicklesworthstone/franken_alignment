//! Real helper-socket reviews and durable observations, not simulated approvals.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/publication_input_cut.rs"] mod cuts;
use driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::publication::deferred::FileDeferrableDriverReport;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::oversight::{CommitteeInput, human::HumanDisposition, supervised::DriverEvidence};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn replace(path: &Path, bytes: &[u8]) {
    let next = path.with_extension("next");
    std::fs::write(&next, bytes).unwrap(); std::fs::rename(next, path).unwrap();
}
fn feed_at(path: &Path, after: u64, through: u64) {
    let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: cuts::FEED,
        clock_domain: profile().delivery.clock_domain, generation: through + 1,
        through, produced_at: ElapsedTick(1) }, after,
        (after + 1..=through).map(|sequence| PublicationChange { source: cuts::FEED,
            sequence, change: WitnessChange::All }).collect()).unwrap();
    replace(path, &batch.to_bytes().unwrap());
}
struct Live {
    rig: Rig,
    action: FrozenAction,
    inputs: CommitteeInput,
    human: FileHumanPermit,
    witness: PublicationInputFile,
    feed: PublicationFeedFile,
    witness_path: PathBuf,
    feed_path: PathBuf,
}
impl Live {
    fn new() -> Self {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let r = host.revision(); host.enable_publication_validation(r, cuts::base::limits()).unwrap();
            let r = host.revision(); host.enable_publication_changes(r, PublicationChangePolicy {
                source: cuts::FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
            }).unwrap();
            let r = host.revision(); host.enable_publication_change_freshness(r, PublicationFreshnessPolicy {
                clock_domain: profile().delivery.clock_domain, max_age_ticks: 1000,
            }).unwrap();
        }
        let _ticket = rig.submit(1); rig.reviewed(1);
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let inputs = rig.inputs.clone().unwrap();
        let witness_path = rig.root.0.join("witness.bin");
        let feed_path = rig.root.0.join("feed.bin");
        let original = cuts::packet(&action, &inputs, 1, 0, &[0, 2, 4], false);
        replace(&witness_path, &original.to_bytes().unwrap()); feed_at(&feed_path, 0, 0);
        let witness = PublicationInputFile::new(&witness_path, cuts::base::SOURCE).unwrap();
        let feed = PublicationFeedFile::new(&feed_path, cuts::FEED).unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let r = host.revision(); host.refresh_publication_feed(r, &feed, || ElapsedTick(1)).unwrap().unwrap();
            let r = host.revision(); host.bind_publication_file_source(r, 1, original, cuts::base::requests()).unwrap();
        }
        let human = rig.human(1001, 31);
        Self { rig, action, inputs, human, witness, feed, witness_path, feed_path }
    }
    fn packet(&self, generation: u64, through: u64, keys: &[u64]) {
        replace(&self.witness_path, &cuts::packet(&self.action, &self.inputs, generation,
            through, keys, false).to_bytes().unwrap());
    }
    fn step(&mut self, now: u64) -> FileDeferrableDriverReport {
        let inputs = self.inputs.clone();
        self.rig.driver.step_with_publication_deferral(&self.witness, Some(&self.feed),
            || ElapsedTick(now), |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) }),
            Some(&self.human), None)
    }
    fn reserved(&self) -> u64 { self.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved }
}

#[test]
fn lag_before_authorization_waits_without_reserving_then_catches_up_and_publishes() {
    let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
    let report = live.step(2);
    assert!(report.waiting_for_producer().unwrap().outcome.deferred());
    assert!(matches!(report.step.publication.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(live.reserved(), 0);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    let original = live.rig.driver.supervisor().host().unwrap().retained_publication_evidence(1).unwrap().clone();
    live.packet(2, 1, &[0, 2, 4]);
    let report = live.step(2); assert!(report.waiting_for_producer().is_none());
    assert!(matches!(report.step.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert!(matches!(live.step(2).step.publication.evidence.result, Ok(FileDriverEvent::PublicationChecked { .. })));
    assert!(matches!(live.step(2).step.publication.evidence.result, Ok(FileDriverEvent::Reconciled { .. })));
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.retained_publication_evidence(1).unwrap(), &original);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn lag_between_authorization_and_dispatch_retains_the_original_reservation_and_key() {
    let mut live = Live::new();
    let calls = Cell::new(0); let changed = Cell::new(false);
    let feed_path = live.feed_path.clone(); let inputs = live.inputs.clone();
    let report = live.rig.driver.step_with_publication_deferral(&live.witness, Some(&live.feed), || {
        if calls.get() == 1 && !changed.replace(true) { feed_at(&feed_path, 0, 1); }
        ElapsedTick(2)
    }, |_, _| { calls.set(calls.get() + 1); Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) }) },
        Some(&live.human), None);
    assert_eq!(calls.get(), 2); assert_eq!(report.captures.len(), 2);
    assert!(!report.captures[0].outcome.deferred()); assert!(report.waiting_for_producer().is_some());
    assert_eq!(live.reserved(), 16);
    assert_eq!(live.rig.driver.supervisor().host().unwrap().human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    live.packet(2, 1, &[0, 2, 4]);
    assert!(matches!(live.step(3).step.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn caught_up_but_changed_negative_evidence_cannot_reuse_the_original_judgment() {
    let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
    assert!(live.step(2).waiting_for_producer().is_some());
    live.packet(2, 1, &[0, 1, 2, 4]);
    let report = live.step(3); assert!(report.waiting_for_producer().is_none());
    assert!(matches!(report.step.publication.evidence.result, Err(JournalError::Contract(Error::Stale))));
    assert_eq!(live.reserved(), 0); assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn equivocation_and_missing_feed_history_are_not_recoverable_lag() {
    for equivocation in [false, true] {
        let mut live = Live::new();
        if equivocation {
            feed_at(&live.feed_path, 0, 1); live.packet(2, 0, &[0, 2, 4]);
            assert!(live.step(2).waiting_for_producer().is_some());
            live.packet(2, 0, &[0, 2, 4, 99]);
        } else { feed_at(&live.feed_path, 1, 2); }
        let report = live.step(3); assert!(report.waiting_for_producer().is_none());
        assert!(report.step.publication.evidence.result.is_err());
        assert!(live.rig.driver.supervisor().host().unwrap().storage_failure().is_some());
        assert_eq!(live.reserved(), 0);
    }
}

#[test]
fn expired_or_revoked_human_keys_never_receive_a_wait_recommendation() {
    for revoke in [false, true] {
        let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
        if revoke {
            let mut host = live.rig.driver.supervisor_mut().host_mut().unwrap();
            let r = host.revision(); live.rig.reviewer.revoke_all(&mut host, r).unwrap();
        }
        let report = live.step(if revoke { 2 } else { 31 });
        assert!(report.waiting_for_producer().is_none());
        assert!(report.step.publication.evidence.result.is_err());
        assert_eq!(live.reserved(), 0);
    }
}

#[test]
fn missing_source_and_changed_committee_are_distinct_from_acknowledged_deferral() {
    for committee in [false, true] {
        let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
        if !committee { std::fs::remove_file(&live.witness_path).unwrap(); }
        let inputs = live.inputs.clone();
        let report = live.rig.driver.step_with_publication_deferral(&live.witness, Some(&live.feed), || ElapsedTick(2),
            |_, _| if committee { Err(Error::Incomplete) } else { Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) }) },
            Some(&live.human), None);
        assert!(report.waiting_for_producer().is_none()); assert!(report.captures.is_empty());
        assert!(report.step.publication.evidence.result.is_err());
        assert!(live.rig.driver.supervisor().host().unwrap().storage_failure().is_none());
    }
}

#[test]
fn post_dispatch_lag_stays_strict_and_never_reopens_a_wait_or_refunds_unknown_work() {
    let mut live = Live::new();
    assert!(matches!(live.step(2).step.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    feed_at(&live.feed_path, 0, 1);
    let report = live.step(3); assert!(report.waiting_for_producer().is_none());
    assert!(report.captures.is_empty());
    assert!(matches!(report.step.publication.evidence.result, Ok(FileDriverEvent::PublicationUnknown { .. })));
    let host = live.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn cancellation_while_waiting_skips_both_providers_and_does_not_restart_the_job() {
    let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
    assert!(live.step(2).waiting_for_producer().is_some());
    {
        let mut host = live.rig.driver.supervisor_mut().host_mut().unwrap();
        let r = host.revision(); host.cancel_request(r, 1).unwrap();
    }
    let report = live.rig.driver.step_with_publication_deferral(&live.witness, Some(&live.feed),
        || panic!("cancelled job must not sample time"), |_, _| panic!("cancelled job must not read"), Some(&live.human), None);
    assert!(report.waiting_for_producer().is_none()); assert!(report.captures.is_empty());
    assert!(matches!(report.step.publication.evidence.result, Ok(FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. })));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
}

#[test]
fn a_caught_provider_panic_cannot_leave_old_capture_eligibility_or_erase_history() {
    let mut live = Live::new(); feed_at(&live.feed_path, 0, 1);
    assert!(live.step(2).waiting_for_producer().is_some());
    let result = catch_unwind(AssertUnwindSafe(|| live.rig.driver.step_with_publication_deferral(
        &live.witness, Some(&live.feed), || ElapsedTick(3), |_, _| panic!("provider interrupted"), Some(&live.human), None)));
    assert!(result.is_err());
    let host = live.rig.driver.supervisor().host().unwrap();
    let source = host.publication_source(1).unwrap().unwrap();
    assert!(!source.fresh); assert_eq!(source.generation, 1);
    assert_eq!(host.publication_input_cut(1).unwrap().unwrap().required_through, 1);
    assert_eq!(host.inspect().control.ledger.reserved, 0); assert_eq!(host.inspect().executions, 0);
}
