//! One concrete bundle acquisition feeds the original change and witness gates.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{FilePublicationProducer, PublicationProducerProfile};
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn producer_profile() -> PublicationProducerProfile {
    PublicationProducerProfile { source: fixture::SOURCE, scope: profile().delivery.scope,
        feed: 41, clock_domain: profile().delivery.clock_domain, after: 0 }
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
struct Live { host: FileOversight, producer: FilePublicationProducer, keys: Keys, _reviewer: FileHumanReviewer }
impl Live {
    fn new(root: &Directory) -> Self {
        let (mut host, reviewer) = fixture::source_host(root);
        host.enable_publication_changes(host.revision(), PublicationChangePolicy { source: 41, after: 0,
            lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } }).unwrap();
        host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
            clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
        }).unwrap();
        let (action, inputs) = fixture::reviewed(&mut host, 1, b"visible");
        let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_profile(),
            fixture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
        host.refresh_publication_feed(host.revision(), &producer.feed_reader().unwrap(), || ElapsedTick(1)).unwrap().unwrap();
        let source = producer.witness_reader(1, &action).unwrap();
        host.bind_publication_file_source(host.revision(), 1, source.read_capture().unwrap(), fixture::requests()).unwrap();
        host.refresh_publication_from_producer(host.revision(), 1, &source, &producer.feed_reader().unwrap(),
            || ElapsedTick(1)).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        Self { host, producer, keys: Keys { action, inputs, automatic, human, request }, _reviewer: reviewer }
    }
    fn refresh(&mut self, now: u64) {
        self.host.refresh_publication_from_producer(self.host.revision(), 1,
            &self.producer.witness_reader(1, &self.keys.action).unwrap(), &self.producer.feed_reader().unwrap(),
            || ElapsedTick(now)).unwrap().unwrap();
    }
    fn dispatch(&mut self) -> Result<(), JournalError> {
        self.host.dispatch(self.host.revision(), &self.keys.automatic, &self.keys.human,
            &self.keys.action, &self.keys.inputs, snapshot())
    }
}

#[test]
fn caught_up_bundle_installs_after_its_own_notifications_and_publishes_with_original_keys() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
    let (identity, feed) = live.host.refresh_publication_from_producer(live.host.revision(), 1,
        &live.producer.witness_reader(1, &live.keys.action).unwrap(), &live.producer.feed_reader().unwrap(),
        || ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(identity.generation, 2); assert_eq!(feed.changes.len(), 1);
    assert_eq!(feed.status.through, 1);
    assert_eq!(live.host.publication_input_cut(1).unwrap().unwrap().last.through, 1);
    assert!(live.host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    live.dispatch().unwrap(); live.refresh(2);
    let result = live.host.publish_checked(live.host.revision(), 1, Some(&live.keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    live.host.reconcile(live.host.revision(), 1).unwrap();
    assert_eq!(live.host.inspect().executions, 1);
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), live.host.inspect());
}

#[test]
fn replacement_after_the_single_read_does_not_mix_generations_and_the_next_boundary_reopens() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
    let source = live.producer.witness_reader(1, &live.keys.action).unwrap();
    let feed = live.producer.feed_reader().unwrap();
    let next = fixture::observations(&live.keys.inputs, 3, &[0, 1, 2, 4, 99]);
    let mut calls = 0;
    let (identity, report) = live.host.refresh_publication_from_producer(live.host.revision(), 1, &source, &feed, || {
        calls += 1;
        live.producer.publish(2, next.clone(), ElapsedTick(2)).unwrap();
        ElapsedTick(2)
    }).unwrap().unwrap();
    assert_eq!(calls, 1); assert_eq!(identity.generation, 2); assert_eq!(report.status.through, 1);
    assert_eq!(live.host.publication_input_cut(1).unwrap().unwrap().last.through, 1);
    // The producer now has a DIFFERENT pair. A new acquisition must see it.
    live.refresh(2);
    assert_eq!(live.host.publication_input_cut(1).unwrap().unwrap().last.through, 2);
    assert_eq!(live.dispatch(), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    assert_eq!(live.host.inspect().executions, 0);
}

#[test]
fn post_dispatch_value_absence_range_and_unrelated_controls_keep_native_receipt_accounting() {
    for rows in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 3, 4], vec![0, 2, 4, 7], vec![0, 2, 4, 99]] {
        let root = Directory::new(); let mut live = Live::new(&root);
        live.refresh(1); live.dispatch().unwrap();
        live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &rows), ElapsedTick(2)).unwrap();
        live.refresh(2);
        let result = live.host.publish_checked(live.host.revision(), 1, Some(&live.keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        let permitted = rows.contains(&99);
        assert_eq!(result.basis, if permitted { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(result.outcome, if permitted { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.host.inspect().control.ledger.charged, 16);
        live.host.reconcile(live.host.revision(), 1).unwrap();
        assert_eq!(live.host.inspect().control.ledger.charged, if permitted { 16 } else { 0 });
    }
}

#[test]
fn failed_read_leaves_both_lanes_withdrawn_and_retry_uses_the_same_unspent_keys() {
    let root = Directory::new(); let mut live = Live::new(&root);
    let source = live.producer.witness_reader(1, &live.keys.action).unwrap();
    let feed = live.producer.feed_reader().unwrap();
    let path = root.0.join("producer/delivery.bin"); let saved = path.with_extension("saved");
    std::fs::rename(&path, &saved).unwrap();
    let result = live.host.refresh_publication_from_producer(live.host.revision(), 1, &source, &feed,
        || panic!("failed read cannot sample time")).unwrap();
    assert_eq!(result, Err(FileCaptureError::Io(std::io::ErrorKind::NotFound)));
    assert!(!live.host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(live.dispatch(), Err(JournalError::Contract(Error::Incomplete)));
    assert!(live.host.storage_failure().is_none());
    assert_eq!(live.host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    std::fs::rename(saved, path).unwrap(); live.refresh(1); live.dispatch().unwrap();
}

#[test]
fn mismatched_raw_or_foreign_reader_pairs_refuse_before_withdrawal_and_io() {
    let root = Directory::new(); let mut live = Live::new(&root);
    let path = root.0.join("producer/delivery.bin");
    let source = live.producer.witness_reader(1, &live.keys.action).unwrap();
    let raw = PublicationFeedFile::new(&path, 41).unwrap();
    let foreign = PublicationFeedFile::from_producer(&path,
        PublicationProducerProfile { after: 1, ..producer_profile() }).unwrap();
    let before = live.host.inspect();
    for feed in [&raw, &foreign] {
        assert_eq!(live.host.refresh_publication_from_producer(live.host.revision(), 1, &source, feed,
            || panic!("unmatched readers cannot sample time")), Err(JournalError::Contract(Error::Binding)));
    }
    let other = PublicationInputFile::from_producer(&path, producer_profile(), 2, &live.keys.action).unwrap();
    assert_eq!(live.host.refresh_publication_from_producer(live.host.revision(), 1, &other,
        &live.producer.feed_reader().unwrap(), || panic!("foreign attempt")), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(live.host.inspect(), before);
}

#[test]
fn unchanged_bundle_cannot_extend_producer_expiry() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.refresh(4); assert_eq!(live.dispatch(), Err(JournalError::Contract(Error::Stale)));
    live.producer.publish(1, live.producer.image().inputs().clone(), ElapsedTick(4)).unwrap();
    live.refresh(4); live.dispatch().unwrap();
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
}

#[test]
fn failed_atomic_install_exposes_only_withdrawals_never_a_half_installed_pair() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
    let before = live.host.revision();
    let result = live.host.refresh_publication_from_producer(before, 1,
        &live.producer.witness_reader(1, &live.keys.action).unwrap(), &live.producer.feed_reader().unwrap(), || {
            std::fs::write(root.store().join("delivery.pending"), b"stage obstruction").unwrap(); ElapsedTick(2)
        });
    assert!(matches!(result, Err(JournalError::Io(_))));
    assert_eq!(live.host.revision(), before + 2); assert!(live.host.storage_failure().is_some());
    assert_eq!(live.host.inspect().executions, 0);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), live.host.inspect());
    drop(live);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 0);
    assert_eq!(host.publication_input_cut(1).unwrap().unwrap().last.through, 0);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn caught_post_read_clock_unwind_cannot_reuse_the_previous_observation() {
    let root = Directory::new(); let mut live = Live::new(&root);
    let source = live.producer.witness_reader(1, &live.keys.action).unwrap();
    let feed = live.producer.feed_reader().unwrap(); let before = live.host.revision();
    assert!(catch_unwind(AssertUnwindSafe(|| live.host.refresh_publication_from_producer(before, 1,
        &source, &feed, || panic!("clock interrupted after decoded bundle")))).is_err());
    assert_eq!(live.host.revision(), before + 2); assert!(live.host.storage_failure().is_some());
    assert_eq!(live.dispatch(), Err(JournalError::Unavailable));
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    assert_eq!(live.host.inspect().executions, 0);
}
