//! Real producer bytes, original journal, review, two keys and endpoint receipts.
//! Fixtures establish enforcement cases, not authentic real-world observations.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, limits, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{PublicationProducerImage, PublicationProducerProfile};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::full_input::ActualHelperInput;
use fa_reference::product_frontier::ProductFrontiers;
use fa_reference::witness::SnapshotEntry;
use fa_reference::witness::refinement::RefinementBudget;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::{Error, Snapshot};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn changes() -> PublicationChangePolicy {
    PublicationChangePolicy { source: 41, after: 0, lookup: RoutingBudget { steps: 10000, bytes: 1000000 } }
}
fn freshness() -> PublicationFreshnessPolicy {
    PublicationFreshnessPolicy { clock_domain: profile().delivery.clock_domain, max_age_ticks: 3 }
}
fn producer_profile() -> PublicationProducerProfile {
    PublicationProducerProfile { source: fixture::SOURCE, scope: profile().delivery.scope,
        feed: 41, clock_domain: profile().delivery.clock_domain, after: 0 }
}
fn create(root: &Directory, enabled: bool, budget: PublicationLimits) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = if enabled {
        FileOversight::create_with_publication_snapshot_fallback(root.store(), profile(), budget, changes(), freshness())
    } else {
        FileOversight::create_with_publication_change_freshness(root.store(), profile(), budget, changes(), freshness())
    }.unwrap();
    // Bootstrap classification must leave the original terminal reserve usable.
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

struct Live {
    host: FileOversight,
    reviewer: FileHumanReviewer,
    action: FrozenAction,
    inputs: CommitteeInput,
    image: PublicationProducerImage,
    witness: PublicationInputFile,
    feed: PublicationFeedFile,
    root: Directory,
}
impl Live {
    fn new(enabled: bool, budget: PublicationLimits) -> Self {
        let root = Directory::new();
        let (mut host, reviewer) = create(&root, enabled, budget);
        let (action, inputs) = fixture::reviewed(&mut host, 1, b"visible");
        let image = PublicationProducerImage::new(producer_profile(),
            fixture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
        host.bind_publication_file_source(host.revision(), 1, image.capture(1, &action).unwrap(), fixture::requests()).unwrap();
        let path = root.0.join("producer.bin");
        let witness = PublicationInputFile::from_producer(&path, producer_profile(), 1, &action).unwrap();
        let feed = PublicationFeedFile::from_producer(&path, producer_profile()).unwrap();
        let live = Self { host, reviewer, action, inputs, image, witness, feed, root };
        live.write(); live
    }
    fn write(&self) {
        let pending = self.root.0.join("producer.next");
        std::fs::write(&pending, self.image.to_bytes().unwrap()).unwrap();
        std::fs::rename(pending, self.root.0.join("producer.bin")).unwrap();
    }
    fn evict_history(&mut self) {
        // Derive every notice through the real producer. Only unrelated keys
        // change. No fabricated coverage or hand-built truncated feed is used.
        for revision in 2..=131 {
            let other = if revision % 2 == 0 { 99 } else { 100 };
            self.image = self.image.advance(self.image.generation(),
                fixture::observations(&self.inputs, revision, &[0, 2, 4, other]), ElapsedTick(1)).unwrap();
        }
        assert!(self.image.batch().after() > 0);
        self.write();
    }
    fn refresh(&mut self, now: u64) {
        let (_, report) = self.host.refresh_publication_from_producer(self.host.revision(), 1,
            &self.witness, &self.feed, || ElapsedTick(now)).unwrap().unwrap();
        if self.image.batch().after() > 0 {
            assert_eq!(report.status.through, 0);
            assert_eq!(report.status.observed_through, self.image.batch().heartbeat().through);
            assert!(!report.status.complete());
            assert_eq!(report.freshness.eligibility, Err(Error::Incomplete));
            assert!(report.changes.is_empty(), "no invented historical notifications");
        }
    }
    fn keys(&mut self) -> Keys {
        let automatic = self.host.authorize(self.host.revision(), 1, &self.inputs, snapshot()).unwrap();
        let request = self.host.request_human_approval(self.host.revision(), 1001, 1, &self.inputs, ElapsedTick(31)).unwrap();
        let revision = self.host.revision();
        let human = self.reviewer.approve(&mut self.host, revision, &request).unwrap();
        Keys { automatic, human, request, action: self.action.clone(), inputs: self.inputs.clone() }
    }
}

#[test]
fn an_evicted_prefix_can_publish_only_after_exact_reacquisition_at_each_boundary() {
    let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1);
    let keys = live.keys();
    let before = live.host.inspect();
    assert_eq!(live.host.dispatch(live.host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(live.host.inspect(), before, "authorization capture is not a dispatch capture");
    live.refresh(1); fixture::dispatch(&mut live.host, &keys); live.refresh(1);
    let result = live.host.publish_checked(live.host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert!(!live.host.publication_change_status().unwrap().complete());
    live.host.reconcile(live.host.revision(), 1).unwrap();
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
    assert_eq!(live.host.inspect().executions, 1);
    assert_eq!(FileOversight::read_publication(live.root.store(), &profile()).unwrap(), live.host.inspect());
}

#[test]
fn default_strict_profile_still_refuses_the_same_current_snapshot_and_missing_history() {
    let mut live = Live::new(false, limits()); live.evict_history();
    assert!(!live.host.publication_snapshot_fallback_enabled().unwrap());
    assert_eq!(live.host.refresh_publication_from_producer(live.host.revision(), 1,
        &live.witness, &live.feed, || ElapsedTick(1)), Err(JournalError::Contract(Error::Incomplete)));
    assert!(live.host.storage_failure().is_some());
    assert_eq!(live.host.inspect().executions, 0);
    assert_eq!(live.host.inspect().control.ledger.available, 100);
}

#[test]
fn every_negative_lane_is_checked_after_dispatch_despite_absent_notifications() {
    for mode in 0..6 {
        let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1);
        let original = live.host.retained_publication_evidence(1).unwrap().clone();
        let keys = live.keys(); live.refresh(1); fixture::dispatch(&mut live.host, &keys);
        let rows: &[u64] = match mode {
            0 => &[2, 4], 1 => &[0, 1, 2, 4], 2 => &[0, 2, 4, 7],
            3 => &[0, 2, 3, 4], _ => &[0, 2, 4, 101],
        };
        let mut next = fixture::observations(&live.inputs, 132, rows);
        if mode == 4 {
            let input = next.opaque().unwrap();
            let mut profile = input.input_profile().clone(); profile.model_epoch += 1;
            let changed = ActualHelperInput::new(input.submitted_bytes().to_vec(), profile,
                input.ordered_parts().to_vec(), input.omissions().to_vec()).unwrap();
            next = FilePublicationInputs::new(next.structured().cloned(), Some(changed));
        }
        live.image = live.image.advance(live.image.generation(), next, ElapsedTick(2)).unwrap();
        live.write(); live.refresh(2);
        let result = live.host.publish_checked(live.host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, if mode == 5 { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(result.outcome, if mode == 5 { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.host.retained_publication_evidence(1).unwrap(), &original);
        assert_eq!(live.host.inspect().control.ledger.charged, 16, "sealing alone is not settlement");
        live.host.reconcile(live.host.revision(), 1).unwrap();
        assert_eq!(live.host.inspect().executions, u64::from(mode == 5));
        assert_eq!(live.host.inspect().control.ledger.charged, if mode == 5 { 16 } else { 0 });
    }
}

#[test]
fn missing_closure_and_exhausted_comparison_budget_never_become_validity() {
    for missing_close in [false, true] {
        let budget = if missing_close { limits() } else { PublicationLimits { validation: RefinementBudget::default(), ..limits() } };
        let mut live = Live::new(true, budget); live.evict_history();
        if missing_close {
            let old = live.image.inputs().structured().unwrap();
            let structured = FileWitnessInput::new(132, 132, 20, old.snapshot().domain_input(),
                [0, 2, 4].into_iter().map(|key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(),
                &ProductFrontiers::new(1, 1).unwrap()).unwrap();
            let next = FilePublicationInputs::new(Some(structured), live.image.inputs().opaque().cloned());
            live.image = live.image.advance(live.image.generation(), next, ElapsedTick(1)).unwrap(); live.write();
        }
        live.refresh(1);
        assert!(matches!(live.host.authorize(live.host.revision(), 1, &live.inputs, snapshot()),
            Err(JournalError::Contract(Error::Incomplete))));
        assert_eq!(live.host.inspect().control.ledger.reserved, 0);
        assert_eq!(live.host.inspect().executions, 0);
    }
}

#[test]
fn producer_lease_expiry_at_first_publication_seals_instead_of_extending_the_window() {
    let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1);
    let keys = live.keys(); live.refresh(1); fixture::dispatch(&mut live.host, &keys);
    live.refresh(3); // Still inside the original producer's [1,4) lease.
    let result = live.host.publish_checked(live.host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(4)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale));
    assert_eq!(result.outcome, sealed());
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
    live.host.reconcile(live.host.revision(), 1).unwrap();
    assert_eq!(live.host.inspect().control.ledger.charged, 0);
}

#[test]
fn new_review_can_bind_an_already_evicted_producer_but_original_binding_is_not_a_fresh_capture() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root, true, limits());
    let action = host.propose(host.revision(), 1, fixture::spec(&host, b"new review"), snapshot()).unwrap();
    let inputs = fixture::inputs(&action, b"complete source view");
    let mut image = PublicationProducerImage::new(producer_profile(), fixture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
    for revision in 2..=131 {
        image = image.advance(image.generation(), fixture::observations(&inputs, revision,
            &[0, 2, 4, if revision % 2 == 0 { 99 } else { 100 }]), ElapsedTick(1)).unwrap();
    }
    let path = root.0.join("producer.bin"); std::fs::write(&path, image.to_bytes().unwrap()).unwrap();
    let source = PublicationInputFile::from_producer(&path, producer_profile(), 1, &action).unwrap();
    let feed = PublicationFeedFile::from_producer(&path, producer_profile()).unwrap();
    host.bind_publication_from_producer(host.revision(), 1, &source, &feed, fixture::requests(), || ElapsedTick(1)).unwrap().unwrap();
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert!(!host.publication_change_status().unwrap().complete());
    fixture::review_existing(&mut host, 1, 101, &inputs);
    assert!(matches!(host.authorize(host.revision(), 1, &inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
    host.refresh_publication_from_producer(host.revision(), 1, &source, &feed, || ElapsedTick(1)).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.refresh_publication_from_producer(host.revision(), 1, &source, &feed, || ElapsedTick(1)).unwrap().unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    host.refresh_publication_from_producer(host.revision(), 1, &source, &feed, || ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap().basis, PublicationBasis::Revalidated);
}

#[test]
fn profile_pinning_is_bidirectional_before_cleanup_or_any_recovery_write() {
    for enabled in [false, true] {
        let root = Directory::new(); let (host, reviewer) = create(&root, enabled, limits());
        assert_eq!(host.publication_snapshot_fallback_enabled().unwrap(), enabled);
        let before = host.inspect(); drop(reviewer); drop(host);
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"not a canonical image").unwrap();
        let wrong = if enabled {
            FileOversight::open_with_publication_change_freshness(root.store(), profile(), limits(), changes(), freshness())
        } else {
            FileOversight::open_with_publication_snapshot_fallback(root.store(), profile(), limits(), changes(), freshness())
        };
        assert!(matches!(wrong, Err(JournalError::Contract(Error::Binding))));
        assert!(pending.exists());
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
        let correct = if enabled {
            FileOversight::open_with_publication_snapshot_fallback(root.store(), profile(), limits(), changes(), freshness())
        } else {
            FileOversight::open_with_publication_change_freshness(root.store(), profile(), limits(), changes(), freshness())
        };
        let (host, _) = correct.unwrap();
        assert_eq!(host.publication_snapshot_fallback_enabled().unwrap(), enabled);
        assert!(!pending.exists()); assert!(host.revision() > before.revision);
    }
}

#[test]
fn missing_file_retry_and_failed_final_install_never_reuse_the_old_observation() {
    let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1); let keys = live.keys();
    std::fs::remove_file(live.root.0.join("producer.bin")).unwrap();
    assert!(matches!(live.host.refresh_publication_from_producer(live.host.revision(), 1,
        &live.witness, &live.feed, || ElapsedTick(1)), Ok(Err(FileCaptureError::Io(_)))));
    assert!(live.host.storage_failure().is_none());
    assert_eq!(live.host.dispatch(live.host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    live.write(); live.refresh(1); fixture::dispatch(&mut live.host, &keys);
    let pending = live.root.store().join("delivery.pending");
    let failed = live.host.refresh_publication_from_producer(live.host.revision(), 1, &live.witness, &live.feed, || {
        std::fs::write(&pending, b"block the final replacement").unwrap(); ElapsedTick(1)
    });
    assert!(matches!(failed, Err(JournalError::Io(_))));
    assert!(live.host.storage_failure().is_some());
    assert_eq!(live.host.publication_snapshot_fallback_enabled(), Err(JournalError::Unavailable));
    assert_eq!(live.host.inspect().executions, 0);
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(live.root.store(), &profile()).unwrap(), live.host.inspect());
}

#[test]
fn generic_recovery_keeps_the_mode_and_execution_receipts_but_not_a_live_snapshot_lease() {
    let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1); let keys = live.keys();
    live.refresh(1); fixture::dispatch(&mut live.host, &keys); live.refresh(1);
    live.host.publish_checked(live.host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    std::fs::remove_file(live.root.0.join("producer.bin")).unwrap();
    let path = live.root.store(); drop(live.reviewer); drop(live.host);
    let (mut host, _) = FileOversight::open(&path, profile()).unwrap();
    assert!(host.publication_snapshot_fallback_enabled().unwrap());
    assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
    let receipt = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(50)).unwrap();
    assert_eq!(receipt.basis, PublicationBasis::PreviouslyResolved);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn bootstrap_is_required_and_clock_unwind_after_decoding_cannot_restore_old_eligibility() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_with_publication_validation(root.store(), profile(), limits()).unwrap();
    let revision = host.revision();
    assert_eq!(host.enable_publication_snapshot_fallback(revision), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), revision);
    let mut strict = Live::new(false, limits());
    let revision = strict.host.revision();
    assert_eq!(strict.host.enable_publication_snapshot_fallback(revision), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(strict.host.revision(), revision);
    let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1);
    let result = catch_unwind(AssertUnwindSafe(|| {
        live.host.refresh_publication_from_producer(live.host.revision(), 1, &live.witness, &live.feed,
            || panic!("no current clock"))
    }));
    assert!(result.is_err()); assert!(live.host.storage_failure().is_some());
    assert_eq!(live.host.inspect().executions, 0);
    assert!(matches!(live.host.authorize(live.host.revision(), 1, &live.inputs, snapshot()), Err(JournalError::Unavailable)));
}

#[test]
fn a_global_snapshot_lease_cannot_authorize_legacy_or_unbound_attempts() {
    for source_bound in [false, true] {
        let mut live = Live::new(true, limits()); live.evict_history(); live.refresh(1);
        let (action, inputs) = fixture::reviewed(&mut live.host, 2, b"no implicit cut");
        let original = fixture::packet(2, &action, &inputs, 1, &[0, 2, 4]);
        if source_bound {
            live.host.bind_publication_file_source(live.host.revision(), 2, original.clone(), fixture::requests()).unwrap();
            fixture::replace_source(&live.root, &original);
            fixture::refresh(&mut live.host, &live.root, 2);
        } else {
            let evidence = fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationEvidence::new(
                original.inputs().clone(), fixture::requests()).unwrap();
            live.host.bind_publication_evidence(live.host.revision(), 2, evidence).unwrap();
            live.host.record_publication_inputs(live.host.revision(), 2, 0, Some(original.inputs().clone())).unwrap();
        }
        assert_eq!(live.host.publication_input_cut(2).unwrap(), None);
        assert!(matches!(live.host.authorize(live.host.revision(), 2, &inputs, snapshot()),
            Err(JournalError::Contract(Error::Incomplete))));
        assert_eq!(live.host.inspect().control.ledger.reserved, 0);
        assert_eq!(live.host.inspect().executions, 0);
    }
}
